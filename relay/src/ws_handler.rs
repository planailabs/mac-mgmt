use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
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
use crate::ssh_listener;

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
        .route("/health", get(health))
        .with_state(state)
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

// ── Daemon registration WebSocket ───────────────────────────────────────

#[derive(Debug, Deserialize)]
struct RegisterQuery {
    instance_id: String,
    agent_name: Option<String>,
}

async fn ws_daemon_register(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    Query(query): Query<RegisterQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let token = match extract_bearer(&headers) {
        Some(t) => t,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };

    let self_info = match validate_token(&state.server_api_url, &token).await {
        Ok(info) => info,
        Err(status) => return status.into_response(),
    };

    if self_info.token_kind != "sync" {
        return StatusCode::FORBIDDEN.into_response();
    }

    ws.on_upgrade(move |socket| {
        handle_daemon_ws(socket, query, self_info, state)
    })
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
    let agent_name = query.agent_name;

    let port = match state.registry.allocate_port() {
        Some(p) => p,
        None => {
            tracing::error!("no available ports for {instance_id}");
            return;
        }
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
        agent_name,
        ssh_port: port,
        connected_at: Utc::now(),
        control_tx,
        listener_handle,
    };

    state.registry.register(conn);
    tracing::info!("daemon {instance_id} registered on port {port}");

    let (mut ws_sink, mut ws_stream) = socket.split();

    // Send registration confirmation
    let reg_msg = serde_json::json!({
        "type": "registered",
        "ssh_port": port
    });
    if ws_sink
        .send(Message::Text(reg_msg.to_string().into()))
        .await
        .is_err()
    {
        state.registry.unregister(&instance_id);
        return;
    }

    // Track pending metrics requests
    let mut pending_metrics: HashMap<String, tokio::sync::oneshot::Sender<MetricsResponse>> =
        HashMap::new();

    // Control loop: forward control messages and handle daemon responses
    loop {
        tokio::select! {
            Some(msg) = control_rx.recv() => {
                match msg {
                    ControlMsg::SessionRequest { session_id } => {
                        let msg = serde_json::json!({
                            "type": "session_request",
                            "session_id": session_id
                        });
                        if ws_sink.send(Message::Text(msg.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    ControlMsg::MetricsRequest { request_id, path, response_tx } => {
                        let msg = serde_json::json!({
                            "type": "metrics_request",
                            "request_id": request_id,
                            "path": path
                        });
                        pending_metrics.insert(request_id, response_tx);
                        if ws_sink.send(Message::Text(msg.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                }
            }
            msg = ws_stream.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(daemon_msg) = serde_json::from_str::<DaemonWsMessage>(&text) {
                            if daemon_msg.r#type == "metrics_response" {
                                if let Some(req_id) = daemon_msg.request_id {
                                    if let Some(tx) = pending_metrics.remove(&req_id) {
                                        let _ = tx.send(MetricsResponse {
                                            status: daemon_msg.status.unwrap_or(502),
                                            content_type: daemon_msg.content_type
                                                .unwrap_or_else(|| "text/plain".to_string()),
                                            body: daemon_msg.body.unwrap_or_default(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(data))) => {
                        let _ = ws_sink.send(Message::Pong(data)).await;
                    }
                    Some(Err(e)) => {
                        tracing::warn!("daemon WS error: {e}");
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    tracing::info!("daemon {instance_id} disconnected, freeing port {port}");
    state.registry.unregister(&instance_id);
}

// ── Daemon data session WebSocket ───────────────────────────────────────

async fn ws_daemon_session(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let token = match extract_bearer(&headers) {
        Some(t) => t,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };

    let self_info = match validate_token(&state.server_api_url, &token).await {
        Ok(info) => info,
        Err(status) => return status.into_response(),
    };

    if self_info.token_kind != "sync" {
        return StatusCode::FORBIDDEN.into_response();
    }

    ws.on_upgrade(move |socket| handle_data_session(socket, session_id))
}

async fn handle_data_session(socket: WebSocket, session_id: String) {
    let tcp_stream = match bridge::take_pending_session(&session_id) {
        Some(s) => s,
        None => {
            tracing::warn!("no pending session for {session_id}");
            return;
        }
    };

    tracing::info!("bridging session {session_id}");
    bridge::bridge_tcp_ws(tcp_stream, socket).await;
    tracing::info!("session {session_id} ended");
}

// ── Metrics proxy ───────────────────────────────────────────────────────

async fn proxy_metrics(
    headers: HeaderMap,
    Path((instance_id, path)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let token = match extract_bearer(&headers) {
        Some(t) => t,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };

    let self_info = match validate_token(&state.server_api_url, &token).await {
        Ok(info) => info,
        Err(status) => return status.into_response(),
    };

    if self_info.token_kind != "admin" && self_info.token_kind != "setting" {
        return StatusCode::FORBIDDEN.into_response();
    }

    // Build the local path with query string
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

    let control_tx = match state.registry.get_control_tx(&instance_id) {
        Some(tx) => tx,
        None => return StatusCode::NOT_FOUND.into_response(),
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
            .unwrap()
            .into_response(),
        Ok(Err(_)) => StatusCode::BAD_GATEWAY.into_response(),
        Err(_) => StatusCode::GATEWAY_TIMEOUT.into_response(),
    }
}

// ── Tunnel list API ─────────────────────────────────────────────────────

async fn list_tunnels(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let token = match extract_bearer(&headers) {
        Some(t) => t,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };

    let self_info = match validate_token(&state.server_api_url, &token).await {
        Ok(info) => info,
        Err(status) => return status.into_response(),
    };

    // Only admin or setting tokens can list tunnels
    if self_info.token_kind != "admin" && self_info.token_kind != "setting" {
        return StatusCode::FORBIDDEN.into_response();
    }

    let mut tunnels = state.registry.list_tunnels();
    if self_info.token_kind == "setting" {
        let cid = self_info.customer_id;
        tunnels.retain(|t| t.customer_id == cid);
    }
    Json(tunnels).into_response()
}
