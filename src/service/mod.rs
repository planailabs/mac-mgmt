mod launchd;
mod systemd;

use anyhow::Result;

pub fn install() -> Result<()> {
    if cfg!(target_os = "macos") {
        launchd::install()
    } else {
        systemd::install()
    }
}

pub fn uninstall() -> Result<()> {
    if cfg!(target_os = "macos") {
        launchd::uninstall()
    } else {
        systemd::uninstall()
    }
}

pub fn restart() -> Result<()> {
    if cfg!(target_os = "macos") {
        launchd::restart()
    } else {
        systemd::restart()
    }
}

/// Look up a user's home directory from /etc/passwd via `getent passwd`.
/// Falls back to `/home/{username}` if getent is unavailable.
#[cfg(target_os = "linux")]
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