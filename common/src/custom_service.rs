//! Config types for user-defined custom services.
//!
//! A `[[custom-service]]` entry in `config.toml` creates a virtual service
//! that can expose tunnels, file tunnels, shell commands, probes, inventory,
//! samples, and security checks — all without writing Rust code.
//!
//! If `spawn` is set the service becomes fully managed (supervised process);
//! otherwise it runs as an integrated service (no process, capabilities only).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{FindingSeverity, InventoryValueType};

// ── Top-level config ─────────────────────────────────────────────────────

/// One custom service definition — maps to a `[[custom-service]]` TOML entry.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CustomServiceConfig {
    /// Service name — used as the identifier in heartbeats, tunnels, etc.
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// If present, the daemon spawns and supervises this process (managed mode).
    /// If absent, the service is integrated (no process).
    #[serde(default)]
    pub spawn: Option<SpawnDef>,

    /// Optional health check command. Exit code 0 = healthy.
    #[serde(default)]
    pub health_check: Option<HealthCheckDef>,

    #[serde(default)]
    pub tunnels: Vec<CustomTunnelDef>,
    #[serde(default)]
    pub files: Vec<CustomFileDef>,
    #[serde(default)]
    pub commands: Vec<CustomCommandDef>,
    #[serde(default)]
    pub probes: Vec<CustomProbeDef>,
    #[serde(default)]
    pub inventory: Vec<CustomInventoryDef>,
    #[serde(default)]
    pub samples: Vec<CustomInventoryDef>,
    #[serde(default)]
    pub security: Vec<CustomSecurityDef>,
}

impl Default for CustomServiceConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            enabled: true,
            spawn: None,
            health_check: None,
            tunnels: Vec::new(),
            files: Vec::new(),
            commands: Vec::new(),
            probes: Vec::new(),
            inventory: Vec::new(),
            samples: Vec::new(),
            security: Vec::new(),
        }
    }
}

fn default_true() -> bool {
    true
}

// ── Spawn ────────────────────────────────────────────────────────────────

/// Process spawn specification. When present, the custom service becomes
/// a managed service with full lifecycle (spawn → health-check → restart).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SpawnDef {
    /// Program to execute (e.g. "grafana-server").
    pub command: String,
    /// Fixed arguments passed to the command.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables set for the spawned process.
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
}

// ── Health check ─────────────────────────────────────────────────────────

/// Health check command. Exit code 0 = healthy, non-zero = unhealthy.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HealthCheckDef {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Timeout in seconds (default: 10).
    #[serde(default = "default_health_timeout")]
    pub timeout_secs: u64,
}

fn default_health_timeout() -> u64 {
    10
}

// ── Tunnels ──────────────────────────────────────────────────────────────

/// A TCP port to expose through the relay.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CustomTunnelDef {
    /// Short, URL-safe name (e.g. "grafana").
    pub name: String,
    /// Host the service listens on.
    #[serde(default = "default_host")]
    pub host: String,
    /// TCP port number.
    pub port: u16,
}

fn default_host() -> String {
    "127.0.0.1".into()
}

// ── File tunnels ─────────────────────────────────────────────────────────

/// A file or folder exposed for remote editing.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CustomFileDef {
    File {
        name: String,
        path: String,
        #[serde(default)]
        writable: bool,
        #[serde(default)]
        description: String,
    },
    Folder {
        name: String,
        path: String,
        #[serde(default)]
        writable: bool,
        #[serde(default)]
        allow_write: Vec<String>,
        #[serde(default)]
        include: Option<Vec<String>>,
        #[serde(default)]
        description: String,
    },
}

// ── Shell commands ───────────────────────────────────────────────────────

/// A predefined shell command exposed for remote execution.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CustomCommandDef {
    /// Short, URL-safe identifier (e.g. "grafana-reload").
    pub name: String,
    /// The program to execute.
    pub command: String,
    /// Fixed arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// Human-readable description for the UI.
    #[serde(default)]
    pub description: String,
    /// If set, the command accepts one user-provided argument.
    #[serde(default)]
    pub arg_template: Option<CustomArgTemplate>,
    /// Custom timeout in seconds (default: 300).
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

/// Template for a user-provided argument on a shell command.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CustomArgTemplate {
    /// Label shown in the UI (e.g. "Model name").
    pub label: String,
    /// Placeholder text for the input field.
    #[serde(default)]
    pub placeholder: String,
    /// Optional regex the argument must match.
    #[serde(default)]
    pub validation: Option<String>,
}

// ── Probes ───────────────────────────────────────────────────────────────

/// Probe classification.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CustomProbeKind {
    Liveness,
    Functional,
}

/// A custom probe definition. Exactly one of `http` or `exec` must be set.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CustomProbeDef {
    pub name: String,
    #[serde(default = "default_probe_kind")]
    pub kind: CustomProbeKind,
    /// HTTP-based probe.
    #[serde(default)]
    pub http: Option<HttpProbeDef>,
    /// Command-based probe (exit 0 = healthy).
    #[serde(default)]
    pub exec: Option<ExecProbeDef>,
}

fn default_probe_kind() -> CustomProbeKind {
    CustomProbeKind::Liveness
}

/// HTTP probe: make a request and check the status code.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HttpProbeDef {
    pub url: String,
    /// HTTP method (default: "GET").
    #[serde(default = "default_http_method")]
    pub method: String,
    /// Expected HTTP status code (default: 200).
    #[serde(default = "default_expected_status")]
    pub expected_status: u16,
    /// Optional request body.
    #[serde(default)]
    pub body: Option<String>,
    /// Optional headers as key-value pairs.
    #[serde(default)]
    pub headers: Vec<[String; 2]>,
    /// Timeout in seconds (default: 10).
    #[serde(default = "default_probe_timeout")]
    pub timeout_secs: u64,
}

fn default_http_method() -> String {
    "GET".into()
}
fn default_expected_status() -> u16 {
    200
}
fn default_probe_timeout() -> u64 {
    10
}

/// Command-based probe: run a command, exit 0 = pass.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExecProbeDef {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Timeout in seconds (default: 10).
    #[serde(default = "default_probe_timeout")]
    pub timeout_secs: u64,
}

// ── Inventory / Samples ──────────────────────────────────────────────────

/// An inventory or sample entry. Exactly one of `static_value` or `command`
/// must be set.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CustomInventoryDef {
    /// Machine-readable key (e.g. "grafana_version").
    pub id: String,
    /// Human-readable label (e.g. "Grafana Version").
    pub name: String,
    /// Display type hint.
    #[serde(default = "default_value_type")]
    pub value_type: InventoryValueType,
    /// Static value (used directly, no command executed).
    #[serde(default)]
    pub static_value: Option<serde_json::Value>,
    /// Command to run to obtain the value.
    #[serde(default)]
    pub command: Option<InventoryCommandDef>,
}

fn default_value_type() -> InventoryValueType {
    InventoryValueType::String
}

/// Command that produces an inventory/sample value.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InventoryCommandDef {
    /// Program to execute.
    pub run: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Optional regex — first capture group becomes the value.
    /// If unset, full trimmed stdout is used.
    #[serde(default)]
    pub parse_regex: Option<String>,
    /// Timeout in seconds (default: 30).
    #[serde(default = "default_cmd_timeout")]
    pub timeout_secs: u64,
}

fn default_cmd_timeout() -> u64 {
    30
}

// ── Security checks ──────────────────────────────────────────────────────

/// A custom security check. Exactly one of `exec` or `file_check` must be set.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CustomSecurityDef {
    /// Machine-readable identifier (e.g. "grafana_anon_auth").
    pub id: String,
    /// Severity if the check fails.
    #[serde(default = "default_severity")]
    pub severity: FindingSeverity,
    /// Human-readable description of what this check validates.
    pub message: String,
    /// Command check: exit 0 = pass.
    #[serde(default)]
    pub exec: Option<SecurityExecDef>,
    /// File existence/permission check.
    #[serde(default)]
    pub file_check: Option<SecurityFileCheckDef>,
}

fn default_severity() -> FindingSeverity {
    FindingSeverity::Medium
}

/// Command-based security check: exit 0 = pass, non-zero = fail.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SecurityExecDef {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Timeout in seconds (default: 30).
    #[serde(default = "default_cmd_timeout")]
    pub timeout_secs: u64,
}

/// File-based security check: validates existence and/or permissions.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SecurityFileCheckDef {
    /// Path to check.
    pub path: String,
    /// If true, the file must exist for the check to pass.
    #[serde(default = "default_true")]
    pub exists: bool,
    /// Octal permission ceiling (e.g. "0640"). File must not be more
    /// permissive than this for the check to pass.
    #[serde(default)]
    pub max_mode: Option<String>,
    /// If set, file owner must match this user.
    #[serde(default)]
    pub owner: Option<String>,
}

// ── Validation ───────────────────────────────────────────────────────────

impl CustomServiceConfig {
    pub fn validate(&self) -> Result<(), crate::ValidationError> {
        if self.name.is_empty() {
            return Err(crate::ValidationError(
                "custom-service: name must not be empty".into(),
            ));
        }
        // name must be URL-safe
        if !self
            .name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(crate::ValidationError(format!(
                "custom-service '{}': name must be URL-safe (alphanumeric, hyphens, underscores)",
                self.name
            )));
        }
        if let Some(spawn) = &self.spawn {
            if spawn.command.is_empty() {
                return Err(crate::ValidationError(format!(
                    "custom-service '{}': spawn.command must not be empty",
                    self.name
                )));
            }
        }
        for probe in &self.probes {
            if probe.http.is_none() && probe.exec.is_none() {
                return Err(crate::ValidationError(format!(
                    "custom-service '{}': probe '{}' must have either http or exec",
                    self.name, probe.name
                )));
            }
            if probe.http.is_some() && probe.exec.is_some() {
                return Err(crate::ValidationError(format!(
                    "custom-service '{}': probe '{}' must have only one of http or exec",
                    self.name, probe.name
                )));
            }
        }
        for inv in self.inventory.iter().chain(self.samples.iter()) {
            if inv.static_value.is_none() && inv.command.is_none() {
                return Err(crate::ValidationError(format!(
                    "custom-service '{}': inventory/sample '{}' must have either static_value or command",
                    self.name, inv.id
                )));
            }
            if inv.static_value.is_some() && inv.command.is_some() {
                return Err(crate::ValidationError(format!(
                    "custom-service '{}': inventory/sample '{}' must have only one of static_value or command",
                    self.name, inv.id
                )));
            }
            if let Some(cmd) = &inv.command {
                if let Some(re) = &cmd.parse_regex {
                    regex::Regex::new(re).map_err(|e| {
                        crate::ValidationError(format!(
                            "custom-service '{}': inventory '{}' has invalid parse_regex: {e}",
                            self.name, inv.id
                        ))
                    })?;
                }
            }
        }
        for sec in &self.security {
            if sec.exec.is_none() && sec.file_check.is_none() {
                return Err(crate::ValidationError(format!(
                    "custom-service '{}': security '{}' must have either exec or file_check",
                    self.name, sec.id
                )));
            }
            if sec.exec.is_some() && sec.file_check.is_some() {
                return Err(crate::ValidationError(format!(
                    "custom-service '{}': security '{}' must have only one of exec or file_check",
                    self.name, sec.id
                )));
            }
            if let Some(fc) = &sec.file_check {
                if let Some(mode) = &fc.max_mode {
                    u32::from_str_radix(mode.trim_start_matches('0'), 8).map_err(|_| {
                        crate::ValidationError(format!(
                            "custom-service '{}': security '{}' has invalid max_mode '{}' (expected octal)",
                            self.name, sec.id, mode
                        ))
                    })?;
                }
            }
        }
        Ok(())
    }
}
