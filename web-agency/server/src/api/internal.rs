//! Internal API for the reverse proxy.
//!
//! Authenticated with a shared Bearer token from a state directory file.
//!
//! - `GET  /api/internal/routes`          — route table for the proxy
//! - `GET  /api/internal/certs`           — all valid certs (decrypted PEM)
//! - `POST /api/internal/certs/{domain}`  — trigger ACME cert issuance
//! - `GET  /api/internal/events`          — SSE stream (reload notifications)

use dioxus::fullstack::axum::{
    self as axum,
    Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{
        Json,
        sse::{Event, Sse},
    },
    routing::{get, post},
};
use serde::Serialize;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::config;

/// Shared state for the internal API.
#[derive(Clone)]
pub struct InternalState {
    pub pool: PgPool,
    pub reload_tx: Arc<broadcast::Sender<()>>,
}

/// Global handle to the reload broadcast sender, set once during init.
static RELOAD_TX: std::sync::OnceLock<Arc<broadcast::Sender<()>>> = std::sync::OnceLock::new();

/// Store the reload sender globally so web components can trigger reloads.
pub fn set_reload_tx(tx: Arc<broadcast::Sender<()>>) {
    let _ = RELOAD_TX.set(tx);
}

/// Notify the proxy of route changes (callable from anywhere, e.g. web components).
pub fn notify_proxy_reload() {
    if let Some(tx) = RELOAD_TX.get() {
        let _ = tx.send(());
    }
}

pub fn router(state: InternalState) -> Router<()> {
    Router::new()
        .route("/api/internal/routes", get(get_routes))
        .route("/api/internal/certs", get(get_certs))
        .route("/api/internal/certs/{domain}", post(issue_cert))
        .route("/api/internal/events", get(sse_events))
        .with_state(state)
}

// ── Auth ──────────────────────────────────────────────────────────────

fn authenticate(headers: &HeaderMap) -> Result<(), (StatusCode, String)> {
    let cfg = config::config();
    let proxy_cfg = cfg
        .proxy
        .as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "proxy not configured".into()))?;

    let expected_token = std::fs::read_to_string(&proxy_cfg.internal_token_path)
        .map_err(|_| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "internal token not available".into(),
            )
        })?;
    let expected_token = expected_token.trim();

    let provided = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");

    if provided.is_empty() || provided != expected_token {
        return Err((StatusCode::UNAUTHORIZED, "invalid token".into()));
    }

    Ok(())
}

// ── Routes ────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct RoutesResponse {
    routes: Vec<RouteEntry>,
    /// Seconds until the earliest relay proxy token expires.  The proxy
    /// should force a route re-fetch before this elapses.
    #[serde(skip_serializing_if = "Option::is_none")]
    token_lifetime_secs: Option<i64>,
}

#[derive(Serialize)]
struct RouteEntry {
    host: String,
    upstream: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    relay: Option<RelayInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth: Option<AuthInfo>,
}

#[derive(Serialize)]
struct RelayInfo {
    /// Full relay URL to proxy to (e.g. https://abc123-ollama.relay.plan.ai)
    url: String,
    /// Minted proxy token for authenticating with the relay
    proxy_token: String,
}

#[derive(Serialize, serde::Deserialize, Clone)]
struct AuthInfo {
    mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    org_id: Option<uuid::Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    basic_credentials: Option<Vec<BasicCredential>>,
}

#[derive(Serialize, serde::Deserialize, Clone)]
struct BasicCredential {
    username: String,
    password_hash: String,
}

/// In-memory cache for minted proxy tokens, keyed by credential ID.
static PROXY_TOKEN_CACHE: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<uuid::Uuid, (String, chrono::DateTime<chrono::Utc>)>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Get or mint a proxy token for the given mac-mgmt credential.
/// Returns `(token, expires_at)`.
async fn get_or_mint_proxy_token(
    pool: &PgPool,
    cred_id: uuid::Uuid,
) -> Result<(String, chrono::DateTime<chrono::Utc>), String> {
    // Check cache
    {
        let cache = PROXY_TOKEN_CACHE.lock().unwrap();
        if let Some((token, expires_at)) = cache.get(&cred_id) {
            if *expires_at > chrono::Utc::now() + chrono::Duration::minutes(5) {
                return Ok((token.clone(), *expires_at));
            }
        }
    }

    // Mint a new token
    let (server_url, admin_token) = crate::credentials::mac_mgmt_credential(pool, cred_id)
        .await
        .map_err(|e| format!("credential error: {e}"))?;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/proxy-token", server_url.trim_end_matches('/')))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({
            "scopes": ["tcp:*"],
        }))
        .send()
        .await
        .map_err(|e| format!("mint request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("mint failed ({status}): {body}"));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("parse mint response: {e}"))?;
    let token = body["proxy_token"]
        .as_str()
        .ok_or("no proxy_token in mint response")?
        .to_string();
    let expires_at = body["expires_at"]
        .as_str()
        .and_then(|s| s.parse::<chrono::DateTime<chrono::Utc>>().ok())
        .unwrap_or_else(|| chrono::Utc::now() + chrono::Duration::hours(24));

    {
        let mut cache = PROXY_TOKEN_CACHE.lock().unwrap();
        cache.insert(cred_id, (token.clone(), expires_at));
    }

    Ok((token, expires_at))
}

/// Build an AuthInfo from webspace fields. Returns None for auth_mode = "none".
async fn build_auth_info(
    pool: &PgPool,
    auth_mode: &str,
    org_id: uuid::Uuid,
    basic_list_id: Option<uuid::Uuid>,
) -> Option<AuthInfo> {
    match auth_mode {
        "oidc" => Some(AuthInfo {
            mode: "oidc".into(),
            org_id: Some(org_id),
            basic_credentials: None,
        }),
        "basic" => {
            let list_id = basic_list_id?;
            let creds = sqlx::query_as::<_, (String, String)>(
                "SELECT username, password_hash FROM basic_auth_credentials WHERE list_id = $1",
            )
            .bind(list_id)
            .fetch_all(pool)
            .await
            .ok()?;
            Some(AuthInfo {
                mode: "basic".into(),
                org_id: None,
                basic_credentials: Some(
                    creds.into_iter().map(|(username, password_hash)| BasicCredential { username, password_hash }).collect(),
                ),
            })
        }
        _ => None,
    }
}

async fn get_routes(
    State(state): State<InternalState>,
    headers: HeaderMap,
) -> Result<Json<RoutesResponse>, (StatusCode, String)> {
    authenticate(&headers)?;

    let cfg = config::config();
    let proxy_cfg = cfg.proxy.as_ref().unwrap();

    let mut earliest_token_expiry: Option<chrono::DateTime<chrono::Utc>> = None;

    let mut routes = vec![RouteEntry {
        host: proxy_cfg.agency_domain.clone(),
        upstream: proxy_cfg.agency_upstream.clone(),
        relay: None,
        auth: None,
    }];

    // Local webspace routes: domain → 127.0.0.1:local_port
    let rows = sqlx::query_as::<_, (String, Option<String>, i32, uuid::Uuid, String, Option<uuid::Uuid>)>(
        "SELECT d.name, s.name, w.local_port, w.organization_id, w.auth_mode, w.auth_basic_list_id \
         FROM webspace_domains wd \
         JOIN webspaces w ON w.id = wd.webspace_id \
         JOIN domains d ON d.id = wd.domain_id \
         LEFT JOIN subdomains s ON s.id = wd.subdomain_id \
         WHERE w.hosting_type = 'local' AND w.local_port IS NOT NULL",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for (domain, subdomain, port, org_id, auth_mode, basic_list_id) in rows {
        let host = match subdomain.as_deref() {
            Some(sub) if sub != "@" => format!("{sub}.{domain}"),
            _ => domain,
        };
        let auth = build_auth_info(&state.pool, &auth_mode, org_id, basic_list_id).await;
        routes.push(RouteEntry {
            host,
            upstream: format!("127.0.0.1:{port}"),
            relay: None,
            auth,
        });
    }

    // Relay webspace routes: domain → relay URL with proxy token
    let relay_rows = sqlx::query_as::<_, (String, Option<String>, String, uuid::Uuid, uuid::Uuid, String, Option<uuid::Uuid>)>(
        "SELECT d.name, s.name, w.relay_url, w.relay_credential_id, w.organization_id, w.auth_mode, w.auth_basic_list_id \
         FROM webspace_domains wd \
         JOIN webspaces w ON w.id = wd.webspace_id \
         JOIN domains d ON d.id = wd.domain_id \
         LEFT JOIN subdomains s ON s.id = wd.subdomain_id \
         WHERE w.hosting_type = 'relay' AND w.relay_url IS NOT NULL AND w.relay_credential_id IS NOT NULL",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for (domain, subdomain, relay_url, cred_id, org_id, auth_mode, basic_list_id) in relay_rows {
        let host = match subdomain.as_deref() {
            Some(sub) if sub != "@" => format!("{sub}.{domain}"),
            _ => domain,
        };

        let upstream = parse_relay_upstream(&relay_url).ok_or_else(|| {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("bad relay URL: {relay_url}"))
        })?;

        let proxy_token = match get_or_mint_proxy_token(&state.pool, cred_id).await {
            Ok((t, expires_at)) => {
                super::counters::COUNTERS.relay_mint_success.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                {
                    let mut map = super::counters::RELAY_MINT_RESULTS.lock().unwrap();
                    map.insert(host.clone(), super::counters::MintResult {
                        org_id,
                        success: true,
                    });
                }
                earliest_token_expiry = Some(match earliest_token_expiry {
                    Some(prev) => prev.min(expires_at),
                    None => expires_at,
                });
                t
            }
            Err(e) => {
                tracing::error!(host = %host, "failed to mint proxy token: {e}");
                super::counters::COUNTERS.relay_mint_failed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                {
                    let mut map = super::counters::RELAY_MINT_RESULTS.lock().unwrap();
                    map.insert(host.clone(), super::counters::MintResult {
                        org_id,
                        success: false,
                    });
                }
                continue;
            }
        };

        let auth = build_auth_info(&state.pool, &auth_mode, org_id, basic_list_id).await;
        routes.push(RouteEntry {
            host,
            upstream,
            relay: Some(RelayInfo {
                url: relay_url,
                proxy_token,
            }),
            auth,
        });
    }

    // Tunnel webspace routes: domain → upstream URL (no auth token)
    let tunnel_rows = sqlx::query_as::<_, (String, Option<String>, String, uuid::Uuid, String, Option<uuid::Uuid>)>(
        "SELECT d.name, s.name, w.relay_url, w.organization_id, w.auth_mode, w.auth_basic_list_id \
         FROM webspace_domains wd \
         JOIN webspaces w ON w.id = wd.webspace_id \
         JOIN domains d ON d.id = wd.domain_id \
         LEFT JOIN subdomains s ON s.id = wd.subdomain_id \
         WHERE w.hosting_type = 'tunnel' AND w.relay_url IS NOT NULL",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for (domain, subdomain, tunnel_url, org_id, auth_mode, basic_list_id) in tunnel_rows {
        let host = match subdomain.as_deref() {
            Some(sub) if sub != "@" => format!("{sub}.{domain}"),
            _ => domain,
        };

        let upstream = parse_relay_upstream(&tunnel_url).ok_or_else(|| {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("bad tunnel URL: {tunnel_url}"))
        })?;

        let auth = build_auth_info(&state.pool, &auth_mode, org_id, basic_list_id).await;
        routes.push(RouteEntry {
            host,
            upstream,
            relay: Some(RelayInfo {
                url: tunnel_url,
                proxy_token: String::new(),
            }),
            auth,
        });
    }

    let token_lifetime_secs = earliest_token_expiry.map(|exp| {
        (exp - chrono::Utc::now()).num_seconds().max(0)
    });

    Ok(Json(RoutesResponse {
        routes,
        token_lifetime_secs,
    }))
}

/// Parse a relay URL like `https://host:port/...` into `host:port`.
fn parse_relay_upstream(url: &str) -> Option<String> {
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host_port = without_scheme.split('/').next()?;
    if host_port.contains(':') {
        Some(host_port.to_string())
    } else if url.starts_with("https://") {
        Some(format!("{host_port}:443"))
    } else {
        Some(format!("{host_port}:80"))
    }
}

// ── Certs ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct CertResponse {
    domain: String,
    chain_pem: String,
    key_pem: String,
}

async fn get_certs(
    State(state): State<InternalState>,
    headers: HeaderMap,
) -> Result<Json<Vec<CertResponse>>, (StatusCode, String)> {
    authenticate(&headers)?;

    let rows = sqlx::query_as::<_, (String, Vec<u8>, Vec<u8>)>(
        "SELECT domain, encrypted_chain, encrypted_key FROM certificates \
         WHERE not_after > now() AND acme_status IN ('valid', 'none') \
         ORDER BY domain",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut certs = Vec::new();
    for (domain, enc_chain, enc_key) in rows {
        let chain = crate::crypto::decrypt(&enc_chain)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("decrypt error: {e}")))?;
        let key = crate::crypto::decrypt(&enc_key)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("decrypt error: {e}")))?;
        certs.push(CertResponse {
            domain,
            chain_pem: String::from_utf8_lossy(&chain).into_owned(),
            key_pem: String::from_utf8_lossy(&key).into_owned(),
        });
    }

    Ok(Json(certs))
}

// ── Cert issuance ─────────────────────────────────────────────────────

#[derive(Serialize)]
struct IssueResponse {
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

async fn issue_cert(
    State(state): State<InternalState>,
    headers: HeaderMap,
    Path(domain): Path<String>,
) -> Result<Json<IssueResponse>, (StatusCode, String)> {
    authenticate(&headers)?;

    // Check if already pending
    let existing = sqlx::query_scalar::<_, String>(
        "SELECT acme_status FROM certificates WHERE domain = $1",
    )
    .bind(&domain)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if existing.as_deref() == Some("pending") {
        return Ok(Json(IssueResponse {
            status: "pending".into(),
            message: None,
        }));
    }

    if existing.as_deref() == Some("valid") {
        // Check if still valid (not expiring soon)
        let not_expired = sqlx::query_scalar::<_, bool>(
            "SELECT not_after > now() + interval '7 days' FROM certificates WHERE domain = $1",
        )
        .bind(&domain)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        if not_expired == Some(true) {
            return Ok(Json(IssueResponse {
                status: "valid".into(),
                message: Some("cert still valid".into()),
            }));
        }
    }

    // Trigger issuance in background
    let pool = state.pool.clone();
    let reload_tx = state.reload_tx.clone();
    let domain_clone = domain.clone();
    tokio::spawn(async move {
        match crate::api::acme::issue_cert(&pool, &domain_clone).await {
            Ok(()) => {
                tracing::info!(domain = %domain_clone, "cert issued successfully");
                let _ = reload_tx.send(());
            }
            Err(e) => {
                tracing::error!(domain = %domain_clone, "cert issuance failed: {e}");
            }
        }
    });

    Ok(Json(IssueResponse {
        status: "pending".into(),
        message: Some("issuance started".into()),
    }))
}

// ── Proxy gate (OIDC auth for webspaces) ─────────────────────────────

fn sign_proxy_gate(internal_token: &str, org_id: uuid::Uuid, expiry_ts: i64) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let msg = format!("proxy-gate:{org_id}:{expiry_ts}");
    let mut mac = Hmac::<Sha256>::new_from_slice(internal_token.as_bytes()).unwrap();
    mac.update(msg.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

#[derive(serde::Deserialize)]
pub struct ProxyGateParams {
    return_url: String,
}

/// OIDC proxy gate — mounted on the web router (behind OIDC auth middleware).
/// The proxy redirects unauthenticated users here. After the user logs in on the
/// agency domain, this endpoint validates org membership and redirects back to
/// the webspace with a signed token.
pub async fn proxy_gate(
    axum::extract::Query(params): axum::extract::Query<ProxyGateParams>,
    axum::extract::Extension(user): axum::extract::Extension<plan_ai_auth::WebUser>,
) -> Result<axum::response::Redirect, (StatusCode, String)> {
    let pool = crate::server_pool()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Parse hostname from return_url
    let return_url = &params.return_url;
    let hostname = return_url
        .strip_prefix("https://")
        .or_else(|| return_url.strip_prefix("http://"))
        .and_then(|s| s.split('/').next())
        .and_then(|s| s.split(':').next())
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "bad return_url".into()))?;

    // Find which org owns the webspace bound to this hostname
    let org_id = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT w.organization_id FROM webspace_domains wd \
         JOIN webspaces w ON w.id = wd.webspace_id \
         JOIN domains d ON d.id = wd.domain_id \
         LEFT JOIN subdomains s ON s.id = wd.subdomain_id \
         WHERE (CASE WHEN s.name IS NOT NULL AND s.name != '@' \
                THEN s.name || '.' || d.name ELSE d.name END) = $1 \
         AND w.auth_mode = 'oidc' \
         LIMIT 1",
    )
    .bind(hostname)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, "no OIDC-protected webspace for this host".into()))?;

    // Check user has org membership (any role) or is admin
    if !user.is_admin && !user.org_ids().contains(&org_id) {
        return Err((StatusCode::FORBIDDEN, "not a member of this organization".into()));
    }

    // Sign a gate token (valid for 5 minutes — the proxy will exchange it for a 24h cookie)
    let cfg = config::config();
    let proxy_cfg = cfg.proxy.as_ref()
        .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, "proxy not configured".into()))?;
    let internal_token = std::fs::read_to_string(&proxy_cfg.internal_token_path)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "internal token not available".into()))?;
    let internal_token = internal_token.trim();

    let expiry_ts = chrono::Utc::now().timestamp() + 300; // 5 minutes
    let sig = sign_proxy_gate(internal_token, org_id, expiry_ts);

    // Append gate params to return_url
    let separator = if return_url.contains('?') { "&" } else { "?" };
    let redirect = format!(
        "{return_url}{separator}__pg_token={sig}&__pg_oid={org_id}&__pg_exp={expiry_ts}"
    );

    Ok(axum::response::Redirect::temporary(&redirect))
}

// ── SSE ───────────────────────────────────────────────────────────────

async fn sse_events(
    State(state): State<InternalState>,
    headers: HeaderMap,
) -> Result<Sse<impl futures_core::Stream<Item = Result<Event, std::convert::Infallible>>>, (StatusCode, String)>
{
    authenticate(&headers)?;

    let mut rx = state.reload_tx.subscribe();

    let stream = async_stream::stream! {
        let mut ping_interval = tokio::time::interval(std::time::Duration::from_secs(15));
        ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                result = rx.recv() => {
                    match result {
                        Ok(()) => {
                            yield Ok(Event::default().event("reload").data("{}"));
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::debug!("SSE subscriber lagged by {n} messages");
                            yield Ok(Event::default().event("reload").data("{}"));
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                _ = ping_interval.tick() => {
                    yield Ok(Event::default().event("ping").data("{}"));
                }
            }
        }
    };

    Ok(Sse::new(stream))
}
