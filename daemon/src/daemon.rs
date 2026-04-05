use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::time;

use crate::config;
use crate::events::DaemonEvent;
use crate::metrics::Metrics;
use crate::notify::Dispatcher;
use crate::sentry_ext;

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
    sentry_ext::set_tag("environment", ENVIRONMENT);
    sentry_ext::set_tag("target", TARGET);
    sentry_ext::breadcrumb("daemon", "daemon started", &[
        ("version", CURRENT_VERSION),
        ("environment", ENVIRONMENT),
        ("target", TARGET),
    ]);

    let mut cfg = config::load().await?;
    let mut current_cfg = cfg.clone();

    let update_interval = humantime::parse_duration(&cfg.daemon.update_interval)
        .context("invalid update_interval")?;
    let health_interval = humantime::parse_duration(&cfg.daemon.health_interval)
        .context("invalid health_interval")?;

    let mut upgrade_window = cfg.daemon.upgrade_window.as_ref().map(|w| {
        mac_mgmt_common::parse_time_window(w).expect("upgrade_window already validated")
    });

    macro_rules! in_upgrade_window {
        () => {
            match &upgrade_window {
                None => true,
                Some((start, end)) => mac_mgmt_common::is_within_window(*start, *end),
            }
        };
    }

    tracing::info!(
        "daemon started, update interval: {:?}, health interval: {:?}",
        update_interval,
        health_interval
    );

    let metrics_port = cfg.metrics.port;

    let server_url = cfg.server.url.clone();
    let server_token = cfg.server.token.clone();
    let skills_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/root"))
        .join(".plan-ai-skills");

    let dispatcher = Arc::new(Dispatcher::new(
        std::mem::take(&mut cfg.notifications.urls),
        cfg.notifications.events.take(),
    ));

    dispatcher.dispatch(&DaemonEvent::DaemonStarted);

    // Remove packages installed from the old nix source (before per-system job names).
    // Must run before ServiceManager::init which calls ensure_installed().
    match tokio::task::spawn_blocking(crate::nix::remove_old_source_packages).await {
        Ok(Err(e)) => tracing::warn!("old nix source cleanup failed: {e}"),
        Err(e) => tracing::warn!("old nix source cleanup task panicked: {e}"),
        _ => {}
    }

    #[cfg(feature = "services")]
    let mut svc_mgr = crate::service_mgmt::ServiceManager::init(&mut cfg, Arc::clone(&dispatcher))?;

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

    let mut update_tick = time::interval(update_interval);
    let mut health_tick = time::interval(health_interval);

    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .context("failed to register SIGTERM handler")?;
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .context("failed to register SIGINT handler")?;

    // Set up config file watcher
    let (config_tx, mut config_rx) = tokio::sync::mpsc::channel(4);
    let _config_tx_keepalive = config_tx.clone();
    let _config_watcher = match crate::config_watch::watch(&crate::config::config_path(), config_tx) {
        Ok(w) => {
            tracing::info!("watching config file for changes");
            Some(w)
        }
        Err(e) => {
            tracing::warn!("failed to set up config watcher: {e}");
            None
        }
    };

    let instance_id = crate::instance_id::get_or_create()
        .context("failed to get/create instance ID")?;
    tracing::info!("instance ID: {instance_id}");

    #[cfg(feature = "relay")]
    let mut relay_mgr = crate::remote_ssh::Manager::new(
        cfg.relay.url.clone(),
        server_url.clone(),
        server_token.clone(),
        instance_id.clone(),
    );

    // Start server push WebSocket if server is configured
    let mut push_rx = if let (Some(url), Some(token)) = (&server_url, &server_token) {
        let (_handle, rx) = crate::server_push::start(url, token);
        Some(rx)
    } else {
        None
    };

    // Run immediate update check and skills/MCP/SSH-key sync on startup
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
    #[cfg(feature = "relay")]
    relay_mgr.sync_ssh_keys().await;

    macro_rules! handle_update {
        () => {
            {
                if in_upgrade_window!() {
                    #[cfg(feature = "self-update")]
                    { let _ = tokio::task::spawn_blocking(crate::self_update::check_and_apply).await; }
                    let _ = tokio::task::spawn_blocking(upgrade_nix).await;
                } else {
                    tracing::info!("outside upgrade window, skipping upgrades");
                }

                if let (Some(url), Some(token)) = (&server_url, &server_token) {
                    if let Err(e) = crate::skills::sync_skills(url, token, &skills_dir).await {
                        tracing::warn!("skills sync failed: {e}");
                    }
                    if let Err(e) = crate::mcp_servers::sync_mcp_servers(url, token).await {
                        tracing::warn!("MCP servers sync failed: {e}");
                    }
                }

                #[cfg(feature = "relay")]
                relay_mgr.sync_ssh_keys().await;

                if in_upgrade_window!() {
                    #[cfg(feature = "services")]
                    svc_mgr.check_upgrades();
                }
            }
        };
    }

    macro_rules! handle_config_reload {
        () => {
            {
                tracing::info!("config file changed, reloading");
                match crate::config::reload().await {
                    Ok(new_cfg) => {
                        if let Err(e) = new_cfg.daemon.validate() {
                            tracing::warn!("new config invalid, keeping old: {e}");
                        } else {
                            if new_cfg.daemon.update_interval != current_cfg.daemon.update_interval {
                                if let Ok(d) = humantime::parse_duration(&new_cfg.daemon.update_interval) {
                                    update_tick = time::interval(d);
                                    tracing::info!("update_interval changed to {}", new_cfg.daemon.update_interval);
                                }
                            }
                            if new_cfg.daemon.health_interval != current_cfg.daemon.health_interval {
                                if let Ok(d) = humantime::parse_duration(&new_cfg.daemon.health_interval) {
                                    health_tick = time::interval(d);
                                    tracing::info!("health_interval changed to {}", new_cfg.daemon.health_interval);
                                }
                            }

                            let new_window = new_cfg.daemon.upgrade_window.as_ref().map(|w| {
                                mac_mgmt_common::parse_time_window(w).expect("already validated")
                            });
                            if new_window != upgrade_window {
                                upgrade_window = new_window;
                                tracing::info!("upgrade_window updated");
                            }

                            dispatcher.reconfigure(
                                new_cfg.notifications.urls.clone(),
                                new_cfg.notifications.events.clone(),
                            );

                            // Schedule service restart for changes that require it
                            let needs_restart =
                                new_cfg.global.llm_provider != current_cfg.global.llm_provider
                                || new_cfg.global.agent_provider != current_cfg.global.agent_provider
                                || format!("{:?}", new_cfg.ollama) != format!("{:?}", current_cfg.ollama)
                                || format!("{:?}", new_cfg.nexa) != format!("{:?}", current_cfg.nexa)
                                || format!("{:?}", new_cfg.openclaw) != format!("{:?}", current_cfg.openclaw);

                            if needs_restart {
                                tracing::info!("service config changed, scheduling restart");
                                #[cfg(feature = "services")]
                                svc_mgr.schedule_restart();
                            }
                            if new_cfg.metrics.port != current_cfg.metrics.port {
                                tracing::warn!("metrics.port changed \u{2014} daemon restart required to apply");
                            }
                            if new_cfg.daemon.log_level != current_cfg.daemon.log_level {
                                tracing::info!(
                                    "log_level changed to {} \u{2014} runtime change requires tracing_subscriber::reload layer",
                                    new_cfg.daemon.log_level
                                );
                            }

                            current_cfg = new_cfg;
                        }
                    }
                    Err(e) => tracing::warn!("config reload failed: {e}"),
                }
            }
        };
    }

    macro_rules! handle_shutdown {
        ($signal:expr) => {
            {
                tracing::info!("received {}, shutting down", $signal);
                sentry_ext::breadcrumb("daemon", &format!("{} received, shutting down", $signal), &[]);
                dispatcher.dispatch(&DaemonEvent::DaemonStopped);
            }
        };
    }

    macro_rules! handle_health_tick {
        () => {
            {
                #[cfg(feature = "services")]
                svc_mgr.health_tick(&metrics, in_upgrade_window!());

                // Send heartbeat if server is configured
                if let (Some(url), Some(token)) = (&server_url, &server_token) {
                    #[cfg(feature = "services")]
                    let services = svc_mgr.collect_statuses();
                    #[cfg(not(feature = "services"))]
                    let services = vec![];

                    let url = url.clone();
                    let token = token.clone();
                    let iid = instance_id.clone();
                    tokio::spawn(async move {
                        send_heartbeat(&url, &token, &iid, services).await;
                    });
                }
            }
        };
    }

    macro_rules! handle_push_cmd {
        ($cmd:expr) => {
            {
                match $cmd {
                    crate::server_push::PushCommand::SyncConfig => {
                        tracing::info!("server push: sync config");
                        // Re-fetch and reload config
                        match crate::config::reload().await {
                            Ok(new_cfg) => {
                                if new_cfg.daemon.validate().is_ok() {
                                    let needs_restart =
                                        new_cfg.global.llm_provider != current_cfg.global.llm_provider
                                        || new_cfg.global.agent_provider != current_cfg.global.agent_provider
                                        || format!("{:?}", new_cfg.ollama) != format!("{:?}", current_cfg.ollama)
                                        || format!("{:?}", new_cfg.nexa) != format!("{:?}", current_cfg.nexa)
                                        || format!("{:?}", new_cfg.openclaw) != format!("{:?}", current_cfg.openclaw);

                                    if needs_restart {
                                        tracing::info!("pushed config requires restart, scheduling");
                                        #[cfg(feature = "services")]
                                        svc_mgr.schedule_restart();
                                    }
                                    current_cfg = new_cfg;
                                }
                            }
                            Err(e) => tracing::warn!("push config reload failed: {e}"),
                        }
                    }
                    crate::server_push::PushCommand::SyncSkills => {
                        if let (Some(url), Some(token)) = (&server_url, &server_token) {
                            if let Err(e) = crate::skills::sync_skills(url, token, &skills_dir).await {
                                tracing::warn!("push skills sync failed: {e}");
                            }
                        }
                    }
                    crate::server_push::PushCommand::SyncMcpServers => {
                        if let (Some(url), Some(token)) = (&server_url, &server_token) {
                            if let Err(e) = crate::mcp_servers::sync_mcp_servers(url, token).await {
                                tracing::warn!("push MCP sync failed: {e}");
                            }
                        }
                    }
                    crate::server_push::PushCommand::SyncSshKeys => {
                        #[cfg(feature = "relay")]
                        relay_mgr.sync_ssh_keys().await;
                    }
                }
            }
        };
    }

    loop {
        #[cfg(feature = "relay")]
        {
            tokio::select! {
                _ = sigterm.recv() => { handle_shutdown!("SIGTERM"); break; }
                _ = sigint.recv() => { handle_shutdown!("SIGINT"); break; }
                _ = update_tick.tick() => { handle_update!(); },
                Some(cmd) = relay_mgr.recv_cmd() => {
                    relay_mgr.handle_cmd(cmd);
                }
                _ = health_tick.tick() => { handle_health_tick!(); }
                _ = crate::config_watch::recv_debounced(&mut config_rx) => {
                    handle_config_reload!();
                }
                Some(cmd) = async {
                    match &mut push_rx { Some(rx) => rx.recv().await, None => std::future::pending().await }
                } => {
                    handle_push_cmd!(cmd);
                }
            }
        }

        #[cfg(not(feature = "relay"))]
        {
            tokio::select! {
                _ = sigterm.recv() => { handle_shutdown!("SIGTERM"); break; }
                _ = sigint.recv() => { handle_shutdown!("SIGINT"); break; }
                _ = update_tick.tick() => { handle_update!(); },
                _ = health_tick.tick() => { handle_health_tick!(); }
                _ = crate::config_watch::recv_debounced(&mut config_rx) => {
                    handle_config_reload!();
                }
                Some(cmd) = async {
                    match &mut push_rx { Some(rx) => rx.recv().await, None => std::future::pending().await }
                } => {
                    handle_push_cmd!(cmd);
                }
            }
        }
    }

    #[cfg(feature = "services")]
    svc_mgr.shutdown().await;

    sentry_ext::breadcrumb("daemon", "daemon shutdown complete", &[]);
    tracing::info!("daemon shutdown complete");

    Ok(())
}

async fn send_heartbeat(
    server_url: &str,
    server_token: &str,
    instance_id: &str,
    services: Vec<serde_json::Value>,
) {
    let version = CURRENT_VERSION;
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "instance_id": instance_id,
        "version": version,
        "services": services,
    });

    let url = format!("{server_url}/api/heartbeat");
    match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client
            .post(&url)
            .bearer_auth(server_token)
            .json(&body)
            .send(),
    )
    .await
    {
        Ok(Ok(resp)) if resp.status().is_success() => {
            tracing::debug!("heartbeat sent");
        }
        Ok(Ok(resp)) => {
            tracing::debug!("heartbeat failed: {}", resp.status());
        }
        Ok(Err(e)) => {
            tracing::debug!("heartbeat failed: {e}");
        }
        Err(_) => {
            tracing::debug!("heartbeat timed out");
        }
    }
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
