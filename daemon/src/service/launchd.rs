use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command;

const PLIST_LABEL: &str = "com.plan-ai.mac-mgmt";
const DOMAIN_TARGET: &str = "system";

fn service_target() -> String {
    format!("{DOMAIN_TARGET}/{PLIST_LABEL}")
}

fn plist_path() -> PathBuf {
    PathBuf::from("/Library/LaunchDaemons").join(format!("{PLIST_LABEL}.plist"))
}

fn current_username() -> Result<String> {
    let uid = unsafe { libc::getuid() };
    let pw = unsafe { libc::getpwuid(uid) };
    if pw.is_null() {
        anyhow::bail!("could not resolve uid {uid} to a username");
    }
    let name = unsafe { std::ffi::CStr::from_ptr((*pw).pw_name) };
    Ok(name.to_string_lossy().into_owned())
}

fn plist_contents() -> Result<String> {
    let bin = std::env::current_exe().context("cannot determine binary path")?;
    let username = current_username()?;
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{PLIST_LABEL}</string>
    <key>UserName</key>
    <string>{username}</string>
    <key>ProgramArguments</key>
    <array>
        <string>/bin/bash</string>
        <string>-l</string>
        <string>-c</string>
        <string>exec {bin} daemon</string>
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
    let path = plist_path();

    // Bootout any existing service first (ignore errors if not loaded)
    let _ = sudo(&["launchctl", "bootout", &service_target()]);

    let contents = plist_contents()?;

    // Write plist via sudo since /Library/LaunchDaemons requires root
    let status = Command::new("sudo")
        .args(["tee", &path.display().to_string()])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(contents.as_bytes())?;
            }
            child.wait()
        })
        .context("failed to write plist")?;

    if !status.success() {
        anyhow::bail!("failed to write plist (sudo tee failed)");
    }
    tracing::info!("wrote {}", path.display());

    let output = sudo(&["launchctl", "bootstrap", DOMAIN_TARGET, &path.display().to_string()])
        .context("failed to run launchctl bootstrap")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("launchctl bootstrap failed: {stderr}");
    }

    tracing::info!("service installed and loaded");
    println!("Service installed: {}", path.display());
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let path = plist_path();

    if path.exists() {
        let _ = sudo(&["launchctl", "bootout", &service_target()]);
        let rm = sudo(&["rm", &path.display().to_string()])
            .context("failed to remove plist")?;
        if !rm.status.success() {
            anyhow::bail!("failed to remove plist (sudo rm failed)");
        }
        tracing::info!("service uninstalled");
        println!("Service uninstalled");
    } else {
        println!("Service not installed");
    }

    Ok(())
}

pub fn start() -> Result<()> {
    let path = plist_path();

    if !path.exists() {
        anyhow::bail!("service not installed");
    }

    let output = sudo(&["launchctl", "bootstrap", DOMAIN_TARGET, &path.display().to_string()])
        .context("failed to run launchctl bootstrap")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("launchctl bootstrap failed: {stderr}");
    }

    tracing::info!("service started");
    println!("Service started");
    Ok(())
}

pub fn stop() -> Result<()> {
    let path = plist_path();

    if !path.exists() {
        anyhow::bail!("service not installed");
    }

    let output = sudo(&["launchctl", "bootout", &service_target()])
        .context("failed to run launchctl bootout")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("launchctl bootout failed: {stderr}");
    }

    tracing::info!("service stopped");
    println!("Service stopped");
    Ok(())
}

pub fn restart() -> Result<()> {
    let path = plist_path();

    if !path.exists() {
        anyhow::bail!("service not installed");
    }

    let _ = sudo(&["launchctl", "bootout", &service_target()]);

    let output = sudo(&["launchctl", "bootstrap", DOMAIN_TARGET, &path.display().to_string()])
        .context("failed to run launchctl bootstrap")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("launchctl bootstrap failed: {stderr}");
    }

    tracing::info!("service restarted");
    println!("Service restarted");
    Ok(())
}

fn sudo(args: &[&str]) -> Result<std::process::Output> {
    Command::new("sudo")
        .args(args)
        .output()
        .context("failed to run sudo")
}
