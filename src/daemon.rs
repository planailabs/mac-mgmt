use anyhow::{Context, Result};
use std::io::Write;
use std::process::Command;
use std::time::Duration;
use tokio::time;

const UPDATE_INTERVAL: Duration = Duration::from_secs(3600); // 1 hour
const HEALTH_INTERVAL: Duration = Duration::from_secs(300); // 5 minutes
const UPDATE_BASE: &str = "https://update.plan.ai";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const ENVIRONMENT: &str = env!("ENVIRONMENT");
const TARGET: &str = env!("TARGET");

pub async fn run() -> Result<()> {
    tracing::info!(
        "daemon started, update interval: {:?}, health interval: {:?}",
        UPDATE_INTERVAL,
        HEALTH_INTERVAL
    );

    // Ensure openclaw is installed via nix
    ensure_openclaw_installed()?;

    // Ensure openclaw is configured
    ensure_openclaw_setup()?;

    // Start openclaw gateway as a child process
    let mut gateway = spawn_gateway()?;
    let mut openclaw_upgrade_pending = false;

    let mut update_interval = time::interval(UPDATE_INTERVAL);
    let mut health_interval = time::interval(HEALTH_INTERVAL);

    // Run immediate checks on startup
    check_and_update();
    check_health();

    loop {
        tokio::select! {
            _ = update_interval.tick() => {
                check_and_update();
                if !openclaw_upgrade_pending {
                    match check_and_upgrade_openclaw() {
                        Ok(upgraded) => openclaw_upgrade_pending = upgraded,
                        Err(e) => tracing::warn!("openclaw upgrade check failed: {e}"),
                    }
                }
            },
            _ = health_interval.tick() => {
                // Restart gateway if it exited
                match gateway.try_wait() {
                    Ok(Some(status)) => {
                        tracing::warn!("openclaw gateway exited with {status}, restarting");
                        gateway = spawn_gateway()?;
                        openclaw_upgrade_pending = false;
                    }
                    Ok(None) => {} // still running
                    Err(e) => tracing::error!("failed to check gateway status: {e}"),
                }

                // Apply pending upgrade when openclaw is idle
                if openclaw_upgrade_pending {
                    if let Err(e) = apply_openclaw_upgrade(&mut gateway) {
                        tracing::warn!("failed to apply openclaw upgrade: {e}");
                    } else {
                        openclaw_upgrade_pending = false;
                    }
                }

                check_health();
            }
        }
    }
}

fn spawn_gateway() -> Result<std::process::Child> {
    let child = Command::new("openclaw")
        .arg("gateway")
        .spawn()
        .context("failed to start openclaw gateway")?;
    tracing::info!("openclaw gateway started (pid: {})", child.id());
    Ok(child)
}

fn check_health() {
    tracing::info!("running openclaw health check");

    match crate::health::check() {
        Ok(true) => tracing::info!("openclaw is healthy"),
        Ok(false) => {
            tracing::warn!("openclaw is unhealthy, running doctor --fix");
            if let Err(e) = crate::health::doctor_fix() {
                tracing::error!("doctor --fix failed: {e}");
            }
        }
        Err(e) => tracing::warn!("health check failed: {e}"),
    }
}

fn ensure_openclaw_installed() -> Result<()> {
    if crate::nix::is_installed("openclaw")? {
        tracing::info!("openclaw is already installed");
        return Ok(());
    }

    tracing::info!("openclaw not found, installing via nix");
    crate::nix::profile_install("nixpkgs#openclaw", false)?;
    Ok(())
}

fn ensure_openclaw_setup() -> Result<()> {
    let home = std::env::var("HOME").context("HOME not set")?;
    let config = std::path::PathBuf::from(home).join(".openclaw/openclaw.json");

    if config.exists() {
        tracing::info!("openclaw config found at {}", config.display());
        return Ok(());
    }

    tracing::info!("openclaw config not found, running openclaw setup");
    let status = Command::new("openclaw")
        .arg("setup")
        .status()
        .context("failed to run openclaw setup")?;

    if !status.success() {
        anyhow::bail!("openclaw setup exited with status {status}");
    }

    Ok(())
}

/// Check for and perform nix upgrade. Returns true if an upgrade was installed
/// and a restart is pending.
fn check_and_upgrade_openclaw() -> Result<bool> {
    if !crate::nix::has_upgrade("nixpkgs#openclaw")? {
        return Ok(false);
    }

    tracing::info!("upgrading openclaw via nix");
    crate::nix::profile_install("nixpkgs#openclaw", true)?;
    tracing::info!("openclaw upgraded, restart pending until idle");
    Ok(true)
}

/// Restart the gateway to apply a pending upgrade, but only if openclaw is idle.
fn apply_openclaw_upgrade(gateway: &mut std::process::Child) -> Result<()> {
    match crate::health::is_busy() {
        Ok(true) => {
            tracing::info!("openclaw is busy, deferring restart for pending upgrade");
            anyhow::bail!("openclaw is busy");
        }
        Err(e) => {
            tracing::warn!("failed to check if openclaw is busy, deferring restart: {e}");
            anyhow::bail!("busy check failed");
        }
        Ok(false) => {}
    }

    tracing::info!("openclaw is idle, restarting gateway to apply upgrade");
    let _ = gateway.kill();
    let _ = gateway.wait();

    *gateway = spawn_gateway()?;
    Ok(())
}

fn check_and_update() {
    tracing::info!("checking for updates (current: {CURRENT_VERSION})");

    if let Err(e) = do_update() {
        tracing::warn!("update failed: {e}");
    }
}

fn fetch_remote_version() -> Result<String> {
    let url = format!("{UPDATE_BASE}/{ENVIRONMENT}/mac-mgmt.version");
    let mut body = Vec::new();
    let mut download = self_update::Download::from_url(&url);
    download.show_progress(false);
    download.download_to(&mut body)?;
    let version = String::from_utf8(body)
        .context("invalid UTF-8 in version file")?
        .trim()
        .to_string();
    Ok(version)
}

pub fn do_update() -> Result<()> {
    let remote_version = fetch_remote_version()?;

    if remote_version == CURRENT_VERSION {
        tracing::info!("already up to date ({CURRENT_VERSION})");
        return Ok(());
    }

    tracing::info!("update available: {CURRENT_VERSION} -> {remote_version}");

    let url = format!("{UPDATE_BASE}/{ENVIRONMENT}/mac-mgmt.tar.gz");
    let mut tmp_archive = tempfile::Builder::new()
        .suffix(".tar.gz")
        .tempfile()
        .context("failed to create temp file")?;

    // Download the tarball
    tracing::info!("downloading {url}");
    let mut download = self_update::Download::from_url(&url);
    download.show_progress(false);
    let mut body = Vec::new();
    download.download_to(&mut body)?;
    tmp_archive.write_all(&body)?;
    tmp_archive.flush()?;

    // Extract the binary from the archive
    let tmp_dir = tempfile::tempdir().context("failed to create temp dir")?;
    self_update::Extract::from_source(tmp_archive.path())
        .archive(self_update::ArchiveKind::Tar(Some(
            self_update::Compression::Gz,
        )))
        .extract_into(tmp_dir.path())?;

    let bin_name = format!("mac-mgmt-{TARGET}");
    let new_bin = tmp_dir.path().join(&bin_name);
    if !new_bin.exists() {
        anyhow::bail!("binary '{bin_name}' not found in archive");
    }

    // Replace the running binary
    self_replace::self_replace(&new_bin).context("failed to replace binary")?;

    tracing::info!("binary updated successfully");
    Ok(())
}
