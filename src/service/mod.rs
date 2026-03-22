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
