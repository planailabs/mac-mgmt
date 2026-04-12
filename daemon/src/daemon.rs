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

pub async fn run(
    log_buf: crate::log_buffer::LogBuffer,
    set_log_level: Box<dyn Fn(&str) + Send>,
) -> Result<()> {
    // Acquire lockfile to ensure only one daemon instance runs at a time.
    let lock_path = config::config_dir().join("daemon.lock");
    std::fs::create_dir_all(lock_path.parent().unwrap()).ok();
    let lock_file = std::fs::File::create(&lock_path)
        .context("failed to create lockfile")?;
    match lock_file.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            anyhow::bail!(
                "another daemon instance is already running (lockfile: {})",
                lock_path.display()
            );
        }
        Err(std::fs::TryLockError::Error(e)) => {
            return Err(e).context("failed to lock lockfile");
        }
    }
    // lock_file is held for the lifetime of run(); the OS releases the lock on drop/exit.

    sentry_ext::set_tag("environment", ENVIRONMENT);
    sentry_ext::set_tag("target", TARGET);
    sentry_ext::breadcrumb("daemon", "daemon started", &[
        ("version", CURRENT_VERSION),
        ("environment", ENVIRONMENT),
        ("target", TARGET),
    ]);

    // Migration: remove legacy UUID-based instance-id file (replaced by
    // host key fingerprint).
    let legacy_id_path = config::config_dir().join("instance-id");
    if legacy_id_path.exists() {
        if let Err(e) = std::fs::remove_file(&legacy_id_path) {
            tracing::warn!("failed to remove legacy instance-id file: {e}");
        } else {
            tracing::info!("removed legacy instance-id file");
        }
    }

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
            upgrade_window.map_or(true, |(start, end)| mac_mgmt_common::is_within_window(start, end))
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

    // Fetch the cluster's nixpkgs pin (if any) before ServiceManager::init runs
    // ensure_installed(), so the very first install uses the pinned URL.
    if let (Some(url), Some(token)) = (&server_url, &server_token) {
        fetch_nixpkgs_pin(url, token).await;
    }

    #[cfg(feature = "services")]
    let mut svc_mgr = crate::service_mgmt::ServiceManager::init(&mut cfg, Arc::clone(&dispatcher), log_buf.clone())?;

    #[cfg(not(feature = "services"))]
    tracing::info!("services feature disabled, skipping service management");

    let metrics = Arc::new(Metrics::new());

    #[cfg(feature = "services")]
    svc_mgr.register_metrics(&metrics);

    // Channel for local sync requests (e.g. from `mac-mgmt sync` via /sync)
    let (sync_tx, mut sync_rx) = tokio::sync::mpsc::channel::<()>(4);
    let _sync_tx_keepalive = sync_tx.clone();

    // Spawn the metrics server
    let metrics_clone = Arc::clone(&metrics);
    let log_buf_clone = log_buf.clone();
    let sync_tx_clone = sync_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::metrics_server::build_rocket(metrics_clone, log_buf_clone, sync_tx_clone, metrics_port).launch().await {
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

    // Now that signal handlers are registered, spawn/connect all services.
    // If a SIGTERM arrives during spawn, the handler will catch it.
    #[cfg(feature = "services")]
    {
        svc_mgr.spawn_all();
        svc_mgr.connect_all().await;
    }

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

    // Derive a stable instance ID from the ed25519 host key fingerprint.
    let host_key = Arc::new(crate::host_keys::load_or_generate()
        .context("failed to load/generate SSH host key")?);
    let instance_id = crate::host_keys::fingerprint_hex(&host_key);
    tracing::info!("instance ID (host key fingerprint): {instance_id}");

    #[cfg(feature = "relay")]
    let mut relay_mgr = crate::remote_ssh::Manager::new(
        cfg.relay.url.clone(),
        server_url.clone(),
        server_token.clone(),
        instance_id.clone(),
        Arc::clone(&host_key),
        metrics_port,
        cfg.relay.remote_ssh_enabled,
    );

    // Start server push WebSocket if server is configured
    let mut push_rx = if let (Some(url), Some(token)) = (&server_url, &server_token) {
        let (_handle, rx) = crate::server_push::start(url, token);
        Some(rx)
    } else {
        None
    };

    // Fetch environment from server for self-update channel
    if let (Some(url), Some(token)) = (&server_url, &server_token) {
        fetch_target_version(url, token).await;
    }

    // Run immediate update check and skills/MCP/SSH-key sync on startup
    #[cfg(feature = "self-update")]
    if in_upgrade_window!() {
        let _ = tokio::task::spawn_blocking(crate::self_update::check_and_apply).await;
        // Notify external wrappers to re-exec with the (possibly new) binary.
        #[cfg(feature = "services")]
        svc_mgr.send_update_self().await;
    } else {
        tracing::info!("outside upgrade window, skipping initial self-update");
    }
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
                // Fetch environment from server before self-update
                if let (Some(url), Some(token)) = (&server_url, &server_token) {
                    fetch_target_version(url, token).await;
                    fetch_nixpkgs_pin(url, token).await;
                }

                if in_upgrade_window!() {
                    #[cfg(feature = "self-update")]
                    {
                        let _ = tokio::task::spawn_blocking(crate::self_update::check_and_apply).await;
                        #[cfg(feature = "services")]
                        svc_mgr.send_update_self().await;
                    }
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
                                svc_mgr.schedule_restart().await;
                            }
                            if new_cfg.metrics.port != current_cfg.metrics.port {
                                tracing::warn!("metrics.port changed \u{2014} daemon restart required to apply");
                            }
                            if new_cfg.daemon.log_level != current_cfg.daemon.log_level {
                                tracing::info!("log_level changed to {}", new_cfg.daemon.log_level);
                                set_log_level(&new_cfg.daemon.log_level);
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
                svc_mgr.health_tick(&metrics, in_upgrade_window!()).await;

                // Send heartbeat if server is configured
                if let (Some(url), Some(token)) = (&server_url, &server_token) {
                    #[cfg(feature = "services")]
                    let services = svc_mgr.collect_statuses();
                    #[cfg(not(feature = "services"))]
                    let services = vec![];

                    #[cfg(feature = "services")]
                    let tunnel_defs = svc_mgr.collect_tunnels();
                    #[cfg(not(feature = "services"))]
                    let tunnel_defs = vec![];

                    // Update the relay client's tunnel map so it can advertise them.
                    #[cfg(feature = "relay")]
                    relay_mgr.update_tunnel_defs(tunnel_defs.clone()).await;

                    let tunnels: Vec<serde_json::Value> = tunnel_defs
                        .into_iter()
                        .map(|t| serde_json::json!({ "name": t.name, "port": t.tcp_port }))
                        .collect();

                    #[cfg(feature = "relay")]
                    let rph = relay_mgr.relay_proxy_hostname().await;
                    #[cfg(not(feature = "relay"))]
                    let rph: Option<String> = None;

                    // Register relay as a virtual service so connectors can depend on it.
                    #[cfg(feature = "services")]
                    if let Some(ref ph) = rph {
                        svc_mgr.set_virtual_service("relay", serde_json::json!({
                            "proxy_hostname": ph,
                        }));
                    }

                    let url = url.clone();
                    let token = token.clone();
                    let iid = instance_id.clone();
                    let hk = Arc::clone(&host_key);
                    tokio::spawn(async move {
                        send_heartbeat(&url, &token, &iid, &hk, services, tunnels, rph).await;
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
                        handle_config_reload!();
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
                    crate::server_push::PushCommand::SelfUpdate => {
                        tracing::info!("server push: self-update requested");
                        if let (Some(url), Some(token)) = (&server_url, &server_token) {
                            fetch_target_version(url, token).await;
                        }
                        if in_upgrade_window!() {
                            #[cfg(feature = "self-update")]
                            {
                                let _ = tokio::task::spawn_blocking(crate::self_update::check_and_apply).await;
                                #[cfg(feature = "services")]
                                svc_mgr.send_update_self().await;
                            }
                        } else {
                            tracing::info!("outside upgrade window, deferring self-update");
                        }
                    }
                    crate::server_push::PushCommand::SyncNixpkgs => {
                        tracing::info!("server push: sync nixpkgs pin");
                        if let (Some(url), Some(token)) = (&server_url, &server_token) {
                            fetch_nixpkgs_pin(url, token).await;
                        }
                        #[cfg(feature = "services")]
                        svc_mgr.check_upgrades();
                    }
                }
            }
        };
    }

    macro_rules! handle_local_sync {
        () => {
            {
                tracing::info!("local sync requested");
                if let (Some(url), Some(token)) = (&server_url, &server_token) {
                    if let Err(e) = crate::skills::sync_skills(url, token, &skills_dir).await {
                        tracing::warn!("local skills sync failed: {e}");
                    }
                    if let Err(e) = crate::mcp_servers::sync_mcp_servers(url, token).await {
                        tracing::warn!("local MCP servers sync failed: {e}");
                    }
                }
                #[cfg(feature = "relay")]
                relay_mgr.sync_ssh_keys().await;
            }
        };
    }

    loop {
        tokio::select! {
            _ = sigterm.recv() => { handle_shutdown!("SIGTERM"); break; }
            _ = sigint.recv() => { handle_shutdown!("SIGINT"); break; }
            _ = update_tick.tick() => { handle_update!(); }
            _ = health_tick.tick() => { handle_health_tick!(); }
            _ = crate::config_watch::recv_debounced(&mut config_rx) => {
                handle_config_reload!();
            }
            Some(cmd) = async {
                if let Some(rx) = &mut push_rx { rx.recv().await } else { std::future::pending().await }
            } => {
                handle_push_cmd!(cmd);
            }
            Some(()) = sync_rx.recv() => {
                handle_local_sync!();
            }
            Some(cmd) = async {
                #[cfg(feature = "relay")]
                { relay_mgr.recv_cmd().await }
                #[cfg(not(feature = "relay"))]
                { std::future::pending::<Option<()>>().await }
            } => {
                #[cfg(feature = "relay")]
                relay_mgr.handle_cmd(cmd);
            }
        }
    }

    #[cfg(feature = "services")]
    svc_mgr.shutdown().await;

    #[cfg(feature = "relay")]
    relay_mgr.cleanup();

    sentry_ext::breadcrumb("daemon", "daemon shutdown complete", &[]);
    tracing::info!("daemon shutdown complete");

    Ok(())
}

/// Fetch the target version from the server and set it for self-update.
async fn fetch_target_version(server_url: &str, server_token: &str) {
    let client = reqwest::Client::new();
    let system = crate::nix::current_system().unwrap_or("");
    let url = format!("{server_url}/api/update?system={system}");
    match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.get(&url).bearer_auth(server_token).send(),
    )
    .await
    {
        Ok(Ok(resp)) if resp.status().is_success() => {
            if let Ok(info) = resp.json::<mac_mgmt_common::UpdateTarget>().await {
                if let Some(ver) = info.target_version {
                    tracing::info!(
                        "server target version: {ver} (store_path: {:?})",
                        info.store_path
                    );
                    #[cfg(feature = "self-update")]
                    crate::self_update::set_target(ver, info.store_path);
                } else {
                    tracing::debug!("no target version set by server");
                }
            }
        }
        Ok(Ok(resp)) => {
            tracing::debug!("update target fetch returned {}", resp.status());
        }
        Ok(Err(e)) => {
            tracing::debug!("update target fetch failed: {e}");
        }
        Err(_) => {
            tracing::debug!("update target fetch timed out");
        }
    }
}

/// Fetch the cluster's nixpkgs commit pin from the server and apply it
/// in-process. Subsequent `nix profile` operations will use this commit's
/// GitLab archive tarball as the flake source.
async fn fetch_nixpkgs_pin(server_url: &str, server_token: &str) {
    let client = reqwest::Client::new();
    let url = format!("{server_url}/api/nixpkgs");
    match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.get(&url).bearer_auth(server_token).send(),
    )
    .await
    {
        Ok(Ok(resp)) if resp.status().is_success() => {
            if let Ok(pin) = resp.json::<mac_mgmt_common::NixpkgsPin>().await {
                tracing::info!("server nixpkgs pin: {:?}", pin.commit);
                crate::nix::set_nixpkgs_commit(pin.commit);
            }
        }
        Ok(Ok(resp)) => {
            tracing::debug!("nixpkgs pin fetch returned {}", resp.status());
        }
        Ok(Err(e)) => {
            tracing::debug!("nixpkgs pin fetch failed: {e}");
        }
        Err(_) => {
            tracing::debug!("nixpkgs pin fetch timed out");
        }
    }
}

async fn send_heartbeat(
    server_url: &str,
    server_token: &str,
    instance_id: &str,
    host_key: &russh::keys::PrivateKey,
    services: Vec<serde_json::Value>,
    tunnels: Vec<serde_json::Value>,
    relay_proxy_hostname: Option<String>,
) {
    use russh::keys::PublicKeyBase64;
    use russh::keys::signature::Signer;

    let client = reqwest::Client::new();
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_default();

    let signed_at = chrono::Utc::now().timestamp();
    let message = format!("{instance_id}:{signed_at}");
    let sig = host_key.try_sign(message.as_bytes());
    let (public_key_b64, sig_b64) = match sig {
        Ok(sig) => {
            use base64::Engine;
            let pk_b64 = host_key.public_key_base64();
            let sig_b64 = base64::engine::general_purpose::STANDARD.encode(sig.as_bytes());
            (pk_b64, sig_b64)
        }
        Err(e) => {
            tracing::warn!("failed to sign heartbeat: {e}");
            return;
        }
    };

    let body = mac_mgmt_common::HeartbeatBody {
        instance_id: instance_id.to_string(),
        version: CURRENT_VERSION.to_string(),
        hostname,
        environment: ENVIRONMENT.to_string(),
        services: serde_json::Value::Array(services),
        tunnels: serde_json::Value::Array(tunnels),
        relay_proxy_hostname,
        public_key: public_key_b64,
        signature: sig_b64,
        signed_at,
    };

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
