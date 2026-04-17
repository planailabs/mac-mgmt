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

/// Status of a single service managed by the supervisor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub name: String,
    /// PID of the running child, if one is currently alive.
    #[serde(default)]
    pub pid: Option<u32>,
    /// Resolved executable path of the running child (e.g. `/proc/<pid>/exe`
    /// on Linux). Present when the supervisor could look it up; absent on
    /// platforms that don't expose this cheaply or when the child has no pid.
    #[serde(default)]
    pub exe: Option<String>,
    /// Canonical path of `spec.program` resolved at spawn time via
    /// `which` + `canonicalize`. Unlike `exe` (which points to the
    /// interpreter for scripts), this always points to the script/binary
    /// itself, making store-path drift detection work for shebang wrappers.
    #[serde(default)]
    pub resolved_program: Option<String>,
}

/// Supervisor → daemon reply.
///
/// The `Services` variant carries both `statuses` (new) and `names` (legacy)
/// so that newer daemons can talk to older supervisors and vice-versa; see
/// [`Response::services_list`] for a helper that normalises the two.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Ok,
    Error { message: String },
    Services {
        #[serde(default)]
        statuses: Vec<ServiceStatus>,
        /// Legacy list of service names kept on the wire for older clients
        /// that predate `statuses`.
        // compat: added 2026-04-15, removable after 2026-07-15
        #[serde(default)]
        names: Vec<String>,
    },
}

impl Response {
    /// Build a `Services` response that's backward-compatible on the wire:
    /// populates both `statuses` and `names` from the same list.
    pub fn services(statuses: Vec<ServiceStatus>) -> Self {
        let names = statuses.iter().map(|s| s.name.clone()).collect();
        Response::Services { statuses, names }
    }
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
