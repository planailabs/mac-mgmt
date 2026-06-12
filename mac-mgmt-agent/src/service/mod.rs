#[cfg(target_os = "macos")]
pub(crate) mod launchd;
#[cfg(not(target_os = "macos"))]
pub(crate) mod systemd;

use anyhow::Result;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Optional hook resolving the stable binary path to bake into service unit
/// files. The daemon registers its self-update `ensure_symlink` here (so the
/// unit points at a symlink that survives atomic update swaps); consumers
/// without a self-updater leave it unset and `current_exe()` is used.
static BIN_PATH_RESOLVER: OnceLock<fn() -> Result<PathBuf>> = OnceLock::new();

/// Register the binary-path resolver used by [`install`]. First call wins;
/// later calls are ignored.
pub fn set_bin_path_resolver(f: fn() -> Result<PathBuf>) {
    let _ = BIN_PATH_RESOLVER.set(f);
}

/// Resolve the binary path to use in service unit files: the registered
/// resolver (see [`set_bin_path_resolver`]) with `current_exe()` fallback.
fn service_bin_path() -> Result<PathBuf> {
    if let Some(resolver) = BIN_PATH_RESOLVER.get() {
        match resolver() {
            Ok(p) => return Ok(p),
            Err(e) => {
                tracing::warn!("bin path resolver failed, falling back to current_exe: {e}");
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
