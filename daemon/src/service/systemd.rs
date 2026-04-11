use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const SERVICE_NAME: &str = "mac-mgmt";
const SYSTEMD_UNIT: &str = "mac-mgmt.service";

fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

/// Run a command, prefixing with sudo if not root.
fn privileged(program: &str, args: &[&str]) -> Result<std::process::ExitStatus> {
    let status = if is_root() {
        Command::new(program)
            .args(args)
            .status()
            .with_context(|| format!("failed to run {program}"))?
    } else {
        Command::new("sudo")
            .arg(program)
            .args(args)
            .status()
            .with_context(|| format!("failed to run sudo {program}"))?
    };
    Ok(status)
}

fn unit_path() -> PathBuf {
    PathBuf::from("/etc/systemd/system").join(SYSTEMD_UNIT)
}

fn service_user() -> String {
    std::env::var("SUDO_USER")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "root".to_string())
}

fn unit_contents() -> Result<String> {
    let bin = std::env::current_exe().context("cannot determine binary path")?;
    let user = service_user();
    let home = super::home_dir_for_user(&user);
    Ok(format!(
        r#"[Unit]
Description={SERVICE_NAME} daemon
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User={user}
Environment=HOME={home}
ExecStart=/bin/bash -lc '{bin} daemon'
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
"#,
        bin = bin.display(),
        home = home.display()
    ))
}

pub fn install() -> Result<()> {
    let path = unit_path();
    let contents = unit_contents()?;

    // Write unit file via tee to handle permissions
    if is_root() {
        fs::write(&path, &contents).context("failed to write systemd unit")?;
    } else {
        use std::io::Write;
        let mut child = Command::new("sudo")
            .args(["tee", &path.to_string_lossy()])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .context("failed to run sudo tee")?;
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(contents.as_bytes())?;
        let status = child.wait()?;
        if !status.success() {
            anyhow::bail!("sudo tee failed");
        }
    }
    tracing::info!("wrote {}", path.display());

    let status = privileged("systemctl", &["daemon-reload"])?;
    if !status.success() {
        anyhow::bail!("systemctl daemon-reload failed");
    }

    let status = privileged("systemctl", &["enable", "--now", SERVICE_NAME])?;
    if !status.success() {
        anyhow::bail!("systemctl enable --now failed");
    }

    tracing::info!("service installed and started");
    println!("Service installed: {}", path.display());
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let path = unit_path();

    if path.exists() {
        let _ = privileged("systemctl", &["disable", "--now", SERVICE_NAME]);

        if is_root() {
            fs::remove_file(&path).context("failed to remove systemd unit")?;
        } else {
            let status = privileged("rm", &[&path.to_string_lossy()])?;
            if !status.success() {
                anyhow::bail!("failed to remove systemd unit");
            }
        }

        let _ = privileged("systemctl", &["daemon-reload"]);

        tracing::info!("service uninstalled");
        println!("Service uninstalled");
    } else {
        println!("Service not installed");
    }

    Ok(())
}

pub fn start() -> Result<()> {
    let status = privileged("systemctl", &["start", SERVICE_NAME])?;
    if !status.success() {
        anyhow::bail!("systemctl start failed");
    }

    tracing::info!("service started");
    println!("Service started");
    Ok(())
}

pub fn stop() -> Result<()> {
    let status = privileged("systemctl", &["stop", SERVICE_NAME])?;
    if !status.success() {
        anyhow::bail!("systemctl stop failed");
    }

    tracing::info!("service stopped");
    println!("Service stopped");
    Ok(())
}

pub fn restart() -> Result<()> {
    let status = privileged("systemctl", &["restart", SERVICE_NAME])?;
    if !status.success() {
        anyhow::bail!("systemctl restart failed");
    }

    tracing::info!("service restarted");
    println!("Service restarted");
    Ok(())
}

// ── Per-service user-level systemd template unit ─────────────────────

const TEMPLATE_UNIT: &str = "mac-mgmt-service@.service";
const TEMPLATE_INSTANCE_PREFIX: &str = "mac-mgmt-service@";

fn user_unit_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("systemd/user")
}

fn template_unit_path() -> PathBuf {
    user_unit_dir().join(TEMPLATE_UNIT)
}

fn instance_unit_name(service_name: &str) -> String {
    format!("{TEMPLATE_INSTANCE_PREFIX}{service_name}.service")
}

fn template_unit_contents() -> Result<String> {
    let bin = std::env::current_exe().context("cannot determine binary path")?;
    Ok(format!(
        r#"[Unit]
Description=mac-mgmt managed service %i
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
Environment=HOME=%h
ExecStart=/bin/bash -lc '{bin} daemon-service-launch %i'
Restart=always
RestartSec=5

[Install]
WantedBy=default.target
"#,
        bin = bin.display()
    ))
}

/// Ensure the systemd template unit file exists and is up to date.
fn ensure_template_unit() -> Result<()> {
    let path = template_unit_path();
    let contents = template_unit_contents()?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create {}", parent.display()))?;
    }

    // Only write if contents changed (avoid unnecessary daemon-reload).
    let needs_write = match std::fs::read_to_string(&path) {
        Ok(existing) => existing != contents,
        Err(_) => true,
    };

    if needs_write {
        std::fs::write(&path, &contents)
            .with_context(|| format!("write {}", path.display()))?;
        tracing::info!("wrote template unit {}", path.display());

        let status = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status()
            .context("systemctl --user daemon-reload")?;
        if !status.success() {
            tracing::warn!("systemctl --user daemon-reload failed");
        }
    }

    Ok(())
}

pub fn install_managed_service(service_name: &str) -> Result<()> {
    ensure_template_unit()?;

    let unit = instance_unit_name(service_name);
    let status = Command::new("systemctl")
        .args(["--user", "enable", "--now", &unit])
        .status()
        .with_context(|| format!("systemctl --user enable --now {unit}"))?;

    if !status.success() {
        anyhow::bail!("systemctl --user enable --now {unit} failed");
    }

    tracing::info!("managed service {service_name} enabled and started");
    Ok(())
}

pub fn uninstall_managed_service(service_name: &str) -> Result<()> {
    let unit = instance_unit_name(service_name);

    let _ = Command::new("systemctl")
        .args(["--user", "disable", "--now", &unit])
        .status();

    // Clean up socket file.
    let sock = crate::service_ipc::socket_path(service_name);
    std::fs::remove_file(&sock).ok();

    tracing::info!("managed service {service_name} disabled and stopped");
    Ok(())
}

pub fn start_managed_service(service_name: &str) -> Result<()> {
    ensure_template_unit()?;

    let unit = instance_unit_name(service_name);
    let status = Command::new("systemctl")
        .args(["--user", "start", &unit])
        .status()
        .with_context(|| format!("systemctl --user start {unit}"))?;

    if !status.success() {
        anyhow::bail!("systemctl --user start {unit} failed");
    }
    Ok(())
}

pub fn stop_managed_service(service_name: &str) -> Result<()> {
    let unit = instance_unit_name(service_name);
    let _ = Command::new("systemctl")
        .args(["--user", "stop", &unit])
        .status();
    Ok(())
}

/// List service names that have enabled per-service systemd user units.
pub fn list_managed_service_units() -> Result<Vec<String>> {
    let output = Command::new("systemctl")
        .args([
            "--user",
            "list-units",
            &format!("{TEMPLATE_INSTANCE_PREFIX}*"),
            "--no-legend",
            "--plain",
            "--no-pager",
        ])
        .output()
        .context("systemctl --user list-units")?;

    let mut names = Vec::new();
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let unit = line.split_whitespace().next().unwrap_or("");
            if let Some(rest) = unit.strip_prefix(TEMPLATE_INSTANCE_PREFIX) {
                if let Some(name) = rest.strip_suffix(".service") {
                    names.push(name.to_string());
                }
            }
        }
    }
    Ok(names)
}
