use anyhow::Result;

/// A service that the daemon manages: installs, spawns, monitors, and upgrades.
pub trait ManagedService {
    /// Human-readable name for logging.
    fn name(&self) -> &str;

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

    /// Check if the service is currently busy (serving requests, running jobs).
    /// Used to defer restarts.
    fn is_busy(&self) -> Result<bool> {
        Ok(false)
    }
}
