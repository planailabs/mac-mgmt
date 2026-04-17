use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command;

use mac_mgmt_services::SpawnSpec;

/// Return the generated unit contents as a string (for hashing).
pub fn generate_unit_contents(name: &str, spec: &SpawnSpec) -> String {
    #[cfg(target_os = "macos")]
    {
        launchd_plist(name, spec).unwrap_or_default()
    }
    #[cfg(not(target_os = "macos"))]
    {
        systemd_unit(name, spec)
    }
}

/// Write the unit file to disk without starting/enabling the service.
pub fn write_unit(name: &str, spec: &SpawnSpec) -> Result<()> {
    #[cfg(target_os = "macos")]
    write_launchd(name, spec)?;
    #[cfg(not(target_os = "macos"))]
    write_systemd(name, spec)?;
    Ok(())
}

/// Enable + start a service that is not yet running.
pub fn enable_and_start(name: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    bootstrap_launchd(name)?;
    #[cfg(not(target_os = "macos"))]
    enable_start_systemd(name)?;
    Ok(())
}

/// Write unit + enable + start in one shot (first install).
pub fn create_and_enable(name: &str, spec: &SpawnSpec) -> Result<()> {
    write_unit(name, spec)?;
    enable_and_start(name)
}

/// Restart a service whose unit file has changed.
pub fn restart(name: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    restart_launchd(name)?;
    #[cfg(not(target_os = "macos"))]
    restart_systemd(name)?;
    Ok(())
}

pub fn stop_and_remove(name: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    remove_launchd(name)?;
    #[cfg(not(target_os = "macos"))]
    remove_systemd(name)?;
    Ok(())
}

pub fn is_active(name: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        launchd_is_loaded(name)
    }
    #[cfg(not(target_os = "macos"))]
    {
        systemd_is_active(name)
    }
}

// ── systemd ────────────────────────────────────────────────────────────

#[cfg(not(target_os = "macos"))]
fn unit_path(name: &str) -> PathBuf {
    PathBuf::from(format!("/etc/systemd/system/{name}.service"))
}

#[cfg(not(target_os = "macos"))]
fn systemd_unit(name: &str, spec: &SpawnSpec) -> String {
    let exec = if spec.args.is_empty() {
        spec.program.clone()
    } else {
        format!("{} {}", spec.program, spec.args.join(" "))
    };
    let env_lines: String = spec
        .env
        .iter()
        .map(|(k, v)| format!("Environment={k}={v}"))
        .collect::<Vec<_>>()
        .join("\n");
    let user = crate::service::systemd::service_user();
    let home = crate::service::home_dir_for_user(&user);
    format!(
        r#"[Unit]
Description={name} (mac-mgmt unmanaged)
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User={user}
Environment=HOME={home}
{env_lines}
ExecStart=/bin/bash -lc '{exec}'
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
"#,
        home = home.display()
    )
}

#[cfg(not(target_os = "macos"))]
fn write_systemd(name: &str, spec: &SpawnSpec) -> Result<()> {
    let path = unit_path(name);
    let contents = systemd_unit(name, spec);
    std::fs::write(&path, &contents)
        .with_context(|| format!("writing {}", path.display()))?;
    Command::new("systemctl")
        .args(["daemon-reload"])
        .status()
        .context("systemctl daemon-reload")?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn enable_start_systemd(name: &str) -> Result<()> {
    // enable is idempotent; start is a no-op if already running.
    Command::new("systemctl")
        .args(["enable", &format!("{name}.service")])
        .status()
        .context("systemctl enable")?;
    Command::new("systemctl")
        .args(["start", &format!("{name}.service")])
        .status()
        .context("systemctl start")?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn restart_systemd(name: &str) -> Result<()> {
    Command::new("systemctl")
        .args(["restart", &format!("{name}.service")])
        .status()
        .context("systemctl restart")?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn remove_systemd(name: &str) -> Result<()> {
    let path = unit_path(name);
    let _ = Command::new("systemctl")
        .args(["disable", "--now", &format!("{name}.service")])
        .status();
    if path.exists() {
        std::fs::remove_file(&path)?;
        let _ = Command::new("systemctl").args(["daemon-reload"]).status();
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn systemd_is_active(name: &str) -> bool {
    Command::new("systemctl")
        .args(["is-active", "--quiet", &format!("{name}.service")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// ── launchd ────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn plist_label(name: &str) -> String {
    format!("com.plan-ai.{name}")
}

#[cfg(target_os = "macos")]
fn plist_path(name: &str) -> PathBuf {
    PathBuf::from(format!(
        "/Library/LaunchDaemons/{}.plist",
        plist_label(name)
    ))
}

#[cfg(target_os = "macos")]
fn launchd_plist(name: &str, spec: &SpawnSpec) -> Result<String> {
    let label = plist_label(name);
    let exec = if spec.args.is_empty() {
        spec.program.clone()
    } else {
        format!("{} {}", spec.program, spec.args.join(" "))
    };
    let username = crate::service::launchd::current_username()
        .unwrap_or_else(|_| "root".into());
    let env_dict: String = spec
        .env
        .iter()
        .map(|(k, v)| {
            format!(
                "        <key>{k}</key>\n        <string>{v}</string>"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let env_section = if env_dict.is_empty() {
        String::new()
    } else {
        format!(
            "    <key>EnvironmentVariables</key>\n    <dict>\n{env_dict}\n    </dict>"
        )
    };
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>UserName</key>
    <string>{username}</string>
    <key>ProgramArguments</key>
    <array>
        <string>/bin/bash</string>
        <string>-l</string>
        <string>-c</string>
        <string>exec {exec}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/{label}.out.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/{label}.err.log</string>
{env_section}
</dict>
</plist>
"#
    ))
}

#[cfg(target_os = "macos")]
fn write_launchd(name: &str, spec: &SpawnSpec) -> Result<()> {
    let path = plist_path(name);
    let contents = launchd_plist(name, spec)?;
    std::fs::write(&path, &contents)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn bootstrap_launchd(name: &str) -> Result<()> {
    let label = plist_label(name);
    let path = plist_path(name);
    Command::new("launchctl")
        .args(["bootstrap", "system", path.to_str().unwrap_or("")])
        .status()
        .with_context(|| format!("launchctl bootstrap system/{label}"))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn restart_launchd(name: &str) -> Result<()> {
    let label = plist_label(name);
    let path = plist_path(name);
    let _ = Command::new("launchctl")
        .args(["bootout", "system", &format!("system/{label}")])
        .status();
    std::thread::sleep(std::time::Duration::from_secs(2));
    Command::new("launchctl")
        .args(["bootstrap", "system", path.to_str().unwrap_or("")])
        .status()
        .context("launchctl bootstrap")?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn remove_launchd(name: &str) -> Result<()> {
    let label = plist_label(name);
    let path = plist_path(name);
    let _ = Command::new("launchctl")
        .args(["bootout", "system", &format!("system/{label}")])
        .status();
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn launchd_is_loaded(name: &str) -> bool {
    let label = plist_label(name);
    Command::new("launchctl")
        .args(["print", &format!("system/{label}")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
