use serde::{Deserialize, Serialize};

/// Daemon → wrapper request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcRequest {
    /// Query current health and busy status.
    Health,
    /// Kill the managed service process and respawn it.
    Restart,
    /// Run `check_and_upgrade()`, then respawn if an upgrade was installed.
    Upgrade,
    /// Graceful shutdown: stop the service and exit the wrapper.
    Shutdown,
    /// Re-exec the wrapper binary itself (for daemon self-update).
    /// The wrapper kills its child, then calls exec() to replace itself
    /// with the new binary. launchd/systemd KeepAlive handles failures.
    UpdateSelf,
}

/// Wrapper → daemon response (reply to a request).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcResponse {
    /// Current service status (reply to Health).
    Status {
        service: String,
        healthy: bool,
        busy: bool,
        upgrade_pending: bool,
        pid: Option<u32>,
        post_start_done: bool,
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
        service: String,
        exit_code: Option<i32>,
    },
    /// Service became healthy (after startup or crash recovery).
    Healthy {
        service: String,
    },
    /// Service became unhealthy.
    Unhealthy {
        service: String,
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
