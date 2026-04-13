use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

use crate::service_ipc::protocol::SpawnSpec;

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
pub trait ManagedService {
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

    /// Return the command spec for spawning this service. Used by the
    /// external process wrapper which doesn't have access to ManagedService.
    fn spawn_spec(&self) -> SpawnSpec;

    /// Start the service process.
    fn spawn(&self) -> Result<std::process::Child>;

    /// Check whether the service is healthy (blocking).
    /// Used by the external-process wrapper and as the default for
    /// `check_health_async`. Services with HTTP-based checks should
    /// override `check_health_async` instead.
    fn check_health(&self) -> Result<bool>;

    /// Non-blocking health check. Defaults to running `check_health()` via
    /// `block_in_place` so it doesn't stall the tokio runtime.
    /// Override for truly async checks (e.g. reqwest HTTP).
    fn check_health_async(&self) -> Pin<Box<dyn Future<Output = Result<bool>> + '_>> {
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
}
