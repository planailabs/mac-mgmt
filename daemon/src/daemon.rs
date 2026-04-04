use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

use crate::config;
use crate::metrics::Metrics;
use crate::remote_ssh::{self, RemoteSshCommand};
use crate::sentry_ext;

const UPDATE_INTERVAL: Duration = Duration::from_secs(3600); // 1 hour
const HEALTH_INTERVAL: Duration = Duration::from_secs(60); // 1 minute
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const ENVIRONMENT: &str = match option_env!("ENVIRONMENT") {
    Some(v) => v,
    None => "dev",
};
const TARGET: &str = match option_env!("TARGET") {
    Some(v) => v,
    None => "unknown",
};

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

    let mut cfg = config::load().await?;
    let metrics_port = cfg.metrics.port;

    let server_url = cfg.server.url.clone();
    let server_token = cfg.server.token.clone();
    let skills_dir = {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        std::path::PathBuf::from(home).join(".plan-ai-skills")
    };

    #[cfg(feature = "services")]
    let mut svc_mgr = crate::service_mgmt::ServiceManager::init(&mut cfg)?;

    #[cfg(not(feature = "services"))]
    tracing::info!("services feature disabled, skipping service management");

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

    // Instance ID + FIFO watcher for remote SSH
    let instance_id = crate::instance_id::get_or_create()
        .context("failed to get/create instance ID")?;
    tracing::info!("instance ID: {instance_id}");

    let relay_url = cfg.relay.url.clone();
    let (ssh_cmd_tx, mut ssh_cmd_rx) = tokio::sync::mpsc::channel::<RemoteSshCommand>(4);
    tokio::spawn(async move {
        if let Err(e) = remote_ssh::fifo_watcher::watch(ssh_cmd_tx).await {
            tracing::error!("FIFO watcher failed: {e:#}");
        }
    });

    let mut relay_task: Option<tokio::task::JoinHandle<()>> = None;

    // Run immediate update check and skills/MCP sync on startup
    #[cfg(feature = "self-update")]
    { let _ = tokio::task::spawn_blocking(crate::self_update::check_and_apply).await; }
    if let (Some(url), Some(token)) = (&server_url, &server_token) {
        if let Err(e) = crate::skills::sync_skills(url, token, &skills_dir).await {
            tracing::warn!("initial skills sync failed: {e}");
        }
        if let Err(e) = crate::mcp_servers::sync_mcp_servers(url, token).await {
            tracing::warn!("initial MCP servers sync failed: {e}");
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
                #[cfg(feature = "self-update")]
                { let _ = tokio::task::spawn_blocking(crate::self_update::check_and_apply).await; }
                let _ = tokio::task::spawn_blocking(upgrade_nix).await;

                if let (Some(url), Some(token)) = (&server_url, &server_token) {
                    if let Err(e) = crate::skills::sync_skills(url, token, &skills_dir).await {
                        tracing::warn!("skills sync failed: {e}");
                    }
                    if let Err(e) = crate::mcp_servers::sync_mcp_servers(url, token).await {
                        tracing::warn!("MCP servers sync failed: {e}");
                    }
                }

                #[cfg(feature = "services")]
                svc_mgr.check_upgrades();
            },
            Some(cmd) = ssh_cmd_rx.recv() => {
                match cmd {
                    RemoteSshCommand::Enable => {
                        if relay_task.as_ref().is_some_and(|t| !t.is_finished()) {
                            tracing::info!("remote SSH already enabled");
                            continue;
                        }
                        let Some(ref url) = relay_url else {
                            tracing::warn!("cannot enable remote SSH: no relay URL configured");
                            continue;
                        };
                        let Some(ref token) = server_token else {
                            tracing::warn!("cannot enable remote SSH: no server token configured");
                            continue;
                        };
                        tracing::info!("enabling remote SSH");
                        let url = url.clone();
                        let token = token.clone();
                        let iid = instance_id.clone();
                        let agent_name = cfg.server.url.as_ref().and_then(|_| None::<String>); // TODO: pass agent_name from global config if available
                        relay_task = Some(tokio::spawn(async move {
                            if let Err(e) = remote_ssh::relay_client::run(&url, &token, &iid, agent_name.as_deref()).await {
                                tracing::error!("relay client exited: {e:#}");
                            }
                        }));
                    }
                    RemoteSshCommand::Disable => {
                        if let Some(task) = relay_task.take() {
                            tracing::info!("disabling remote SSH");
                            task.abort();
                        }
                    }
                }
            }
            _ = health_interval.tick() => {
                #[cfg(feature = "services")]
                svc_mgr.health_tick(&metrics);
            }
        }
    }

    #[cfg(feature = "services")]
    svc_mgr.shutdown().await;

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
