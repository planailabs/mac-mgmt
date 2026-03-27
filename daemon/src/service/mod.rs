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
    #[cfg(target_os = "macos")]
    return launchd::uninstall();
    #[cfg(not(target_os = "macos"))]
    return systemd::uninstall();
}

pub fn restart() -> Result<()> {
    #[cfg(target_os = "macos")]
    return launchd::restart();
    #[cfg(not(target_os = "macos"))]
    return systemd::restart();
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
