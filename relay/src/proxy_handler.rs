use axum::Router;
use axum::body::Body;
use axum::extract::{Json, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use axum::extract::Path;

use crate::auth::{SelfInfo, validate_token};
use crate::daemon_registry::DaemonRegistry;

/// Cache validated proxy tokens for 5 minutes to avoid hitting the server API
/// on every single proxied request.
static TOKEN_CACHE: std::sync::LazyLock<
    tokio::sync::RwLock<HashMap<String, (SelfInfo, std::time::Instant)>>,
> = std::sync::LazyLock::new(|| tokio::sync::RwLock::new(HashMap::new()));

// Short TTL so a revoked token stops authenticating quickly (was 300s, which
// left a 5-minute window after revocation). Still amortizes validation across
// the burst of requests a single page load produces.
const TOKEN_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(30);

async fn validate_token_cached(server_api_url: &str, token: &str) -> Result<SelfInfo, StatusCode> {
    // Check cache first.
    {
        let cache = TOKEN_CACHE.read().await;
        if let Some((info, created)) = cache.get(token) {
            if created.elapsed() < TOKEN_CACHE_TTL {
                return Ok(info.clone());
            }
        }
    }
    // Cache miss — validate against server.
    let info = validate_token(server_api_url, token).await?;
    {
        let mut cache = TOKEN_CACHE.write().await;
        cache.insert(token.to_string(), (info.clone(), std::time::Instant::now()));
        // Evict expired entries periodically.
        if cache.len() > 1000 {
            cache.retain(|_, (_, t)| t.elapsed() < TOKEN_CACHE_TTL);
        }
    }
    Ok(info)
}

const PROXY_TOKEN_COOKIE: &str = "proxy_token";

#[derive(Clone)]
pub struct ProxyState {
    pub registry: Arc<DaemonRegistry>,
    pub server_api_url: String,
    pub proxy_hostname: String,
    /// Origin suffixes allowed for CORS on the file API (e.g. `["localhost"]`).
    pub cors_origins: Vec<String>,
    /// libp2p relay swarm for sending control requests to daemons via p2p.
    pub relay_swarm: Option<Arc<crate::p2p::RelaySwarm>>,
    /// External web URL of the management server (e.g. "https://plan.ai").
    /// Initialised from `/api/server-info` at startup or lazily on first
    /// unauthenticated request. When set, the relay shows a "Log in" button
    /// on the unauthorized page.
    pub server_web_url: Arc<tokio::sync::OnceCell<String>>,
    /// External URL of the relay's main domain (cfg.proxy_url). When set,
    /// the unauthorized page also offers certificate login via
    /// `{relay_url}/cert-login/{instance}/{service}`.
    pub relay_url: Option<String>,
}

/// Shared CORS config passed via axum Extension.
#[derive(Clone)]
struct CorsConfig(Arc<Vec<String>>);

/// Build the Axum router for proxy endpoints (served on wildcard subdomains).
pub fn router(state: ProxyState) -> Router {
    let cors = CorsConfig(Arc::new(state.cors_origins.clone()));
    Router::new()
        // Bootstrap: stores token as cookie, redirects to /
        .route("/proxy", get(proxy_bootstrap))
        // JSON request/response API (kept for programmatic use)
        .route("/proxy_request", post(proxy_request))
        // File tunnel API (browser → relay direct)
        .route("/api/files/{tunnel_name}", get(file_list))
        .route("/api/files/{tunnel_name}/read", get(file_read))
        .route("/api/files/{tunnel_name}/write", post(file_write))
        // Shell tunnel API
        .route("/api/shell/{command_name}/exec", post(shell_exec))
        // Log tunnel API
        .route("/api/logs", get(log_proxy))
        // Daemon presence check (no forwarding, just registry lookup)
        .route("/api/ping", get(daemon_ping))
        // Catch-all: reverse proxy for HTTP
        .fallback(proxy_catchall)
        .layer(middleware::from_fn(proxy_security_headers))
        .layer(axum::Extension(cors))
        .with_state(state)
}

/// Security headers + CORS for the proxy subdomain.
async fn proxy_security_headers(
    axum::Extension(cors): axum::Extension<CorsConfig>,
    request: axum::extract::Request,
    next: Next,
) -> axum::response::Response {
    let cors_origins = &cors.0;
    let origin = request
        .headers()
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let is_preflight = request.method() == axum::http::Method::OPTIONS;

    // Check if the Origin exactly matches any allowed CORS origin.
    // Entries must be full URLs (e.g. "http://localhost:8080").
    let allowed_origin = origin.as_ref().and_then(|o| {
        if cors_origins.iter().any(|allowed| o == allowed) {
            Some(o.clone())
        } else {
            None
        }
    });

    // Handle CORS preflight (OPTIONS) — return immediately without hitting the handler.
    if is_preflight {
        if let Some(ref ao) = allowed_origin {
            return axum::response::Response::builder()
                .status(StatusCode::NO_CONTENT)
                .header("access-control-allow-origin", ao.as_str())
                .header("access-control-allow-methods", "GET, POST, OPTIONS")
                .header(
                    "access-control-allow-headers",
                    "authorization, content-type, x-proxy-token",
                )
                .header("access-control-expose-headers", "x-file-mtime, x-file-size")
                .header("access-control-max-age", "3600")
                .body(Body::empty())
                .unwrap()
                .into_response();
        } else {
            return StatusCode::FORBIDDEN.into_response();
        }
    }

    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert(
        axum::http::header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );

    // Add CORS headers if the origin is allowed.
    if let Some(ao) = allowed_origin {
        if let Ok(val) = HeaderValue::from_str(&ao) {
            headers.insert("access-control-allow-origin", val);
        }
        headers.insert(
            "access-control-expose-headers",
            HeaderValue::from_static("x-file-mtime, x-file-size"),
        );
    } else {
        // No CORS — keep strict isolation for same-origin proxy requests.
        headers.insert(
            "cross-origin-opener-policy",
            HeaderValue::from_static("same-origin"),
        );
        headers.insert(
            "cross-origin-resource-policy",
            HeaderValue::from_static("same-origin"),
        );
    }

    response
}

// ── Subdomain parsing ──────────────────────────────────────────────────

/// Extract (instance_id_prefix, tunnel_name) from the Host header.
///
/// The subdomain format is `{hex_prefix}-{tunnel_name}` where the prefix is
/// ≥12 hex characters. Since the prefix is hex-only (no dashes), the first
/// dash is always the separator — tunnel names with dashes (e.g. `ai-proxy`)
/// parse correctly.
fn parse_subdomain(headers: &HeaderMap, proxy_hostname: &str) -> Option<(String, String)> {
    let host = headers.get("host").and_then(|v| v.to_str().ok())?;
    let host_no_port = host.split(':').next().unwrap_or(host);
    let subdomain = host_no_port.strip_suffix(&format!(".{proxy_hostname}"))?;

    let dash_pos = subdomain.find('-')?;
    let instance_prefix = &subdomain[..dash_pos];
    let tunnel_name = &subdomain[dash_pos + 1..];

    if instance_prefix.len() < 12
        || !instance_prefix.chars().all(|c| c.is_ascii_hexdigit())
        || tunnel_name.is_empty()
    {
        return None;
    }

    Some((instance_prefix.to_string(), tunnel_name.to_string()))
}

/// Extract the instance ID prefix from a subdomain. Handles both formats:
/// - `{prefix}.relay.plan.ai` (file-tunnel access, no tunnel name)
/// - `{prefix}-{tunnel}.relay.plan.ai` (existing TCP tunnel access)
/// Returns just the instance prefix in both cases.
fn parse_instance_prefix(headers: &HeaderMap, proxy_hostname: &str) -> Option<String> {
    let host = headers.get("host").and_then(|v| v.to_str().ok())?;
    let host_no_port = host.split(':').next().unwrap_or(host);
    let subdomain = host_no_port.strip_suffix(&format!(".{proxy_hostname}"))?;

    let prefix = if let Some(dash_pos) = subdomain.find('-') {
        let candidate = &subdomain[..dash_pos];
        if candidate.len() >= 12 && candidate.chars().all(|c| c.is_ascii_hexdigit()) {
            candidate
        } else {
            subdomain
        }
    } else {
        subdomain
    };

    if prefix.len() >= 12 && prefix.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(prefix.to_string())
    } else {
        None
    }
}

/// Extract proxy_token from X-Proxy-Token header or cookie.
/// Authorization is never consumed — it belongs to the upstream service.
fn extract_token(headers: &HeaderMap) -> Option<String> {
    if let Some(val) = headers.get("x-proxy-token").and_then(|v| v.to_str().ok()) {
        if !val.is_empty() {
            return Some(val.to_string());
        }
    }
    extract_cookie_token(headers)
}

/// Remove the proxy_token cookie from a cookie header value, keeping the rest.
fn strip_proxy_cookie(cookie_header: &str) -> String {
    cookie_header
        .split(';')
        .map(|s| s.trim())
        .filter(|pair| {
            pair.split_once('=')
                .map(|(name, _)| name.trim() != PROXY_TOKEN_COOKIE)
                .unwrap_or(true)
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// Extract proxy_token from cookie.
fn extract_cookie_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all("cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|s| s.split(';'))
        .map(|s| s.trim())
        .find_map(|pair| {
            let (name, value) = pair.split_once('=')?;
            if name.trim() == PROXY_TOKEN_COOKIE {
                Some(value.trim().to_string())
            } else {
                None
            }
        })
}

/// Check whether a token's scopes include the required scope.
/// Empty scopes vec means wildcard (all scopes allowed, for backwards compat).
fn has_scope(scopes: &[String], required: &str) -> bool {
    scopes.is_empty()
        || scopes
            .iter()
            .any(|s| s == "*" || s == required || (required.starts_with("tcp:") && s == "tcp:*"))
}

/// Build a SelfInfo for a valid relay-local proxy token. The token itself is
/// instance-bound (strictly stronger than cluster scoping), so the cluster ID
/// is filled from the registry to satisfy the caller's cluster check.
fn local_self_info(token: &str, state: &ProxyState, instance_id: &str) -> Option<SelfInfo> {
    let scopes = crate::auth::validate_local_proxy_token(token, instance_id)?;
    Some(SelfInfo {
        cluster_id: None,
        cluster_name: None,
        organization_id: None,
        token_kind: "proxy".to_string(),
        cluster_ids: state
            .registry
            .get_cluster_id(instance_id)
            .into_iter()
            .collect(),
        scopes,
    })
}

/// Validate the proxy token and check cluster scoping.
async fn authenticate_proxy(
    headers: &HeaderMap,
    state: &ProxyState,
    instance_id: &str,
) -> Result<SelfInfo, axum::response::Response> {
    let token = extract_token(headers)
        .ok_or_else(|| {
            (StatusCode::UNAUTHORIZED, "Missing proxy_token. Use X-Proxy-Token header or visit /proxy?proxy_token=TOKEN first.").into_response()
        })?;

    // Relay-local tokens (minted by /cert-login) are bound to one instance
    // and one tunnel — validated entirely locally, no server round-trip.
    if let Some(info) = local_self_info(&token, state, instance_id) {
        return Ok(info);
    }

    let self_info = validate_token_cached(&state.server_api_url, &token)
        .await
        .map_err(|s| s.into_response())?;

    if self_info.token_kind != "proxy" && self_info.token_kind != "admin" {
        return Err(StatusCode::FORBIDDEN.into_response());
    }

    if let Some(cid) = state.registry.get_cluster_id(instance_id) {
        if !self_info.cluster_ids.contains(&cid) {
            return Err(StatusCode::FORBIDDEN.into_response());
        }
    }
    // If cluster_id is not known (daemon didn't send it during registration),
    // allow access — the server already validated the token's cluster scope.

    Ok(self_info)
}

/// Authenticate and require a specific scope. Returns 403 if the scope is missing.
async fn authenticate_proxy_scoped(
    headers: &HeaderMap,
    state: &ProxyState,
    instance_id: &str,
    required_scope: &str,
) -> Result<(), axum::response::Response> {
    let self_info = authenticate_proxy(headers, state, instance_id).await?;
    if !has_scope(&self_info.scopes, required_scope) {
        return Err((StatusCode::FORBIDDEN, "insufficient scope").into_response());
    }
    Ok(())
}

/// Resolve the relay swarm and peer ID for an instance, or return an error response.
fn resolve_swarm_and_peer(
    state: &ProxyState,
    instance_id: &str,
) -> Result<(Arc<crate::p2p::RelaySwarm>, libp2p::PeerId), axum::response::Response> {
    let swarm = state
        .relay_swarm
        .as_ref()
        .ok_or_else(|| (StatusCode::SERVICE_UNAVAILABLE, "p2p not available").into_response())?
        .clone();
    let peer_id = state
        .registry
        .resolve_peer_id(instance_id)
        .ok_or_else(|| StatusCode::NOT_FOUND.into_response())?;
    Ok((swarm, peer_id))
}

// ── GET /proxy?proxy_token=... — bootstrap ─────────────────────────────

#[derive(Deserialize)]
struct ProxyBootstrapQuery {
    proxy_token: String,
}

/// Store the proxy_token as an HttpOnly, SameSite=Strict cookie scoped to
/// this subdomain. Serves a small HTML page that navigates to / client-side
/// so the browser treats it as a same-site navigation (required for Strict).
async fn proxy_bootstrap(
    headers: HeaderMap,
    Query(query): Query<ProxyBootstrapQuery>,
    State(state): State<ProxyState>,
) -> axum::response::Response {
    let Some((instance_id, tunnel_name)) = parse_subdomain(&headers, &state.proxy_hostname) else {
        return (StatusCode::BAD_REQUEST, "Invalid proxy hostname").into_response();
    };

    if !state.registry.has_tunnel(&instance_id, &tunnel_name) {
        return (StatusCode::NOT_FOUND, "Tunnel not found").into_response();
    }

    // HttpOnly + SameSite=Strict cookie. No Domain attribute — the browser
    // scopes it to the exact origin (subdomain:port), preventing leakage to
    // sibling subdomains or the parent domain.
    let cookie = format!(
        "{PROXY_TOKEN_COOKIE}={}; Path=/; HttpOnly; SameSite=Strict; Max-Age=21600",
        query.proxy_token,
    );

    // Serve an HTML page instead of a 302 redirect. A redirect after cross-site
    // navigation doesn't send SameSite=Strict cookies. Client-side navigation
    // from within the page is same-site and works correctly.
    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("set-cookie", cookie)
        .header("content-type", "text/html; charset=utf-8")
        .body(Body::from(bootstrap_html()))
        .unwrap()
        .into_response()
}

/// Shared CSS for the dark-themed tunnel pages (loading spinner, auth error).
const SHARED_STYLE: &str = r#"
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body {
    height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 24px;
    background: #1e1e2e;
    color: #cdd6f4;
    font-family: system-ui, -apple-system, sans-serif;
  }
  .text { font-size: 14px; color: #6c7086; letter-spacing: 0.02em; }
"#;

/// Build the "connecting to tunnel" bootstrap HTML with the shared style inlined.
fn bootstrap_html() -> String {
    format!(
        r#"<!DOCTYPE html>
<html><head>
<meta charset="utf-8">
<meta http-equiv="refresh" content="0;url=/">
<style>{SHARED_STYLE}
  .spinner {{
    width: 48px; height: 48px;
    border: 3px solid #313244;
    border-top-color: #89b4fa;
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }}
  @keyframes spin {{ to {{ transform: rotate(360deg); }} }}
</style>
</head>
<body>
  <div class="spinner"></div>
  <div class="text">Connecting to tunnel...</div>
</body>
</html>"#,
    )
}

/// Build the "authentication required" HTML page, optionally with sign-in
/// and certificate-login buttons.
fn unauthorized_html(
    login_url: Option<&str>,
    cert_login_url: Option<&str>,
    accept_language: &str,
) -> axum::response::Response {
    let lang = plan_ai_html::Lang::from_accept_language(accept_language);
    let button = login_url
        .map(|url| {
            format!(
                r#"<a href="{}" class="btn btn-primary btn-lg" style="display:flex;margin-top:1rem">{}</a>"#,
                plan_ai_html::escape(url),
                plan_ai_html::tr(lang, "log-in-to-planai"),
            )
        })
        .unwrap_or_default();
    let cert_button = cert_login_url
        .map(|url| {
            format!(
                r#"<a href="{}" class="btn btn-secondary btn-lg" style="display:flex;margin-top:.6rem">{}</a>"#,
                plan_ai_html::escape(url),
                plan_ai_html::tr(lang, "log-in-with-certificate"),
            )
        })
        .unwrap_or_default();
    let title = plan_ai_html::tr(lang, "auth-required-title");
    let body = format!(
        r#"<h1 class="h-page">{title}</h1><p class="help">{}</p>{button}{cert_button}"#,
        plan_ai_html::tr(lang, "auth-required-body"),
    );
    let html = plan_ai_html::Page::new(&title, body).lang(lang).render();
    axum::response::Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header("content-type", "text/html; charset=utf-8")
        .body(Body::from(html))
        .unwrap()
        .into_response()
}

/// Fetch the server's external web URL from `/api/server-info`.
pub(crate) async fn fetch_server_web_url(server_api_url: &str) -> Result<String, anyhow::Error> {
    #[derive(serde::Deserialize)]
    struct ServerInfo {
        web_url: String,
    }
    let url = format!("{}/api/server-info", server_api_url.trim_end_matches('/'));
    let resp: ServerInfo = reqwest::Client::new()
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(resp.web_url.trim_end_matches('/').to_string())
}

/// Render a localized HTML error page for browser-facing failures.
fn error_page(
    headers: &axum::http::HeaderMap,
    code: StatusCode,
    title_key: &str,
    body_key: &str,
) -> axum::response::Response {
    let lang = plan_ai_html::Lang::from_accept_language(
        headers
            .get("accept-language")
            .and_then(|v| v.to_str().ok())
            .unwrap_or(""),
    );
    let html = plan_ai_html::error_page(lang, title_key, body_key);
    (code, [("content-type", "text/html; charset=utf-8")], html).into_response()
}

/// True when the client prefers an HTML error page (browser navigation).
fn wants_html(headers: &HeaderMap) -> bool {
    headers
        .get("accept")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.contains("text/html"))
}

/// Localized HTML error page that also shows the upstream error detail.
fn error_page_with_detail(
    headers: &HeaderMap,
    code: StatusCode,
    detail: &str,
) -> axum::response::Response {
    let lang = plan_ai_html::Lang::from_accept_language(
        headers
            .get("accept-language")
            .and_then(|v| v.to_str().ok())
            .unwrap_or(""),
    );
    let (title_key, body_key) = if code == StatusCode::NOT_FOUND {
        ("not-found-title", "not-found-body")
    } else {
        ("unreachable-title", "unreachable-body")
    };
    let title = plan_ai_html::tr(lang, title_key);
    let body = plan_ai_html::components::heading(&title)
        + &plan_ai_html::components::muted(&plan_ai_html::tr(lang, body_key))
        + &plan_ai_html::components::error(detail);
    let html = plan_ai_html::Page::new(&title, body).lang(lang).render();
    (code, [("content-type", "text/html; charset=utf-8")], html).into_response()
}

/// Build the styled sign-in page for a tunnel, resolving the server web URL
/// lazily for the login button.
async fn unauthorized_response(
    state: &ProxyState,
    headers: &HeaderMap,
    instance_id: &str,
    tunnel_name: &str,
) -> axum::response::Response {
    let web_url = state
        .server_web_url
        .get_or_try_init(|| fetch_server_web_url(&state.server_api_url))
        .await
        .ok();
    let prefix = &instance_id[..std::cmp::min(12, instance_id.len())];
    let login_url = web_url.map(|url| format!("{url}/easy-access/direct/{prefix}/{tunnel_name}"));
    let cert_login_url = state.relay_url.as_deref().map(|url| {
        format!(
            "{}/cert-login/{prefix}/{tunnel_name}",
            url.trim_end_matches('/')
        )
    });
    let accept_language = headers
        .get("accept-language")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    unauthorized_html(
        login_url.as_deref(),
        cert_login_url.as_deref(),
        accept_language,
    )
}

// ── Catch-all: reverse proxy ───────────────────────────────────────────

/// Handles all non-special requests by proxying them to the daemon's tunnel
/// via libp2p.
async fn proxy_catchall(
    State(state): State<ProxyState>,
    connect_info: axum::extract::ConnectInfo<std::net::SocketAddr>,
    req: Request,
) -> axum::response::Response {
    let headers = req.headers().clone();
    let method = req.method().clone();
    let path = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str().to_string())
        .unwrap_or_else(|| "/".to_string());

    let Some((instance_id, tunnel_name)) = parse_subdomain(&headers, &state.proxy_hostname) else {
        return error_page(
            &headers,
            StatusCode::NOT_FOUND,
            "unknown-host-title",
            "unknown-host-body",
        );
    };

    // If no token at all, show a styled "sign in" page instead of a plain 401.
    if extract_token(&headers).is_none() {
        return unauthorized_response(&state, &headers, &instance_id, &tunnel_name).await;
    }

    let self_info = match authenticate_proxy(&headers, &state, &instance_id).await {
        Ok(info) => info,
        // Stale or invalid token in a browser: show the sign-in page again
        // rather than a bare 401.
        Err(resp) if resp.status() == StatusCode::UNAUTHORIZED => {
            return unauthorized_response(&state, &headers, &instance_id, &tunnel_name).await;
        }
        Err(resp) => {
            let code = resp.status();
            let (title_key, body_key) = if code == StatusCode::FORBIDDEN {
                ("forbidden-title", "forbidden-body")
            } else {
                ("error-title", "error-body")
            };
            return error_page(&headers, code, title_key, body_key);
        }
    };
    let required_scope = format!("tcp:{tunnel_name}");
    if !has_scope(&self_info.scopes, &required_scope) {
        return error_page(
            &headers,
            StatusCode::FORBIDDEN,
            "forbidden-title",
            "forbidden-body",
        );
    }

    let (swarm, peer_id) = match resolve_swarm_and_peer(&state, &instance_id) {
        Ok(v) => v,
        // Daemon offline or p2p down — either way the service is unreachable.
        Err(resp) => {
            return error_page(
                &headers,
                resp.status(),
                "unreachable-title",
                "unreachable-body",
            );
        }
    };

    if !state.registry.has_tunnel(&instance_id, &tunnel_name) {
        return error_page(
            &headers,
            StatusCode::NOT_FOUND,
            "not-found-title",
            "not-found-body",
        );
    }

    // Collect request headers to forward.
    // Strip hop-by-hop and relay-internal headers. For cookies, remove our
    // proxy_token but forward the rest so upstream services keep their
    // session cookies. The x-proxy-token header is relay-only auth and must
    // not leak to the upstream service.
    let mut fwd_headers: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(k, v)| {
            let lk = k.as_str().to_lowercase();
            if lk == "connection" || lk == "transfer-encoding" || lk == "x-proxy-token" {
                return None;
            }
            if lk == "cookie" {
                let filtered = strip_proxy_cookie(v.to_str().unwrap_or(""));
                return if filtered.is_empty() {
                    None
                } else {
                    Some(("cookie".to_string(), filtered))
                };
            }
            Some((k.to_string(), v.to_str().unwrap_or("").to_string()))
        })
        .collect();

    // Add reverse-proxy headers (X-Real-IP, X-Forwarded-For, X-Forwarded-Proto)
    let client_ip = connect_info.0.ip().to_string();
    fwd_headers.push(("x-real-ip".to_string(), client_ip.clone()));
    let xff = match headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        Some(existing) => format!("{existing}, {client_ip}"),
        None => client_ip,
    };
    fwd_headers.push(("x-forwarded-for".to_string(), xff));
    let proto = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("http");
    fwd_headers.push(("x-forwarded-proto".to_string(), proto.to_string()));

    // Collect request body
    let body_bytes = match axum::body::to_bytes(req.into_body(), 16 * 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => {
            return error_page(
                &headers,
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload-too-large-title",
                "payload-too-large-body",
            );
        }
    };

    let body_b64 = if body_bytes.is_empty() {
        None
    } else {
        use base64::Engine;
        Some(base64::engine::general_purpose::STANDARD.encode(&body_bytes))
    };

    tracing::debug!("proxy {method} {path} -> {instance_id}/{tunnel_name} via tunnel stream");

    // Open a tunnel data substream to the daemon and stream the proxy request/response.
    let handshake = serde_json::json!({
        "type": "proxy",
        "tunnel_name": tunnel_name,
        "method": method.to_string(),
        "path": path,
        "headers": fwd_headers,
        "body": body_b64,
    });

    let mut tunnel = match tokio::time::timeout(
        Duration::from_secs(10),
        swarm.open_tunnel_stream(peer_id),
    )
    .await
    {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            tracing::warn!(%peer_id, "failed to open tunnel stream: {e}");
            return error_page(
                &headers,
                StatusCode::BAD_GATEWAY,
                "unreachable-title",
                "unreachable-body",
            );
        }
        Err(_) => {
            return error_page(
                &headers,
                StatusCode::GATEWAY_TIMEOUT,
                "unreachable-title",
                "unreachable-body",
            );
        }
    };

    // Send handshake frame.
    {
        use futures_util::AsyncWriteExt;
        let data = serde_json::to_vec(&handshake).unwrap_or_default();
        let len = (data.len() as u32).to_be_bytes();
        if tunnel.write_all(&len).await.is_err() || tunnel.write_all(&data).await.is_err() {
            return error_page(
                &headers,
                StatusCode::BAD_GATEWAY,
                "unreachable-title",
                "unreachable-body",
            );
        }
        let _ = tunnel.flush().await;
    }

    // Read streamed response: JSON header + binary body chunks.
    let hdr = match crate::tunnel_io::read_json_frame(&mut tunnel).await {
        Ok(h) => h,
        Err(_) => {
            return error_page(
                &headers,
                StatusCode::BAD_GATEWAY,
                "unreachable-title",
                "unreachable-body",
            );
        }
    };

    let status = hdr["status"].as_u64().unwrap_or(502) as u16;

    if let Some(error) = hdr["error"].as_str() {
        let code = StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
        // Browsers get a styled page; keep plain text for programmatic clients
        // that parse the error body.
        if wants_html(&headers) {
            return error_page_with_detail(&headers, code, error);
        }
        return (code, error.to_string()).into_response();
    }

    let resp_headers: Vec<(String, String)> = hdr["headers"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    let pair = v.as_array()?;
                    Some((
                        pair.first()?.as_str()?.to_string(),
                        pair.get(1)?.as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();

    let mut builder = axum::response::Response::builder().status(status);
    for (k, v) in &resp_headers {
        let lk = k.to_lowercase();
        // transfer-encoding + content-length describe the upstream's framing, which we
        // re-frame here (the body below is streamed, so axum uses chunked framing), so
        // drop them. But content-encoding describes the BODY's compression (gzip/zstd/
        // br) and we forward the body verbatim — still compressed — so it MUST be
        // preserved, else the client renders raw compressed bytes as garbage.
        if lk == "transfer-encoding" || lk == "content-length" {
            continue;
        }
        if let Ok(val) = HeaderValue::from_str(v) {
            builder = builder.header(k.as_str(), val);
        }
    }

    // Stream body chunks through as they arrive instead of buffering the
    // whole body: SSE and other long-lived responses never end, so
    // buffering would hang the request forever.
    let body_stream = crate::tunnel_io::read_binary_chunks_stream(tunnel);
    builder
        .body(Body::from_stream(body_stream))
        .unwrap()
        .into_response()
}

// ── POST /proxy_request — JSON request/response API ────────────────────

#[derive(Debug, Deserialize)]
struct ProxyRequestBody {
    proxy_token: String,
    method: String,
    path: String,
    headers: std::collections::HashMap<String, String>,
    body: Option<String>,
}

async fn proxy_request(
    req_headers: HeaderMap,
    State(state): State<ProxyState>,
    Json(body): Json<ProxyRequestBody>,
) -> axum::response::Response {
    let Some((instance_id, tunnel_name)) = parse_subdomain(&req_headers, &state.proxy_hostname)
    else {
        return (StatusCode::BAD_REQUEST, "Invalid proxy hostname").into_response();
    };

    let self_info = match local_self_info(&body.proxy_token, &state, &instance_id) {
        Some(info) => info,
        None => match validate_token(&state.server_api_url, &body.proxy_token).await {
            Ok(info) => info,
            Err(status) => return status.into_response(),
        },
    };

    if self_info.token_kind != "proxy" && self_info.token_kind != "admin" {
        return StatusCode::FORBIDDEN.into_response();
    }

    if let Some(cid) = state.registry.get_cluster_id(&instance_id) {
        if !self_info.cluster_ids.contains(&cid) {
            return StatusCode::FORBIDDEN.into_response();
        }
    }

    let required_scope = format!("tcp:{tunnel_name}");
    if !has_scope(&self_info.scopes, &required_scope) {
        return (StatusCode::FORBIDDEN, "insufficient scope").into_response();
    }

    let (swarm, peer_id) = match resolve_swarm_and_peer(&state, &instance_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let request_id = Uuid::new_v4().to_string();
    let headers: Vec<(String, String)> = body.headers.into_iter().collect();
    let req = serde_json::json!({
        "type": "proxy_request",
        "request_id": request_id,
        "tunnel_name": tunnel_name,
        "method": body.method,
        "path": body.path,
        "headers": headers,
        "body": body.body,
    });
    match tokio::time::timeout(Duration::from_secs(60), swarm.send_request(peer_id, req)).await {
        Ok(Ok(resp)) => Json(resp).into_response(),
        Ok(Err(e)) => {
            tracing::warn!(%peer_id, "p2p proxy request failed: {e}");
            StatusCode::BAD_GATEWAY.into_response()
        }
        Err(_) => StatusCode::GATEWAY_TIMEOUT.into_response(),
    }
}

// ── File tunnel endpoints ──────────────────────────────────────────────
//
// Served on the proxy subdomain (both {instance}.relay and {instance}-{tunnel}.relay).
// The instance is extracted from the subdomain; the file tunnel name from the path.
// Auth: Bearer token or proxy_token cookie.

#[derive(Debug, Deserialize)]
struct FileQuery {
    path: Option<String>,
}

/// List files in a file tunnel.
async fn file_list(
    headers: HeaderMap,
    Path(tunnel_name): Path<String>,
    Query(query): Query<FileQuery>,
    State(state): State<ProxyState>,
) -> axum::response::Response {
    let Some(instance_id) = parse_instance_prefix(&headers, &state.proxy_hostname) else {
        return (StatusCode::BAD_REQUEST, "Invalid proxy hostname").into_response();
    };
    if let Err(resp) = authenticate_proxy_scoped(&headers, &state, &instance_id, "files:read").await
    {
        return resp;
    }
    let (swarm, peer_id) = match resolve_swarm_and_peer(&state, &instance_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let path_str = query.path.as_deref().unwrap_or("/");
    tracing::info!(instance = %instance_id, tunnel = %tunnel_name, path = %path_str, "file_list");

    let handshake = serde_json::json!({
        "type": "file_list",
        "tunnel_name": tunnel_name,
        "path": query.path,
    });
    match crate::tunnel_io::open_and_read_json(&swarm, peer_id, handshake, Duration::from_secs(30))
        .await
    {
        Ok(resp) => Json(resp).into_response(),
        Err(status) => status.into_response(),
    }
}

/// Read a file via tunnel substream.
async fn file_read(
    headers: HeaderMap,
    Path(tunnel_name): Path<String>,
    Query(query): Query<FileQuery>,
    State(state): State<ProxyState>,
) -> axum::response::Response {
    let Some(instance_id) = parse_instance_prefix(&headers, &state.proxy_hostname) else {
        return (StatusCode::BAD_REQUEST, "Invalid proxy hostname").into_response();
    };
    if let Err(resp) = authenticate_proxy_scoped(&headers, &state, &instance_id, "files:read").await
    {
        return resp;
    }
    let (swarm, peer_id) = match resolve_swarm_and_peer(&state, &instance_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let path_str = query.path.as_deref().unwrap_or("?");
    tracing::info!(instance = %instance_id, tunnel = %tunnel_name, path = %path_str, "file_read");

    let handshake = serde_json::json!({
        "type": "file_read",
        "tunnel_name": tunnel_name,
        "path": query.path,
    });

    // Open tunnel, read stream_framing response (JSON header + binary chunks).
    let mut tunnel =
        match crate::tunnel_io::open(&swarm, peer_id, &handshake, Duration::from_secs(10)).await {
            Ok(t) => t,
            Err(status) => return status.into_response(),
        };

    let hdr = match crate::tunnel_io::read_json_frame(&mut tunnel).await {
        Ok(h) => h,
        Err(s) => return s.into_response(),
    };

    let status = hdr["status"].as_u64().unwrap_or(502) as u16;
    if status != 200 {
        let error = hdr["error"].as_str().unwrap_or("error");
        return axum::response::Response::builder()
            .status(status)
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({ "error": error }).to_string(),
            ))
            .unwrap()
            .into_response();
    }

    let size = hdr["size"].as_u64().unwrap_or(0);
    let mtime = hdr["mtime"].as_i64().unwrap_or(0);

    let body_data = crate::tunnel_io::read_binary_body(&mut tunnel).await;

    axum::response::Response::builder()
        .status(200)
        .header("content-type", "application/octet-stream")
        .header("x-file-mtime", mtime.to_string())
        .header("x-file-size", size.to_string())
        .header("access-control-expose-headers", "x-file-mtime, x-file-size")
        .body(Body::from(body_data))
        .unwrap()
        .into_response()
}

#[derive(Debug, Deserialize)]
struct FileWriteQuery {
    path: Option<String>,
    expected_mtime: Option<i64>,
}

/// Write a file via tunnel substream.
async fn file_write(
    headers: HeaderMap,
    Path(tunnel_name): Path<String>,
    Query(query): Query<FileWriteQuery>,
    State(state): State<ProxyState>,
    body: Body,
) -> axum::response::Response {
    use futures_util::StreamExt;

    let Some(instance_id) = parse_instance_prefix(&headers, &state.proxy_hostname) else {
        return (StatusCode::BAD_REQUEST, "Invalid proxy hostname").into_response();
    };
    if let Err(resp) =
        authenticate_proxy_scoped(&headers, &state, &instance_id, "files:write").await
    {
        return resp;
    }
    let (swarm, peer_id) = match resolve_swarm_and_peer(&state, &instance_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let path_str = query.path.as_deref().unwrap_or("?");
    tracing::info!(instance = %instance_id, tunnel = %tunnel_name, path = %path_str, "file_write");

    // Collect request body.
    let mut body_stream = body.into_data_stream();
    let mut body_bytes = Vec::new();
    while let Some(chunk) = body_stream.next().await {
        match chunk {
            Ok(bytes) => body_bytes.extend_from_slice(&bytes),
            Err(_) => break,
        }
    }

    use base64::Engine;
    let body_b64 = base64::engine::general_purpose::STANDARD.encode(&body_bytes);

    let handshake = serde_json::json!({
        "type": "file_write",
        "tunnel_name": tunnel_name,
        "path": query.path,
        "expected_mtime": query.expected_mtime,
        "data": body_b64,
    });

    match crate::tunnel_io::open_and_read_json(&swarm, peer_id, handshake, Duration::from_secs(60))
        .await
    {
        Ok(resp) => {
            let status = resp["status"].as_u64().unwrap_or(500) as u16;
            axum::response::Response::builder()
                .status(status)
                .header("content-type", "application/json")
                .body(Body::from(resp.to_string()))
                .unwrap()
                .into_response()
        }
        Err(status) => status.into_response(),
    }
}

// ── Shell tunnel endpoint ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ShellExecBody {
    #[serde(default)]
    user_arg: Option<String>,
}

/// Execute a predefined shell command via libp2p request.
async fn shell_exec(
    headers: HeaderMap,
    Path(command_name): Path<String>,
    State(state): State<ProxyState>,
    Json(body): Json<ShellExecBody>,
) -> axum::response::Response {
    let Some(instance_id) = parse_instance_prefix(&headers, &state.proxy_hostname) else {
        return (StatusCode::BAD_REQUEST, "Invalid proxy hostname").into_response();
    };
    if let Err(resp) = authenticate_proxy_scoped(&headers, &state, &instance_id, "shell:exec").await
    {
        return resp;
    }
    let (swarm, peer_id) = match resolve_swarm_and_peer(&state, &instance_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let arg_str = body.user_arg.as_deref().unwrap_or("");
    tracing::info!(instance = %instance_id, command = %command_name, arg = %arg_str, "shell_exec");

    // Open a tunnel substream for shell execution.
    let handshake = serde_json::json!({
        "type": "shell",
        "command_name": command_name,
        "user_arg": body.user_arg,
    });

    let mut tunnel = match tokio::time::timeout(
        Duration::from_secs(10),
        swarm.open_tunnel_stream(peer_id),
    )
    .await
    {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            tracing::warn!(%instance_id, "shell_exec: failed to open tunnel: {e}");
            return StatusCode::BAD_GATEWAY.into_response();
        }
        Err(_) => return StatusCode::GATEWAY_TIMEOUT.into_response(),
    };

    // Send handshake.
    {
        use futures_util::AsyncWriteExt;
        let data = serde_json::to_vec(&handshake).unwrap_or_default();
        if tunnel
            .write_all(&(data.len() as u32).to_be_bytes())
            .await
            .is_err()
            || tunnel.write_all(&data).await.is_err()
        {
            return StatusCode::BAD_GATEWAY.into_response();
        }
        let _ = tunnel.flush().await;
    }

    // Stream output as SSE events in real-time.
    use axum::response::sse::{Event, KeepAlive, Sse};
    use futures_util::StreamExt;

    let json_stream = crate::tunnel_io::read_json_frames_stream(tunnel);
    let sse_stream = json_stream.map(|result| match result {
        Ok(frame) => {
            let data = serde_json::to_string(&frame).unwrap_or_default();
            Ok::<_, std::convert::Infallible>(Event::default().data(data))
        }
        Err(_) => Ok(Event::default().data("{\"error\":\"stream error\"}")),
    });

    Sse::new(sse_stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

// ── Log tunnel endpoint ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct LogQuery {
    n: Option<usize>,
    service: Option<String>,
    after: Option<usize>,
}

/// Lightweight daemon presence check. Returns 200 if the daemon is connected
/// to the relay, 404 if not. No forwarding — just a registry lookup.
async fn daemon_ping(
    headers: HeaderMap,
    State(state): State<ProxyState>,
) -> axum::response::Response {
    let Some(instance_id) = parse_instance_prefix(&headers, &state.proxy_hostname) else {
        return (StatusCode::BAD_REQUEST, "Invalid proxy hostname").into_response();
    };
    if authenticate_proxy(&headers, &state, &instance_id)
        .await
        .is_err()
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let online = state.registry.is_connected(&instance_id);
    tracing::debug!(instance = %instance_id, online, "daemon_ping");
    if online {
        (StatusCode::OK, "ok").into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

/// Proxy the daemon's /logs endpoint via libp2p.
async fn log_proxy(
    headers: HeaderMap,
    Query(query): Query<LogQuery>,
    State(state): State<ProxyState>,
) -> axum::response::Response {
    let Some(instance_id) = parse_instance_prefix(&headers, &state.proxy_hostname) else {
        return (StatusCode::BAD_REQUEST, "Invalid proxy hostname").into_response();
    };
    if let Err(resp) = authenticate_proxy_scoped(&headers, &state, &instance_id, "logs:read").await
    {
        return resp;
    }
    let (swarm, peer_id) = match resolve_swarm_and_peer(&state, &instance_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    // Build the daemon-local /logs query string
    let mut params = Vec::new();
    if let Some(n) = query.n {
        params.push(format!("n={n}"));
    }
    if let Some(ref svc) = query.service {
        params.push(format!("service={svc}"));
    }
    if let Some(after) = query.after {
        params.push(format!("after={after}"));
    }
    let path = if params.is_empty() {
        "/logs".to_string()
    } else {
        format!("/logs?{}", params.join("&"))
    };

    let request_id = Uuid::new_v4().to_string();
    let req = serde_json::json!({
        "type": "metrics_request",
        "request_id": request_id,
        "path": path,
    });

    match tokio::time::timeout(Duration::from_secs(30), swarm.send_request(peer_id, req)).await {
        Ok(Ok(resp)) => {
            let status = resp["status"].as_u64().unwrap_or(502) as u16;
            let content_type = resp["content_type"].as_str().unwrap_or("application/json");
            let body = resp["body"].as_str().unwrap_or("");
            axum::response::Response::builder()
                .status(status)
                .header("content-type", content_type)
                .body(Body::from(body.to_string()))
                .unwrap()
                .into_response()
        }
        Ok(Err(e)) => {
            tracing::warn!(instance = %instance_id, "log_proxy p2p failed: {e}");
            StatusCode::BAD_GATEWAY.into_response()
        }
        Err(_) => StatusCode::GATEWAY_TIMEOUT.into_response(),
    }
}
