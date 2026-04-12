use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Json, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use axum::routing::{any, get, post};
use axum::Router;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::bridge;
use crate::daemon_registry::{ControlMsg, DaemonRegistry, ProxyResponse};
use crate::ws_handler::validate_token;

#[derive(Clone)]
pub struct ProxyState {
    pub registry: Arc<DaemonRegistry>,
    pub server_api_url: String,
    pub proxy_hostname: String,
}

/// Build the Axum router for proxy endpoints (served on wildcard subdomains).
pub fn router(state: ProxyState) -> Router {
    Router::new()
        .route("/proxy", get(proxy_iframe))
        .route("/proxy_sw.js", get(proxy_service_worker))
        .route("/proxy_request", post(proxy_request))
        .route("/proxy_stream", any(proxy_stream))
        .route("/proxy_ws", any(proxy_ws))
        .layer(middleware::from_fn(proxy_security_headers))
        .with_state(state)
}

/// Permissive security headers for proxy pages (must allow iframes and SW).
async fn proxy_security_headers(
    request: axum::extract::Request,
    next: Next,
) -> axum::response::Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        axum::http::header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'self' 'unsafe-inline'; frame-ancestors *"),
    );
    headers.insert(
        axum::http::header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    // Explicitly do NOT set X-Frame-Options — we want to be embeddable.
    response
}

// ── Subdomain parsing ──────────────────────────────────────────────────

/// Extract (instance_id_prefix, tunnel_name) from the Host header.
/// Format: `{short_id}-{tunnel_name}.{proxy_hostname}`
/// The short_id is a hex prefix (12+ chars) of the full 64-char SHA256 instance ID.
/// The relay resolves the prefix to the full ID via `DaemonRegistry::resolve_prefix`.
fn parse_subdomain(headers: &HeaderMap, proxy_hostname: &str) -> Option<(String, String)> {
    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())?;

    // Strip port if present
    let host_no_port = host.split(':').next().unwrap_or(host);

    // Must end with .{proxy_hostname}
    let subdomain = host_no_port.strip_suffix(&format!(".{proxy_hostname}"))?;

    // Split on the last '-' to separate instance_id_prefix from tunnel_name.
    // Tunnel names are simple identifiers (no hyphens), while instance IDs are hex.
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

// ── GET /proxy — iframe bootstrap page ─────────────────────────────────

async fn proxy_iframe(
    headers: HeaderMap,
    State(state): State<ProxyState>,
) -> axum::response::Response {
    let Some((instance_id, tunnel_name)) = parse_subdomain(&headers, &state.proxy_hostname) else {
        return (StatusCode::BAD_REQUEST, "Invalid proxy hostname").into_response();
    };

    if state.registry.find_tunnel(&instance_id, &tunnel_name).is_none() {
        return (StatusCode::NOT_FOUND, "Tunnel not found").into_response();
    }

    let html = PROXY_IFRAME_HTML.replace("{{TUNNEL_NAME}}", &tunnel_name);

    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/html; charset=utf-8")
        .body(axum::body::Body::from(html))
        .unwrap()
        .into_response()
}

const PROXY_IFRAME_HTML: &str = r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>Tunnel Proxy</title>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body { display: flex; flex-direction: column; height: 100vh; overflow: hidden; font-family: system-ui, -apple-system, sans-serif; }
  #bar {
    display: flex; align-items: center; gap: 8px;
    padding: 6px 10px;
    background: #1e1e2e; border-bottom: 1px solid #313244;
  }
  #bar .label {
    color: #89b4fa; font-size: 12px; font-weight: 600;
    white-space: nowrap; user-select: none;
  }
  #url {
    flex: 1; padding: 5px 10px;
    background: #313244; color: #cdd6f4; border: 1px solid #45475a;
    border-radius: 6px; font-size: 13px; font-family: ui-monospace, monospace;
    outline: none;
  }
  #url:focus { border-color: #89b4fa; }
  #frame { flex: 1; border: none; width: 100%; display: none; }
  #spinner {
    flex: 1; display: flex; align-items: center; justify-content: center;
    background: #1e1e2e; color: #6c7086; font-size: 14px; gap: 10px;
  }
  #spinner .dot {
    width: 8px; height: 8px; border-radius: 50%; background: #89b4fa;
    animation: pulse 1.2s ease-in-out infinite;
  }
  #spinner .dot:nth-child(2) { animation-delay: 0.2s; }
  #spinner .dot:nth-child(3) { animation-delay: 0.4s; }
  @keyframes pulse {
    0%, 80%, 100% { opacity: 0.2; transform: scale(0.8); }
    40% { opacity: 1; transform: scale(1.2); }
  }
</style>
</head>
<body>
<div id="bar">
  <span class="label">{{TUNNEL_NAME}}</span>
  <input id="url" type="text" spellcheck="false" autocomplete="off">
</div>
<div id="spinner">
  <span class="dot"></span><span class="dot"></span><span class="dot"></span>
  <span>Connecting to tunnel...</span>
</div>
<iframe id="frame"></iframe>
<script type="module">
  const tunnelName = '{{TUNNEL_NAME}}';
  const proxyToken = new URLSearchParams(location.search).get('proxy_token');
  if (!proxyToken) {
    document.body.textContent = 'Missing proxy_token parameter';
    throw new Error('missing proxy_token');
  }

  const urlBar = document.getElementById('url');
  const frame = document.getElementById('frame');
  const spinner = document.getElementById('spinner');

  // Register service worker
  const reg = await navigator.serviceWorker.register('/proxy_sw.js', { type: 'module' });

  // If there's already a controlling SW but we got a new one, reload so the
  // new SW intercepts all requests from the start.
  const needsReload = navigator.serviceWorker.controller && reg.waiting;

  // Wait for the SW to be active
  await new Promise((resolve) => {
    const sw = reg.active ?? reg.installing ?? reg.waiting;
    if (sw.state === 'activated') { resolve(); return; }
    sw.addEventListener('statechange', () => {
      if (sw.state === 'activated') resolve();
    });
  });

  // Send token to the active SW
  reg.active.postMessage({ type: 'init', proxyToken });

  // If a stale SW was controlling the page, reload so the new one takes over
  if (needsReload) {
    location.reload();
    throw new Error('reloading for new service worker');
  }

  // Wait for the SW to be controlling this page
  if (!navigator.serviceWorker.controller) {
    await new Promise((resolve) => {
      navigator.serviceWorker.addEventListener('controllerchange', resolve, { once: true });
    });
    // Re-send token after controller change
    reg.active.postMessage({ type: 'init', proxyToken });
  }

  // Hide spinner, show iframe
  spinner.style.display = 'none';
  frame.style.display = 'block';

  function navigate(path) {
    frame.src = path;
    urlBar.value = path;
  }

  urlBar.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      const path = urlBar.value.startsWith('/') ? urlBar.value : '/' + urlBar.value;
      navigate(path);
    }
  });

  // Track iframe navigation
  function syncUrlBar() {
    try {
      const loc = frame.contentWindow.location.pathname + frame.contentWindow.location.search;
      if (urlBar.value !== loc && document.activeElement !== urlBar) {
        urlBar.value = loc;
      }
    } catch (_) { /* cross-origin, ignore */ }
  }
  frame.addEventListener('load', syncUrlBar);
  setInterval(syncUrlBar, 500);

  navigate('/');
</script>
</body></html>"#;

// ── GET /proxy_sw.js — service worker (ESM) ────────────────────────────

async fn proxy_service_worker() -> axum::response::Response {
    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/javascript; charset=utf-8")
        .header("service-worker-allowed", "/")
        .body(axum::body::Body::from(PROXY_SW_JS))
        .unwrap()
        .into_response()
}

const PROXY_SW_JS: &str = r#"// proxy_sw.js — service worker for tunnel proxy (ESM module)
let proxyToken = '';

self.addEventListener('message', (e) => {
  if (e.data.type === 'init') {
    proxyToken = e.data.proxyToken;
  }
});

// Claim clients immediately so the SW is active on first page load.
self.addEventListener('activate', (e) => {
  e.waitUntil(self.clients.claim());
});

self.addEventListener('install', () => {
  self.skipWaiting();
});

// Recover proxy_token from controlled clients if lost (e.g. after SW restart).
async function ensureToken() {
  if (proxyToken) return;
  const clients = await self.clients.matchAll({ type: 'window' });
  for (const client of clients) {
    try {
      const url = new URL(client.url);
      const token = url.searchParams.get('proxy_token');
      if (token) { proxyToken = token; return; }
    } catch (_) {}
  }
}

const STRIPPED_HEADERS = new Set([
  'content-encoding', 'transfer-encoding', 'content-length',
  'x-frame-options', 'content-security-policy', 'x-content-type-options',
  'cross-origin-opener-policy', 'cross-origin-embedder-policy',
  'cross-origin-resource-policy', 'permissions-policy',
]);

self.addEventListener('fetch', (event) => {
  const url = new URL(event.request.url);

  // Pass through proxy management URLs directly to the relay server
  if (url.pathname.startsWith('/proxy')) return;

  event.respondWith(proxyFetch(event.request, url));
});

async function proxyFetch(request, url) {
  await ensureToken();

  const wsProto = location.protocol === 'https:' ? 'wss:' : 'ws:';
  const wsUrl = `${wsProto}//${location.host}/proxy_stream?proxy_token=${encodeURIComponent(proxyToken)}`;

  return new Promise((resolve, reject) => {
    const ws = new WebSocket(wsUrl);
    ws.binaryType = 'arraybuffer';

    ws.onopen = async () => {
      const headers = Object.fromEntries(request.headers.entries());
      const hasBody = !['GET', 'HEAD'].includes(request.method);

      // Send request details
      ws.send(JSON.stringify({
        method: request.method,
        path: url.pathname + url.search,
        headers,
        has_body: hasBody,
      }));

      // Stream request body if present
      if (hasBody) {
        const body = new Uint8Array(await request.arrayBuffer());
        // Send in 1MB chunks
        for (let i = 0; i < body.length; i += 1024 * 1024) {
          ws.send(body.slice(i, i + 1024 * 1024));
        }
        ws.send('end_request');
      }
    };

    let headersReceived = false;
    let respStatus = 200;
    let controller;

    const bodyStream = new ReadableStream({
      start(c) { controller = c; },
    });

    ws.onmessage = (event) => {
      if (!headersReceived) {
        // First text message: response headers
        headersReceived = true;
        try {
          const data = JSON.parse(event.data);
          respStatus = data.status ?? 200;
          const respHeaders = new Headers();
          for (const [k, v] of (data.headers ?? [])) {
            if (STRIPPED_HEADERS.has(k.toLowerCase())) continue;
            try { respHeaders.append(k, v); } catch(_) {}
          }
          resolve(new Response(bodyStream, { status: respStatus, headers: respHeaders }));
        } catch (e) {
          resolve(new Response('Proxy error: invalid headers', { status: 502 }));
          ws.close();
        }
        return;
      }
      // Subsequent binary messages: body chunks
      if (event.data instanceof ArrayBuffer) {
        controller.enqueue(new Uint8Array(event.data));
      }
    };

    ws.onclose = () => {
      if (!headersReceived) {
        resolve(new Response('Proxy error: connection closed', { status: 502 }));
      }
      try { controller.close(); } catch(_) {}
    };

    ws.onerror = () => {
      if (!headersReceived) {
        resolve(new Response('Proxy error: WebSocket error', { status: 502 }));
      }
      try { controller.error(new Error('WS error')); } catch(_) {}
    };
  });
}
"#;

// ── POST /proxy_request — the main proxy pipe ──────────────────────────

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
    // Parse subdomain
    let Some((instance_id, tunnel_name)) = parse_subdomain(&req_headers, &state.proxy_hostname)
    else {
        return (StatusCode::BAD_REQUEST, "Invalid proxy hostname").into_response();
    };

    // Validate proxy token
    let self_info = match validate_token(&state.server_api_url, &body.proxy_token).await {
        Ok(info) => info,
        Err(status) => return status.into_response(),
    };

    if self_info.token_kind != "proxy" && self_info.token_kind != "admin" {
        return StatusCode::FORBIDDEN.into_response();
    }

    // Cluster scoping: verify the daemon's cluster is accessible to the token
    if let Some(daemon_cluster_id) = state.registry.get_cluster_id(&instance_id) {
        if !self_info.cluster_ids.contains(&daemon_cluster_id) {
            return StatusCode::FORBIDDEN.into_response();
        }
    }

    // Find the tunnel
    let Some((control_tx, _tcp_port)) = state.registry.find_tunnel(&instance_id, &tunnel_name)
    else {
        return (StatusCode::NOT_FOUND, "Tunnel not found").into_response();
    };

    // Build header list for the proxy request
    let headers: Vec<(String, String)> = body
        .headers
        .into_iter()
        .collect();

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

    // Wait for response with 60s timeout for long-lived connections
    match tokio::time::timeout(std::time::Duration::from_secs(60), response_rx).await {
        Ok(Ok(resp)) => {
            let json_headers: Vec<(String, String)> = resp.headers;
            // Return as JSON for the service worker to reconstruct
            let response = serde_json::json!({
                "status": resp.status,
                "headers": json_headers,
                "body": if resp.body.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(resp.body) },
            });
            Json(response).into_response()
        }
        Ok(Err(_)) => StatusCode::BAD_GATEWAY.into_response(),
        Err(_) => StatusCode::GATEWAY_TIMEOUT.into_response(),
    }
}

// ── Streaming proxy (/proxy_stream) ────────────────────────────────────
//
// The SW opens a WS to /proxy_stream. Protocol:
// 1. SW sends text: JSON { proxy_token, method, path, headers, has_body }
// 2. If has_body: SW sends binary chunks, then text "end_request"
// 3. Relay creates a proxy session, daemon connects data WS
// 4. Relay bridges SW WS ↔ daemon data WS for the streamed response
//    (daemon sends text headers, then binary body chunks, then close)

#[derive(Debug, Deserialize)]
struct ProxyStreamQuery {
    proxy_token: String,
}

async fn proxy_stream(
    ws: WebSocketUpgrade,
    req_headers: HeaderMap,
    Query(query): Query<ProxyStreamQuery>,
    State(state): State<ProxyState>,
) -> axum::response::Response {
    let Some((instance_id, tunnel_name)) = parse_subdomain(&req_headers, &state.proxy_hostname)
    else {
        return (StatusCode::BAD_REQUEST, "Invalid proxy hostname").into_response();
    };

    // Validate proxy token
    let self_info = match validate_token(&state.server_api_url, &query.proxy_token).await {
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

    ws.on_upgrade(move |socket| handle_proxy_stream(socket, control_tx, tunnel_name))
        .into_response()
}

async fn handle_proxy_stream(
    browser_ws: axum::extract::ws::WebSocket,
    control_tx: tokio::sync::mpsc::Sender<ControlMsg>,
    tunnel_name: String,
) {
    let session_id = Uuid::new_v4().to_string();
    let session_secret = Uuid::new_v4().to_string();

    // Register the browser WS BEFORE notifying the daemon to avoid a race
    // where the daemon connects its data WS before the pending session exists.
    bridge::register_pending_proxy_session(
        session_id.clone(),
        session_secret.clone(),
        browser_ws,
    );

    // Now tell the daemon to connect a data WS for this session.
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
        tracing::warn!("proxy_stream: daemon control channel closed");
        // Remove the orphaned pending session
        bridge::take_pending_proxy_session(&session_id, &session_secret);
    }
}

// ── WebSocket proxy (/proxy_ws) ────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ProxyWsQuery {
    proxy_token: String,
    path: Option<String>,
}

/// WebSocket proxy: upgrades the browser connection, creates a proxy session,
/// and bridges the browser WS with the daemon's data WS to the local service.
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

    // Validate proxy token
    let self_info = match validate_token(&state.server_api_url, &query.proxy_token).await {
        Ok(info) => info,
        Err(status) => return status.into_response(),
    };

    if self_info.token_kind != "proxy" && self_info.token_kind != "admin" {
        return StatusCode::FORBIDDEN.into_response();
    }

    // Cluster scoping
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

        // Register BEFORE notifying daemon to avoid race.
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
            bridge::take_pending_proxy_session(&session_id, &session_secret);
        }
    })
    .into_response()
}
