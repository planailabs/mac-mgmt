use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Command + args + env for spawning a service process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnSpec {
    pub program: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// Daemon → wrapper request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcRequest {
    /// Spawn (or respawn) the service with the given command spec.
    Spawn(SpawnSpec),
    /// Kill the child and exit the wrapper.
    Shutdown,
    /// Re-exec the wrapper binary itself (for daemon self-update).
    UpdateSelf,
}

/// Wrapper → daemon response (reply to a request).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcResponse {
    Ok,
    Error { message: String },
}

/// Wrapper → daemon unsolicited notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcNotification {
    /// Service process exited.
    Crashed { exit_code: Option<i32> },
    /// Log line from the service process.
    Log { line: String, is_stderr: bool },
}

/// Wire envelope — every line on the socket is one of these.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IpcMessage {
    Request(IpcRequest),
    Response(IpcResponse),
    Notification(IpcNotification),
}
