use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const PLIST_LABEL: &str = "com.plan-ai.mac-mgmt";

fn domain_target() -> String {
    let uid = unsafe { libc::getuid() };
    format!("gui/{uid}")
}

fn service_target() -> String {
    format!("{}/{PLIST_LABEL}", domain_target())
}

fn plist_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    Ok(PathBuf::from(home)
        .join("Library/LaunchAgents")
        .join(format!("{PLIST_LABEL}.plist")))
}

fn plist_contents() -> Result<String> {
    let bin = std::env::current_exe().context("cannot determine binary path")?;
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{PLIST_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{bin}</string>
        <string>daemon</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/{PLIST_LABEL}.out.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/{PLIST_LABEL}.err.log</string>
</dict>
</plist>
"#,
        bin = bin.display()
    ))
}

pub fn install() -> Result<()> {
    let path = plist_path()?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("failed to create LaunchAgents directory")?;
    }

    // Bootout any existing service first (ignore errors if not loaded)
    let _ = Command::new("launchctl")
        .args(["bootout", &service_target()])
        .output();

    let contents = plist_contents()?;
    fs::write(&path, &contents).context("failed to write plist")?;
    tracing::info!("wrote {}", path.display());

    if let Err(e) = load_service(&path) {
        tracing::warn!("could not load service immediately: {e:#}");
        println!(
            "Service installed: {}\n\
             Note: could not load immediately (will start on next login)",
            path.display()
        );
    } else {
        tracing::info!("service installed and loaded");
        println!("Service installed: {}", path.display());
    }

    Ok(())
}

/// Try to load the service using bootstrap (modern) then load (legacy).
fn load_service(path: &PathBuf) -> Result<()> {
    // Try modern API first
    let output = Command::new("launchctl")
        .args(["bootstrap", &domain_target()])
        .arg(path)
        .output()
        .context("failed to run launchctl")?;

    if output.status.success() {
        return Ok(());
    }

    // Fall back to legacy API
    let output = Command::new("launchctl")
        .args(["load", "-w"])
        .arg(path)
        .output()
        .context("failed to run launchctl")?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    anyhow::bail!("launchctl load failed: {stderr}");
}

pub fn uninstall() -> Result<()> {
    let path = plist_path()?;

    if path.exists() {
        let _ = Command::new("launchctl")
            .args(["bootout", &service_target()])
            .output();

        fs::remove_file(&path).context("failed to remove plist")?;
        tracing::info!("service uninstalled");
        println!("Service uninstalled");
    } else {
        println!("Service not installed");
    }

    Ok(())
}

pub fn restart() -> Result<()> {
    let path = plist_path()?;

    if !path.exists() {
        anyhow::bail!("service not installed");
    }

    let _ = Command::new("launchctl")
        .args(["bootout", &service_target()])
        .output();

    if let Err(e) = load_service(&path) {
        tracing::warn!("could not reload service: {e:#}");
        println!("Service stopped but could not reload (will start on next login)");
    } else {
        tracing::info!("service restarted");
        println!("Service restarted");
    }

    Ok(())
}
