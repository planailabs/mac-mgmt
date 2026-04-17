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

pub(crate) fn current_username() -> Result<String> {
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
    let contents = plist_contents()?;

    if path.exists() {
        if let Ok(output) = sudo(&["cat", &path.display().to_string()]) {
            if output.status.success() && String::from_utf8_lossy(&output.stdout) == contents {
                tracing::info!("plist unchanged, ensuring loaded");
                let _ = sudo(&["launchctl", "bootstrap", DOMAIN_TARGET, &path.display().to_string()]);
                println!("Service already installed: {}", path.display());
                return Ok(());
            }
        }
        tracing::info!("plist changed, updating");
        let _ = sudo(&["launchctl", "bootout", &service_target()]);
    }

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

    // launchd needs a moment to fully tear down the previous instance
    // before bootstrap will succeed cleanly.
    std::thread::sleep(std::time::Duration::from_secs(10));

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

// ── Managed-services supervisor (system-level LaunchDaemon) ─────────

const SUPERVISOR_LABEL: &str = "com.plan-ai.mac-mgmt.services";
const LEGACY_MANAGED_PREFIX: &str = "com.plan-ai.mac-mgmt.";

fn supervisor_plist_path() -> PathBuf {
    PathBuf::from("/Library/LaunchDaemons").join(format!("{SUPERVISOR_LABEL}.plist"))
}

fn supervisor_system_target() -> String {
    format!("system/{SUPERVISOR_LABEL}")
}

fn supervisor_plist_contents() -> Result<String> {
    let bin = std::env::current_exe().context("cannot determine binary path")?;
    let username = current_username()?;
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{SUPERVISOR_LABEL}</string>
    <key>UserName</key>
    <string>{username}</string>
    <key>ProgramArguments</key>
    <array>
        <string>/bin/bash</string>
        <string>-l</string>
        <string>-c</string>
        <string>exec {bin} services</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/{SUPERVISOR_LABEL}.out.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/{SUPERVISOR_LABEL}.err.log</string>
</dict>
</plist>
"#,
        bin = bin.display()
    ))
}

pub fn install_services_manager() -> Result<()> {
    cleanup_legacy_user_agents();

    let path = supervisor_plist_path();
    let contents = supervisor_plist_contents()?;

    let changed = match sudo(&["cat", &path.display().to_string()]) {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout) != contents
        }
        _ => true,
    };

    if changed {
        if path.exists() {
            let _ = sudo(&["launchctl", "bootout", &supervisor_system_target()]);
        }
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
            .context("failed to write supervisor plist")?;
        if !status.success() {
            anyhow::bail!("sudo tee {} failed", path.display());
        }
        tracing::info!("wrote {}", path.display());
    }

    let output = sudo(&[
        "launchctl",
        "bootstrap",
        "system",
        &path.display().to_string(),
    ])
    .context("launchctl bootstrap")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Already-loaded is fine; surface anything else.
        if !stderr.contains("already") {
            anyhow::bail!("launchctl bootstrap failed: {stderr}");
        }
    }

    Ok(())
}

pub fn uninstall_services_manager() -> Result<()> {
    cleanup_legacy_user_agents();

    let path = supervisor_plist_path();
    let _ = sudo(&["launchctl", "bootout", &supervisor_system_target()]);
    if path.exists() {
        let _ = sudo(&["rm", &path.display().to_string()]);
    }
    Ok(())
}

/// Migration: tear down any user-level LaunchAgents from older versions
/// (the per-service `com.plan-ai.mac-mgmt.<name>.plist` files and the
/// original user-level supervisor agent).
fn cleanup_legacy_user_agents() {
    let Some(home) = dirs::home_dir() else { return };
    let agents = home.join("Library/LaunchAgents");
    let uid = unsafe { libc::getuid() };
    let Ok(entries) = std::fs::read_dir(&agents) else {
        return;
    };
    for entry in entries.flatten() {
        let fname = entry.file_name();
        let name = fname.to_string_lossy();
        let Some(label_plist) = name.strip_suffix(".plist") else {
            continue;
        };
        if !label_plist.starts_with(LEGACY_MANAGED_PREFIX) {
            continue;
        }
        tracing::info!("removing legacy user agent {name}");
        let _ = Command::new("launchctl")
            .args(["bootout", &format!("gui/{uid}/{label_plist}")])
            .output();
        let _ = std::fs::remove_file(entry.path());
    }
}
