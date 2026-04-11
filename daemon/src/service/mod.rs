#[cfg(target_os = "macos")]
mod launchd;
#[cfg(not(target_os = "macos"))]
mod systemd;

use anyhow::Result;

pub fn install() -> Result<()> {
    #[cfg(target_os = "macos")]
    return launchd::install();
    #[cfg(not(target_os = "macos"))]
    return systemd::install();
}

pub fn uninstall() -> Result<()> {
    // Clean up any per-service units first.
    if let Ok(services) = list_managed_service_units() {
        for name in &services {
            if let Err(e) = cleanup_managed_service(name) {
                tracing::warn!("failed to clean up managed service {name}: {e}");
            }
        }
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

// ── Per-service managed unit operations ──────────────────────────────

/// Check if a per-service system unit is already installed.
#[allow(dead_code)]
pub fn is_managed_service_installed(name: &str) -> bool {
    #[cfg(target_os = "macos")]
    return launchd::is_managed_service_installed(name);
    #[cfg(not(target_os = "macos"))]
    return systemd::is_managed_service_installed(name);
}

/// Install and start a per-service system unit (user-level).
/// If already installed with identical config, just ensures it's running.
pub fn install_managed_service(name: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    return launchd::install_managed_service(name);
    #[cfg(not(target_os = "macos"))]
    return systemd::install_managed_service(name);
}

/// Stop and remove a per-service system unit.
pub fn uninstall_managed_service(name: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    return launchd::uninstall_managed_service(name);
    #[cfg(not(target_os = "macos"))]
    return systemd::uninstall_managed_service(name);
}

/// Start an existing per-service system unit.
pub fn start_managed_service(name: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    return launchd::start_managed_service(name);
    #[cfg(not(target_os = "macos"))]
    return systemd::start_managed_service(name);
}

/// Stop a per-service system unit (without removing it).
#[allow(dead_code)]
pub fn stop_managed_service(name: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    return launchd::stop_managed_service(name);
    #[cfg(not(target_os = "macos"))]
    return systemd::stop_managed_service(name);
}

/// List service names that have installed per-service system units.
pub fn list_managed_service_units() -> Result<Vec<String>> {
    #[cfg(target_os = "macos")]
    return launchd::list_managed_service_units();
    #[cfg(not(target_os = "macos"))]
    return systemd::list_managed_service_units();
}

/// Full cleanup: gracefully stop, remove unit file, remove socket.
pub fn cleanup_managed_service(name: &str) -> Result<()> {
    // Try to send a shutdown command via the socket first.
    let sock = crate::service_ipc::socket_path(name);
    if sock.exists() {
        // Best-effort: connect and send shutdown. If it fails, we'll
        // just stop the unit directly.
        let _ = std::os::unix::net::UnixStream::connect(&sock).and_then(|mut s| {
            use std::io::Write;
            let msg = r#"{"kind":"request","type":"shutdown"}"#;
            writeln!(s, "{msg}")?;
            // Give it a moment to process.
            std::thread::sleep(std::time::Duration::from_millis(500));
            Ok(())
        });
    }

    uninstall_managed_service(name)?;
    Ok(())
}

/// Look up a user's home directory from /etc/passwd via `getent passwd`.
/// Falls back to `/home/{username}` if getent is unavailable.
#[cfg(not(target_os = "macos"))]
fn home_dir_for_user(username: &str) -> std::path::PathBuf {
    // Try getent passwd which works with NSS (LDAP, NIS, etc.)
    if let Ok(output) = std::process::Command::new("getent")
        .args(["passwd", username])
        .output()
    {
        if output.status.success() {
            let line = String::from_utf8_lossy(&output.stdout);
            // Format: username:x:uid:gid:gecos:home:shell
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
