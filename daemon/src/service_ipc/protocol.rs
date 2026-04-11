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
    /// Query wrapper status (child pid, running).
    Status,
    /// Kill the child and exit the wrapper.
    Shutdown,
    /// Re-exec the wrapper binary itself (for daemon self-update).
    UpdateSelf,
}

/// Wrapper → daemon response (reply to a request).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcResponse {
    /// Current wrapper status.
    Status {
        pid: Option<u32>,
        running: bool,
    },
    /// Command was accepted and executed.
    Ack {
        command: String,
    },
    /// Command failed.
    Error {
        command: String,
        message: String,
    },
}

/// Wrapper → daemon unsolicited notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcNotification {
    /// Service process exited unexpectedly.
    Crashed {
        exit_code: Option<i32>,
    },
    /// Log line from the service's stdout or stderr.
    Log {
        line: String,
        is_stderr: bool,
    },
}

/// Top-level wire envelope: every line on the socket is one of these.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IpcMessage {
    Request(IpcRequest),
    Response(IpcResponse),
    Notification(IpcNotification),
}

impl From<IpcRequest> for IpcMessage {
    fn from(r: IpcRequest) -> Self {
        Self::Request(r)
    }
}

impl From<IpcResponse> for IpcMessage {
    fn from(r: IpcResponse) -> Self {
        Self::Response(r)
    }
}

impl From<IpcNotification> for IpcMessage {
    fn from(n: IpcNotification) -> Self {
        Self::Notification(n)
    }
}
