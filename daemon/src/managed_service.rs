use anyhow::Result;

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

    /// Pre-spawn checks: stop stale instances, clean up locks, etc.
    /// Called once before the first `spawn` if the service is managed.
    fn preflight(&self) -> Result<()> {
        Ok(())
    }

    /// Ensure the service binary/package is installed.
    fn ensure_installed(&self) -> Result<()>;

    /// Ensure the service is configured (first-run setup, etc.).
    fn ensure_setup(&self) -> Result<()>;

    /// Start the service process.
    fn spawn(&self) -> Result<std::process::Child>;

    /// Check whether the service is healthy.
    fn check_health(&self) -> Result<bool>;

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
}
