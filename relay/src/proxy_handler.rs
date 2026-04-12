use axum::extract::{Json, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

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
    let Some((_instance_id, _tunnel_name)) = parse_subdomain(&headers, &state.proxy_hostname) else {
        return (StatusCode::BAD_REQUEST, "Invalid proxy hostname").into_response();
    };

    // Don't check tunnel existence here — serve the iframe page regardless.
    // The service worker's /proxy_request calls will fail with a clear error
    // if the daemon or tunnel isn't available.
    let html = PROXY_IFRAME_HTML;

    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/html; charset=utf-8")
        .body(axum::body::Body::from(html))
        .unwrap()
        .into_response()
}

const PROXY_IFRAME_HTML: &str = r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>Tunnel Proxy</title></head>
<body style="margin:0;padding:0;overflow:hidden">
<iframe id="frame" style="width:100%;height:100vh;border:none"></iframe>
<script type="module">
  const proxyToken = new URLSearchParams(location.search).get('proxy_token');
  if (!proxyToken) {
    document.body.textContent = 'Missing proxy_token parameter';
    throw new Error('missing proxy_token');
  }

  const reg = await navigator.serviceWorker.register('/proxy_sw.js', { type: 'module' });

  // Wait for the service worker to be active
  await new Promise((resolve) => {
    const sw = reg.active ?? reg.installing ?? reg.waiting;
    if (sw.state === 'activated') { resolve(); return; }
    sw.addEventListener('statechange', () => {
      if (sw.state === 'activated') resolve();
    });
  });

  // Send token to SW via message channel
  const sw = reg.active;
  sw.postMessage({ type: 'init', proxyToken });

  // Small delay so the SW processes the message before the iframe starts fetching
  await new Promise(r => setTimeout(r, 50));

  document.getElementById('frame').src = '/proxy_content/';
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

self.addEventListener('fetch', (event) => {
  const url = new URL(event.request.url);

  // Pass through proxy management URLs directly to the relay server
  if (url.pathname.startsWith('/proxy')) return;

  event.respondWith(proxyFetch(event.request, url));
});

async function proxyFetch(request, url) {
  const body = ['GET', 'HEAD'].includes(request.method)
    ? null
    : arrayToBase64(new Uint8Array(await request.arrayBuffer()));

  const headers = Object.fromEntries(request.headers.entries());

  const resp = await fetch('/proxy_request', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      proxy_token: proxyToken,
      method: request.method,
      path: url.pathname + url.search,
      headers,
      body,
    }),
  });

  if (!resp.ok) {
    return new Response('Proxy error: ' + resp.status, { status: resp.status });
  }

  const data = await resp.json();
  const respHeaders = new Headers();
  for (const [k, v] of (data.headers ?? [])) {
    // Skip headers that would conflict with the service worker response
    const lk = k.toLowerCase();
    if (lk === 'content-encoding' || lk === 'transfer-encoding' || lk === 'content-length') continue;
    try { respHeaders.append(k, v); } catch(_) {}
  }
  const respBody = data.body ? base64ToArray(data.body) : null;

  return new Response(respBody, { status: data.status, headers: respHeaders });
}

function arrayToBase64(bytes) {
  let binary = '';
  for (const b of bytes) binary += String.fromCharCode(b);
  return btoa(binary);
}

function base64ToArray(b64) {
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
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
