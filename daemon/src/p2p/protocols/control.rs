//! Control protocol types — request/response enums for RPC between
//! relay↔daemon and daemon↔daemon.

use serde::{Deserialize, Serialize};

// ── Request types ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlRequest {
    SessionRequest {
        session_id: String,
        session_secret: String,
    },
    MetricsRequest {
        request_id: String,
        path: String,
    },
    ProxyRequest {
        request_id: String,
        tunnel_name: String,
        method: String,
        path: String,
        headers: Vec<(String, String)>,
        body: Option<String>,
    },
    ProxyStreamRequest {
        request_id: String,
        tunnel_name: String,
        method: String,
        path: String,
        headers: serde_json::Value,
        body: Option<String>,
    },
    ProxySessionRequest {
        session_id: String,
        session_secret: String,
        tunnel_name: String,
        mode: String,
        path: String,
    },
    FileListRequest {
        request_id: String,
        tunnel_name: String,
        path: Option<String>,
    },
    FileSessionRequest {
        session_id: String,
        session_secret: String,
        tunnel_name: String,
        mode: String,
        path: Option<String>,
        expected_mtime: Option<i64>,
    },
    ShellSessionRequest {
        session_id: String,
        session_secret: String,
        command_name: String,
        user_arg: Option<String>,
    },
    /// Register with the relay node.
    Register {
        instance_id: String,
        cluster_id: Option<String>,
        hostname: Option<String>,
        agent_name: Option<String>,
    },
    /// Advertise tunnel definitions to the relay.
    TunnelAdvertisement {
        tunnels: serde_json::Value,
        file_tunnels: serde_json::Value,
        shell_tunnels: serde_json::Value,
    },
}

// ── Response types ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlResponse {
    Ok,
    Error {
        message: String,
    },
    MetricsResponse {
        request_id: String,
        body: String,
    },
    ProxyResponse {
        request_id: String,
        status: u16,
        headers: Vec<(String, String)>,
        body: String,
    },
    ProxyStreamHeaders {
        request_id: String,
        status: u16,
        headers: Vec<(String, String)>,
    },
    ProxyStreamChunk {
        request_id: String,
        data: String,
    },
    ProxyStreamEnd {
        request_id: String,
    },
    FileResponse {
        request_id: String,
        data: serde_json::Value,
    },
}

// Wire format for RPC: length-prefixed JSON frames, handled by
// daemon/src/p2p/rpc.rs and relay/src/p2p.rs handle_daemon_rpc.
// The Codec impl and wire helpers were removed — these types are
// serialized directly as serde_json::Value on the RPC stream.
