use anyhow::{Context, Result};
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

use crate::config;
use crate::managed_service::ManagedService;
use crate::metrics::Metrics;
use crate::sentry_ext;
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

    sentry_ext::set_tag("environment", ENVIRONMENT);
    sentry_ext::set_tag("target", TARGET);
    sentry_ext::breadcrumb("daemon", "daemon started", &[
        ("version", CURRENT_VERSION),
        ("environment", ENVIRONMENT),
        ("target", TARGET),
    ]);

    let cfg = config::load().await?;
    let metrics_port = cfg.metrics.port;

    let server_url = cfg.server.url.clone();
    let server_token = cfg.server.token.clone();
    let skills_dir = {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        std::path::PathBuf::from(home).join(".plan-ai-skills")
    };

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
        sentry_ext::breadcrumb("service", &format!("{name} initialized"), &[
            ("service", &name),
            ("pid", &child.id().to_string()),
        ]);
        states.push(ServiceState {
            service,
            child,
            upgrade_pending: false,
            skip_health_check: true,
            post_start_done: false,
        });
    }

    let metrics = Arc::new(Metrics::new());

    // Spawn the metrics server
    let metrics_clone = Arc::clone(&metrics);
    tokio::spawn(async move {
        if let Err(e) = crate::metrics_server::build_rocket(metrics_clone, metrics_port).launch().await {
            tracing::error!("metrics server failed: {e}");
            sentry_ext::capture_error(&format!("metrics server failed: {e}"), &[]);
        }
    });
    tracing::info!("metrics server started on port {metrics_port}");

    let mut update_interval = time::interval(UPDATE_INTERVAL);
    let mut health_interval = time::interval(HEALTH_INTERVAL);

    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .context("failed to register SIGTERM handler")?;
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .context("failed to register SIGINT handler")?;

    // Run immediate update check and skills sync on startup
    let _ = tokio::task::spawn_blocking(check_and_update).await;
    if let (Some(url), Some(token)) = (&server_url, &server_token) {
        if let Err(e) = crate::skills::sync_skills(url, token, &skills_dir).await {
            tracing::warn!("initial skills sync failed: {e}");
        }
    }

    loop {
        tokio::select! {
            _ = sigterm.recv() => {
                tracing::info!("received SIGTERM, shutting down");
                sentry_ext::breadcrumb("daemon", "SIGTERM received, shutting down", &[]);
                break;
            }
            _ = sigint.recv() => {
                tracing::info!("received SIGINT, shutting down");
                sentry_ext::breadcrumb("daemon", "SIGINT received, shutting down", &[]);
                break;
            }
            _ = update_interval.tick() => {
                let _ = tokio::task::spawn_blocking(check_and_update).await;
                let _ = tokio::task::spawn_blocking(upgrade_nix).await;

                if let (Some(url), Some(token)) = (&server_url, &server_token) {
                    if let Err(e) = crate::skills::sync_skills(url, token, &skills_dir).await {
                        tracing::warn!("skills sync failed: {e}");
                    }
                }

                for state in &mut states {
                    if !state.upgrade_pending {
                        let name = state.service.name();
                        sentry_ext::set_tag("service", name);
                        match state.service.check_and_upgrade() {
                            Ok(true) => {
                                state.upgrade_pending = true;
                                sentry_ext::breadcrumb("upgrade", &format!("{name} upgrade pending"), &[("service", name)]);
                            }
                            Ok(false) => {}
                            Err(e) => {
                                tracing::warn!("{name} upgrade check failed: {e}");
                                sentry_ext::capture_error(
                                    &format!("{name} upgrade check failed: {e}"),
                                    &[("service", name)],
                                );
                            }
                        }
                    }
                }
            },
            _ = health_interval.tick() => {
                for state in &mut states {
                    let name = state.service.name();
                    sentry_ext::set_tag("service", name);

                    // Restart if exited
                    match state.child.try_wait() {
                        Ok(Some(status)) => {
                            tracing::warn!("{name} exited with {status}, restarting");
                            let code = status.code().map(|c| c.to_string()).unwrap_or("signal".to_string());
                            sentry_ext::capture_error(
                                &format!("{name} process exited unexpectedly"),
                                &[("service", name), ("exit_code", &code)],
                            );
                            state.child = state.service.spawn()?;
                            state.upgrade_pending = false;
                            state.skip_health_check = true;
                            state.post_start_done = false;
                        }
                        Ok(None) => {}
                        Err(e) => tracing::error!("failed to check {name} status: {e}"),
                    }

                    // Apply pending upgrade when idle
                    let busy = if state.upgrade_pending {
                        match state.service.is_busy() {
                            Ok(false) => {
                                tracing::info!("{name} is idle, restarting to apply upgrade");
                                let _ = state.child.kill();
                                let _ = state.child.wait();
                                state.child = state.service.spawn()?;
                                state.upgrade_pending = false;
                                state.skip_health_check = true;
                                state.post_start_done = false;
                                sentry_ext::breadcrumb("upgrade", &format!("{name} restarted for upgrade"), &[("service", name)]);
                                false
                            }
                            Ok(true) => {
                                tracing::info!("{name} is busy, deferring upgrade restart");
                                true
                            }
                            Err(e) => {
                                tracing::warn!("{name} busy check failed: {e}");
                                false
                            }
                        }
                    } else {
                        false
                    };

                    // Health check
                    let healthy = if state.skip_health_check {
                        tracing::info!("skipping health check, {name} recently started");
                        state.skip_health_check = false;
                        true // assume healthy right after start
                    } else {
                        match state.service.check_health() {
                            Ok(true) => {
                                tracing::info!("{name} is healthy");
                                if !state.post_start_done {
                                    if let Err(e) = state.service.post_start() {
                                        tracing::error!("{name} post_start failed: {e}");
                                        sentry_ext::capture_error(
                                            &format!("{name} post_start failed: {e}"),
                                            &[("service", name)],
                                        );
                                    }
                                    state.post_start_done = true;
                                }
                                true
                            }
                            Ok(false) => {
                                tracing::warn!("{name} is unhealthy, attempting repair");
                                sentry_ext::breadcrumb("health", &format!("{name} unhealthy, repairing"), &[("service", name)]);
                                if let Err(e) = state.service.repair() {
                                    tracing::error!("{name} repair failed: {e}");
                                    sentry_ext::capture_error(
                                        &format!("{name} repair failed: {e}"),
                                        &[("service", name)],
                                    );
                                }
                                false
                            }
                            Err(e) => {
                                tracing::warn!("{name} health check failed: {e}");
                                false
                            }
                        }
                    };

                    // Update prometheus metrics
                    metrics.service_healthy.with_label_values(&[name]).set(if healthy { 1 } else { 0 });
                    metrics.service_upgrade_pending.with_label_values(&[name]).set(if state.upgrade_pending { 1 } else { 0 });
                    metrics.service_busy.with_label_values(&[name]).set(if busy { 1 } else { 0 });
                }
            }
        }
    }

    // Shutdown: send SIGTERM to all services, then wait up to 10s before SIGKILL
    for state in &mut states {
        let name = state.service.name();
        let pid = state.child.id();
        tracing::info!("sending SIGTERM to {name} (pid {pid})");
        unsafe { libc::kill(pid as i32, libc::SIGTERM); }
    }

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    for state in &mut states {
        let name = state.service.name();
        loop {
            match state.child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if tokio::time::Instant::now() >= deadline => {
                    tracing::warn!("{name} did not exit in time, sending SIGKILL");
                    let _ = state.child.kill();
                    let _ = state.child.wait();
                    break;
                }
                Ok(None) => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(e) => {
                    tracing::error!("failed to check {name} exit status: {e}");
                    break;
                }
            }
        }
        tracing::info!("{name} stopped");
    }
    sentry_ext::breadcrumb("daemon", "daemon shutdown complete", &[]);
    tracing::info!("daemon shutdown complete");

    Ok(())
}

fn upgrade_nix() {
    tracing::info!("checking for nix upgrade");
    if let Err(e) = crate::nix::upgrade_nix() {
        tracing::warn!("nix upgrade failed: {e}");
        sentry_ext::capture_error(
            &format!("nix upgrade failed: {e}"),
            &[],
        );
    }
}

fn check_and_update() {
    tracing::info!("checking for updates (current: {CURRENT_VERSION})");
    sentry_ext::breadcrumb("self-update", "checking for updates", &[("version", CURRENT_VERSION)]);

    if let Err(e) = do_update(false) {
        tracing::warn!("update failed: {e}");
        sentry_ext::capture_error(
            &format!("self-update failed: {e}"),
            &[("version", CURRENT_VERSION)],
        );
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
    sentry_ext::breadcrumb("self-update", "downloading update", &[
        ("from", CURRENT_VERSION),
        ("to", &remote_version),
    ]);

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
    sentry_ext::breadcrumb("self-update", "binary updated", &[
        ("from", CURRENT_VERSION),
        ("to", &remote_version),
    ]);
    Ok(())
}
