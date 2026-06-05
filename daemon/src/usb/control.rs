//! Loopback control + status API for the USB stack.
//!
//! Bound to `[::1]` only. The native overview desktop app (and the repo tests)
//! drive it to list services from the `mac-mgmt-services` supervisor and
//! start/stop/restart them, and to read offline state. Install/update are
//! online-only actions (greyed in the overview when offline).

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::Deserialize;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, watch};

/// Shared state for the control server.
#[derive(Clone)]
pub struct ControlState {
    /// Supervisor unix socket path.
    pub socket_path: PathBuf,
    /// True when the stack is running offline (install/update disabled).
    pub offline: bool,
    /// Channel to the stack loop for online install requests.
    pub install_tx: mpsc::Sender<InstallReq>,
    /// Signal stack shutdown (e.g. overview window closed).
    pub shutdown_tx: watch::Sender<bool>,
}

/// A request to (online-)install a service, answered by the stack loop.
pub struct InstallReq {
    pub name: String,
    pub resp: oneshot::Sender<Result<(), String>>,
}

#[derive(Deserialize)]
struct ServiceReq {
    name: String,
}

/// Build the control router.
pub fn router(state: ControlState) -> Router {
    Router::new()
        .route("/status", get(status))
        .route("/usb/start", post(start))
        .route("/usb/stop", post(stop))
        .route("/usb/restart", post(restart))
        .route("/usb/install", post(install))
        .route("/usb/shutdown", post(shutdown))
        .with_state(state)
}

/// Connect a short-lived supervisor client.
async fn client(state: &ControlState) -> Result<mac_mgmt_services::Client, String> {
    mac_mgmt_services::Client::connect(&state.socket_path, Duration::from_secs(5))
        .await
        .map_err(|e| format!("cannot reach supervisor: {e}"))
}

/// GET /status — list services + offline flag.
async fn status(State(state): State<ControlState>) -> impl IntoResponse {
    let mut c = match client(&state).await {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "offline": state.offline, "error": e })),
            );
        }
    };
    let services = match c.list().await {
        Ok(list) => list
            .into_iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "pid": s.pid,
                    "running": s.pid.is_some(),
                    "program": s.spec.as_ref().map(|sp| sp.program.clone()),
                })
            })
            .collect::<Vec<_>>(),
        Err(e) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "offline": state.offline, "error": e.to_string() })),
            );
        }
    };
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "offline": state.offline,
            "services": services,
        })),
    )
}

/// Map a `Result<(), String>` to an HTTP response.
fn svc_result(action: &str, name: &str, r: Result<(), String>) -> (StatusCode, Json<serde_json::Value>) {
    match r {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({ "ok": true, "action": action, "name": name })),
        ),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "ok": false, "action": action, "name": name, "error": e })),
        ),
    }
}

async fn start(State(state): State<ControlState>, Json(req): Json<ServiceReq>) -> impl IntoResponse {
    let r = match client(&state).await {
        Ok(mut c) => c.start_service(&req.name).await.map_err(|e| e.to_string()),
        Err(e) => Err(e),
    };
    svc_result("start", &req.name, r)
}

async fn stop(State(state): State<ControlState>, Json(req): Json<ServiceReq>) -> impl IntoResponse {
    let r = match client(&state).await {
        Ok(mut c) => c.stop_service(&req.name).await.map_err(|e| e.to_string()),
        Err(e) => Err(e),
    };
    svc_result("stop", &req.name, r)
}

async fn restart(
    State(state): State<ControlState>,
    Json(req): Json<ServiceReq>,
) -> impl IntoResponse {
    let r = match client(&state).await {
        Ok(mut c) => c.restart_service(&req.name).await.map_err(|e| e.to_string()),
        Err(e) => Err(e),
    };
    svc_result("restart", &req.name, r)
}

/// POST /usb/install — online only. Forwards to the stack loop (which owns the
/// ServiceManager + nix). Returns 409 when offline.
async fn install(
    State(state): State<ControlState>,
    Json(req): Json<ServiceReq>,
) -> impl IntoResponse {
    if state.offline {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "ok": false,
                "error": "install is disabled in offline mode",
            })),
        );
    }
    let (tx, rx) = oneshot::channel();
    if state
        .install_tx
        .send(InstallReq {
            name: req.name.clone(),
            resp: tx,
        })
        .await
        .is_err()
    {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "ok": false, "error": "stack loop is gone" })),
        );
    }
    let r = rx.await.unwrap_or_else(|_| Err("install dropped".into()));
    svc_result("install", &req.name, r)
}

async fn shutdown(State(state): State<ControlState>) -> impl IntoResponse {
    let _ = state.shutdown_tx.send(true);
    (StatusCode::OK, Json(serde_json::json!({ "ok": true })))
}
