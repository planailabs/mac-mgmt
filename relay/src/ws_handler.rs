use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use axum::routing::{any, get};
use axum::{Json, Router};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use crate::bridge;
use crate::daemon_registry::{ControlMsg, DaemonConn, DaemonRegistry, MetricsResponse};
use crate::metrics_federation::{
    PROMETHEUS_CONTENT_TYPE, encode_families, parse_and_relabel, push_gauge_strs,
};
use crate::ssh_listener;

/// Maximum WebSocket message size (256 KB).
const MAX_WS_MESSAGE_SIZE: usize = 256 * 1024;

#[derive(Clone)]
struct AppState {
    registry: Arc<DaemonRegistry>,
    server_api_url: String,
}

pub fn router(registry: Arc<DaemonRegistry>, server_api_url: String) -> Router {
    let state = AppState {
        registry,
        server_api_url,
    };

    Router::new()
        .route("/api/daemon/register", any(ws_daemon_register))
        .route("/api/daemon/session/{session_id}", any(ws_daemon_session))
        .route(
            "/api/daemon/{instance_id}/metrics/{*path}",
            get(proxy_metrics),
        )
        .route("/api/tunnels", get(list_tunnels))
        .route("/metrics", get(federated_metrics))
        .route("/health", get(health))
        .layer(middleware::from_fn(security_headers))
        .layer(tower::limit::ConcurrencyLimitLayer::new(256))
        .with_state(state)
}

/// Add security headers (CSP, X-Content-Type-Options, X-Frame-Options) to all responses.
async fn security_headers(
    request: axum::extract::Request,
    next: Next,
) -> axum::response::Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        axum::http::header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'"),
    );
    headers.insert(
        axum::http::header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        axum::http::header::X_FRAME_OPTIONS,
        HeaderValue::from_static("DENY"),
    );
    response
}

async fn health() -> &'static str {
    "ok"
}

// ── Token validation ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SelfInfo {
    customer_id: Option<Uuid>,
    customer_name: Option<String>,
    token_kind: String,
}

async fn validate_token(server_api_url: &str, token: &str) -> Result<SelfInfo, StatusCode> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{server_api_url}/api/self"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("token validation request failed: {e}");
            StatusCode::BAD_GATEWAY
        })?;

    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(StatusCode::UNAUTHORIZED);
    }
    if !resp.status().is_success() {
        tracing::error!("server API returned {}", resp.status());
        return Err(StatusCode::BAD_GATEWAY);
    }

    resp.json::<SelfInfo>().await.map_err(|e| {
        tracing::error!("failed to parse self info: {e}");
        StatusCode::BAD_GATEWAY
    })
}

fn extract_bearer(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(String::from)
}

/// Authenticate a request and require a specific token kind.
async fn require_auth(
    headers: &HeaderMap,
    server_api_url: &str,
    allowed_kinds: &[&str],
) -> Result<SelfInfo, axum::response::Response> {
    let Some(token) = extract_bearer(headers) else {
        return Err(StatusCode::UNAUTHORIZED.into_response());
    };
    let self_info = validate_token(server_api_url, &token)
        .await
        .map_err(|s| s.into_response())?;
    if !allowed_kinds.contains(&self_info.token_kind.as_str()) {
        return Err(StatusCode::FORBIDDEN.into_response());
    }
    Ok(self_info)
}

/// Reject WebSocket upgrades that carry a browser Origin header (anti-CSRF).
/// API clients (daemons) do not send Origin; only browsers do.
fn reject_browser_origin(headers: &HeaderMap) -> Result<(), axum::response::Response> {
    if headers.contains_key("origin") {
        tracing::warn!("rejecting WebSocket upgrade with Origin header (possible CSRF)");
        return Err(StatusCode::FORBIDDEN.into_response());
    }
    Ok(())
}

// ── Daemon registration WebSocket ───────────────────────────────────────

#[derive(Debug, Deserialize)]
struct RegisterQuery {
    instance_id: String,
    agent_name: Option<String>,
    hostname: Option<String>,
}

async fn ws_daemon_register(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    Query(query): Query<RegisterQuery>,
    State(state): State<AppState>,
) -> axum::response::Response {
    if let Err(resp) = reject_browser_origin(&headers) {
        return resp;
    }

    let self_info = match require_auth(&headers, &state.server_api_url, &["sync"]).await {
        Ok(info) => info,
        Err(resp) => return resp,
    };

    if state.registry.is_full() {
        tracing::error!("max daemon connections reached, rejecting {}", query.instance_id);
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }

    ws.max_message_size(MAX_WS_MESSAGE_SIZE)
        .on_upgrade(move |socket| handle_daemon_ws(socket, query, self_info, state))
        .into_response()
}

/// JSON message received from daemon on the control WebSocket.
#[derive(Debug, Deserialize)]
struct DaemonWsMessage {
    r#type: String,
    request_id: Option<String>,
    status: Option<u16>,
    content_type: Option<String>,
    body: Option<String>,
}

async fn handle_daemon_ws(
    socket: WebSocket,
    query: RegisterQuery,
    self_info: SelfInfo,
    state: AppState,
) {
    let instance_id = query.instance_id;
    tracing::info!(
        "daemon WS connected: instance={instance_id} customer={:?} agent={:?} hostname={:?}",
        self_info.customer_name,
        query.agent_name,
        query.hostname,
    );

    let Some(port) = state.registry.allocate_port(&instance_id) else {
        tracing::error!("no available ports for {instance_id}, rejecting");
        return;
    };

    let (control_tx, mut control_rx) = tokio::sync::mpsc::channel::<ControlMsg>(16);

    let listener_handle = ssh_listener::spawn(
        Arc::clone(&state.registry),
        instance_id.clone(),
        port,
    );

    let conn = DaemonConn {
        instance_id: instance_id.clone(),
        customer_id: self_info.customer_id,
        customer_name: self_info.customer_name,
        agent_name: query.agent_name,
        hostname: query.hostname,
        ssh_port: port,
        connected_at: Utc::now(),
        control_tx,
        listener_handle,
    };

    state.registry.register(conn);
    tracing::info!("daemon {instance_id} registered on port {port}");

    let (mut ws_sink, mut ws_stream) = socket.split();

    // Send registration confirmation
    let reg_msg = serde_json::json!({ "type": "registered", "ssh_port": port });
    if let Err(e) = ws_sink
        .send(Message::Text(reg_msg.to_string().into()))
        .await
    {
        tracing::warn!("failed to send registration ack to {instance_id}: {e}");
        state.registry.unregister(&instance_id);
        return;
    }
    tracing::debug!("sent registration ack to {instance_id} (port {port})");

    let mut pending_metrics: HashMap<String, tokio::sync::oneshot::Sender<MetricsResponse>> =
        HashMap::new();

    // Send periodic WS pings so middleboxes (e.g. nginx proxy_read_timeout)
    // don't silently drop idle control connections.
    let mut ping_tick = tokio::time::interval(Duration::from_secs(30));
    ping_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    ping_tick.tick().await; // consume the immediate first tick

    loop {
        tokio::select! {
            _ = ping_tick.tick() => {
                if ws_sink.send(Message::Ping(Vec::new().into())).await.is_err() {
                    break;
                }
            }
            Some(msg) = control_rx.recv() => {
                let json = match msg {
                    ControlMsg::SessionRequest { session_id, session_secret } => {
                        serde_json::json!({
                            "type": "session_request",
                            "session_id": session_id,
                            "session_secret": session_secret,
                        })
                    }
                    ControlMsg::MetricsRequest { request_id, path, response_tx } => {
                        tracing::debug!("forwarding metrics request {request_id} ({path}) to {instance_id}");
                        pending_metrics.insert(request_id.clone(), response_tx);
                        serde_json::json!({ "type": "metrics_request", "request_id": request_id, "path": path })
                    }
                };
                if ws_sink.send(Message::Text(json.to_string().into())).await.is_err() {
                    break;
                }
            }
            msg = ws_stream.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(m) = serde_json::from_str::<DaemonWsMessage>(&text) {
                            if m.r#type == "metrics_response" {
                                if let Some(req_id) = m.request_id {
                                    if let Some(tx) = pending_metrics.remove(&req_id) {
                                        let status = m.status.unwrap_or(502);
                                        tracing::debug!("metrics response {req_id} status={status}");
                                        let _ = tx.send(MetricsResponse {
                                            status,
                                            content_type: m.content_type.unwrap_or_else(|| "text/plain".into()),
                                            body: m.body.unwrap_or_default(),
                                        });
                                    } else {
                                        tracing::warn!("metrics response for unknown request {req_id}");
                                    }
                                }
                            } else {
                                tracing::debug!("unknown daemon msg type: {}", m.r#type);
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(data))) => { let _ = ws_sink.send(Message::Pong(data)).await; }
                    Some(Err(e)) => { tracing::warn!("daemon WS error: {e}"); break; }
                    _ => {}
                }
            }
        }
    }

    tracing::info!("daemon {instance_id} disconnected, freeing port {port}");
    state.registry.unregister(&instance_id);
}

// ── Daemon data session WebSocket ───────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SessionQuery {
    session_secret: String,
}

async fn ws_daemon_session(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    Query(query): Query<SessionQuery>,
    State(state): State<AppState>,
) -> axum::response::Response {
    if let Err(resp) = reject_browser_origin(&headers) {
        return resp;
    }

    if let Err(resp) = require_auth(&headers, &state.server_api_url, &["sync"]).await {
        return resp;
    }

    ws.max_message_size(MAX_WS_MESSAGE_SIZE)
        .on_upgrade(move |socket| handle_data_session(socket, session_id, query.session_secret))
        .into_response()
}

async fn handle_data_session(socket: WebSocket, session_id: String, session_secret: String) {
    tracing::debug!("daemon data WS upgraded for session {session_id}");
    let Some(tcp_stream) = bridge::take_pending_session(&session_id, &session_secret) else {
        tracing::warn!("daemon connected for session {session_id} but no valid pending session");
        return;
    };

    tracing::info!("bridging session {session_id}");
    bridge::bridge_tcp_ws(tcp_stream, socket).await;
    tracing::info!("session {session_id} ended");
}

// ── Metrics proxy ───────────────────────────────────────────────────────

/// Allowed characters in a metrics proxy path segment.
fn is_safe_path(path: &str) -> bool {
    !path.contains("..") && path.chars().all(|c| c.is_alphanumeric() || "-_/.?&=".contains(c))
}

async fn proxy_metrics(
    headers: HeaderMap,
    Path((instance_id, path)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> axum::response::Response {
    if let Err(resp) = require_auth(&headers, &state.server_api_url, &["admin", "setting"]).await {
        tracing::debug!("metrics proxy auth failed for {instance_id}");
        return resp;
    }

    if !is_safe_path(&path) {
        tracing::warn!("metrics proxy: rejecting unsafe path: {path}");
        return StatusCode::BAD_REQUEST.into_response();
    }

    tracing::debug!("metrics proxy: instance={instance_id} path={path}");

    let full_path = if query.is_empty() {
        format!("/{path}")
    } else {
        let qs: String = query
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");
        format!("/{path}?{qs}")
    };

    let Some(control_tx) = state.registry.get_control_tx(&instance_id) else {
        tracing::warn!("metrics proxy: daemon {instance_id} not connected");
        return StatusCode::NOT_FOUND.into_response();
    };

    let request_id = Uuid::new_v4().to_string();
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();

    if control_tx
        .send(ControlMsg::MetricsRequest {
            request_id,
            path: full_path,
            response_tx,
        })
        .await
        .is_err()
    {
        return StatusCode::BAD_GATEWAY.into_response();
    }

    match tokio::time::timeout(Duration::from_secs(10), response_rx).await {
        Ok(Ok(resp)) => axum::response::Response::builder()
            .status(resp.status)
            .header("content-type", resp.content_type)
            .body(axum::body::Body::from(resp.body))
            .expect("response body from string never fails")
            .into_response(),
        Ok(Err(_)) => {
            tracing::warn!("metrics proxy: daemon {instance_id} dropped response channel");
            StatusCode::BAD_GATEWAY.into_response()
        }
        Err(_) => {
            tracing::warn!("metrics proxy: daemon {instance_id} timed out");
            StatusCode::GATEWAY_TIMEOUT.into_response()
        }
    }
}

// ── Tunnel list API ─────────────────────────────────────────────────────

async fn list_tunnels(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> axum::response::Response {
    let self_info = match require_auth(&headers, &state.server_api_url, &["admin", "setting"]).await
    {
        Ok(info) => info,
        Err(resp) => return resp,
    };

    let mut tunnels = state.registry.list_tunnels();
    if self_info.token_kind == "setting" {
        let cid = self_info.customer_id;
        tunnels.retain(|t| t.customer_id == cid);
    }
    Json(tunnels).into_response()
}

// ── Federated metrics endpoint ──────────────────────────────────────────

const FEDERATION_SCRAPE_TIMEOUT: Duration = Duration::from_secs(5);
const FEDERATION_CONCURRENCY: usize = 32;

/// One per-target scrape result, ready to be folded into the merged exposition.
struct ScrapeOutcome {
    instance_id: String,
    hostname: String,
    customer_id: String,
    families: Vec<prometheus::proto::MetricFamily>,
    up: bool,
    duration_secs: f64,
}

async fn federated_metrics(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> axum::response::Response {
    let self_info = match require_auth(&headers, &state.server_api_url, &["admin", "setting"]).await
    {
        Ok(info) => info,
        Err(resp) => return resp,
    };

    let mut tunnels = state.registry.list_tunnels();
    if self_info.token_kind == "setting" {
        let cid = self_info.customer_id;
        tunnels.retain(|t| t.customer_id == cid);
    }

    let registry = state.registry.clone();
    let outcomes: Vec<ScrapeOutcome> = futures_util::stream::iter(tunnels.into_iter().map(
        move |tunnel| {
            let registry = registry.clone();
            async move { scrape_one(&registry, tunnel).await }
        },
    ))
    .buffer_unordered(FEDERATION_CONCURRENCY)
    .collect()
    .await;

    let target_count = outcomes.len();

    // Assemble all families into a single ordered map so families with the
    // same name (which can happen across daemons) are merged into one
    // MetricFamily — the encoder requires that.
    let mut families: std::collections::BTreeMap<String, prometheus::proto::MetricFamily> =
        std::collections::BTreeMap::new();

    for outcome in outcomes {
        let labels: [(&str, &str); 3] = [
            ("instance_id", outcome.instance_id.as_str()),
            ("hostname", outcome.hostname.as_str()),
            ("customer_id", outcome.customer_id.as_str()),
        ];
        push_gauge_strs(
            &mut families,
            "mac_mgmt_relay_scrape_up",
            &labels,
            if outcome.up { 1.0 } else { 0.0 },
        );
        push_gauge_strs(
            &mut families,
            "mac_mgmt_relay_scrape_duration_seconds",
            &labels,
            outcome.duration_secs,
        );
        for fam in outcome.families {
            merge_family(&mut families, fam);
        }
    }

    push_gauge_strs(
        &mut families,
        "mac_mgmt_relay_scrape_targets",
        &[],
        target_count as f64,
    );

    let families_vec: Vec<_> = families.into_values().collect();
    match encode_families(&families_vec) {
        Ok(buf) => axum::response::Response::builder()
            .status(StatusCode::OK)
            .header("content-type", PROMETHEUS_CONTENT_TYPE)
            .body(axum::body::Body::from(buf))
            .expect("response builder")
            .into_response(),
        Err(e) => {
            tracing::error!("federated metrics encode failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn scrape_one(
    registry: &Arc<DaemonRegistry>,
    tunnel: crate::daemon_registry::TunnelInfo,
) -> ScrapeOutcome {
    let started = std::time::Instant::now();
    let instance_id = tunnel.instance_id.clone();
    let hostname = tunnel.hostname.clone().unwrap_or_default();
    let customer_id = tunnel
        .customer_id
        .map(|c| c.to_string())
        .unwrap_or_default();

    let Some(control_tx) = registry.get_control_tx(&instance_id) else {
        tracing::debug!("federated metrics: {instance_id} has no control channel");
        return ScrapeOutcome {
            instance_id,
            hostname,
            customer_id,
            families: Vec::new(),
            up: false,
            duration_secs: started.elapsed().as_secs_f64(),
        };
    };

    let request_id = Uuid::new_v4().to_string();
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();

    if control_tx
        .send(ControlMsg::MetricsRequest {
            request_id,
            path: "/metrics".to_string(),
            response_tx,
        })
        .await
        .is_err()
    {
        return ScrapeOutcome {
            instance_id,
            hostname,
            customer_id,
            families: Vec::new(),
            up: false,
            duration_secs: started.elapsed().as_secs_f64(),
        };
    }

    let result = tokio::time::timeout(FEDERATION_SCRAPE_TIMEOUT, response_rx).await;

    match result {
        Ok(Ok(resp)) if resp.status == 200 => {
            match parse_and_relabel(&resp.body, &instance_id, &hostname, &customer_id) {
                Ok(families) => ScrapeOutcome {
                    instance_id,
                    hostname,
                    customer_id,
                    families,
                    up: true,
                    duration_secs: started.elapsed().as_secs_f64(),
                },
                Err(e) => {
                    tracing::warn!("federated metrics: parse error from {instance_id}: {e}");
                    ScrapeOutcome {
                        instance_id,
                        hostname,
                        customer_id,
                        families: Vec::new(),
                        up: false,
                        duration_secs: started.elapsed().as_secs_f64(),
                    }
                }
            }
        }
        Ok(Ok(resp)) => {
            tracing::warn!(
                "federated metrics: {instance_id} returned status {}",
                resp.status
            );
            ScrapeOutcome {
                instance_id,
                hostname,
                customer_id,
                families: Vec::new(),
                up: false,
                duration_secs: started.elapsed().as_secs_f64(),
            }
        }
        Ok(Err(_)) => {
            tracing::warn!("federated metrics: {instance_id} dropped response channel");
            ScrapeOutcome {
                instance_id,
                hostname,
                customer_id,
                families: Vec::new(),
                up: false,
                duration_secs: started.elapsed().as_secs_f64(),
            }
        }
        Err(_) => {
            tracing::warn!("federated metrics: {instance_id} timed out");
            ScrapeOutcome {
                instance_id,
                hostname,
                customer_id,
                families: Vec::new(),
                up: false,
                duration_secs: started.elapsed().as_secs_f64(),
            }
        }
    }
}

/// Merge a parsed family into the accumulator. Families with the same name
/// have their `Metric` rows concatenated; the merged family keeps the type
/// of the first occurrence (which is consistent across daemons because they
/// all run the same exporter).
fn merge_family(
    families: &mut std::collections::BTreeMap<String, prometheus::proto::MetricFamily>,
    incoming: prometheus::proto::MetricFamily,
) {
    use prometheus::proto::MetricFamily;
    let name = incoming.name().to_string();
    match families.get_mut(&name) {
        Some(existing) => {
            for m in incoming.metric {
                existing.mut_metric().push(m);
            }
        }
        None => {
            let mut fam = MetricFamily::default();
            fam.set_name(name.clone());
            fam.set_field_type(incoming.type_());
            for m in incoming.metric {
                fam.mut_metric().push(m);
            }
            families.insert(name, fam);
        }
    }
}
