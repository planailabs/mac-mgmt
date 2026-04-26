use axum::Router;
use axum::body::Body;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{FromRequest, Json, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use axum::extract::Path;
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;

use crate::bridge;
use crate::daemon_registry::{ControlMsg, DaemonRegistry, ProxyResponse, ProxyStreamEvent};
use crate::ws_handler::{SelfInfo, validate_token};

/// Cache validated proxy tokens for 5 minutes to avoid hitting the server API
/// on every single proxied request.
static TOKEN_CACHE: std::sync::LazyLock<
    tokio::sync::RwLock<HashMap<String, (SelfInfo, std::time::Instant)>>,
> = std::sync::LazyLock::new(|| tokio::sync::RwLock::new(HashMap::new()));

const TOKEN_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(300);

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
        // Catch-all: reverse proxy for HTTP and WS upgrades
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
                    "authorization, content-type",
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
fn parse_subdomain(headers: &HeaderMap, proxy_hostname: &str) -> Option<(String, String)> {
    let host = headers.get("host").and_then(|v| v.to_str().ok())?;
    let host_no_port = host.split(':').next().unwrap_or(host);
    let subdomain = host_no_port.strip_suffix(&format!(".{proxy_hostname}"))?;

    let dash_pos = subdomain.rfind('-')?;
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

    // If subdomain contains a dash and the part before the last dash is long
    // enough to be an instance prefix, parse it. Otherwise treat the whole
    // subdomain as the prefix.
    let prefix = if let Some(dash_pos) = subdomain.rfind('-') {
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

/// Extract proxy_token from Bearer header or cookie.
fn extract_token(headers: &HeaderMap) -> Option<String> {
    // Try Bearer header first (for API/CORS calls from the web UI).
    if let Some(auth) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        if let Some(token) = auth.strip_prefix("Bearer ") {
            return Some(token.to_string());
        }
    }
    // Fall back to cookie.
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
        || scopes.iter().any(|s| {
            s == "*"
                || s == required
                || (required.starts_with("tcp:") && s == "tcp:*")
        })
}

/// Validate the proxy token (from Bearer header or cookie) and check cluster scoping.
/// Returns the SelfInfo so callers can check scopes.
async fn authenticate_proxy(
    headers: &HeaderMap,
    state: &ProxyState,
    instance_id: &str,
) -> Result<SelfInfo, axum::response::Response> {
    let token = extract_token(headers)
        .ok_or_else(|| {
            (StatusCode::UNAUTHORIZED, "Missing proxy_token. Use Authorization: Bearer <token> or visit /proxy?proxy_token=TOKEN first.").into_response()
        })?;

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
    } else if self_info.token_kind != "admin" {
        // Unscoped instance — deny non-admin access.
        return Err(StatusCode::FORBIDDEN.into_response());
    }

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

    if state
        .registry
        .find_tunnel(&instance_id, &tunnel_name)
        .is_none()
    {
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
        .body(Body::from(PROXY_BOOTSTRAP_HTML))
        .unwrap()
        .into_response()
}

const PROXY_BOOTSTRAP_HTML: &str = r#"<!DOCTYPE html>
<html><head>
<meta charset="utf-8">
<meta http-equiv="refresh" content="0;url=/">
<style>
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
  .spinner {
    width: 48px; height: 48px;
    border: 3px solid #313244;
    border-top-color: #89b4fa;
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
  .text { font-size: 14px; color: #6c7086; letter-spacing: 0.02em; }
</style>
</head>
<body>
  <div class="spinner"></div>
  <div class="text">Connecting to tunnel...</div>
</body>
</html>"#;

// ── Catch-all: reverse proxy ───────────────────────────────────────────

/// Handles all non-special requests by proxying them to the daemon's tunnel.
/// Detects WebSocket upgrades and routes them through proxy sessions.
/// Regular HTTP (including SSE) is streamed via a proxy session.
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
        return (StatusCode::BAD_REQUEST, "Invalid proxy hostname").into_response();
    };

    let self_info = match authenticate_proxy(&headers, &state, &instance_id).await {
        Ok(info) => info,
        Err(resp) => return resp,
    };
    let required_scope = format!("tcp:{tunnel_name}");
    if !has_scope(&self_info.scopes, &required_scope) {
        return (StatusCode::FORBIDDEN, "insufficient scope").into_response();
    }

    let Some((control_tx, _)) = state.registry.find_tunnel(&instance_id, &tunnel_name) else {
        return (StatusCode::NOT_FOUND, "Tunnel not found").into_response();
    };

    // WebSocket upgrade: route through proxy session bridge
    let is_ws_upgrade = headers
        .get("upgrade")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("websocket"));

    if is_ws_upgrade {
        let ws = match WebSocketUpgrade::from_request(req, &state).await {
            Ok(ws) => ws,
            Err(e) => return e.into_response(),
        };

        let session_id = Uuid::new_v4().to_string();
        let session_secret = Uuid::new_v4().to_string();

        return ws
            .on_upgrade(move |socket| async move {
                tracing::info!("WS upgrade for tunnel {tunnel_name} path {path}");

                bridge::register_pending_proxy_session(
                    session_id.clone(),
                    session_secret.clone(),
                    socket,
                );

                if control_tx
                    .send(ControlMsg::ProxySessionRequest {
                        session_id: session_id.clone(),
                        session_secret: session_secret.clone(),
                        tunnel_name,
                        mode: "websocket".to_string(),
                        path,
                    })
                    .await
                    .is_err()
                {
                    tracing::warn!("proxy catchall WS: daemon control channel closed");
                    bridge::remove_pending_proxy_session(&session_id);
                }
            })
            .into_response();
    }

    // Collect request headers to forward.
    // Strip hop-by-hop headers. For cookies, remove our proxy_token but
    // forward the rest so upstream services keep their session cookies.
    let mut fwd_headers: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(k, v)| {
            let lk = k.as_str().to_lowercase();
            if lk == "connection" || lk == "transfer-encoding" {
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
        Err(_) => return (StatusCode::PAYLOAD_TOO_LARGE, "Request body too large").into_response(),
    };

    let body_b64 = if body_bytes.is_empty() {
        None
    } else {
        use base64::Engine;
        Some(base64::engine::general_purpose::STANDARD.encode(&body_bytes))
    };

    // Send a streaming proxy request over the existing control channel.
    // No new WS connection needed — responses stream back as tagged messages.
    tracing::debug!("proxy {method} {path} -> {instance_id}/{tunnel_name}");
    let request_id = Uuid::new_v4().to_string();
    let (stream_tx, mut stream_rx) = tokio::sync::mpsc::channel::<ProxyStreamEvent>(64);

    if control_tx
        .send(ControlMsg::ProxyStream {
            request_id,
            tunnel_name,
            method: method.to_string(),
            path,
            headers: fwd_headers,
            body: body_b64,
            response_tx: stream_tx,
        })
        .await
        .is_err()
    {
        return StatusCode::BAD_GATEWAY.into_response();
    }

    // Wait for response headers from daemon (first event).
    let headers_event =
        match tokio::time::timeout(std::time::Duration::from_secs(60), stream_rx.recv()).await {
            Ok(Some(ProxyStreamEvent::Headers { status, headers })) => (status, headers),
            _ => return StatusCode::GATEWAY_TIMEOUT.into_response(),
        };

    let (status, resp_headers) = headers_event;

    // Stream body chunks as they arrive from daemon via the control channel.
    let body_stream = futures_util::stream::unfold(stream_rx, |mut rx| async move {
        match rx.recv().await {
            Some(ProxyStreamEvent::BodyChunk(data)) => Some((
                Ok::<_, std::convert::Infallible>(axum::body::Bytes::from(data)),
                rx,
            )),
            Some(ProxyStreamEvent::End) | None => None,
            Some(ProxyStreamEvent::Headers { .. }) => None, // unexpected
        }
    });

    let mut builder = axum::response::Response::builder().status(status);
    for (k, v) in &resp_headers {
        let lk = k.to_lowercase();
        if lk == "transfer-encoding" || lk == "content-length" || lk == "content-encoding" {
            continue;
        }
        if let Ok(val) = HeaderValue::from_str(v) {
            builder = builder.header(k.as_str(), val);
        }
    }

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

    let self_info = match validate_token(&state.server_api_url, &body.proxy_token).await {
        Ok(info) => info,
        Err(status) => return status.into_response(),
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

    let Some((control_tx, _)) = state.registry.find_tunnel(&instance_id, &tunnel_name) else {
        return (StatusCode::NOT_FOUND, "Tunnel not found").into_response();
    };

    let headers: Vec<(String, String)> = body.headers.into_iter().collect();

    let request_id = Uuid::new_v4().to_string();
    let (response_tx, response_rx) = tokio::sync::oneshot::channel::<ProxyResponse>();

    if control_tx
        .send(ControlMsg::ProxyRequest {
            request_id,
            tunnel_name,
            method: body.method,
            path: body.path,
            headers,
            body: body.body,
            response_tx,
        })
        .await
        .is_err()
    {
        return StatusCode::BAD_GATEWAY.into_response();
    }

    match tokio::time::timeout(std::time::Duration::from_secs(60), response_rx).await {
        Ok(Ok(resp)) => {
            let response = serde_json::json!({
                "status": resp.status,
                "headers": resp.headers,
                "body": if resp.body.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(resp.body) },
            });
            Json(response).into_response()
        }
        Ok(Err(_)) => StatusCode::BAD_GATEWAY.into_response(),
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
    if let Err(resp) = authenticate_proxy_scoped(&headers, &state, &instance_id, "files:read").await {
        return resp;
    }
    let Some(control_tx) = state.registry.resolve_control_tx(&instance_id) else {
        tracing::debug!(instance = %instance_id, tunnel = %tunnel_name, "file_list: daemon not found");
        return StatusCode::NOT_FOUND.into_response();
    };

    let path_str = query.path.as_deref().unwrap_or("/");
    tracing::info!(instance = %instance_id, tunnel = %tunnel_name, path = %path_str, "file_list");

    let request_id = Uuid::new_v4().to_string();
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    if control_tx
        .send(ControlMsg::FileListRequest {
            request_id,
            tunnel_name: tunnel_name.clone(),
            path: query.path,
            response_tx,
        })
        .await
        .is_err()
    {
        tracing::warn!(instance = %instance_id, tunnel = %tunnel_name, "file_list: control channel closed");
        return StatusCode::BAD_GATEWAY.into_response();
    }
    match tokio::time::timeout(Duration::from_secs(30), response_rx).await {
        Ok(Ok(resp)) => {
            tracing::debug!(instance = %instance_id, tunnel = %tunnel_name, status = resp.status, "file_list completed");
            axum::response::Response::builder()
                .status(resp.status)
                .header("content-type", "application/json")
                .body(Body::from(resp.body.to_string()))
                .unwrap()
                .into_response()
        }
        Ok(Err(_)) => {
            tracing::warn!(instance = %instance_id, tunnel = %tunnel_name, "file_list: daemon dropped response");
            StatusCode::BAD_GATEWAY.into_response()
        }
        Err(_) => {
            tracing::warn!(instance = %instance_id, tunnel = %tunnel_name, "file_list: timeout (30s)");
            StatusCode::GATEWAY_TIMEOUT.into_response()
        }
    }
}

/// Read a file via a dedicated data WebSocket session.
async fn file_read(
    headers: HeaderMap,
    Path(tunnel_name): Path<String>,
    Query(query): Query<FileQuery>,
    State(state): State<ProxyState>,
) -> axum::response::Response {
    use futures_util::StreamExt;

    let Some(instance_id) = parse_instance_prefix(&headers, &state.proxy_hostname) else {
        return (StatusCode::BAD_REQUEST, "Invalid proxy hostname").into_response();
    };
    if let Err(resp) = authenticate_proxy_scoped(&headers, &state, &instance_id, "files:read").await {
        return resp;
    }
    let Some(control_tx) = state.registry.resolve_control_tx(&instance_id) else {
        tracing::debug!(instance = %instance_id, tunnel = %tunnel_name, "file_read: daemon not found");
        return StatusCode::NOT_FOUND.into_response();
    };

    let path_str = query.path.as_deref().unwrap_or("?");
    tracing::info!(instance = %instance_id, tunnel = %tunnel_name, path = %path_str, "file_read");

    let session_id = Uuid::new_v4().to_string();
    let session_secret = Uuid::new_v4().to_string();

    let (ws_tx, ws_rx) = tokio::sync::oneshot::channel();
    if !bridge::register_pending_proxy_session_with_callback(
        session_id.clone(),
        session_secret.clone(),
        ws_tx,
    ) {
        tracing::warn!(instance = %instance_id, tunnel = %tunnel_name, "file_read: too many pending sessions");
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }

    if control_tx
        .send(ControlMsg::FileSessionRequest {
            session_id: session_id.clone(),
            session_secret,
            tunnel_name: tunnel_name.clone(),
            mode: "read".to_string(),
            path: query.path.clone(),
            expected_mtime: None,
        })
        .await
        .is_err()
    {
        tracing::warn!(instance = %instance_id, tunnel = %tunnel_name, "file_read: control channel closed");
        bridge::remove_pending_proxy_session(&session_id);
        return StatusCode::BAD_GATEWAY.into_response();
    }

    let daemon_ws = match tokio::time::timeout(Duration::from_secs(60), ws_rx).await {
        Ok(Ok(ws)) => ws,
        _ => {
            tracing::warn!(instance = %instance_id, tunnel = %tunnel_name, "file_read: daemon session timeout (60s)");
            bridge::remove_pending_proxy_session(&session_id);
            return StatusCode::GATEWAY_TIMEOUT.into_response();
        }
    };

    let (mut daemon_sink, mut daemon_stream) = daemon_ws.split();
    let header_msg = match tokio::time::timeout(Duration::from_secs(30), daemon_stream.next()).await
    {
        Ok(Some(Ok(axum::extract::ws::Message::Text(text)))) => text,
        _ => {
            let _ = daemon_sink
                .send(axum::extract::ws::Message::Close(None))
                .await;
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    let header: serde_json::Value = match serde_json::from_str(&header_msg) {
        Ok(v) => v,
        Err(_) => {
            let _ = daemon_sink
                .send(axum::extract::ws::Message::Close(None))
                .await;
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    let status = header["status"].as_u64().unwrap_or(500) as u16;
    if status != 200 {
        let error = header["error"].as_str().unwrap_or("unknown error");
        tracing::warn!(instance = %instance_id, tunnel = %tunnel_name, status, error, "file_read: daemon error");
        let _ = daemon_sink
            .send(axum::extract::ws::Message::Close(None))
            .await;
        return axum::response::Response::builder()
            .status(status)
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({ "error": error }).to_string(),
            ))
            .unwrap()
            .into_response();
    }

    let size = header["size"].as_u64().unwrap_or(0);
    let mtime = header["mtime"].as_i64().unwrap_or(0);
    tracing::debug!(instance = %instance_id, tunnel = %tunnel_name, size, mtime, "file_read: streaming");

    let body_stream = futures_util::stream::unfold(daemon_stream, |mut stream| async move {
        match stream.next().await {
            Some(Ok(axum::extract::ws::Message::Binary(data))) => Some((
                Ok::<_, std::convert::Infallible>(axum::body::Bytes::from(data.to_vec())),
                stream,
            )),
            _ => None,
        }
    });

    axum::response::Response::builder()
        .status(200)
        .header("content-type", "application/octet-stream")
        .header("x-file-mtime", mtime.to_string())
        .header("x-file-size", size.to_string())
        .header("access-control-expose-headers", "x-file-mtime, x-file-size")
        .body(Body::from_stream(body_stream))
        .unwrap()
        .into_response()
}

#[derive(Debug, Deserialize)]
struct FileWriteQuery {
    path: Option<String>,
    expected_mtime: Option<i64>,
}

/// Write a file via a dedicated data WebSocket session.
async fn file_write(
    headers: HeaderMap,
    Path(tunnel_name): Path<String>,
    Query(query): Query<FileWriteQuery>,
    State(state): State<ProxyState>,
    body: Body,
) -> axum::response::Response {
    use futures_util::{SinkExt, StreamExt};

    let Some(instance_id) = parse_instance_prefix(&headers, &state.proxy_hostname) else {
        return (StatusCode::BAD_REQUEST, "Invalid proxy hostname").into_response();
    };
    if let Err(resp) = authenticate_proxy_scoped(&headers, &state, &instance_id, "files:write").await {
        return resp;
    }
    let Some(control_tx) = state.registry.resolve_control_tx(&instance_id) else {
        tracing::debug!(instance = %instance_id, tunnel = %tunnel_name, "file_write: daemon not found");
        return StatusCode::NOT_FOUND.into_response();
    };

    let path_str = query.path.as_deref().unwrap_or("?");
    tracing::info!(instance = %instance_id, tunnel = %tunnel_name, path = %path_str, "file_write");

    let session_id = Uuid::new_v4().to_string();
    let session_secret = Uuid::new_v4().to_string();

    let (ws_tx, ws_rx) = tokio::sync::oneshot::channel();
    if !bridge::register_pending_proxy_session_with_callback(
        session_id.clone(),
        session_secret.clone(),
        ws_tx,
    ) {
        tracing::warn!(instance = %instance_id, tunnel = %tunnel_name, "file_write: too many pending sessions");
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }

    if control_tx
        .send(ControlMsg::FileSessionRequest {
            session_id: session_id.clone(),
            session_secret,
            tunnel_name: tunnel_name.clone(),
            mode: "write".to_string(),
            path: query.path.clone(),
            expected_mtime: query.expected_mtime,
        })
        .await
        .is_err()
    {
        tracing::warn!(instance = %instance_id, tunnel = %tunnel_name, "file_write: control channel closed");
        bridge::remove_pending_proxy_session(&session_id);
        return StatusCode::BAD_GATEWAY.into_response();
    }

    let daemon_ws = match tokio::time::timeout(Duration::from_secs(60), ws_rx).await {
        Ok(Ok(ws)) => ws,
        _ => {
            tracing::warn!(instance = %instance_id, tunnel = %tunnel_name, "file_write: daemon session timeout (60s)");
            bridge::remove_pending_proxy_session(&session_id);
            return StatusCode::GATEWAY_TIMEOUT.into_response();
        }
    };

    let (mut daemon_sink, mut daemon_stream) = daemon_ws.split();

    // Wait for "ready" or error from daemon
    let ready_msg = match tokio::time::timeout(Duration::from_secs(30), daemon_stream.next()).await
    {
        Ok(Some(Ok(axum::extract::ws::Message::Text(text)))) => text,
        _ => {
            let _ = daemon_sink
                .send(axum::extract::ws::Message::Close(None))
                .await;
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&ready_msg) {
        if v.get("status").is_some() && v.get("ready").is_none() {
            let _ = daemon_sink
                .send(axum::extract::ws::Message::Close(None))
                .await;
            let status = v["status"].as_u64().unwrap_or(500) as u16;
            return axum::response::Response::builder()
                .status(status)
                .header("content-type", "application/json")
                .body(Body::from(v.to_string()))
                .unwrap()
                .into_response();
        }
    }

    // Stream request body to daemon
    use http_body_util::BodyExt;
    let mut body_stream = body.into_data_stream();
    while let Some(chunk) = body_stream.next().await {
        match chunk {
            Ok(bytes) => {
                if daemon_sink
                    .send(axum::extract::ws::Message::Binary(bytes.to_vec().into()))
                    .await
                    .is_err()
                {
                    return StatusCode::BAD_GATEWAY.into_response();
                }
            }
            Err(_) => break,
        }
    }

    if daemon_sink
        .send(axum::extract::ws::Message::Text("end_request".into()))
        .await
        .is_err()
    {
        return StatusCode::BAD_GATEWAY.into_response();
    }

    // Read result
    let result_msg = match tokio::time::timeout(Duration::from_secs(60), daemon_stream.next()).await
    {
        Ok(Some(Ok(axum::extract::ws::Message::Text(text)))) => text,
        _ => {
            tracing::warn!(instance = %instance_id, tunnel = %tunnel_name, "file_write: result timeout (60s)");
            return StatusCode::GATEWAY_TIMEOUT.into_response();
        }
    };

    tracing::debug!(instance = %instance_id, tunnel = %tunnel_name, result = %result_msg, "file_write: completed");

    let _ = daemon_sink
        .send(axum::extract::ws::Message::Close(None))
        .await;

    let result: serde_json::Value = serde_json::from_str(&result_msg).unwrap_or_default();
    let status = result["status"].as_u64().unwrap_or(500) as u16;

    axum::response::Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from(result.to_string()))
        .unwrap()
        .into_response()
}

// ── Shell tunnel endpoint ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ShellExecBody {
    #[serde(default)]
    user_arg: Option<String>,
}

/// Execute a predefined shell command via a data WebSocket session.
/// Streams output back as SSE (text/event-stream).
async fn shell_exec(
    headers: HeaderMap,
    Path(command_name): Path<String>,
    State(state): State<ProxyState>,
    Json(body): Json<ShellExecBody>,
) -> axum::response::Response {
    let Some(instance_id) = parse_instance_prefix(&headers, &state.proxy_hostname) else {
        return (StatusCode::BAD_REQUEST, "Invalid proxy hostname").into_response();
    };
    if let Err(resp) = authenticate_proxy_scoped(&headers, &state, &instance_id, "shell:exec").await {
        return resp;
    }
    let Some(control_tx) = state.registry.resolve_control_tx(&instance_id) else {
        tracing::debug!(instance = %instance_id, command = %command_name, "shell_exec: daemon not found");
        return StatusCode::NOT_FOUND.into_response();
    };

    let arg_str = body.user_arg.as_deref().unwrap_or("");
    tracing::info!(instance = %instance_id, command = %command_name, arg = %arg_str, "shell_exec");

    let session_id = Uuid::new_v4().to_string();
    let session_secret = Uuid::new_v4().to_string();

    let (ws_tx, ws_rx) = tokio::sync::oneshot::channel();
    if !bridge::register_pending_proxy_session_with_callback(
        session_id.clone(),
        session_secret.clone(),
        ws_tx,
    ) {
        tracing::warn!(instance = %instance_id, command = %command_name, "shell_exec: too many pending sessions");
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }

    if control_tx
        .send(ControlMsg::ShellSessionRequest {
            session_id: session_id.clone(),
            session_secret,
            command_name: command_name.clone(),
            user_arg: body.user_arg,
        })
        .await
        .is_err()
    {
        tracing::warn!(instance = %instance_id, command = %command_name, "shell_exec: control channel closed");
        bridge::remove_pending_proxy_session(&session_id);
        return StatusCode::BAD_GATEWAY.into_response();
    }

    let daemon_ws = match tokio::time::timeout(Duration::from_secs(60), ws_rx).await {
        Ok(Ok(ws)) => ws,
        _ => {
            tracing::warn!(instance = %instance_id, command = %command_name, "shell_exec: daemon session timeout (60s)");
            bridge::remove_pending_proxy_session(&session_id);
            return StatusCode::GATEWAY_TIMEOUT.into_response();
        }
    };

    let (_daemon_sink, daemon_stream) = daemon_ws.split();

    // Stream daemon WS text messages as SSE events
    let body_stream = futures_util::stream::unfold(daemon_stream, |mut stream| async move {
        loop {
            match stream.next().await {
                Some(Ok(axum::extract::ws::Message::Text(text))) => {
                    let data = format!("data: {text}\n\n");
                    return Some((
                        Ok::<_, std::convert::Infallible>(axum::body::Bytes::from(data)),
                        stream,
                    ));
                }
                Some(Ok(axum::extract::ws::Message::Close(_))) | None => return None,
                _ => continue,
            }
        }
    });

    axum::response::Response::builder()
        .status(200)
        .header("content-type", "text/event-stream")
        .header("cache-control", "no-cache")
        .body(Body::from_stream(body_stream))
        .unwrap()
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
    if authenticate_proxy(&headers, &state, &instance_id).await.is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let online = state.registry.resolve_control_tx(&instance_id).is_some();
    tracing::debug!(instance = %instance_id, online, "daemon_ping");
    if online {
        (StatusCode::OK, "ok").into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

/// Proxy the daemon's /logs endpoint via MetricsRequest.
async fn log_proxy(
    headers: HeaderMap,
    Query(query): Query<LogQuery>,
    State(state): State<ProxyState>,
) -> axum::response::Response {
    let Some(instance_id) = parse_instance_prefix(&headers, &state.proxy_hostname) else {
        return (StatusCode::BAD_REQUEST, "Invalid proxy hostname").into_response();
    };
    if let Err(resp) = authenticate_proxy_scoped(&headers, &state, &instance_id, "logs:read").await {
        return resp;
    }
    let Some(control_tx) = state.registry.resolve_control_tx(&instance_id) else {
        return StatusCode::NOT_FOUND.into_response();
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
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();

    if control_tx
        .send(ControlMsg::MetricsRequest {
            request_id,
            path,
            response_tx,
        })
        .await
        .is_err()
    {
        return StatusCode::BAD_GATEWAY.into_response();
    }

    match tokio::time::timeout(Duration::from_secs(30), response_rx).await {
        Ok(Ok(resp)) => axum::response::Response::builder()
            .status(resp.status)
            .header("content-type", resp.content_type)
            .body(Body::from(resp.body))
            .unwrap()
            .into_response(),
        Ok(Err(_)) => StatusCode::BAD_GATEWAY.into_response(),
        Err(_) => StatusCode::GATEWAY_TIMEOUT.into_response(),
    }
}
