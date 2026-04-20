use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time;

use crate::assessment::{self, Assessor};
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

// ── Daemon state ─────────────────────────────────────────────────────

struct Daemon {
    server_url: Option<String>,
    server_token: Option<String>,
    skills_dir: PathBuf,
    dispatcher: Arc<Dispatcher>,
    metrics: Arc<Metrics>,
    upgrade_window: Option<(chrono::NaiveTime, chrono::NaiveTime)>,
    current_cfg: config::Config,
    instance_id: String,
    host_key: Arc<russh::keys::PrivateKey>,
    assessor: Arc<Assessor>,
    /// `true` until the first successful heartbeat has been ack'd by the
    /// server. Gates the initial `/api/assessment` POST so it lands after
    /// the heartbeat that creates its parent row — the FK added in
    /// migration 031 would otherwise reject the insert.
    initial_assessment_pending: Arc<AtomicBool>,
    #[cfg(feature = "services")]
    svc_mgr: crate::service_mgmt::ServiceManager,
}

impl Daemon {
    fn in_upgrade_window(&self) -> bool {
        self.upgrade_window.map_or(true, |(start, end)| {
            mac_mgmt_common::is_within_window(start, end)
        })
    }

    /// Spawn a background task to sync skills and MCP servers.
    fn spawn_sync_skills_and_mcp(&self) {
        if let (Some(url), Some(token)) = (&self.server_url, &self.server_token) {
            let u = url.clone();
            let t = token.clone();
            let sd = self.skills_dir.clone();
            tokio::spawn(async move {
                if let Err(e) = crate::skills::sync_skills(&u, &t, &sd).await {
                    tracing::warn!("skills sync failed: {e}");
                }
                if let Err(e) = crate::mcp_servers::sync_mcp_servers(&u, &t).await {
                    tracing::warn!("MCP servers sync failed: {e}");
                }
            });
        }
    }

    // ── Event handlers ───────────────────────────────────────────────

    async fn handle_update(&mut self) {
        // Network-heavy operations run in background tasks.
        if let (Some(url), Some(token)) = (&self.server_url, &self.server_token) {
            let u = url.clone();
            let t = token.clone();
            tokio::spawn(async move {
                fetch_target_version(&u, &t).await;
                fetch_nixpkgs_pin(&u, &t).await;
            });
        }

        if self.in_upgrade_window() {
            #[cfg(feature = "self-update")]
            {
                let _ = tokio::time::timeout(
                    std::time::Duration::from_secs(120),
                    tokio::task::spawn_blocking(crate::self_update::check_and_apply),
                )
                .await;
                #[cfg(feature = "services")]
                self.svc_mgr.send_update_self().await;
            }
            #[cfg(not(feature = "sim"))]
            tokio::task::spawn_blocking(upgrade_nix);
        } else {
            tracing::info!("outside upgrade window, skipping upgrades");
        }

        self.spawn_sync_skills_and_mcp();

        if self.in_upgrade_window() {
            #[cfg(all(feature = "services", not(feature = "sim")))]
            self.svc_mgr.check_upgrades();
        }
    }

    async fn handle_config_reload(
        &mut self,
        update_tick: &mut time::Interval,
        health_tick: &mut time::Interval,
        set_log_level: &(dyn Fn(&str) + Send + Sync),
        mut config_poll_tick: Option<&mut time::Interval>,
    ) {
        tracing::info!("reloading config");
        match tokio::time::timeout(std::time::Duration::from_secs(10), crate::config::reload())
            .await
        {
            Err(_) => {
                tracing::warn!("config reload timed out (10s), keeping old config");
            }
            Ok(Err(e)) => {
                tracing::warn!("config reload failed: {e}");
            }
            Ok(Ok(new_cfg)) => {
                if let Err(e) = new_cfg.daemon.validate() {
                    tracing::warn!("new config invalid, keeping old: {e}");
                    return;
                }

                if new_cfg.daemon.update_interval != self.current_cfg.daemon.update_interval {
                    if let Ok(d) = humantime::parse_duration(&new_cfg.daemon.update_interval) {
                        *update_tick = time::interval(d);
                        if let Some(cpt) = config_poll_tick.as_deref_mut() {
                            *cpt = time::interval(d);
                        }
                        tracing::info!(
                            "update_interval changed to {}",
                            new_cfg.daemon.update_interval
                        );
                    }
                }
                if new_cfg.daemon.health_interval != self.current_cfg.daemon.health_interval {
                    if let Ok(d) = humantime::parse_duration(&new_cfg.daemon.health_interval) {
                        *health_tick = time::interval(d);
                        tracing::info!(
                            "health_interval changed to {}",
                            new_cfg.daemon.health_interval
                        );
                    }
                }

                let new_window = new_cfg
                    .daemon
                    .upgrade_window
                    .as_ref()
                    .map(|w| mac_mgmt_common::parse_time_window(w).expect("already validated"));
                if new_window != self.upgrade_window {
                    self.upgrade_window = new_window;
                    tracing::info!("upgrade_window updated");
                }

                self.dispatcher.reconfigure(
                    new_cfg.notifications.urls.clone(),
                    new_cfg.notifications.events.clone(),
                );

                // Schedule service restart for changes that require it.
                let needs_restart = new_cfg.global.default_llm
                    != self.current_cfg.global.default_llm
                    || new_cfg.global.default_agent != self.current_cfg.global.default_agent
                    || format!("{:?}", new_cfg.ollama) != format!("{:?}", self.current_cfg.ollama)
                    || format!("{:?}", new_cfg.openclaw)
                        != format!("{:?}", self.current_cfg.openclaw)
                    || format!("{:?}", new_cfg.opencode)
                        != format!("{:?}", self.current_cfg.opencode);

                if needs_restart {
                    tracing::info!("service config changed, scheduling restart");
                    #[cfg(feature = "services")]
                    self.svc_mgr.schedule_restart().await;
                }
                if new_cfg.metrics.port != self.current_cfg.metrics.port {
                    tracing::warn!(
                        "metrics.port changed \u{2014} daemon restart required to apply"
                    );
                }
                if new_cfg.daemon.log_level != self.current_cfg.daemon.log_level {
                    tracing::info!("log_level changed to {}", new_cfg.daemon.log_level);
                    set_log_level(&new_cfg.daemon.log_level);
                }

                self.assessor.update_config(new_cfg.clone()).await;
                self.current_cfg = new_cfg;
            }
        }
    }

    fn handle_shutdown(&self, signal: &str) {
        tracing::info!("received {signal}, shutting down");
        sentry_ext::breadcrumb("daemon", &format!("{signal} received, shutting down"), &[]);
        self.dispatcher.dispatch(&DaemonEvent::DaemonStopped);
    }

    fn send_heartbeat(
        &self,
        relay_proxy_hostname: Option<String>,
        relay_proxy_url: Option<String>,
    ) {
        if let (Some(url), Some(token)) = (&self.server_url, &self.server_token) {
            #[cfg(feature = "services")]
            let services = self.svc_mgr.collect_statuses();
            #[cfg(not(feature = "services"))]
            let services: Vec<serde_json::Value> = vec![];

            #[cfg(feature = "services")]
            let tunnels: Vec<serde_json::Value> = self
                .svc_mgr
                .collect_tunnels()
                .iter()
                .map(|t| serde_json::json!({ "name": t.name, "port": t.tcp_port }))
                .collect();
            #[cfg(not(feature = "services"))]
            let tunnels: Vec<serde_json::Value> = vec![];

            #[cfg(feature = "services")]
            let file_tunnels: Vec<serde_json::Value> = self
                .svc_mgr
                .collect_file_tunnels()
                .iter()
                .map(|ft| {
                    let mut val = serde_json::json!({
                        "name": ft.name(),
                        "service": ft.service,
                        "path": ft.path(),
                        "writable": ft.writable(),
                        "description": ft.description(),
                    });
                    if let crate::managed_service::FileTunnelDef::Folder { include, .. } = &ft.def {
                        val.as_object_mut()
                            .unwrap()
                            .insert("kind".into(), "directory".into());
                        val.as_object_mut().unwrap().insert(
                            "include".into(),
                            serde_json::to_value(include).unwrap_or(serde_json::Value::Null),
                        );
                    } else {
                        val.as_object_mut()
                            .unwrap()
                            .insert("kind".into(), "file".into());
                    }
                    val
                })
                .collect();
            #[cfg(not(feature = "services"))]
            let file_tunnels: Vec<serde_json::Value> = vec![];

            #[cfg(feature = "services")]
            let shell_tunnels: Vec<serde_json::Value> = self
                .svc_mgr
                .collect_shell_tunnels()
                .iter()
                .map(|st| {
                    serde_json::json!({
                        "name": st.def.name,
                        "service": st.service,
                        "description": st.def.description,
                        "requires_arg": st.def.arg_template.is_some(),
                        "arg_label": st.def.arg_template.as_ref().map(|t| &t.label),
                        "arg_placeholder": st.def.arg_template.as_ref().map(|t| &t.placeholder),
                    })
                })
                .collect();
            #[cfg(not(feature = "services"))]
            let shell_tunnels: Vec<serde_json::Value> = vec![];

            let sample = self.assessor.latest_sample_snapshot();
            let services_extended = self.assessor.latest_probes_snapshot();

            let url = url.clone();
            let token = token.clone();
            let iid = self.instance_id.clone();
            let hk = Arc::clone(&self.host_key);
            let pending = Arc::clone(&self.initial_assessment_pending);
            let assessor = Arc::clone(&self.assessor);
            tokio::spawn(async move {
                let ok = do_send_heartbeat(
                    &url,
                    &token,
                    &iid,
                    &hk,
                    services,
                    tunnels,
                    file_tunnels,
                    shell_tunnels,
                    relay_proxy_hostname,
                    relay_proxy_url,
                    sample,
                    services_extended,
                )
                .await;

                // First successful heartbeat creates the parent row for
                // assessments + probes via migration 031's FK. Only fire
                // the initial inventory send once, and only after we know
                // the parent is there. swap(false) is a compare-and-set
                // so concurrent heartbeat sends at startup don't race.
                if ok && pending.swap(false, Ordering::Relaxed) {
                    assessor.send_inventory(&url, &token, &iid, &hk).await;
                }
            });
        }
    }

    /// Called immediately before each heartbeat. Cheap — just refreshes the cached
    /// dynamic sample so `send_heartbeat` can snapshot it synchronously.
    async fn refresh_assessment_sample(&self) {
        self.assessor.refresh_sample().await;
    }

    /// Fire-and-forget: build and send a full inventory+security assessment.
    fn send_assessment_inventory(&self) {
        if let (Some(url), Some(token)) = (&self.server_url, &self.server_token) {
            let u = url.clone();
            let t = token.clone();
            let iid = self.instance_id.clone();
            let hk = Arc::clone(&self.host_key);
            let assessor = Arc::clone(&self.assessor);
            tokio::spawn(async move {
                assessor.send_inventory(&u, &t, &iid, &hk).await;
            });
        }
    }

    /// Fire-and-forget: run all deep probes.
    fn run_assessment_probes(&self) {
        if let (Some(url), Some(token)) = (&self.server_url, &self.server_token) {
            let u = url.clone();
            let t = token.clone();
            let iid = self.instance_id.clone();
            let hk = Arc::clone(&self.host_key);
            let assessor = Arc::clone(&self.assessor);
            tokio::spawn(async move {
                assessor.run_probes(&u, &t, &iid, &hk).await;
            });
        }
    }

    /// Respond to `PushCommand::RequestAssessment` — runs inventory + probes now.
    fn handle_request_assessment(&self) {
        if let (Some(url), Some(token)) = (&self.server_url, &self.server_token) {
            let u = url.clone();
            let t = token.clone();
            let iid = self.instance_id.clone();
            let hk = Arc::clone(&self.host_key);
            let assessor = Arc::clone(&self.assessor);
            tokio::spawn(async move {
                assessor.request(&u, &t, &iid, &hk).await;
            });
        }
    }

    async fn handle_health_tick(&mut self) {
        #[cfg(feature = "services")]
        if tokio::time::timeout(
            std::time::Duration::from_secs(30),
            self.svc_mgr
                .health_tick(&self.metrics, self.in_upgrade_window()),
        )
        .await
        .is_err()
        {
            tracing::warn!("health tick timed out (30s), continuing");
        }
    }

    /// Handle a push command. Returns true if SSH keys should be synced.
    /// Note: SyncConfig is handled directly in the event loop (needs interval refs).
    async fn handle_push_cmd(&mut self, cmd: crate::server_push::PushCommand) -> bool {
        use crate::server_push::PushCommand;
        match cmd {
            PushCommand::Ping => unreachable!("Ping filtered in SSE parser"),
            PushCommand::SyncConfig => {
                unreachable!("SyncConfig handled in event loop")
            }
            PushCommand::SyncSkills => {
                self.spawn_sync_skills_and_mcp();
                false
            }
            PushCommand::SyncMcpServers => {
                if let (Some(url), Some(token)) = (&self.server_url, &self.server_token) {
                    let u = url.clone();
                    let t = token.clone();
                    tokio::spawn(async move {
                        if let Err(e) = crate::mcp_servers::sync_mcp_servers(&u, &t).await {
                            tracing::warn!("push MCP sync failed: {e}");
                        }
                    });
                }
                false
            }
            PushCommand::SyncSshKeys => true,
            PushCommand::SelfUpdate => {
                tracing::info!("server push: self-update requested");
                if let (Some(url), Some(token)) = (&self.server_url, &self.server_token) {
                    let u = url.clone();
                    let t = token.clone();
                    tokio::spawn(async move {
                        fetch_target_version(&u, &t).await;
                    });
                }
                if self.in_upgrade_window() {
                    #[cfg(feature = "self-update")]
                    {
                        let _ = tokio::time::timeout(
                            std::time::Duration::from_secs(120),
                            tokio::task::spawn_blocking(crate::self_update::check_and_apply),
                        )
                        .await;
                        #[cfg(feature = "services")]
                        self.svc_mgr.send_update_self().await;
                    }
                } else {
                    tracing::info!("outside upgrade window, deferring self-update");
                }
                false
            }
            PushCommand::SyncNixpkgs => {
                tracing::info!("server push: sync nixpkgs pin");
                if let (Some(url), Some(token)) = (&self.server_url, &self.server_token) {
                    let u = url.clone();
                    let t = token.clone();
                    tokio::spawn(async move {
                        fetch_nixpkgs_pin(&u, &t).await;
                    });
                }
                #[cfg(feature = "services")]
                self.svc_mgr.check_upgrades();
                false
            }
            PushCommand::RequestAssessment => {
                tracing::info!("server push: system assessment requested");
                self.handle_request_assessment();
                false
            }
        }
    }

    fn handle_local_sync(&self) {
        tracing::info!("local sync requested");
        self.spawn_sync_skills_and_mcp();
    }

    #[cfg(all(feature = "services", feature = "relay"))]
    fn update_relay_tunnel_defs(&self, relay_mgr: &crate::remote_ssh::Manager) {
        let td = self.svc_mgr.collect_tunnels();
        relay_mgr.update_tunnel_defs(td);
    }

    #[cfg(all(feature = "services", feature = "relay"))]
    fn update_relay_file_tunnel_defs(&self, relay_mgr: &crate::remote_ssh::Manager) {
        let fd = self.svc_mgr.collect_file_tunnels();
        relay_mgr.update_file_tunnel_defs(fd);
    }

    #[cfg(all(feature = "services", feature = "relay"))]
    fn update_relay_shell_tunnel_defs(&self, relay_mgr: &crate::remote_ssh::Manager) {
        let sd = self.svc_mgr.collect_shell_tunnels();
        relay_mgr.update_shell_tunnel_defs(sd);
    }

    #[cfg(feature = "services")]
    fn update_relay_config(&mut self, proxy_hostname: Option<String>) {
        if let Some(ph) = proxy_hostname {
            self.svc_mgr.config_store.set(
                "relay",
                serde_json::json!({
                    "proxy_hostname": ph,
                    "instance_id_prefix": &self.instance_id[..12],
                }),
            );
        }
    }

    #[cfg(feature = "services")]
    async fn shutdown(&mut self) {
        self.svc_mgr.shutdown().await;
    }
}

// ── Entry point ──────────────────────────────────────────────────────

pub async fn run(
    log_buf: crate::log_buffer::LogBuffer,
    set_log_level: Box<dyn Fn(&str) + Send + Sync>,
) -> Result<()> {
    // Acquire lockfile to ensure only one daemon instance runs at a time.
    let lock_path = config::config_dir().join("daemon.lock");
    std::fs::create_dir_all(lock_path.parent().unwrap()).ok();
    let lock_file = std::fs::File::create(&lock_path).context("failed to create lockfile")?;
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

    sentry_ext::set_tag("environment", ENVIRONMENT);
    sentry_ext::set_tag("target", TARGET);
    sentry_ext::breadcrumb(
        "daemon",
        "daemon started",
        &[
            ("version", CURRENT_VERSION),
            ("environment", ENVIRONMENT),
            ("target", TARGET),
        ],
    );

    // Migration: remove legacy UUID-based instance-id file.
    let legacy_id_path = config::config_dir().join("instance-id");
    if legacy_id_path.exists() {
        if let Err(e) = std::fs::remove_file(&legacy_id_path) {
            tracing::warn!("failed to remove legacy instance-id file: {e}");
        } else {
            tracing::info!("removed legacy instance-id file");
        }
    }

    let mut cfg = config::load().await?;
    let current_cfg = cfg.clone();

    let update_interval = humantime::parse_duration(&cfg.daemon.update_interval)
        .context("invalid update_interval")?;
    let health_interval = humantime::parse_duration(&cfg.daemon.health_interval)
        .context("invalid health_interval")?;

    let upgrade_window =
        cfg.daemon.upgrade_window.as_ref().map(|w| {
            mac_mgmt_common::parse_time_window(w).expect("upgrade_window already validated")
        });

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

    // If nix fell off the profile (botched upgrade, manual removal, etc),
    // try to recover from a store binary before anything else runs.
    crate::nix::ensure_nix_on_path();

    // Fetch the cluster's nixpkgs pin before ServiceManager::init runs.
    if let (Some(url), Some(token)) = (&server_url, &server_token) {
        fetch_nixpkgs_pin(url, token).await;
    }

    // When the unmanaged marker exists, services are run by
    // systemd/launchd (installed via `mac-mgmt install-services`).
    // Neuter the daemon's service manager by clearing the providers so
    // it builds zero services — heartbeat, relay, self-update etc.
    // still run normally.
    #[cfg(feature = "services")]
    {
        let unmanaged_marker = crate::config::config_dir().join(".unmanaged");
        if unmanaged_marker.exists() {
            tracing::info!(
                "unmanaged mode active — services managed externally; \
                 daemon will not spawn or monitor them"
            );
            cfg.global.default_llm = mac_mgmt_common::LlmProvider::None;
            cfg.global.default_agent = mac_mgmt_common::AgentProvider::None;
        }
    }

    #[cfg(feature = "services")]
    let mut svc_mgr = crate::service_mgmt::ServiceManager::init(
        &mut cfg,
        Arc::clone(&dispatcher),
        log_buf.clone(),
    )?;

    #[cfg(not(feature = "services"))]
    tracing::info!("services feature disabled, skipping service management");

    let metrics = Arc::new(Metrics::new());

    #[cfg(feature = "services")]
    svc_mgr.register_metrics(&metrics);

    // Channel for local sync requests (e.g. from `mac-mgmt sync` via /sync).
    let (sync_tx, mut sync_rx) = tokio::sync::mpsc::channel::<()>(4);
    let _sync_tx_keepalive = sync_tx.clone();

    let assessor = Arc::new(Assessor::new());
    assessor.update_config(current_cfg.clone()).await;
    assessor.attach_metrics(Arc::clone(&metrics)).await;

    // Spawn the metrics server.
    let metrics_clone = Arc::clone(&metrics);
    let log_buf_clone = log_buf.clone();
    let sync_tx_clone = sync_tx.clone();
    let assessor_clone = Arc::clone(&assessor);
    tokio::spawn(async move {
        if let Err(e) = crate::metrics_server::build_rocket(
            metrics_clone,
            log_buf_clone,
            sync_tx_clone,
            assessor_clone,
            metrics_port,
        )
        .launch()
        .await
        {
            tracing::error!("metrics server failed: {e}");
            sentry_ext::capture_error(&format!("metrics server failed: {e}"), &[]);
        }
    });
    tracing::info!("metrics server started on port {metrics_port}");

    let mut update_tick = time::interval(update_interval);
    let mut health_tick = time::interval(health_interval);
    let mut heartbeat_tick = time::interval(health_interval);
    // Poll remote config on the same cadence as updates — catches server-side
    // config changes even when the SSE SyncConfig push is missed or unavailable.
    let mut config_poll_tick = time::interval(update_interval);
    let mut assessment_inventory_tick = time::interval(assessment::DEFAULT_INVENTORY_INTERVAL);
    let mut assessment_probe_tick = time::interval(assessment::jittered(
        assessment::DEFAULT_PROBE_INTERVAL,
        120,
    ));

    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .context("failed to register SIGTERM handler")?;
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .context("failed to register SIGINT handler")?;

    // Now that signal handlers are registered, connect to the supervisor
    // and register every service.
    #[cfg(feature = "services")]
    svc_mgr.connect_all().await;

    // Set up config file watcher.
    let (config_tx, mut config_rx) = tokio::sync::mpsc::channel(4);
    let _config_tx_keepalive = config_tx.clone();
    let _config_watcher = match crate::config_watch::watch(&crate::config::config_path(), config_tx)
    {
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
    let host_key = Arc::new(
        crate::host_keys::load_or_generate().context("failed to load/generate SSH host key")?,
    );
    let instance_id = crate::host_keys::fingerprint_hex(&host_key);
    tracing::info!("instance ID (host key fingerprint): {instance_id}");

    #[cfg(feature = "relay")]
    let (mut relay_mgr, mut relay_heartbeat_rx) = crate::remote_ssh::Manager::new(
        cfg.relay.url.clone(),
        server_url.clone(),
        server_token.clone(),
        instance_id.clone(),
        Arc::clone(&host_key),
        metrics_port,
        cfg.relay.remote_ssh_enabled,
    );

    // Start server push WebSocket if server is configured.
    let mut push_rx = if let (Some(url), Some(token)) = (&server_url, &server_token) {
        let (_handle, rx) = crate::server_push::start(url, token);
        Some(rx)
    } else {
        None
    };

    // Build the Daemon struct with all long-lived state.
    let mut daemon = Daemon {
        server_url,
        server_token,
        skills_dir: skills_dir.clone(),
        dispatcher,
        metrics,
        upgrade_window,
        current_cfg,
        instance_id,
        host_key,
        assessor,
        initial_assessment_pending: Arc::new(AtomicBool::new(true)),
        #[cfg(feature = "services")]
        svc_mgr,
    };

    // Run startup sync in background.
    if let (Some(url), Some(token)) = (&daemon.server_url, &daemon.server_token) {
        let u = url.clone();
        let t = token.clone();
        tokio::spawn(async move {
            fetch_target_version(&u, &t).await;
        });
    }

    #[cfg(feature = "self-update")]
    if daemon.in_upgrade_window() {
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(120),
            tokio::task::spawn_blocking(crate::self_update::check_and_apply),
        )
        .await;
        #[cfg(feature = "services")]
        daemon.svc_mgr.send_update_self().await;
    } else {
        tracing::info!("outside upgrade window, skipping initial self-update");
    }

    daemon.spawn_sync_skills_and_mcp();

    // The initial assessment snapshot is emitted by send_heartbeat() once
    // the first 2xx ack lands — the heartbeat INSERT creates the parent
    // row that migration 031's FK requires on the assessments insert.

    #[cfg(feature = "relay")]
    relay_mgr.sync_ssh_keys();

    // Helper macros purely for cfg-gated relay proxy access.
    macro_rules! relay_proxy_hostname {
        () => {{
            #[cfg(feature = "relay")]
            {
                relay_mgr.relay_proxy_hostname()
            }
            #[cfg(not(feature = "relay"))]
            {
                None::<String>
            }
        }};
    }
    macro_rules! relay_proxy_url {
        () => {{
            #[cfg(feature = "relay")]
            {
                relay_mgr.relay_proxy_url()
            }
            #[cfg(not(feature = "relay"))]
            {
                None::<String>
            }
        }};
    }

    // ── Main event loop ──────────────────────────────────────────────

    loop {
        tokio::select! {
            _ = sigterm.recv() => { daemon.handle_shutdown("SIGTERM"); break; }
            _ = sigint.recv() => { daemon.handle_shutdown("SIGINT"); break; }

            _ = update_tick.tick() => {
                daemon.handle_update().await;
                #[cfg(feature = "relay")]
                relay_mgr.sync_ssh_keys();
            }

            _ = health_tick.tick() => {
                daemon.handle_health_tick().await;
                #[cfg(all(feature = "services", feature = "relay"))]
                daemon.update_relay_tunnel_defs(&relay_mgr);
                #[cfg(all(feature = "services", feature = "relay"))]
                daemon.update_relay_file_tunnel_defs(&relay_mgr);
                #[cfg(all(feature = "services", feature = "relay"))]
                daemon.update_relay_shell_tunnel_defs(&relay_mgr);
                #[cfg(feature = "services")]
                daemon.update_relay_config(relay_proxy_hostname!());
                daemon.refresh_assessment_sample().await;
                // Send heartbeat BEFORE connectors — connectors run blocking
                // CLI commands that can hang for minutes.
                daemon.send_heartbeat(relay_proxy_hostname!(), relay_proxy_url!());
                #[cfg(feature = "services")]
                tokio::task::block_in_place(|| daemon.svc_mgr.run_connectors_tick());
            }

            _ = heartbeat_tick.tick() => {
                daemon.refresh_assessment_sample().await;
                daemon.send_heartbeat(relay_proxy_hostname!(), relay_proxy_url!());
            }

            _ = assessment_inventory_tick.tick() => {
                daemon.send_assessment_inventory();
            }

            _ = assessment_probe_tick.tick() => {
                daemon.run_assessment_probes();
            }

            _ = crate::config_watch::recv_debounced(&mut config_rx) => {
                daemon.handle_config_reload(&mut update_tick, &mut health_tick, &*set_log_level, Some(&mut config_poll_tick)).await;
            }

            _ = config_poll_tick.tick() => {
                daemon.handle_config_reload(&mut update_tick, &mut health_tick, &*set_log_level, Some(&mut config_poll_tick)).await;
            }

            Some(cmd) = async {
                if let Some(rx) = &mut push_rx { rx.recv().await } else { std::future::pending().await }
            } => {
                // SyncConfig needs special handling (needs interval refs).
                if matches!(cmd, crate::server_push::PushCommand::SyncConfig) {
                    tracing::info!("server push: sync config");
                    daemon.handle_config_reload(&mut update_tick, &mut health_tick, &*set_log_level, Some(&mut config_poll_tick)).await;
                } else {
                    let needs_ssh_sync = daemon.handle_push_cmd(cmd).await;
                    #[cfg(feature = "relay")]
                    if needs_ssh_sync { relay_mgr.sync_ssh_keys(); }
                    #[cfg(not(feature = "relay"))]
                    let _ = needs_ssh_sync;
                }
            }

            Some(()) = sync_rx.recv() => {
                daemon.handle_local_sync();
                #[cfg(feature = "relay")]
                relay_mgr.sync_ssh_keys();
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

            Some(()) = async {
                #[cfg(feature = "relay")]
                { relay_heartbeat_rx.recv().await }
                #[cfg(not(feature = "relay"))]
                { std::future::pending::<Option<()>>().await }
            } => {
                tracing::debug!("relay signalled heartbeat");
                daemon.send_heartbeat(relay_proxy_hostname!(), relay_proxy_url!());
            }
        }
    }

    #[cfg(feature = "services")]
    daemon.shutdown().await;

    #[cfg(feature = "relay")]
    relay_mgr.cleanup();

    sentry_ext::breadcrumb("daemon", "daemon shutdown complete", &[]);
    tracing::info!("daemon shutdown complete");

    Ok(())
}

// ── Free functions ───────────────────────────────────────────────────

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

/// Fetch the cluster's nixpkgs commit pin from the server and apply it.
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

/// Returns `true` when the POST lands with a 2xx. Used by the caller to
/// gate one-shot follow-up work (e.g. the initial assessment inventory
/// send, which would otherwise race the heartbeat that creates its FK
/// parent row — see migration 031).
async fn do_send_heartbeat(
    server_url: &str,
    server_token: &str,
    instance_id: &str,
    host_key: &russh::keys::PrivateKey,
    services: Vec<serde_json::Value>,
    tunnels: Vec<serde_json::Value>,
    file_tunnels: Vec<serde_json::Value>,
    shell_tunnels: Vec<serde_json::Value>,
    relay_proxy_hostname: Option<String>,
    relay_proxy_url: Option<String>,
    sample: Option<mac_mgmt_common::DynamicSample>,
    services_extended: Vec<mac_mgmt_common::ServiceExtState>,
) -> bool {
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
            return false;
        }
    };

    let svc_count = services.len();
    let tunnel_count = tunnels.len();

    let body = mac_mgmt_common::HeartbeatBody {
        instance_id: instance_id.to_string(),
        version: CURRENT_VERSION.to_string(),
        hostname: hostname.clone(),
        environment: ENVIRONMENT.to_string(),
        services: serde_json::Value::Array(services),
        tunnels: serde_json::Value::Array(tunnels),
        file_tunnels: serde_json::Value::Array(file_tunnels),
        shell_tunnels: serde_json::Value::Array(shell_tunnels),
        relay_proxy_hostname,
        relay_proxy_url,
        nixpkgs_commit: crate::nix::current_nixpkgs_commit(),
        git_sha: Some(crate::GIT_SHA.to_string()),
        public_key: public_key_b64,
        signature: sig_b64,
        signed_at,
        sample,
        services_extended,
    };

    let url = format!("{server_url}/api/heartbeat");
    tracing::debug!(
        "heartbeat → {url} instance={instance_id} host={hostname} signed_at={signed_at} services={svc_count} tunnels={tunnel_count}"
    );
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
            tracing::debug!("heartbeat accepted");
            true
        }
        Ok(Ok(resp)) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            tracing::warn!("heartbeat rejected: {status} — {body}");
            false
        }
        Ok(Err(e)) => {
            tracing::warn!("heartbeat failed: {e}");
            false
        }
        Err(_) => {
            tracing::warn!("heartbeat timed out");
            false
        }
    }
}

#[cfg(not(feature = "sim"))]
fn upgrade_nix() {
    tracing::info!("checking for nix upgrade");
    if let Err(e) = crate::nix::upgrade_nix() {
        tracing::warn!("nix upgrade failed: {e}");
        sentry_ext::capture_error(&format!("nix upgrade failed: {e}"), &[]);
    }
}

// ── Simulation entrypoint ───────────────────────────────────────────

/// Entrypoint for simulation testing.
///
/// Accepts a pre-built config and host key, skipping lockfile, sentry,
/// signal handlers, metrics server, config watcher, and real filesystem I/O.
/// Runs the same core event loop as production `run()`.
///
/// When the `services` feature is enabled, creates a minimal ServiceManager
/// with zero services (no nix calls, no supervisor connection).
/// When the `relay` feature is enabled, spawns the relay manager if a
/// relay URL is configured in the config.
#[cfg(feature = "sim")]
pub async fn run_sim(
    cfg: config::Config,
    host_key: Arc<russh::keys::PrivateKey>,
    mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) -> Result<()> {
    let current_cfg = cfg.clone();

    let update_interval = humantime::parse_duration(&cfg.daemon.update_interval)
        .context("invalid update_interval")?;
    let health_interval = humantime::parse_duration(&cfg.daemon.health_interval)
        .context("invalid health_interval")?;

    let upgrade_window =
        cfg.daemon.upgrade_window.as_ref().map(|w| {
            mac_mgmt_common::parse_time_window(w).expect("upgrade_window already validated")
        });

    let metrics_port = cfg.metrics.port;
    let server_url = cfg.server.url.clone();
    let server_token = cfg.server.token.clone();
    let skills_dir = std::path::PathBuf::from("/tmp/sim-skills");

    let dispatcher = Arc::new(Dispatcher::new(Vec::new(), None));

    // Services: create a minimal ServiceManager with zero services.
    #[cfg(feature = "services")]
    let mut svc_mgr = crate::service_mgmt::ServiceManager::sim_init(
        Arc::clone(&dispatcher),
        crate::log_buffer::LogBuffer::new(),
    );

    let metrics = Arc::new(Metrics::new());

    #[cfg(feature = "services")]
    svc_mgr.register_metrics(&metrics);

    let (_sync_tx, mut sync_rx) = tokio::sync::mpsc::channel::<()>(4);

    let mut update_tick = time::interval(update_interval);
    let mut health_tick = time::interval(health_interval);
    let mut heartbeat_tick = time::interval(health_interval);
    let mut assessment_inventory_tick = time::interval(assessment::DEFAULT_INVENTORY_INTERVAL);
    let mut assessment_probe_tick = time::interval(assessment::jittered(
        assessment::DEFAULT_PROBE_INTERVAL,
        120,
    ));

    let instance_id = crate::host_keys::fingerprint_hex(&host_key);
    tracing::info!("sim daemon started, instance ID: {instance_id}");

    // Relay: spawn relay manager if relay URL is configured.
    #[cfg(feature = "relay")]
    let (mut relay_mgr, mut relay_heartbeat_rx) = crate::remote_ssh::Manager::new(
        cfg.relay.url.clone(),
        server_url.clone(),
        server_token.clone(),
        instance_id.clone(),
        Arc::clone(&host_key),
        metrics_port,
        cfg.relay.remote_ssh_enabled,
    );

    // Start server push SSE if server is configured.
    let mut push_rx = if let (Some(url), Some(token)) = (&server_url, &server_token) {
        let (_handle, rx) = crate::server_push::start(url, token);
        Some(rx)
    } else {
        None
    };

    let assessor = Arc::new(Assessor::new());
    assessor.update_config(current_cfg.clone()).await;
    assessor.attach_metrics(Arc::clone(&metrics)).await;

    let mut daemon = Daemon {
        server_url,
        server_token,
        skills_dir,
        dispatcher,
        metrics,
        upgrade_window,
        current_cfg,
        instance_id,
        host_key,
        assessor,
        initial_assessment_pending: Arc::new(AtomicBool::new(true)),
        #[cfg(feature = "services")]
        svc_mgr,
    };

    // Run startup sync in background.
    if let (Some(url), Some(token)) = (&daemon.server_url, &daemon.server_token) {
        let u = url.clone();
        let t = token.clone();
        tokio::spawn(async move {
            fetch_target_version(&u, &t).await;
        });
    }

    daemon.spawn_sync_skills_and_mcp();

    #[cfg(feature = "relay")]
    relay_mgr.sync_ssh_keys();

    // Re-use the relay macros from the production event loop.
    macro_rules! relay_proxy_hostname {
        () => {{
            #[cfg(feature = "relay")]
            {
                relay_mgr.relay_proxy_hostname()
            }
            #[cfg(not(feature = "relay"))]
            {
                None::<String>
            }
        }};
    }
    macro_rules! relay_proxy_url {
        () => {{
            #[cfg(feature = "relay")]
            {
                relay_mgr.relay_proxy_url()
            }
            #[cfg(not(feature = "relay"))]
            {
                None::<String>
            }
        }};
    }

    // ── Main event loop (sim) ───────────────────────────────────────
    loop {
        tokio::select! {
            _ = &mut shutdown_rx => {
                tracing::info!("sim shutdown signal received");
                break;
            }

            _ = update_tick.tick() => {
                daemon.handle_update().await;
                #[cfg(feature = "relay")]
                relay_mgr.sync_ssh_keys();
            }

            _ = health_tick.tick() => {
                daemon.handle_health_tick().await;
                #[cfg(all(feature = "services", feature = "relay"))]
                daemon.update_relay_tunnel_defs(&relay_mgr);
                #[cfg(all(feature = "services", feature = "relay"))]
                daemon.update_relay_file_tunnel_defs(&relay_mgr);
                #[cfg(all(feature = "services", feature = "relay"))]
                daemon.update_relay_shell_tunnel_defs(&relay_mgr);
                #[cfg(feature = "services")]
                daemon.update_relay_config(relay_proxy_hostname!());
                daemon.refresh_assessment_sample().await;
                daemon.send_heartbeat(relay_proxy_hostname!(), relay_proxy_url!());
                // In sim mode, call connectors directly (block_in_place panics
                // on current_thread runtime used by #[tokio::test]).
                #[cfg(feature = "services")]
                daemon.svc_mgr.run_connectors_tick();
            }

            _ = heartbeat_tick.tick() => {
                daemon.refresh_assessment_sample().await;
                daemon.send_heartbeat(relay_proxy_hostname!(), relay_proxy_url!());
            }

            _ = assessment_inventory_tick.tick() => {
                daemon.send_assessment_inventory();
            }

            _ = assessment_probe_tick.tick() => {
                daemon.run_assessment_probes();
            }

            Some(cmd) = async {
                if let Some(rx) = &mut push_rx { rx.recv().await } else { std::future::pending().await }
            } => {
                if matches!(cmd, crate::server_push::PushCommand::SyncConfig) {
                    tracing::info!("server push: sync config");
                    daemon.handle_config_reload(&mut update_tick, &mut health_tick, &|_| {}, None).await;
                } else {
                    let needs_ssh_sync = daemon.handle_push_cmd(cmd).await;
                    #[cfg(feature = "relay")]
                    if needs_ssh_sync { relay_mgr.sync_ssh_keys(); }
                    #[cfg(not(feature = "relay"))]
                    let _ = needs_ssh_sync;
                }
            }

            Some(()) = sync_rx.recv() => {
                daemon.handle_local_sync();
                #[cfg(feature = "relay")]
                relay_mgr.sync_ssh_keys();
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

            Some(()) = async {
                #[cfg(feature = "relay")]
                { relay_heartbeat_rx.recv().await }
                #[cfg(not(feature = "relay"))]
                { std::future::pending::<Option<()>>().await }
            } => {
                tracing::debug!("relay signalled heartbeat");
                daemon.send_heartbeat(relay_proxy_hostname!(), relay_proxy_url!());
            }
        }
    }

    #[cfg(feature = "services")]
    daemon.shutdown().await;

    tracing::info!("sim daemon shutdown complete");
    Ok(())
}

/// Simulation entrypoint with mock services and an in-process supervisor.
///
/// Like `run_sim()` but creates a ServiceManager backed by a real supervisor
/// (running as a tokio task) and registers the provided mock services with it.
/// Used for testing the supervisor lifecycle, health checking, and tunnel
/// advertisement under fault injection.
#[cfg(all(feature = "sim", feature = "services"))]
pub async fn run_sim_with_services(
    cfg: config::Config,
    host_key: Arc<russh::keys::PrivateKey>,
    mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    mock_services: Vec<Box<dyn crate::managed_service::ManagedService>>,
) -> Result<()> {
    let current_cfg = cfg.clone();

    let update_interval = humantime::parse_duration(&cfg.daemon.update_interval)
        .context("invalid update_interval")?;
    let health_interval = humantime::parse_duration(&cfg.daemon.health_interval)
        .context("invalid health_interval")?;

    let upgrade_window =
        cfg.daemon.upgrade_window.as_ref().map(|w| {
            mac_mgmt_common::parse_time_window(w).expect("upgrade_window already validated")
        });

    let metrics_port = cfg.metrics.port;
    let server_url = cfg.server.url.clone();
    let server_token = cfg.server.token.clone();
    let skills_dir = std::path::PathBuf::from("/tmp/sim-skills");

    let dispatcher = Arc::new(Dispatcher::new(Vec::new(), None));
    let log_buf = crate::log_buffer::LogBuffer::new();

    let mut svc_mgr = crate::service_mgmt::ServiceManager::sim_init_with_supervisor(
        Arc::clone(&dispatcher),
        log_buf.clone(),
        mock_services,
    );

    // Let the in-process supervisor bind its socket.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let metrics = Arc::new(Metrics::new());
    svc_mgr.register_metrics(&metrics);

    let (_sync_tx, mut sync_rx) = tokio::sync::mpsc::channel::<()>(4);

    let mut update_tick = time::interval(update_interval);
    let mut health_tick = time::interval(health_interval);
    let mut heartbeat_tick = time::interval(health_interval);
    let mut assessment_inventory_tick = time::interval(assessment::DEFAULT_INVENTORY_INTERVAL);
    let mut assessment_probe_tick = time::interval(assessment::jittered(
        assessment::DEFAULT_PROBE_INTERVAL,
        120,
    ));

    let instance_id = crate::host_keys::fingerprint_hex(&host_key);
    tracing::info!("sim daemon (with supervisor) started, instance ID: {instance_id}");

    #[cfg(feature = "relay")]
    let (mut relay_mgr, mut relay_heartbeat_rx) = crate::remote_ssh::Manager::new(
        cfg.relay.url.clone(),
        server_url.clone(),
        server_token.clone(),
        instance_id.clone(),
        Arc::clone(&host_key),
        metrics_port,
        cfg.relay.remote_ssh_enabled,
    );

    let mut push_rx = if let (Some(url), Some(token)) = (&server_url, &server_token) {
        let (_handle, rx) = crate::server_push::start(url, token);
        Some(rx)
    } else {
        None
    };

    let assessor = Arc::new(Assessor::new());
    assessor.update_config(current_cfg.clone()).await;
    assessor.attach_metrics(Arc::clone(&metrics)).await;

    // Connect to supervisor and register services.
    svc_mgr.connect_all().await;

    macro_rules! relay_proxy_hostname {
        () => {{
            #[cfg(feature = "relay")]
            {
                relay_mgr.relay_proxy_hostname()
            }
            #[cfg(not(feature = "relay"))]
            {
                None::<String>
            }
        }};
    }
    macro_rules! relay_proxy_url {
        () => {{
            #[cfg(feature = "relay")]
            {
                relay_mgr.relay_proxy_url()
            }
            #[cfg(not(feature = "relay"))]
            {
                None::<String>
            }
        }};
    }

    let mut daemon = Daemon {
        server_url,
        server_token,
        skills_dir,
        dispatcher,
        metrics,
        upgrade_window,
        current_cfg,
        instance_id,
        host_key,
        assessor,
        initial_assessment_pending: Arc::new(AtomicBool::new(true)),
        svc_mgr,
    };

    daemon.spawn_sync_skills_and_mcp();

    #[cfg(feature = "relay")]
    relay_mgr.sync_ssh_keys();

    // ── Main event loop (sim with supervisor) ───────────────────────
    loop {
        tokio::select! {
            _ = &mut shutdown_rx => {
                tracing::info!("sim shutdown signal received");
                break;
            }

            _ = update_tick.tick() => {
                daemon.handle_update().await;
                #[cfg(feature = "relay")]
                relay_mgr.sync_ssh_keys();
            }

            _ = health_tick.tick() => {
                daemon.handle_health_tick().await;
                #[cfg(feature = "relay")]
                daemon.update_relay_tunnel_defs(&relay_mgr);
                #[cfg(feature = "relay")]
                daemon.update_relay_file_tunnel_defs(&relay_mgr);
                #[cfg(feature = "relay")]
                daemon.update_relay_shell_tunnel_defs(&relay_mgr);
                daemon.update_relay_config(relay_proxy_hostname!());
                daemon.refresh_assessment_sample().await;
                daemon.send_heartbeat(relay_proxy_hostname!(), relay_proxy_url!());
                daemon.svc_mgr.run_connectors_tick();
            }

            _ = heartbeat_tick.tick() => {
                daemon.refresh_assessment_sample().await;
                daemon.send_heartbeat(relay_proxy_hostname!(), relay_proxy_url!());
            }

            _ = assessment_inventory_tick.tick() => {
                daemon.send_assessment_inventory();
            }

            _ = assessment_probe_tick.tick() => {
                daemon.run_assessment_probes();
            }

            Some(cmd) = async {
                if let Some(rx) = &mut push_rx { rx.recv().await } else { std::future::pending().await }
            } => {
                if matches!(cmd, crate::server_push::PushCommand::SyncConfig) {
                    tracing::info!("server push: sync config");
                    daemon.handle_config_reload(&mut update_tick, &mut health_tick, &|_| {}, None).await;
                } else {
                    let needs_ssh_sync = daemon.handle_push_cmd(cmd).await;
                    #[cfg(feature = "relay")]
                    if needs_ssh_sync { relay_mgr.sync_ssh_keys(); }
                    #[cfg(not(feature = "relay"))]
                    let _ = needs_ssh_sync;
                }
            }

            Some(()) = sync_rx.recv() => {
                daemon.handle_local_sync();
                #[cfg(feature = "relay")]
                relay_mgr.sync_ssh_keys();
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

            Some(()) = async {
                #[cfg(feature = "relay")]
                { relay_heartbeat_rx.recv().await }
                #[cfg(not(feature = "relay"))]
                { std::future::pending::<Option<()>>().await }
            } => {
                tracing::debug!("relay signalled heartbeat");
                daemon.send_heartbeat(relay_proxy_hostname!(), relay_proxy_url!());
            }
        }
    }

    daemon.shutdown().await;
    tracing::info!("sim daemon (with supervisor) shutdown complete");
    Ok(())
}
