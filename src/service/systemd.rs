use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const SERVICE_NAME: &str = "mac-mgmt";
const SYSTEMD_UNIT: &str = "mac-mgmt.service";

fn unit_path() -> PathBuf {
    PathBuf::from("/etc/systemd/system").join(SYSTEMD_UNIT)
}

fn service_user() -> Result<String> {
    std::env::var("SUDO_USER").context("SUDO_USER not set — run with sudo")
}

fn unit_contents() -> Result<String> {
    let bin = std::env::current_exe().context("cannot determine binary path")?;
    let user = service_user()?;
    Ok(format!(
        r#"[Unit]
Description={SERVICE_NAME} daemon
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User={user}
ExecStart={bin} daemon
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
"#,
        bin = bin.display()
    ))
}

pub fn install() -> Result<()> {
    let path = unit_path();
    let contents = unit_contents()?;
    fs::write(&path, &contents).context("failed to write systemd unit")?;
    tracing::info!("wrote {}", path.display());

    let status = Command::new("systemctl")
        .arg("daemon-reload")
        .status()
        .context("failed to run systemctl daemon-reload")?;

    if !status.success() {
        anyhow::bail!("systemctl daemon-reload failed");
    }

    let status = Command::new("systemctl")
        .args(["enable", "--now", SERVICE_NAME])
        .status()
        .context("failed to run systemctl enable")?;

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
        let _ = Command::new("systemctl")
            .args(["disable", "--now", SERVICE_NAME])
            .status();

        fs::remove_file(&path).context("failed to remove systemd unit")?;

        let _ = Command::new("systemctl")
            .arg("daemon-reload")
            .status();

        tracing::info!("service uninstalled");
        println!("Service uninstalled");
    } else {
        println!("Service not installed");
    }

    Ok(())
}
