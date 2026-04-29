//! Handles incoming control protocol requests from relay or peers.
//!
//! Dispatches to metrics, proxy, file, and shell handlers based on
//! the request type.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::RwLock;

use super::protocols::control::{ControlRequest, ControlResponse};
use super::proxy_helpers::{self, TunnelTarget};
use crate::file_tunnels::FileTunnelRegistry;
use crate::shell_tunnels::ShellTunnelRegistry;

/// Shared state for handling control requests.
pub struct HandlerState {
    pub ssh_allowed: Arc<AtomicBool>,
    pub tunnel_defs: Arc<RwLock<HashMap<String, TunnelTarget>>>,
    pub file_tunnel_registry: Arc<RwLock<FileTunnelRegistry>>,
    pub shell_tunnel_registry: Arc<RwLock<ShellTunnelRegistry>>,
    pub metrics_port: u16,
    pub fake_origin_local: bool,
    pub client: reqwest::Client,
}

/// Process a control request and return the response.
pub async fn handle_control_request(
    state: &HandlerState,
    request: ControlRequest,
) -> ControlResponse {
    match request {
        ControlRequest::MetricsRequest { request_id, path } => {
            handle_metrics(state, request_id, path).await
        }
        ControlRequest::ProxyRequest {
            request_id,
            tunnel_name,
            method,
            path,
            headers,
            body,
        } => {
            handle_proxy(
                state,
                request_id,
                tunnel_name,
                method,
                path,
                headers,
                body,
            )
            .await
        }
        ControlRequest::SessionRequest { .. } => {
            if !state.ssh_allowed.load(Ordering::Relaxed) {
                return ControlResponse::Error {
                    message: "SSH access denied".into(),
                };
            }
            // TODO: set up SSH session via libp2p substream
            ControlResponse::Ok
        }
        ControlRequest::FileListRequest {
            request_id,
            tunnel_name,
            path,
        } => {
            handle_file_list(state, request_id, tunnel_name, path).await
        }
        _ => ControlResponse::Error {
            message: "not yet implemented".into(),
        },
    }
}

async fn handle_metrics(
    state: &HandlerState,
    request_id: String,
    path: String,
) -> ControlResponse {
    let url = format!("http://127.0.0.1:{}{path}", state.metrics_port);
    match state
        .client
        .get(&url)
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) => {
            let body = resp.text().await.unwrap_or_default();
            ControlResponse::MetricsResponse { request_id, body }
        }
        Err(e) => ControlResponse::Error {
            message: format!("metrics fetch failed: {e}"),
        },
    }
}

async fn handle_proxy(
    state: &HandlerState,
    request_id: String,
    tunnel_name: String,
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Option<String>,
) -> ControlResponse {
    let tunnel_defs = state.tunnel_defs.read().await;
    let Some(target) = tunnel_defs.get(&tunnel_name) else {
        return ControlResponse::Error {
            message: format!("unknown tunnel: {tunnel_name}"),
        };
    };
    let target = target.clone();
    drop(tunnel_defs);

    let req = proxy_helpers::build_proxy_request(
        &state.client,
        &target,
        &method,
        &path,
        state.fake_origin_local,
    );
    let req = proxy_helpers::apply_headers_vec(req, &headers, state.fake_origin_local, &target);
    let req = proxy_helpers::apply_body_b64(req, body);

    match req.timeout(Duration::from_secs(30)).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let resp_headers: Vec<(String, String)> = resp
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();
            let body_bytes = resp.bytes().await.unwrap_or_default();
            use base64::Engine;
            let body_b64 =
                base64::engine::general_purpose::STANDARD.encode(&body_bytes);
            ControlResponse::ProxyResponse {
                request_id,
                status,
                headers: resp_headers,
                body: body_b64,
            }
        }
        Err(e) => ControlResponse::Error {
            message: format!("proxy request failed: {e}"),
        },
    }
}

async fn handle_file_list(
    state: &HandlerState,
    request_id: String,
    tunnel_name: String,
    path: Option<String>,
) -> ControlResponse {
    let registry = state.file_tunnel_registry.read().await;
    let Some(tunnel) = registry.get(&tunnel_name) else {
        return ControlResponse::Error {
            message: format!("unknown file tunnel: {tunnel_name}"),
        };
    };

    let (status, data) =
        crate::file_tunnels::handle_list(tunnel, path.as_deref());
    if status == 200 {
        ControlResponse::FileResponse { request_id, data }
    } else {
        ControlResponse::Error {
            message: format!("file list failed ({status}): {data}"),
        }
    }
}
