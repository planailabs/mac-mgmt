use arc_swap::ArcSwap;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::cert_store::{CertEntry, CertKey, CertStore};
use crate::config::ProxyConfig;
use crate::proxy::{AuthMode, Route};

#[derive(Debug, Deserialize)]
struct RoutesResponse {
    routes: Vec<RouteEntry>,
    /// Seconds until the earliest relay proxy token expires.
    token_lifetime_secs: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RouteEntry {
    host: String,
    #[serde(default = "default_path_prefix")]
    path_prefix: String,
    upstream: String,
    #[serde(default)]
    static_webspace_id: Option<String>,
    relay: Option<RelayInfoEntry>,
    auth: Option<AuthInfoEntry>,
}

fn default_path_prefix() -> String {
    "/".to_string()
}

#[derive(Debug, Deserialize)]
struct RelayInfoEntry {
    url: String,
    proxy_token: String,
}

#[derive(Debug, Deserialize)]
struct AuthInfoEntry {
    mode: String,
    org_id: Option<uuid::Uuid>,
    basic_list_id: Option<uuid::Uuid>,
}

fn auth_headers(token: &str) -> reqwest::header::HeaderMap {
    let mut h = reqwest::header::HeaderMap::new();
    h.insert(
        reqwest::header::AUTHORIZATION,
        format!("Bearer {token}").parse().unwrap(),
    );
    h
}

fn build_client(token: &str) -> reqwest::Client {
    reqwest::Client::builder()
        .default_headers(auth_headers(token))
        .timeout(Duration::from_secs(30))
        .build()
        .expect("failed to build HTTP client")
}

/// Separate client for SSE — no response timeout (the connection is long-lived),
/// but a connect timeout prevents hanging if the server is unreachable.
fn build_sse_client(token: &str) -> reqwest::Client {
    reqwest::Client::builder()
        .default_headers(auth_headers(token))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .expect("failed to build SSE client")
}

/// Fetch routes from the server API and update the shared route table.
/// Returns `Some(token_lifetime_secs)` on success, `None` on failure.
async fn reload_routes(
    client: &reqwest::Client,
    server_url: &str,
    routes: &ArcSwap<HashMap<String, Vec<(String, Route, AuthMode)>>>,
) -> Option<Option<i64>> {
    match client
        .get(format!("{server_url}/api/internal/routes"))
        .send()
        .await
    {
        Ok(resp) => match resp.json::<RoutesResponse>().await {
            Ok(body) => {
                let token_lifetime_secs = body.token_lifetime_secs;
                let entries = body.routes;
                let mut map: HashMap<String, Vec<(String, Route, AuthMode)>> = HashMap::new();
                for entry in entries {
                    let route = if let Some(webspace_id) = entry.static_webspace_id {
                        Route::StaticOrigin {
                            upstream: entry.upstream,
                            webspace_id,
                        }
                    } else if let Some(relay) = entry.relay {
                        let relay_host = relay
                            .url
                            .strip_prefix("https://")
                            .or_else(|| relay.url.strip_prefix("http://"))
                            .and_then(|s| s.split('/').next())
                            .and_then(|s| s.split(':').next())
                            .unwrap_or("")
                            .to_string();
                        let tls = relay.url.starts_with("https://");
                        Route::Relay {
                            upstream: entry.upstream,
                            url: relay.url,
                            sni: relay_host.clone(),
                            relay_host,
                            tls,
                            proxy_token: relay.proxy_token,
                        }
                    } else {
                        Route::Direct(entry.upstream)
                    };

                    let auth = match entry.auth {
                        Some(a) if a.mode == "oidc" => AuthMode::Oidc {
                            org_id: a.org_id.unwrap_or_default(),
                        },
                        Some(a) if a.mode == "basic" => match a.basic_list_id {
                            Some(list_id) => AuthMode::Basic { list_id },
                            None => AuthMode::None,
                        },
                        _ => AuthMode::None,
                    };

                    map.entry(entry.host)
                        .or_default()
                        .push((entry.path_prefix, route, auth));
                }
                // Longest path prefix first, so the proxy matches the most specific folder.
                for folders in map.values_mut() {
                    folders.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
                }
                let mut route_count = 0usize;
                for (host, folders) in &map {
                    for (path_prefix, route, auth) in folders {
                        route_count += 1;
                        let route_desc = match route {
                            Route::Direct(upstream) => format!("direct → {upstream}"),
                            Route::StaticOrigin {
                                upstream,
                                webspace_id,
                            } => {
                                format!("static → {upstream} (webspace {webspace_id})")
                            }
                            Route::Relay { upstream, tls, .. } => {
                                let scheme = if *tls { "tls" } else { "plain" };
                                format!("relay → {upstream} ({scheme})")
                            }
                        };
                        let auth_desc = match auth {
                            AuthMode::None => "none",
                            AuthMode::Oidc { .. } => "oidc",
                            AuthMode::Basic { .. } => "basic",
                        };
                        tracing::info!(host, path = %path_prefix, route = %route_desc, auth = auth_desc, "route");
                    }
                }
                tracing::info!(
                    hosts = map.len(),
                    routes = route_count,
                    ?token_lifetime_secs,
                    "loaded routes"
                );
                routes.store(Arc::new(map));
                Some(token_lifetime_secs)
            }
            Err(e) => {
                tracing::error!("failed to parse routes: {e}");
                None
            }
        },
        Err(e) => {
            tracing::error!("failed to fetch routes: {e}");
            None
        }
    }
}

/// Fetch certs from server and update the cert store in memory.
/// Returns `true` on success.
async fn reload_certs(client: &reqwest::Client, server_url: &str, cert_store: &CertStore) -> bool {
    match client
        .get(format!("{server_url}/api/internal/certs"))
        .send()
        .await
    {
        Ok(resp) => match resp.json::<Vec<CertEntry>>().await {
            Ok(entries) => {
                let mut map = HashMap::new();
                for entry in entries {
                    match CertKey::from_pem(&entry.chain_pem, &entry.key_pem) {
                        Ok(ck) => {
                            map.insert(entry.domain, Arc::new(ck));
                        }
                        Err(e) => tracing::error!(domain = %entry.domain, "bad cert PEM: {e}"),
                    }
                }
                tracing::info!(count = map.len(), "loaded certs");
                cert_store.replace_all(map);
                true
            }
            Err(e) => {
                tracing::error!("failed to parse certs: {e}");
                false
            }
        },
        Err(e) => {
            tracing::error!("failed to fetch certs: {e}");
            false
        }
    }
}

/// Request cert issuance for routes that don't have certs yet.
async fn trigger_missing_certs(
    client: &reqwest::Client,
    server_url: &str,
    routes: &ArcSwap<HashMap<String, Vec<(String, Route, AuthMode)>>>,
    cert_store: &CertStore,
) {
    let route_hosts: Vec<String> = routes.load().keys().cloned().collect();
    for host in route_hosts {
        if !cert_store.has_cert(&host) {
            tracing::info!(domain = %host, "requesting cert issuance");
            if let Err(e) = client
                .post(format!("{server_url}/api/internal/certs/{host}"))
                .send()
                .await
            {
                tracing::warn!(domain = %host, "cert issuance request failed: {e}");
            }
        }
    }
}

/// Reload routes, certs, and trigger missing cert issuance.
/// Returns the token lifetime from the routes response (if any).
async fn reload_all(
    client: &reqwest::Client,
    server_url: &str,
    routes: &ArcSwap<HashMap<String, Vec<(String, Route, AuthMode)>>>,
    cert_store: &CertStore,
) -> Option<i64> {
    let token_lifetime = reload_routes(client, server_url, routes)
        .await
        .and_then(|tl| tl);
    let _ = reload_certs(client, server_url, cert_store).await;
    trigger_missing_certs(client, server_url, routes, cert_store).await;
    token_lifetime
}

/// Build a Pingora background service that spawns the sync loop.
///
/// The sync task carries no important state — it only writes into shared
/// `ArcSwap`/`CertStore` atomics.  Rather than carefully coordinating
/// shutdown, we spawn the loop as a detached tokio task and return from
/// `start()` immediately so Pingora never blocks on us during shutdown.
pub fn build_service(
    cfg: ProxyConfig,
    routes: Arc<ArcSwap<HashMap<String, Vec<(String, Route, AuthMode)>>>>,
    cert_store: Arc<CertStore>,
) -> pingora::services::background::GenBackgroundService<SyncTask> {
    pingora::services::background::background_service(
        "sync",
        SyncTask {
            cfg,
            routes,
            cert_store,
        },
    )
}

pub struct SyncTask {
    cfg: ProxyConfig,
    routes: Arc<ArcSwap<HashMap<String, Vec<(String, Route, AuthMode)>>>>,
    cert_store: Arc<CertStore>,
}

#[async_trait::async_trait]
impl pingora::services::background::BackgroundService for SyncTask {
    async fn start(&self, _shutdown: pingora::server::ShutdownWatch) {
        let cfg = self.cfg.clone();
        let routes = self.routes.clone();
        let cert_store = self.cert_store.clone();
        tokio::spawn(async move {
            sync_loop(cfg, routes, cert_store).await;
        });
    }
}

/// Schedule the next token refresh.  On success uses 90% of the token
/// lifetime (minimum 60s).  On failure (`None`) retries in 5s.
fn schedule_refresh(interval: &mut tokio::time::Interval, lifetime_secs: Option<i64>) {
    match lifetime_secs {
        Some(secs) => {
            let refresh_in = ((secs as f64) * 0.9).max(60.0) as u64;
            tracing::info!(refresh_in_secs = refresh_in, "scheduled token refresh");
            interval.reset_after(Duration::from_secs(refresh_in));
        }
        None => {
            tracing::warn!("reload failed, retrying in 5s");
            interval.reset_after(Duration::from_secs(5));
        }
    }
}

async fn sync_loop(
    cfg: ProxyConfig,
    routes: Arc<ArcSwap<HashMap<String, Vec<(String, Route, AuthMode)>>>>,
    cert_store: Arc<CertStore>,
) {
    let token = cfg.internal_token();
    let client = build_client(&token);
    let sse_client = build_sse_client(&token);
    let server_url = &cfg.server_url;

    // Reload timer — drives both token refresh and failure retries.
    // Every tick path reschedules via `reset_after`, so the period is only a
    // worst-case backstop. It must stay small enough that `deadline + period`
    // never overflows an Instant inside `poll_tick` (u64::MAX/2 panicked here).
    let mut reload_timer = tokio::time::interval(Duration::from_secs(3600));
    reload_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    reload_timer.tick().await; // consume the first immediate tick

    // Prime the reload timer from the initial route data.
    let tl = reload_all(&client, server_url, &routes, &cert_store).await;
    schedule_refresh(&mut reload_timer, tl);

    loop {
        tracing::info!("connecting to SSE event stream");

        let resp = sse_client
            .get(format!("{server_url}/api/internal/events"))
            .send()
            .await;

        match resp {
            Ok(resp) => {
                use eventsource_stream::Eventsource;
                use futures::StreamExt;

                // Reload on connect, exactly like on a reload event: anything
                // that changed while we weren't connected was never signalled.
                tracing::info!("SSE connected, reloading");
                let tl = reload_all(&client, server_url, &routes, &cert_store).await;
                schedule_refresh(&mut reload_timer, tl);

                let mut stream = resp.bytes_stream().eventsource();
                loop {
                    tokio::select! {
                        _ = reload_timer.tick() => {
                            tracing::info!("reload timer fired");
                        }
                        event = stream.next() => match event {
                            Some(Ok(ev)) if ev.event == "reload" => {
                                tracing::info!("received reload event");
                            }
                            Some(Ok(_)) => continue,
                            Some(Err(e)) => { tracing::warn!("SSE error: {e}"); break; }
                            None => break,
                        },
                        _ = tokio::time::sleep(Duration::from_secs(60)) => {
                            tracing::warn!("SSE inactivity timeout, reconnecting");
                            break;
                        }
                    }

                    // Both reload_timer and SSE reload events fall through here.
                    // On failure, schedule_refresh sets a 5s retry via reload_timer.
                    let tl = reload_all(&client, server_url, &routes, &cert_store).await;
                    schedule_refresh(&mut reload_timer, tl);
                }
            }
            Err(e) => tracing::error!("failed to connect to SSE: {e}"),
        }

        tracing::info!("SSE disconnected, reconnecting in 5s");
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Reload before reconnecting so we have fresh data even if SSE takes a while.
        let tl = reload_all(&client, server_url, &routes, &cert_store).await;
        schedule_refresh(&mut reload_timer, tl);
    }
}
