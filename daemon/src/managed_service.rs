use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

pub use mac_mgmt_services::SpawnSpec;

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
        /// Per-file validation rules. After a write, the first validator whose
        /// glob matches the written filename is run. If it exits non-zero the
        /// write is rolled back. `{}` in command args is replaced with the
        /// file's absolute path; if no `{}` is present the command runs as-is.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        validators: Vec<FileValidator>,
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

/// A glob → command pair: after writing a file whose name matches `glob`,
/// the command is executed. Non-zero exit rolls back the write.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileValidator {
    /// Glob pattern to match filenames (e.g. `"*.json"`, `"openclaw.json"`, `"*"`).
    pub glob: String,
    /// Command + args. `{}` in any arg is replaced with the written file's
    /// absolute path.
    pub command: Vec<String>,
}

/// Whether the daemon should spawn and manage a long-running process,
/// or only install the package (no child process).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceMode {
    /// Full lifecycle: install → setup → spawn → health-check → upgrade/restart.
    Managed,
    /// Install and upgrade only; no process to spawn or monitor.
    InstallOnly,
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
        Box::pin(std::future::ready(
            tokio::task::block_in_place(|| self.check_health()),
        ))
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

    /// Return Prometheus metric collectors owned by this service.
    /// Called once during init; the daemon registers them with its Registry.
    /// Services should create and store their metrics in their struct and
    /// update them in `collect_metrics()`.
    fn metric_collectors(&self) -> Vec<Box<dyn prometheus::core::Collector>> {
        Vec::new()
    }

    /// Update custom metrics from the running service. Called on each
    /// health tick. Services should update the metrics they registered
    /// via `metric_collectors()`.
    fn collect_metrics(&self) {}

    /// Return the TCP tunnels this service exposes for browser proxying
    /// through the relay. Override to advertise one or more tunnels.
    fn expose_tunnels(&self) -> Vec<TunnelDef> {
        Vec::new()
    }

    /// Return the files/directories this service exposes for remote editing
    /// through the relay. Override to advertise config files.
    fn expose_files(&self) -> Vec<FileTunnelDef> {
        Vec::new()
    }
}
