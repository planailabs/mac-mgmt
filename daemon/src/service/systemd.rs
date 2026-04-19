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

pub(crate) fn service_user() -> String {
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

// ── Managed-services supervisor (system-level systemd unit) ──────────

const SUPERVISOR_NAME: &str = "mac-mgmt-services";
const SUPERVISOR_UNIT: &str = "mac-mgmt-services.service";

fn supervisor_unit_path() -> PathBuf {
    PathBuf::from("/etc/systemd/system").join(SUPERVISOR_UNIT)
}

fn supervisor_unit_contents() -> Result<String> {
    let bin = std::env::current_exe().context("cannot determine binary path")?;
    let user = service_user();
    let home = super::home_dir_for_user(&user);
    Ok(format!(
        r#"[Unit]
Description=mac-mgmt managed-services supervisor
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User={user}
Environment=HOME={home}
ExecStart=/bin/bash -lc '{bin} services'
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
"#,
        bin = bin.display(),
        home = home.display()
    ))
}

/// Write `contents` to `path` as root, using `sudo tee` when needed.
fn write_privileged(path: &std::path::Path, contents: &str) -> Result<()> {
    if is_root() {
        fs::write(path, contents).with_context(|| format!("write {}", path.display()))?;
        return Ok(());
    }
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
        anyhow::bail!("sudo tee {} failed", path.display());
    }
    Ok(())
}

pub fn install_services_manager() -> Result<()> {
    cleanup_legacy_user_units();

    let path = supervisor_unit_path();
    let contents = supervisor_unit_contents()?;

    let needs_write = match fs::read_to_string(&path) {
        Ok(existing) => existing != contents,
        Err(_) => true,
    };

    if needs_write {
        write_privileged(&path, &contents)?;
        tracing::info!("wrote supervisor unit {}", path.display());

        let status = privileged("systemctl", &["daemon-reload"])?;
        if !status.success() {
            tracing::warn!("systemctl daemon-reload failed");
        }
    }

    let status = privileged("systemctl", &["enable", "--now", SUPERVISOR_NAME])?;
    if !status.success() {
        anyhow::bail!("systemctl enable --now {SUPERVISOR_NAME} failed");
    }

    if needs_write {
        let status = privileged("systemctl", &["restart", SUPERVISOR_NAME])?;
        if !status.success() {
            anyhow::bail!("systemctl restart {SUPERVISOR_NAME} failed");
        }
    }

    tracing::info!("supervisor unit installed and running");
    Ok(())
}

pub fn uninstall_services_manager() -> Result<()> {
    cleanup_legacy_user_units();

    let path = supervisor_unit_path();
    if path.exists() {
        let _ = privileged("systemctl", &["disable", "--now", SUPERVISOR_NAME]);
        if is_root() {
            let _ = fs::remove_file(&path);
        } else {
            let _ = privileged("rm", &[&path.to_string_lossy()]);
        }
        let _ = privileged("systemctl", &["daemon-reload"]);
    }
    Ok(())
}

/// Migration: remove any user-level mac-mgmt units left behind by older
/// versions of the daemon (the per-service `mac-mgmt-service@*.service`
/// template and the initial user-level `mac-mgmt-services.service`).
fn cleanup_legacy_user_units() {
    let user_unit_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("systemd/user");

    let mut had_any = false;
    let Ok(entries) = fs::read_dir(&user_unit_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let fname = entry.file_name();
        let name = fname.to_string_lossy();
        if !(name.starts_with("mac-mgmt-service@")
            || name == "mac-mgmt-services.service"
            || name == "mac-mgmt-service@.service")
        {
            continue;
        }
        had_any = true;
        let unit = name.to_string();
        tracing::info!("removing legacy user unit {unit}");
        let mut cmd = Command::new("systemctl");
        cmd.arg("--user").args(["disable", "--now", &unit]);
        let uid = unsafe { libc::getuid() };
        if std::env::var("XDG_RUNTIME_DIR").is_err() {
            cmd.env("XDG_RUNTIME_DIR", format!("/run/user/{uid}"));
        }
        if std::env::var("DBUS_SESSION_BUS_ADDRESS").is_err() {
            cmd.env(
                "DBUS_SESSION_BUS_ADDRESS",
                format!("unix:path=/run/user/{uid}/bus"),
            );
        }
        let _ = cmd.status();
        let _ = fs::remove_file(entry.path());
    }
    if had_any {
        let mut cmd = Command::new("systemctl");
        cmd.arg("--user").arg("daemon-reload");
        let uid = unsafe { libc::getuid() };
        if std::env::var("XDG_RUNTIME_DIR").is_err() {
            cmd.env("XDG_RUNTIME_DIR", format!("/run/user/{uid}"));
        }
        if std::env::var("DBUS_SESSION_BUS_ADDRESS").is_err() {
            cmd.env(
                "DBUS_SESSION_BUS_ADDRESS",
                format!("unix:path=/run/user/{uid}/bus"),
            );
        }
        let _ = cmd.status();
    }
}
