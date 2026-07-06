use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Tracked container instance.
#[derive(Clone, Debug, Serialize)]
pub struct InstanceInfo {
    pub name: String,
    pub image: String,
    pub created_at: DateTime<Utc>,
}

/// Output of a command execution inside a container.
#[derive(Debug)]
pub struct ExecOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Full specification for launching an instance, beyond the simple
/// `launch(image, name)` path: lets callers pass a non-container type,
/// a specific image server (e.g. an OCI registry remote), extra profiles,
/// and instance config keys such as `cloud-init.user-data`.
#[derive(Clone, Debug, Default)]
pub struct LaunchSpec {
    pub name: String,
    /// Image alias on `image_server` (or a local alias when `image_server` is None).
    pub image_alias: String,
    /// Image server URL. `None` = launch from a local image alias.
    pub image_server: Option<String>,
    /// Source protocol, e.g. "simplestreams" or "oci". Defaults to "simplestreams".
    pub protocol: Option<String>,
    /// "container" (default) or "virtual-machine".
    pub instance_type: Option<String>,
    /// Whether the instance is ephemeral (auto-deleted on stop). Default false
    /// for orchestrated cluster nodes (we manage teardown explicitly).
    pub ephemeral: bool,
    pub profiles: Vec<String>,
    /// Instance config keys, e.g. {"cloud-init.user-data": "...", "security.nesting": "true"}.
    pub config: serde_json::Map<String, serde_json::Value>,
}

/// A cached OS image entry from the Incus image server.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OsImage {
    pub alias: String,
    pub description: String,
    pub os: String,
    pub release: String,
    pub variant: String,
    pub image_type: String,
}

// -- MCP tool parameter structs --

#[derive(Deserialize, JsonSchema)]
pub struct SystemCreateParams {
    /// OS image alias (e.g. "ubuntu/26.04", "alpine/3.21", "debian/12").
    /// Use os_list to discover available aliases.
    pub os: String,
    /// Optional instance name. Auto-generated if omitted.
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct OsListParams {
    /// Optional filter string to narrow results (e.g. "ubuntu", "alpine", "debian").
    /// Matches against alias, OS name, and description.
    #[serde(default)]
    pub filter: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct SystemExecuteParams {
    /// Shell command to execute inside the container. Runs via `sh -c "..."`,
    /// so pipes, redirects, and shell features work.
    pub command: String,
    /// Container name. Defaults to the last created container if omitted.
    #[serde(default)]
    pub name: Option<String>,
    /// Timeout in seconds (default 120, max 600).
    #[serde(default)]
    pub timeout: Option<u64>,
}

#[derive(Deserialize, JsonSchema)]
pub struct SystemDestroyParams {
    /// Container name to destroy.
    pub name: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct SystemFileWriteParams {
    /// Container name. Defaults to last created if omitted.
    #[serde(default)]
    pub name: Option<String>,
    /// Absolute path inside the container where the file will be written.
    pub path: String,
    /// File content (text).
    pub content: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct SystemFileReadParams {
    /// Container name. Defaults to last created if omitted.
    #[serde(default)]
    pub name: Option<String>,
    /// Absolute path inside the container to read.
    pub path: String,
}

impl SystemExecuteParams {
    pub fn effective_timeout(&self) -> Duration {
        let secs = self.timeout.unwrap_or(120).min(600);
        Duration::from_secs(secs)
    }
}
