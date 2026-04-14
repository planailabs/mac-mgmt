use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Command + args + env describing how to spawn a single process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnSpec {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// Daemon → supervisor request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    /// Register (or replace) a named service. If the spec matches the
    /// currently running one, this is a no-op; otherwise the running process
    /// is killed and the new spec is spawned.
    Register { name: String, spec: SpawnSpec },
    /// Stop and forget a named service.
    Unregister { name: String },
    /// Return the list of currently registered service names.
    List,
    /// Kill all children and exit the supervisor.
    Shutdown,
    /// Re-exec the supervisor binary (used when the mac-mgmt binary was
    /// upgraded and the supervisor needs to pick up the new code).
    UpdateSelf,
}

/// Supervisor → daemon reply.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Ok,
    Error { message: String },
    Services { names: Vec<String> },
}

/// Supervisor → daemon unsolicited notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Notification {
    /// A managed child exited; the supervisor will respawn it.
    Crashed { name: String, exit_code: Option<i32> },
    /// A line of stdout/stderr from a managed child.
    Log { name: String, line: String, is_stderr: bool },
}

/// Wire envelope — every line on the socket is one of these.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Message {
    Request(Request),
    Response(Response),
    Notification(Notification),
}
