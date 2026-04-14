#[cfg(target_os = "macos")]
mod launchd;
#[cfg(not(target_os = "macos"))]
mod systemd;

use anyhow::Result;

pub fn install() -> Result<()> {
    #[cfg(target_os = "macos")]
    launchd::install()?;
    #[cfg(not(target_os = "macos"))]
    systemd::install()?;

    // Install the services supervisor alongside the daemon.
    if let Err(e) = install_services_manager() {
        tracing::warn!("failed to install services manager: {e}");
    }

    Ok(())
}

pub fn uninstall() -> Result<()> {
    // Tear down the managed-services supervisor alongside the daemon.
    if let Err(e) = uninstall_services_manager() {
        tracing::warn!("failed to uninstall services manager: {e}");
    }

    #[cfg(target_os = "macos")]
    return launchd::uninstall();
    #[cfg(not(target_os = "macos"))]
    return systemd::uninstall();
}

pub fn start() -> Result<()> {
    #[cfg(target_os = "macos")]
    return launchd::start();
    #[cfg(not(target_os = "macos"))]
    return systemd::start();
}

pub fn stop() -> Result<()> {
    #[cfg(target_os = "macos")]
    return launchd::stop();
    #[cfg(not(target_os = "macos"))]
    return systemd::stop();
}

pub fn restart() -> Result<()> {
    #[cfg(target_os = "macos")]
    return launchd::restart();
    #[cfg(not(target_os = "macos"))]
    return systemd::restart();
}

// ── Managed-services supervisor unit ────────────────────────────────

/// Install (or refresh) the OS unit that runs the services supervisor.
pub fn install_services_manager() -> Result<()> {
    #[cfg(target_os = "macos")]
    return launchd::install_services_manager();
    #[cfg(not(target_os = "macos"))]
    return systemd::install_services_manager();
}

/// Stop and remove the supervisor OS unit.
pub fn uninstall_services_manager() -> Result<()> {
    #[cfg(target_os = "macos")]
    return launchd::uninstall_services_manager();
    #[cfg(not(target_os = "macos"))]
    return systemd::uninstall_services_manager();
}

/// Look up a user's home directory from /etc/passwd via `getent passwd`.
/// Falls back to `/home/{username}` if getent is unavailable.
#[cfg(not(target_os = "macos"))]
fn home_dir_for_user(username: &str) -> std::path::PathBuf {
    if let Ok(output) = std::process::Command::new("getent")
        .args(["passwd", username])
        .output()
    {
        if output.status.success() {
            let line = String::from_utf8_lossy(&output.stdout);
            if let Some(home) = line.split(':').nth(5) {
                let home = home.trim();
                if !home.is_empty() {
                    return std::path::PathBuf::from(home);
                }
            }
        }
    }
    std::path::PathBuf::from(format!("/home/{username}"))
}
