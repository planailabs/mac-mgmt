//! Axum-based mock management server for simulation testing.
//!
//! Implements the daemon-facing API surface with fully observable and
//! injectable state. Tests can pre-populate responses, inspect daemon
//! behaviour, and inject faults mid-test.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::sse::{Event, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use mac_mgmt_common::{HeartbeatBody, NixpkgsPin, PushEvent, UpdateTarget};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

// ── Observable state ────────────────────────────────────────────────

/// Snapshot of a received heartbeat for assertion.
#[derive(Debug, Clone)]
pub struct ReceivedHeartbeat {
    pub body: HeartbeatBody,
    pub received_at: std::time::Instant,
}

/// Fault configuration for a single endpoint.
#[derive(Debug, Clone, Default)]
pub struct EndpointFault {
    /// If set, return this status code instead of the normal response.
    pub fail_status: Option<u16>,
    /// If true, close the connection without responding.
    pub drop_connection: bool,
}

/// All mock server state — fully observable from test code.
pub struct MockServerState {
    // ── What the server returns ──────────────────────────────
    pub config: Mutex<serde_json::Value>,
    pub update_target: Mutex<UpdateTarget>,
    pub nixpkgs_pin: Mutex<NixpkgsPin>,
    pub skills: Mutex<HashMap<String, String>>,
    pub mcp_servers: Mutex<serde_json::Value>,
    pub ssh_keys: Mutex<Vec<SshKeyEntry>>,

    // ── What the daemon sent (for assertions) ────────────────
    pub heartbeats: Mutex<Vec<ReceivedHeartbeat>>,
    pub assessments: Mutex<Vec<serde_json::Value>>,
    pub probes: Mutex<Vec<serde_json::Value>>,

    // ── SSE push control ─────────────────────────────────────
    pub push_tx: broadcast::Sender<PushEvent>,

    // ── Fault injection ──────────────────────────────────────
    pub faults: Mutex<HashMap<String, EndpointFault>>,

    // ── Request counters (for assertions) ────────────────────
    pub request_counts: Mutex<HashMap<String, u64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshKeyEntry {
    pub public_key: String,
}

impl MockServerState {
    pub fn new() -> Arc<Self> {
        let (push_tx, _) = broadcast::channel(64);
        Arc::new(Self {
            config: Mutex::new(serde_json::json!({})),
            update_target: Mutex::new(UpdateTarget {
                target_version: None,
                store_path: None,
            }),
            nixpkgs_pin: Mutex::new(NixpkgsPin { commit: None }),
            skills: Mutex::new(HashMap::new()),
            mcp_servers: Mutex::new(serde_json::json!({})),
            ssh_keys: Mutex::new(Vec::new()),
            heartbeats: Mutex::new(Vec::new()),
            assessments: Mutex::new(Vec::new()),
            probes: Mutex::new(Vec::new()),
            push_tx,
            faults: Mutex::new(HashMap::new()),
            request_counts: Mutex::new(HashMap::new()),
        })
    }

    /// Push a command to all connected daemon SSE clients.
    pub fn push(&self, event: PushEvent) {
        let _ = self.push_tx.send(event);
    }

    /// Set a fault for a given endpoint path (e.g. "/api/heartbeat").
    pub fn set_fault(&self, endpoint: &str, fault: EndpointFault) {
        self.faults
            .lock()
            .unwrap()
            .insert(endpoint.to_string(), fault);
    }

    /// Clear all faults.
    pub fn clear_faults(&self) {
        self.faults.lock().unwrap().clear();
    }

    /// Get the count of requests to a given endpoint.
    pub fn request_count(&self, endpoint: &str) -> u64 {
        *self
            .request_counts
            .lock()
            .unwrap()
            .get(endpoint)
            .unwrap_or(&0)
    }

    /// Get all received heartbeats.
    pub fn get_heartbeats(&self) -> Vec<ReceivedHeartbeat> {
        self.heartbeats.lock().unwrap().clone()
    }

    /// Get heartbeats from a specific instance.
    pub fn heartbeats_from(&self, instance_id: &str) -> Vec<ReceivedHeartbeat> {
        self.heartbeats
            .lock()
            .unwrap()
            .iter()
            .filter(|h| h.body.instance_id == instance_id)
            .cloned()
            .collect()
    }

    /// Clear heartbeat log.
    pub fn clear_heartbeats(&self) {
        self.heartbeats.lock().unwrap().clear();
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

fn bump_counter(state: &MockServerState, endpoint: &str) {
    *state
        .request_counts
        .lock()
        .unwrap()
        .entry(endpoint.to_string())
        .or_insert(0) += 1;
}

fn check_fault(state: &MockServerState, endpoint: &str) -> Option<StatusCode> {
    let faults = state.faults.lock().unwrap();
    if let Some(fault) = faults.get(endpoint) {
        if let Some(status) = fault.fail_status {
            return Some(StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR));
        }
    }
    None
}

// ── Route handlers ──────────────────────────────────────────────────

async fn get_config(State(state): State<Arc<MockServerState>>) -> impl IntoResponse {
    bump_counter(&state, "/api/config");
    if let Some(status) = check_fault(&state, "/api/config") {
        return (status, Json(serde_json::json!({}))).into_response();
    }
    let cfg = state.config.lock().unwrap().clone();
    Json(cfg).into_response()
}

async fn get_update(State(state): State<Arc<MockServerState>>) -> impl IntoResponse {
    bump_counter(&state, "/api/update");
    if let Some(status) = check_fault(&state, "/api/update") {
        return (status, Json(serde_json::json!({}))).into_response();
    }
    let target = state.update_target.lock().unwrap().clone();
    Json(target).into_response()
}

async fn get_nixpkgs(State(state): State<Arc<MockServerState>>) -> impl IntoResponse {
    bump_counter(&state, "/api/nixpkgs");
    if let Some(status) = check_fault(&state, "/api/nixpkgs") {
        return (status, Json(serde_json::json!({}))).into_response();
    }
    let pin = state.nixpkgs_pin.lock().unwrap().clone();
    Json(pin).into_response()
}

async fn get_skills(State(state): State<Arc<MockServerState>>) -> impl IntoResponse {
    bump_counter(&state, "/api/skills");
    if let Some(status) = check_fault(&state, "/api/skills") {
        return (status, Json(serde_json::json!({}))).into_response();
    }
    let skills = state.skills.lock().unwrap().clone();
    Json(skills).into_response()
}

async fn get_mcp_servers(State(state): State<Arc<MockServerState>>) -> impl IntoResponse {
    bump_counter(&state, "/api/mcp-servers");
    if let Some(status) = check_fault(&state, "/api/mcp-servers") {
        return (status, Json(serde_json::json!({}))).into_response();
    }
    let servers = state.mcp_servers.lock().unwrap().clone();
    Json(servers).into_response()
}

async fn get_ssh_keys(State(state): State<Arc<MockServerState>>) -> impl IntoResponse {
    bump_counter(&state, "/api/ssh-keys");
    if let Some(status) = check_fault(&state, "/api/ssh-keys") {
        return (status, Json(serde_json::json!({}))).into_response();
    }
    let keys = state.ssh_keys.lock().unwrap().clone();
    Json(keys).into_response()
}

async fn post_heartbeat(
    State(state): State<Arc<MockServerState>>,
    Json(body): Json<HeartbeatBody>,
) -> impl IntoResponse {
    bump_counter(&state, "/api/heartbeat");
    if let Some(status) = check_fault(&state, "/api/heartbeat") {
        return status.into_response();
    }
    state.heartbeats.lock().unwrap().push(ReceivedHeartbeat {
        body,
        received_at: std::time::Instant::now(),
    });
    StatusCode::OK.into_response()
}

async fn post_assessment(
    State(state): State<Arc<MockServerState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    bump_counter(&state, "/api/assessment");
    if let Some(status) = check_fault(&state, "/api/assessment") {
        return status;
    }
    state.assessments.lock().unwrap().push(body);
    StatusCode::OK
}

async fn post_assessment_probe(
    State(state): State<Arc<MockServerState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    bump_counter(&state, "/api/assessment/probe");
    if let Some(status) = check_fault(&state, "/api/assessment/probe") {
        return status;
    }
    state.probes.lock().unwrap().push(body);
    StatusCode::OK
}

async fn sse_events(
    State(state): State<Arc<MockServerState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    bump_counter(&state, "/api/events");
    let rx = state.push_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| match result {
        Ok(event) => {
            let json = serde_json::to_string(&event).unwrap();
            Some(Ok(Event::default().data(json)))
        }
        Err(_) => None,
    });
    Sse::new(stream)
}

// ── Router ──────────────────────────────────────────────────────────

pub fn router(state: Arc<MockServerState>) -> Router {
    Router::new()
        .route("/api/config", get(get_config))
        .route("/api/update", get(get_update))
        .route("/api/nixpkgs", get(get_nixpkgs))
        .route("/api/skills", get(get_skills))
        .route("/api/mcp-servers", get(get_mcp_servers))
        .route("/api/ssh-keys", get(get_ssh_keys))
        .route("/api/heartbeat", post(post_heartbeat))
        .route("/api/assessment", post(post_assessment))
        .route("/api/assessment/probe", post(post_assessment_probe))
        .route("/api/events", get(sse_events))
        .with_state(state)
}

/// Start the mock server on a random available port. Returns the address
/// and state handle.
pub async fn start() -> (SocketAddr, Arc<MockServerState>) {
    let state = MockServerState::new();
    let app = router(Arc::clone(&state));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (addr, state)
}
