use anyhow::{Context, Result};
use std::io::Write;
use std::time::Duration;
use tokio::time;

use crate::config;
use crate::managed_service::ManagedService;
use crate::services::{ollama::Ollama, openclaw::OpenClaw};

const UPDATE_INTERVAL: Duration = Duration::from_secs(3600); // 1 hour
const HEALTH_INTERVAL: Duration = Duration::from_secs(60); // 1 minute
const UPDATE_BASE: &str = "https://update.plan.ai";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const ENVIRONMENT: &str = env!("ENVIRONMENT");
const TARGET: &str = env!("TARGET");

struct ServiceState {
    service: Box<dyn ManagedService>,
    child: std::process::Child,
    upgrade_pending: bool,
    skip_health_check: bool,
    post_start_done: bool,
}

pub async fn run() -> Result<()> {
    tracing::info!(
        "daemon started, update interval: {:?}, health interval: {:?}",
        UPDATE_INTERVAL,
        HEALTH_INTERVAL
    );

    let cfg = config::load()?;

    let services: Vec<Box<dyn ManagedService>> = vec![
        Box::new(OpenClaw::new(cfg.openclaw)),
        Box::new(Ollama::new(cfg.ollama)),
    ];

    let mut states: Vec<ServiceState> = Vec::new();
    for service in services {
        let name = service.name().to_string();
        service.ensure_installed()?;
        service.ensure_setup()?;
        let child = service.spawn()?;
        tracing::info!("{name} initialized");
        states.push(ServiceState {
            service,
            child,
            upgrade_pending: false,
            skip_health_check: true,
            post_start_done: false,
        });
    }

    let mut update_interval = time::interval(UPDATE_INTERVAL);
    let mut health_interval = time::interval(HEALTH_INTERVAL);

    // Run immediate update check on startup
    check_and_update();

    loop {
        tokio::select! {
            _ = update_interval.tick() => {
                check_and_update();
                for state in &mut states {
                    if !state.upgrade_pending {
                        let name = state.service.name();
                        match state.service.check_and_upgrade() {
                            Ok(upgraded) => state.upgrade_pending = upgraded,
                            Err(e) => tracing::warn!("{name} upgrade check failed: {e}"),
                        }
                    }
                }
            },
            _ = health_interval.tick() => {
                for state in &mut states {
                    let name = state.service.name();

                    // Restart if exited
                    match state.child.try_wait() {
                        Ok(Some(status)) => {
                            tracing::warn!("{name} exited with {status}, restarting");
                            state.child = state.service.spawn()?;
                            state.upgrade_pending = false;
                            state.skip_health_check = true;
                            state.post_start_done = false;
                        }
                        Ok(None) => {}
                        Err(e) => tracing::error!("failed to check {name} status: {e}"),
                    }

                    // Apply pending upgrade when idle
                    if state.upgrade_pending {
                        match state.service.is_busy() {
                            Ok(false) => {
                                tracing::info!("{name} is idle, restarting to apply upgrade");
                                let _ = state.child.kill();
                                let _ = state.child.wait();
                                state.child = state.service.spawn()?;
                                state.upgrade_pending = false;
                                state.skip_health_check = true;
                                state.post_start_done = false;
                            }
                            Ok(true) => tracing::info!("{name} is busy, deferring upgrade restart"),
                            Err(e) => tracing::warn!("{name} busy check failed: {e}"),
                        }
                    }

                    // Health check
                    if state.skip_health_check {
                        tracing::info!("skipping health check, {name} recently started");
                        state.skip_health_check = false;
                    } else {
                        match state.service.check_health() {
                            Ok(true) => {
                                tracing::info!("{name} is healthy");
                                if !state.post_start_done {
                                    if let Err(e) = state.service.post_start() {
                                        tracing::error!("{name} post_start failed: {e}");
                                    }
                                    state.post_start_done = true;
                                }
                            }
                            Ok(false) => {
                                tracing::warn!("{name} is unhealthy, attempting repair");
                                if let Err(e) = state.service.repair() {
                                    tracing::error!("{name} repair failed: {e}");
                                }
                            }
                            Err(e) => tracing::warn!("{name} health check failed: {e}"),
                        }
                    }
                }
            }
        }
    }
}

fn check_and_update() {
    tracing::info!("checking for updates (current: {CURRENT_VERSION})");

    if let Err(e) = do_update(false) {
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

pub fn do_update(force: bool) -> Result<()> {
    let remote_version = fetch_remote_version()?;

    if !force && remote_version == CURRENT_VERSION {
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
