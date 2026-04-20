#[cfg(target_os = "macos")]
pub(crate) mod launchd;
#[cfg(not(target_os = "macos"))]
pub(crate) mod systemd;

use anyhow::Result;
use std::path::PathBuf;

/// Resolve the binary path to use in service unit files.
///
/// When the `self-update` feature is enabled, this first ensures the
/// binary is a symlink into the nix store (so future updates are atomic
/// symlink swaps that never hit "text file busy"). The returned path is
/// the stable symlink — not the resolved nix store path — so the unit
/// survives across updates.
///
/// Without `self-update`, falls back to `current_exe()`.
fn service_bin_path() -> Result<PathBuf> {
    #[cfg(feature = "self-update")]
    {
        match crate::self_update::ensure_symlink() {
            Ok(p) => return Ok(p),
            Err(e) => {
                tracing::warn!("ensure_symlink failed, falling back to current_exe: {e}");
            }
        }
    }
    std::env::current_exe().map_err(Into::into)
}

pub fn install() -> Result<()> {
    let bin = service_bin_path()?;

    #[cfg(target_os = "macos")]
    launchd::install(&bin)?;
    #[cfg(not(target_os = "macos"))]
    systemd::install(&bin)?;

    // Install the services supervisor alongside the daemon.
    if let Err(e) = install_services_manager_with_bin(&bin) {
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
    let bin = service_bin_path()?;
    install_services_manager_with_bin(&bin)
}

fn install_services_manager_with_bin(bin: &std::path::Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    return launchd::install_services_manager(bin);
    #[cfg(not(target_os = "macos"))]
    return systemd::install_services_manager(bin);
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
pub(crate) fn home_dir_for_user(username: &str) -> std::path::PathBuf {
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
