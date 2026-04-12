use axum::body::Body;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{FromRequest, Json, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use axum::routing::{any, get, post};
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::bridge;
use crate::daemon_registry::{ControlMsg, DaemonRegistry, ProxyResponse};
use crate::ws_handler::validate_token;

const PROXY_TOKEN_COOKIE: &str = "proxy_token";

#[derive(Clone)]
pub struct ProxyState {
    pub registry: Arc<DaemonRegistry>,
    pub server_api_url: String,
    pub proxy_hostname: String,
}

/// Build the Axum router for proxy endpoints (served on wildcard subdomains).
pub fn router(state: ProxyState) -> Router {
    Router::new()
        // Bootstrap: stores token as cookie, redirects to /
        .route("/proxy", get(proxy_bootstrap))
        // JSON request/response API (kept for programmatic use)
        .route("/proxy_request", post(proxy_request))
        // WebSocket tunneling
        .route("/proxy_ws", any(proxy_ws))
        // Catch-all: reverse proxy for HTTP, and WS upgrade handler
        .fallback(proxy_catchall)
        .layer(middleware::from_fn(proxy_security_headers))
        .with_state(state)
}

/// Security headers that isolate the subdomain.
async fn proxy_security_headers(
    request: axum::extract::Request,
    next: Next,
) -> axum::response::Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    // Isolate from other subdomains
    headers.insert(
        axum::http::header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        "cross-origin-opener-policy",
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("same-origin"),
    );
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

/// Validate the proxy token (from cookie) and check cluster scoping.
/// Returns the tunnel's control_tx on success.
async fn authenticate_proxy(
    headers: &HeaderMap,
    state: &ProxyState,
    instance_id: &str,
) -> Result<(), axum::response::Response> {
    let token = extract_cookie_token(headers)
        .ok_or_else(|| {
            (StatusCode::UNAUTHORIZED, "Missing proxy_token cookie. Visit /proxy?proxy_token=TOKEN first.").into_response()
        })?;

    let self_info = validate_token(&state.server_api_url, &token)
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

    Ok(())
}

// ── GET /proxy?proxy_token=... — bootstrap ─────────────────────────────

#[derive(Deserialize)]
struct ProxyBootstrapQuery {
    proxy_token: String,
}

/// Store the proxy_token as an HttpOnly, SameSite=Strict cookie scoped to
/// this subdomain, then redirect to /.
async fn proxy_bootstrap(
    headers: HeaderMap,
    Query(query): Query<ProxyBootstrapQuery>,
    State(state): State<ProxyState>,
) -> axum::response::Response {
    let Some((instance_id, tunnel_name)) = parse_subdomain(&headers, &state.proxy_hostname) else {
        return (StatusCode::BAD_REQUEST, "Invalid proxy hostname").into_response();
    };

    if state.registry.find_tunnel(&instance_id, &tunnel_name).is_none() {
        return (StatusCode::NOT_FOUND, "Tunnel not found").into_response();
    }

    // Build a cookie scoped to this subdomain only.
    let host = headers.get("host").and_then(|v| v.to_str().ok()).unwrap_or("");
    let host_no_port = host.split(':').next().unwrap_or(host);

    let cookie = format!(
        "{PROXY_TOKEN_COOKIE}={}; Path=/; HttpOnly; SameSite=Strict; Domain={host_no_port}; Max-Age=21600",
        query.proxy_token,
    );

    axum::response::Response::builder()
        .status(StatusCode::FOUND)
        .header("location", "/")
        .header("set-cookie", cookie)
        .body(Body::empty())
        .unwrap()
        .into_response()
}

// ── Catch-all: reverse proxy ───────────────────────────────────────────

/// Handles all non-special requests by proxying them to the daemon's tunnel.
/// Detects WebSocket upgrades and routes them through proxy sessions.
/// Regular HTTP (including SSE) is streamed via a proxy session.
async fn proxy_catchall(
    State(state): State<ProxyState>,
    req: Request,
) -> axum::response::Response {
    let headers = req.headers().clone();
    let method = req.method().clone();
    let path = req.uri().path_and_query()
        .map(|pq| pq.as_str().to_string())
        .unwrap_or_else(|| "/".to_string());

    let Some((instance_id, tunnel_name)) = parse_subdomain(&headers, &state.proxy_hostname) else {
        return (StatusCode::BAD_REQUEST, "Invalid proxy hostname").into_response();
    };

    if let Err(resp) = authenticate_proxy(&headers, &state, &instance_id).await {
        return resp;
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

        return ws.on_upgrade(move |socket| async move {
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

    // Collect request headers to forward
    let fwd_headers: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(k, v)| {
            let lk = k.as_str().to_lowercase();
            // Skip hop-by-hop and cookie (contains proxy_token)
            if lk == "connection" || lk == "transfer-encoding" || lk == "cookie" {
                return None;
            }
            Some((k.to_string(), v.to_str().unwrap_or("").to_string()))
        })
        .collect();

    // Collect request body
    let body_bytes = match axum::body::to_bytes(req.into_body(), 16 * 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => return (StatusCode::PAYLOAD_TOO_LARGE, "Request body too large").into_response(),
    };

    let has_body = !body_bytes.is_empty();

    // Create a proxy session for streaming
    let session_id = Uuid::new_v4().to_string();
    let session_secret = Uuid::new_v4().to_string();

    // Create a channel to receive the daemon's data WS
    let (ws_tx, ws_rx) = tokio::sync::oneshot::channel::<axum::extract::ws::WebSocket>();

    // Register the session. When the daemon connects its data WS,
    // bridge::take_pending_proxy_session returns it in handle_data_session.
    // But we need direct access here, so we use a different approach:
    // use a oneshot channel-based pending session.
    bridge::register_pending_proxy_session_with_callback(
        session_id.clone(),
        session_secret.clone(),
        ws_tx,
    );

    // Tell daemon to connect
    if control_tx
        .send(ControlMsg::ProxySessionRequest {
            session_id: session_id.clone(),
            session_secret: session_secret.clone(),
            tunnel_name,
            mode: "stream".to_string(),
            path: "/".to_string(),
        })
        .await
        .is_err()
    {
        return StatusCode::BAD_GATEWAY.into_response();
    }

    // Wait for the daemon's data WS to connect
    let daemon_ws = match tokio::time::timeout(
        std::time::Duration::from_secs(30),
        ws_rx,
    ).await {
        Ok(Ok(ws)) => ws,
        _ => return StatusCode::GATEWAY_TIMEOUT.into_response(),
    };

    let (mut daemon_sink, mut daemon_stream) = daemon_ws.split();

    // Send request details to daemon
    let req_json = serde_json::json!({
        "method": method.as_str(),
        "path": path,
        "headers": serde_json::Value::Object(
            fwd_headers.into_iter()
                .map(|(k, v)| (k, serde_json::Value::String(v)))
                .collect()
        ),
        "has_body": has_body,
    });

    if daemon_sink
        .send(axum::extract::ws::Message::Text(req_json.to_string().into()))
        .await
        .is_err()
    {
        return StatusCode::BAD_GATEWAY.into_response();
    }

    // Send request body if present (1MB chunks)
    if has_body {
        for chunk in body_bytes.chunks(1024 * 1024) {
            if daemon_sink
                .send(axum::extract::ws::Message::Binary(chunk.to_vec().into()))
                .await
                .is_err()
            {
                return StatusCode::BAD_GATEWAY.into_response();
            }
        }
        if daemon_sink
            .send(axum::extract::ws::Message::Text("end_request".into()))
            .await
            .is_err()
        {
            return StatusCode::BAD_GATEWAY.into_response();
        }
    }

    // Read response headers from daemon (first text message)
    let resp_headers_msg = match daemon_stream.next().await {
        Some(Ok(axum::extract::ws::Message::Text(t))) => t,
        _ => return StatusCode::BAD_GATEWAY.into_response(),
    };

    let resp_meta: serde_json::Value = match serde_json::from_str(&resp_headers_msg) {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_GATEWAY.into_response(),
    };

    let status = resp_meta["status"].as_u64().unwrap_or(502) as u16;
    let resp_headers = resp_meta["headers"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|pair| {
                    let k = pair.get(0)?.as_str()?;
                    let v = pair.get(1)?.as_str()?;
                    Some((k.to_string(), v.to_string()))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Build the streaming HTTP response from daemon WS body chunks
    let body_stream = futures_util::stream::unfold(daemon_stream, |mut stream| async move {
        loop {
            match stream.next().await {
                Some(Ok(axum::extract::ws::Message::Binary(data))) => {
                    return Some((Ok::<_, std::convert::Infallible>(axum::body::Bytes::from(data.to_vec())), stream));
                }
                Some(Ok(axum::extract::ws::Message::Close(_))) | None => return None,
                Some(Err(_)) => return None,
                _ => continue, // skip text/ping/pong
            }
        }
    });

    let mut builder = axum::response::Response::builder().status(status);
    for (k, v) in &resp_headers {
        let lk = k.to_lowercase();
        // Strip headers that interfere with the proxy
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

// ── WebSocket proxy (/proxy_ws) ────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ProxyWsQuery {
    proxy_token: Option<String>,
    path: Option<String>,
}

async fn proxy_ws(
    ws: WebSocketUpgrade,
    req_headers: HeaderMap,
    Query(query): Query<ProxyWsQuery>,
    State(state): State<ProxyState>,
) -> axum::response::Response {
    let Some((instance_id, tunnel_name)) = parse_subdomain(&req_headers, &state.proxy_hostname)
    else {
        return (StatusCode::BAD_REQUEST, "Invalid proxy hostname").into_response();
    };

    // Accept token from query param OR cookie
    let token = query.proxy_token.clone()
        .or_else(|| extract_cookie_token(&req_headers));
    let Some(token) = token else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    let self_info = match validate_token(&state.server_api_url, &token).await {
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

    let Some((control_tx, _)) = state.registry.find_tunnel(&instance_id, &tunnel_name) else {
        return (StatusCode::NOT_FOUND, "Tunnel not found").into_response();
    };

    let session_id = Uuid::new_v4().to_string();
    let session_secret = Uuid::new_v4().to_string();
    let path = query.path.unwrap_or_else(|| "/".to_string());

    ws.on_upgrade(move |socket| async move {
        tracing::info!("browser WS upgraded for proxy session (tunnel {tunnel_name})");

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
            tracing::warn!("proxy_ws: daemon control channel closed");
            bridge::remove_pending_proxy_session(&session_id);
        }
    })
    .into_response()
}
