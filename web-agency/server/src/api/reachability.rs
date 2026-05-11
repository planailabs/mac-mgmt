//! Periodic reachability checks for webspace domains.
//!
//! Every 15 minutes, checks each bound hostname for:
//! - HTTP(S) reachability and SSL validity
//! - Proxy verification via `/.well-known/web-agency.json`
//!
//! Results are stored in `reachability_results` for the metrics endpoint.

use sqlx::PgPool;
use std::collections::HashSet;
use std::time::Duration;
use tokio::sync::Semaphore;
use uuid::Uuid;

const CHECK_INTERVAL: Duration = Duration::from_secs(15 * 60);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_CONCURRENT_CHECKS: usize = 10;

struct WebspaceHost {
    webspace_id: Uuid,
    organization_id: Uuid,
    hostname: String,
    hosting_type: String,
}

/// Spawn the periodic reachability check background task.
pub fn spawn(pool: PgPool) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(30)).await;
        loop {
            if let Err(e) = run_checks(&pool).await {
                tracing::error!("reachability check failed: {e}");
            }
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}

async fn run_checks(pool: &PgPool) -> anyhow::Result<()> {
    let hosts = sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>, String)>(
        "SELECT w.id, w.organization_id, d.name, s.name, w.hosting_type \
         FROM webspace_domains wd \
         JOIN webspaces w ON w.id = wd.webspace_id \
         JOIN domains d ON d.id = wd.domain_id \
         LEFT JOIN subdomains s ON s.id = wd.subdomain_id \
         WHERE w.hosting_type IN ('local', 'relay', 'tunnel', 'cloudflare_pages')",
    )
    .fetch_all(pool)
    .await?;

    let webspace_hosts: Vec<WebspaceHost> = hosts
        .into_iter()
        .map(|(webspace_id, organization_id, domain, subdomain, hosting_type)| {
            let hostname = match subdomain.as_deref() {
                Some(sub) if sub != "@" => format!("{sub}.{domain}"),
                _ => domain,
            };
            WebspaceHost {
                webspace_id,
                organization_id,
                hostname,
                hosting_type,
            }
        })
        .collect();

    let current_hostnames: HashSet<String> =
        webspace_hosts.iter().map(|h| h.hostname.clone()).collect();

    tracing::info!(count = webspace_hosts.len(), "starting reachability checks");

    let semaphore = std::sync::Arc::new(Semaphore::new(MAX_CONCURRENT_CHECKS));
    let client = build_strict_client();
    let fallback_client = build_insecure_client();
    let pool = pool.clone();

    let mut tasks = tokio::task::JoinSet::new();
    for host in webspace_hosts {
        let sem = semaphore.clone();
        let client = client.clone();
        let fallback_client = fallback_client.clone();
        let pool = pool.clone();
        tasks.spawn(async move {
            let _permit = sem.acquire().await;
            check_host(&pool, &client, &fallback_client, &host).await;
        });
    }

    while let Some(result) = tasks.join_next().await {
        if let Err(e) = result {
            tracing::warn!("reachability task panicked: {e}");
        }
    }

    // Clean up stale entries for removed webspaces/domains
    if !current_hostnames.is_empty() {
        let hostnames_vec: Vec<&str> = current_hostnames.iter().map(|s| s.as_str()).collect();
        sqlx::query(
            "DELETE FROM reachability_results WHERE hostname != ALL($1)",
        )
        .bind(&hostnames_vec)
        .execute(&pool)
        .await?;
    }

    // Prune stale relay mint results
    {
        let mut map = super::counters::RELAY_MINT_RESULTS.lock().unwrap();
        map.retain(|hostname, _| current_hostnames.contains(hostname));
    }

    tracing::info!("reachability checks complete");
    Ok(())
}

/// Client that rejects invalid TLS certificates.
fn build_strict_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent("web-agency-reachability/1.0")
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .expect("failed to build strict HTTP client")
}

/// Client that accepts invalid TLS certificates (for checking HTTP reachability
/// when SSL is broken).
fn build_insecure_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent("web-agency-reachability/1.0")
        .redirect(reqwest::redirect::Policy::limited(5))
        .danger_accept_invalid_certs(true)
        .build()
        .expect("failed to build insecure HTTP client")
}

async fn check_host(
    pool: &PgPool,
    strict_client: &reqwest::Client,
    fallback_client: &reqwest::Client,
    host: &WebspaceHost,
) {
    let url = format!("https://{}/", host.hostname);
    let well_known_url = format!("https://{}/.well-known/web-agency.json", host.hostname);

    let start = std::time::Instant::now();
    let mut http_ok = false;
    let mut ssl_ok = false;
    let mut proxy_ok = false;
    let mut error_message: Option<String> = None;

    // Check HTTPS with strict TLS
    match strict_client.get(&url).send().await {
        Ok(resp) => {
            ssl_ok = true;
            http_ok = resp.status().is_success() || resp.status().is_redirection();
        }
        Err(e) => {
            if e.is_connect() || e.is_timeout() {
                // TLS or connection failure — try with insecure client to distinguish
                // SSL error from total unreachability
                error_message = Some(format!("{e}"));
                match fallback_client.get(&url).send().await {
                    Ok(resp) => {
                        http_ok = resp.status().is_success() || resp.status().is_redirection();
                        // ssl_ok stays false — TLS was invalid
                    }
                    Err(_) => {
                        // Totally unreachable
                    }
                }
            } else {
                error_message = Some(format!("{e}"));
            }
        }
    }

    // Check .well-known/web-agency.json (use the client appropriate for SSL status)
    let wk_client = if ssl_ok { strict_client } else { fallback_client };
    match wk_client.get(&well_known_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(body) = resp.text().await {
                let expected_service = match host.hosting_type.as_str() {
                    "cloudflare_pages" => "web-agency-pages",
                    _ => "web-agency-proxy",
                };
                proxy_ok = body.contains(expected_service);
            }
        }
        Ok(_) => {}
        Err(e) => {
            if error_message.is_none() {
                error_message = Some(format!("well-known: {e}"));
            }
        }
    }

    let latency_ms = start.elapsed().as_millis() as i32;

    // Upsert result
    if let Err(e) = sqlx::query(
        "INSERT INTO reachability_results \
             (webspace_id, organization_id, hostname, http_ok, ssl_ok, proxy_ok, latency_ms, error_message, checked_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now()) \
         ON CONFLICT (hostname) DO UPDATE SET \
             webspace_id = $1, organization_id = $2, \
             http_ok = $4, ssl_ok = $5, proxy_ok = $6, \
             latency_ms = $7, error_message = $8, checked_at = now()",
    )
    .bind(host.webspace_id)
    .bind(host.organization_id)
    .bind(&host.hostname)
    .bind(http_ok)
    .bind(ssl_ok)
    .bind(proxy_ok)
    .bind(latency_ms)
    .bind(&error_message)
    .execute(pool)
    .await
    {
        tracing::warn!(hostname = %host.hostname, "failed to upsert reachability result: {e}");
    }
}
