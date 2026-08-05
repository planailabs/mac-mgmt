use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

pub use mac_mgmt_services::SpawnSpec;

/// A named path that a service owns — config files, data dirs, model caches.
/// Used for backup path collection and unmanaged service presence detection.
#[derive(Debug, Clone)]
pub struct DataPath {
    /// Short label (e.g. "config", "data", "models", "cache").
    pub name: &'static str,
    /// Absolute path on disk.
    pub path: PathBuf,
    /// Whether this path should be included in backups.
    /// Large model caches (e.g. ollama models) default to false.
    pub backup: bool,
}

/// A TCP tunnel that a managed service exposes for proxying through the relay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelDef {
    /// Short, URL-safe name (e.g. "ollama", "openclaw").
    pub name: String,
    /// Host the service listens on (e.g. "127.0.0.1").
    pub host: String,
    /// TCP port the service listens on.
    pub tcp_port: u16,
}

// ── File tunnels ────────────────────────────────────────────────────────

/// A file or directory that a managed service exposes for remote editing
/// through the relay. This is what the service defines.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FileTunnelDef {
    File {
        /// Short, URL-safe label (e.g. "ollama-env", "openclaw-config").
        name: String,
        /// Absolute path to the file on disk.
        path: String,
        /// Whether writes are allowed (`false` = read-only).
        writable: bool,
        /// Human-readable description for the UI.
        description: String,
    },
    Folder {
        /// Short, URL-safe label (e.g. "ollama-env", "openclaw-config").
        name: String,
        /// Absolute path to the directory root on disk.
        path: String,
        /// Whether writes are allowed (`false` = read-only).
        writable: bool,
        /// Glob patterns for files that are allowed to be written.
        /// If empty, all files/folders are writable (if `writable` is true).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        allow_write: Vec<String>,
        /// Only expose files matching these globs.
        /// `None` = all files. Examples: `["*.json", "*.toml"]`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        include: Option<Vec<String>>,
        /// Per-file validators. The first [`Validator`] whose pattern matches
        /// the written filename is run.  Validation failure rolls back the
        /// write.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        validators: Vec<Validator>,
        /// Human-readable description for the UI.
        description: String,
    },
}

impl FileTunnelDef {
    pub fn name(&self) -> &str {
        match self {
            FileTunnelDef::File { name, .. } => name,
            FileTunnelDef::Folder { name, .. } => name,
        }
    }

    pub fn path(&self) -> &str {
        match self {
            FileTunnelDef::File { path, .. } => path,
            FileTunnelDef::Folder { path, .. } => path,
        }
    }

    pub fn writable(&self) -> bool {
        match self {
            FileTunnelDef::File { writable, .. } => *writable,
            FileTunnelDef::Folder { writable, .. } => *writable,
        }
    }

    pub fn description(&self) -> &str {
        match self {
            FileTunnelDef::File { description, .. } => description,
            FileTunnelDef::Folder { description, .. } => description,
        }
    }
}

/// A complete file tunnel definition, including the owning service and kind.
/// This is what the daemon uses internally and advertises to the relay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTunnel {
    #[serde(flatten)]
    pub def: FileTunnelDef,
    /// The owning service name.
    pub service: String,
}

impl std::ops::Deref for FileTunnel {
    type Target = FileTunnelDef;
    fn deref(&self) -> &Self::Target {
        &self.def
    }
}

// Re-export for backwards compatibility and convenience.
pub use crate::validator::Validator;

// ── Shell tunnels ──────────────────────────────────────────────────────

/// A predefined shell command that a managed service exposes for remote
/// execution through the relay. Only registered commands can be run —
/// no arbitrary shell access.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShellCommandDef {
    /// Short, URL-safe identifier (e.g. "ollama-list", "nvidia-smi-query").
    pub name: String,
    /// The program to execute (e.g. "ollama", "nvidia-smi").
    pub command: String,
    /// Fixed arguments always passed to the command (e.g. `["list"]`).
    pub args: Vec<String>,
    /// Human-readable description for the UI.
    pub description: String,
    /// If set, the command accepts one user-provided argument appended
    /// after `args`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arg_template: Option<ShellArgTemplate>,
    /// Custom timeout in seconds. Defaults to 300 (5 min) if unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

/// Template for a user-provided argument on a shell command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellArgTemplate {
    /// Label shown in the UI (e.g. "Model name").
    pub label: String,
    /// Placeholder text for the input field.
    pub placeholder: String,
    /// Optional regex the argument must match (validated daemon-side).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<String>,
}

/// A shell command definition with its owning service name.
/// This is what the daemon uses internally and advertises to the relay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellTunnel {
    #[serde(flatten)]
    pub def: ShellCommandDef,
    /// The owning service name.
    pub service: String,
}

/// Whether the daemon should spawn and manage a long-running process,
/// or only install the package (no child process).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceMode {
    /// Full lifecycle: install → setup → spawn → health-check → upgrade/restart.
    Managed,
    /// Install and upgrade only; no process to spawn or monitor.
    InstallOnly,
    /// Process is run by the daemon itself (e.g. embedded HTTP server).
    /// Participates in health checks and tunnel exposure but is not
    /// registered with the services supervisor.
    Integrated,
}

/// A service that the daemon manages: installs, spawns, monitors, and upgrades.
pub trait ManagedService: Send + Sync {
    /// Human-readable name for logging.
    fn name(&self) -> &str;

    /// Whether this service is fully managed (spawned) or install-only.
    fn service_mode(&self) -> ServiceMode {
        ServiceMode::Managed
    }

    /// The binary name this service spawns (e.g., "ollama", "openclaw").
    /// Used to detect store path drift between the running binary and
    /// the nix profile. Defaults to `name()`.
    fn binary_name(&self) -> &str {
        self.name()
    }

    /// Pre-spawn checks: stop stale instances, clean up locks, etc.
    /// Called once before the first `spawn` if the service is managed.
    fn preflight(&self) -> Result<()> {
        Ok(())
    }

    /// Ensure the service binary/package is installed.
    fn ensure_installed(&self) -> Result<()>;

    /// Ensure the service is configured (first-run setup, etc.).
    fn ensure_setup(&self) -> Result<()>;

    /// Return the command spec for spawning this service. The daemon hands
    /// this off to the services supervisor over RPC.
    fn spawn_spec(&self) -> SpawnSpec;

    /// Check whether the service is healthy (blocking).
    /// Used as the default implementation for `check_health_async`. Services
    /// with HTTP-based checks should override `check_health_async` instead.
    fn check_health(&self) -> Result<bool>;

    /// Non-blocking health check. Defaults to running `check_health()` via
    /// `block_in_place` so it doesn't stall the tokio runtime.
    /// Override for truly async checks (e.g. reqwest HTTP).
    fn check_health_async(&self) -> Pin<Box<dyn Future<Output = Result<bool>> + Send + '_>> {
        Box::pin(std::future::ready(tokio::task::block_in_place(|| {
            self.check_health()
        })))
    }

    /// Attempt to auto-repair an unhealthy service.
    fn repair(&self) -> Result<()>;

    /// Check if an upgrade is available and install it (but don't restart yet).
    /// Returns true if an upgrade was installed and a restart is pending.
    fn check_and_upgrade(&self) -> Result<bool>;

    /// Called once after the service has been spawned and is healthy.
    /// Use for one-time setup that requires the service to be running.
    fn post_start(&self) -> Result<()> {
        Ok(())
    }

    /// Persist connector-provided env vars somewhere companion processes
    /// can pick them up (e.g. hermes mirrors them into ~/.hermes/.env so
    /// hermes-dashboard and hermes-webui see the same env as the gateway).
    /// Called after connector env collection. Default: no-op.
    fn persist_connector_env(&self, _env: &std::collections::HashMap<String, String>) {}

    /// Check if the service is currently busy (serving requests, running jobs).
    /// Used to defer restarts.
    fn is_busy(&self) -> Result<bool> {
        Ok(false)
    }

    /// Re-apply configuration to a running service. Called when the daemon
    /// config changes and the service needs to pick up new settings.
    /// The default implementation is a no-op.
    fn configure(&self) -> Result<()> {
        Ok(())
    }

    /// Whether the service can pick up config changes without a restart.
    /// If true, `schedule_restart` will call `configure()` instead of
    /// killing and respawning the process.
    fn supports_hot_reload(&self) -> bool {
        false
    }

    /// Check if the service needs a restart due to external changes
    /// (e.g. env file updates). Called on each health tick.
    fn needs_restart(&self) -> bool {
        false
    }

    /// Register the OpenTelemetry instruments this service reports. Called
    /// once during init, after the meter provider is installed. Services keep
    /// the observed value in their own struct (an atomic, so the observable
    /// callback can read it) and refresh it in `collect_metrics`; see
    /// [`crate::metrics::observable_gauge`].
    fn register_metrics(&self) {}

    /// Update custom metrics from the running service. Called on each
    /// health tick. Services should update the values behind the instruments
    /// they registered in `register_metrics()`.
    fn collect_metrics(&self) {}

    /// Return the TCP tunnels this service exposes for browser proxying
    /// through the relay. Override to advertise one or more tunnels.
    fn expose_tunnels(&self) -> Vec<TunnelDef> {
        Vec::new()
    }

    /// Return overrides for this service's tunnels. Keyed by tunnel name
    /// (must match a name from `expose_tunnels()`). When a request path
    /// matches an override's regex, the override function is called; if it
    /// returns `Some(path)` the daemon sends a 302 redirect instead of
    /// proxying.
    fn tunnel_overrides(
        &self,
    ) -> std::collections::HashMap<String, Vec<crate::p2p::proxy_helpers::TunnelOverride>> {
        std::collections::HashMap::new()
    }

    /// Return the files/directories this service exposes for remote editing
    /// through the relay. Override to advertise config files.
    fn expose_files(&self) -> Vec<FileTunnelDef> {
        Vec::new()
    }

    /// Return predefined shell commands this service exposes for remote
    /// execution through the relay. Override to advertise commands.
    fn expose_shell_commands(&self) -> Vec<ShellCommandDef> {
        Vec::new()
    }

    /// Per-service static inventory (version, installed models, etc.).
    /// Called at the ~6h inventory cadence. Override to report facts.
    fn service_inventory(
        &self,
    ) -> Pin<Box<dyn Future<Output = Vec<mac_mgmt_common::InventoryEntry>> + Send + '_>> {
        Box::pin(std::future::ready(Vec::new()))
    }

    /// Per-service dynamic sample (loaded models, active sessions, etc.).
    /// Called every heartbeat (~1m). Override to report live state.
    fn service_sample(
        &self,
    ) -> Pin<Box<dyn Future<Output = Vec<mac_mgmt_common::InventoryEntry>> + Send + '_>> {
        Box::pin(std::future::ready(Vec::new()))
    }

    /// Per-service security findings (auth config, exposed APIs, etc.).
    /// Called at the ~6h inventory cadence. Override to report checks.
    fn service_security(
        &self,
    ) -> Pin<Box<dyn Future<Output = Vec<mac_mgmt_common::SecurityFinding>> + Send + '_>> {
        Box::pin(std::future::ready(Vec::new()))
    }

    /// Return the data paths this service owns — config files, data dirs,
    /// model caches. Used for backup path collection and unmanaged service
    /// presence detection. Paths with `backup: false` are excluded from
    /// backups unless `include_models` is set.
    fn data_paths(&self, _home: &std::path::Path) -> Vec<DataPath> {
        Vec::new()
    }
}
