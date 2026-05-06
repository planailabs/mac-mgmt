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
        sse::{Event, KeepAlive, Sse},
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

/// Send a reload notification to all connected proxies.
pub fn notify_reload(state: &InternalState) {
    let _ = state.reload_tx.send(());
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
struct RouteEntry {
    host: String,
    upstream: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    relay: Option<RelayInfo>,
}

#[derive(Serialize)]
struct RelayInfo {
    /// Full relay URL to proxy to (e.g. https://abc123-ollama.relay.plan.ai)
    url: String,
    /// Minted proxy token for authenticating with the relay
    proxy_token: String,
}

/// In-memory cache for minted proxy tokens, keyed by credential ID.
static PROXY_TOKEN_CACHE: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<uuid::Uuid, (String, chrono::DateTime<chrono::Utc>)>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Get or mint a proxy token for the given mac-mgmt credential.
async fn get_or_mint_proxy_token(
    pool: &PgPool,
    cred_id: uuid::Uuid,
) -> Result<String, String> {
    // Check cache
    {
        let cache = PROXY_TOKEN_CACHE.lock().unwrap();
        if let Some((token, expires_at)) = cache.get(&cred_id) {
            if *expires_at > chrono::Utc::now() + chrono::Duration::minutes(5) {
                return Ok(token.clone());
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
            "label": "web-agency-relay",
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
    let token = body["token"]
        .as_str()
        .ok_or("no token in mint response")?
        .to_string();

    // Cache for 6 hours (the default proxy token lifetime)
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(6);
    {
        let mut cache = PROXY_TOKEN_CACHE.lock().unwrap();
        cache.insert(cred_id, (token.clone(), expires_at));
    }

    Ok(token)
}

async fn get_routes(
    State(state): State<InternalState>,
    headers: HeaderMap,
) -> Result<Json<Vec<RouteEntry>>, (StatusCode, String)> {
    authenticate(&headers)?;

    let cfg = config::config();
    let proxy_cfg = cfg.proxy.as_ref().unwrap();

    let mut routes = vec![RouteEntry {
        host: proxy_cfg.agency_domain.clone(),
        upstream: proxy_cfg.agency_upstream.clone(),
        relay: None,
    }];

    // Local webspace routes: domain → 127.0.0.1:local_port
    let rows = sqlx::query_as::<_, (String, Option<String>, i32)>(
        "SELECT d.name, s.name, w.local_port \
         FROM webspace_domains wd \
         JOIN webspaces w ON w.id = wd.webspace_id \
         JOIN domains d ON d.id = wd.domain_id \
         LEFT JOIN subdomains s ON s.id = wd.subdomain_id \
         WHERE w.hosting_type = 'local' AND w.local_port IS NOT NULL",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for (domain, subdomain, port) in rows {
        let host = match subdomain.as_deref() {
            Some(sub) if sub != "@" => format!("{sub}.{domain}"),
            _ => domain,
        };
        routes.push(RouteEntry {
            host,
            upstream: format!("127.0.0.1:{port}"),
            relay: None,
        });
    }

    // Relay webspace routes: domain → relay URL with proxy token
    let relay_rows = sqlx::query_as::<_, (String, Option<String>, String, uuid::Uuid)>(
        "SELECT d.name, s.name, w.relay_url, w.relay_credential_id \
         FROM webspace_domains wd \
         JOIN webspaces w ON w.id = wd.webspace_id \
         JOIN domains d ON d.id = wd.domain_id \
         LEFT JOIN subdomains s ON s.id = wd.subdomain_id \
         WHERE w.hosting_type = 'relay' AND w.relay_url IS NOT NULL AND w.relay_credential_id IS NOT NULL",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for (domain, subdomain, relay_url, cred_id) in relay_rows {
        let host = match subdomain.as_deref() {
            Some(sub) if sub != "@" => format!("{sub}.{domain}"),
            _ => domain,
        };

        // Parse relay URL to get the upstream host:port.
        // URL is like https://instance-tunnel.relay.example.com
        let upstream = parse_relay_upstream(&relay_url).ok_or_else(|| {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("bad relay URL: {relay_url}"))
        })?;

        let proxy_token = match get_or_mint_proxy_token(&state.pool, cred_id).await {
            Ok(t) => t,
            Err(e) => {
                tracing::error!(host = %host, "failed to mint proxy token: {e}");
                continue;
            }
        };

        routes.push(RouteEntry {
            host,
            upstream,
            relay: Some(RelayInfo {
                url: relay_url,
                proxy_token,
            }),
        });
    }

    Ok(Json(routes))
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

// ── SSE ───────────────────────────────────────────────────────────────

async fn sse_events(
    State(state): State<InternalState>,
    headers: HeaderMap,
) -> Result<Sse<impl futures_core::Stream<Item = Result<Event, std::convert::Infallible>>>, (StatusCode, String)>
{
    authenticate(&headers)?;

    let mut rx = state.reload_tx.subscribe();

    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
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
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
