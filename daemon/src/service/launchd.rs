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
    let contents = plist_contents()?;

    // If plist exists with identical contents, just ensure it's loaded.
    if path.exists() {
        if let Ok(output) = sudo(&["cat", &path.display().to_string()]) {
            if output.status.success() && String::from_utf8_lossy(&output.stdout) == contents {
                tracing::info!("plist unchanged, ensuring loaded");
                let _ = sudo(&["launchctl", "bootstrap", DOMAIN_TARGET, &path.display().to_string()]);
                println!("Service already installed: {}", path.display());
                return Ok(());
            }
        }
        // Contents changed — bootout before rewriting.
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

// ── Per-service user-level LaunchAgents ──────────────────────────────

const MANAGED_PLIST_PREFIX: &str = "com.plan-ai.mac-mgmt.";

fn current_uid() -> u32 {
    unsafe { libc::getuid() }
}

fn managed_plist_label(service_name: &str) -> String {
    format!("{MANAGED_PLIST_PREFIX}{service_name}")
}

fn managed_plist_path(service_name: &str) -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("Library/LaunchAgents")
        .join(format!("{}.plist", managed_plist_label(service_name)))
}

fn managed_gui_target(service_name: &str) -> String {
    format!("gui/{}/{}", current_uid(), managed_plist_label(service_name))
}

fn managed_gui_domain() -> String {
    format!("gui/{}", current_uid())
}

fn managed_plist_contents(service_name: &str) -> Result<String> {
    let bin = std::env::current_exe().context("cannot determine binary path")?;
    let label = managed_plist_label(service_name);
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>/bin/bash</string>
        <string>-l</string>
        <string>-c</string>
        <string>exec {bin} daemon-service-launch {service_name}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/{label}.out.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/{label}.err.log</string>
</dict>
</plist>
"#,
        bin = bin.display()
    ))
}

/// Check if a per-service LaunchAgent plist is installed.
pub fn is_managed_service_installed(service_name: &str) -> bool {
    managed_plist_path(service_name).exists()
}

pub fn install_managed_service(service_name: &str) -> Result<()> {
    let path = managed_plist_path(service_name);
    let domain = managed_gui_domain();
    let contents = managed_plist_contents(service_name)?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create {}", parent.display()))?;
    }

    // If already installed with identical contents, just ensure it's running.
    if path.exists() {
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        if existing == contents {
            tracing::info!("managed service {service_name} plist unchanged, ensuring running");
            // bootstrap is a no-op if already loaded, but succeeds.
            let _ = Command::new("launchctl")
                .args(["bootstrap", &domain, &path.display().to_string()])
                .output();
            return Ok(());
        }
        // Contents changed — bootout the old one before rewriting.
        tracing::info!("managed service {service_name} plist changed, updating");
        let _ = Command::new("launchctl")
            .args(["bootout", &managed_gui_target(service_name)])
            .output();
    }

    std::fs::write(&path, &contents)
        .with_context(|| format!("write {}", path.display()))?;
    tracing::info!("wrote {}", path.display());

    let output = Command::new("launchctl")
        .args(["bootstrap", &domain, &path.display().to_string()])
        .output()
        .context("launchctl bootstrap")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("launchctl bootstrap failed: {stderr}");
    }

    tracing::info!("managed service {service_name} installed and loaded");
    Ok(())
}

pub fn uninstall_managed_service(service_name: &str) -> Result<()> {
    let path = managed_plist_path(service_name);

    let _ = Command::new("launchctl")
        .args(["bootout", &managed_gui_target(service_name)])
        .output();

    if path.exists() {
        std::fs::remove_file(&path)
            .with_context(|| format!("remove {}", path.display()))?;
        tracing::info!("removed {}", path.display());
    }

    // Also clean up the socket file.
    let sock = crate::service_ipc::socket_path(service_name);
    std::fs::remove_file(&sock).ok();

    Ok(())
}

pub fn start_managed_service(service_name: &str) -> Result<()> {
    let path = managed_plist_path(service_name);
    if !path.exists() {
        anyhow::bail!("managed service {service_name} not installed");
    }
    let domain = managed_gui_domain();
    let output = Command::new("launchctl")
        .args(["bootstrap", &domain, &path.display().to_string()])
        .output()
        .context("launchctl bootstrap")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("launchctl bootstrap failed: {stderr}");
    }
    Ok(())
}

pub fn stop_managed_service(service_name: &str) -> Result<()> {
    let _ = Command::new("launchctl")
        .args(["bootout", &managed_gui_target(service_name)])
        .output();
    Ok(())
}

/// List service names that have installed per-service LaunchAgent plists.
pub fn list_managed_service_units() -> Result<Vec<String>> {
    let agents_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("Library/LaunchAgents");

    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&agents_dir) {
        for entry in entries.flatten() {
            let fname = entry.file_name();
            let fname = fname.to_string_lossy();
            if let Some(rest) = fname.strip_prefix(MANAGED_PLIST_PREFIX) {
                if let Some(name) = rest.strip_suffix(".plist") {
                    names.push(name.to_string());
                }
            }
        }
    }
    Ok(names)
}
