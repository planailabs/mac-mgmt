use anyhow::{Context, Result};
use std::io::Write;
use std::process::Command;
use std::time::Duration;
use tokio::time;

const UPDATE_INTERVAL: Duration = Duration::from_secs(3600); // 1 hour
const HEALTH_INTERVAL: Duration = Duration::from_secs(300); // 5 minutes
const UPDATE_BASE: &str = "https://update.plan.ai";
const BIN_NAME: &str = "mac-mgmt";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const ENVIRONMENT: &str = env!("ENVIRONMENT");

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
    let mut gateway = Command::new("openclaw")
        .arg("gateway")
        .spawn()
        .context("failed to start openclaw gateway")?;
    tracing::info!("openclaw gateway started (pid: {})", gateway.id());

    let mut update_interval = time::interval(UPDATE_INTERVAL);
    let mut health_interval = time::interval(HEALTH_INTERVAL);

    // Run immediate checks on startup
    check_and_update();
    check_health();

    loop {
        tokio::select! {
            _ = update_interval.tick() => check_and_update(),
            _ = health_interval.tick() => {
                // Restart gateway if it exited
                match gateway.try_wait() {
                    Ok(Some(status)) => {
                        tracing::warn!("openclaw gateway exited with {status}, restarting");
                        gateway = Command::new("openclaw")
                            .arg("gateway")
                            .spawn()
                            .context("failed to restart openclaw gateway")?;
                        tracing::info!("openclaw gateway restarted (pid: {})", gateway.id());
                    }
                    Ok(None) => {} // still running
                    Err(e) => tracing::error!("failed to check gateway status: {e}"),
                }
                check_health();
            }
        }
    }
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

    let new_bin = tmp_dir.path().join(BIN_NAME);
    if !new_bin.exists() {
        anyhow::bail!("binary '{BIN_NAME}' not found in archive");
    }

    // Replace the running binary
    self_replace::self_replace(&new_bin).context("failed to replace binary")?;

    tracing::info!("binary updated successfully");
    Ok(())
}
