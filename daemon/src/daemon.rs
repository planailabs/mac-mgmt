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
    #[cfg(feature = "services")]
    ai_proxy_handle: Option<crate::ai_proxy::AiProxyHandle>,
    #[cfg(feature = "healer")]
    healer: Arc<mac_mgmt_healer::HealerState>,
    #[cfg(feature = "healer")]
    healer_unhealthy_counter: u32,
    #[cfg(feature = "memvault")]
    memvault: Option<crate::memvault::MemvaultHandle>,
    /// Skip sending update-self to the supervisor on the first update tick.
    /// The daemon already ran check_and_apply during startup; telling the
    /// supervisor to reexec before the daemon itself has restarted is
    /// premature and causes a needless supervisor cycle.
    first_update: bool,
    /// Serializes heartbeat POSTs so a faster-arriving later send can't
    /// overtake an earlier one and leave the server with a stale snapshot.
    heartbeat_lock: Arc<tokio::sync::Mutex<()>>,
}

impl Daemon {
    fn in_upgrade_window(&self) -> bool {
        self.upgrade_window.map_or(true, |(start, end)| {
            mac_mgmt_common::is_within_window(start, end)
        })
    }

    #[cfg(feature = "services")]
    fn upgrade_gate(&self) -> crate::service_mgmt::UpgradeGate {
        use crate::service_mgmt::UpgradeGate;
        match self.upgrade_window {
            None => UpgradeGate::Anytime,
            Some((start, end)) if mac_mgmt_common::is_within_window(start, end) => {
                UpgradeGate::InWindow
            }
            Some(_) => UpgradeGate::Deferred,
        }
    }

    /// Spawn a background task to sync skills, MCP servers, and packages.
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
                if let Err(e) = crate::packages::sync_packages(&u, &t).await {
                    tracing::warn!("package sync failed: {e}");
                }
            });
        }
    }

    // ── Event handlers ───────────────────────────────────────────────

    async fn handle_update(&mut self) {
        if let (Some(url), Some(token)) = (&self.server_url, &self.server_token) {
            // Await all fetches so check_upgrades sees the latest pin + caches.
            fetch_target_version(url, token).await;
            fetch_nixpkgs_pin(url, token).await;
            fetch_nix_caches(url, token).await;
        }

        if self.in_upgrade_window() {
            #[cfg(feature = "self-update")]
            {
                let _ = tokio::time::timeout(
                    std::time::Duration::from_secs(120),
                    tokio::task::spawn_blocking(crate::self_update::check_and_apply),
                )
                .await;
                // Tell the supervisor to reexec so it picks up the new binary too.
                // Children survive the reexec — they're reparented seamlessly.
                // Skip on the first tick: the daemon itself hasn't restarted yet,
                // so asking the supervisor to reexec now is premature.
                #[cfg(feature = "services")]
                if !self.first_update {
                    self.svc_mgr.send_update_self().await;
                }
            }
            #[cfg(not(feature = "sim"))]
            tokio::task::spawn_blocking(upgrade_nix);
        } else {
            tracing::info!("outside upgrade window, skipping upgrades");
        }

        self.spawn_sync_skills_and_mcp();

        // Refresh cached JSON schemas so updated schemas are picked up
        // without a daemon restart.
        #[cfg(feature = "services")]
        self.refresh_schemas().await;

        if self.in_upgrade_window() {
            #[cfg(all(feature = "services", not(feature = "sim")))]
            self.svc_mgr.check_upgrades();
        }

        self.first_update = false;
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
            Ok(Ok(mut new_cfg)) => {
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
                    || new_cfg.ollama != self.current_cfg.ollama
                    || new_cfg.openclaw != self.current_cfg.openclaw
                    || new_cfg.opencode != self.current_cfg.opencode
                    || new_cfg.lms != self.current_cfg.lms
                    || new_cfg.cloud != self.current_cfg.cloud;

                // Update config store and rebuild connectors first so
                // pre-start connectors patch config files before services
                // are restarted / hot-reloaded.
                #[cfg(feature = "services")]
                self.svc_mgr.reload_connectors(&new_cfg);
                #[cfg(feature = "services")]
                self.svc_mgr.retry_failed_installs();

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

                // Reload AI proxy config if running.
                #[cfg(feature = "services")]
                if let Some(ref handle) = self.ai_proxy_handle {
                    handle
                        .reload_config(&new_cfg.ai_proxy, &new_cfg.ollama, &new_cfg.unsloth)
                        .await;
                }

                // Preserve the runtime-only probe token across config reloads.
                #[cfg(feature = "services")]
                {
                    new_cfg.ai_proxy.probe_token = self.current_cfg.ai_proxy.probe_token.clone();
                }

                self.assessor.update_config(new_cfg.clone()).await;
                self.current_cfg = new_cfg;
            }
        }
    }

    /// Run a restic backup cycle: backup → forget+prune.
    #[cfg(feature = "services")]
    async fn handle_backup(&self) {
        use crate::services::restic::Restic;
        let cfg = &self.current_cfg.backup;
        if !cfg.enabled || cfg.repository.is_empty() {
            return;
        }

        let includes_path = config::config_dir().join("restic-includes.txt");
        if !includes_path.exists() {
            tracing::warn!("backup: restic-includes.txt not found, skipping");
            return;
        }

        let password_file = Restic::password_file_path(cfg);
        if !password_file.exists() {
            tracing::warn!(
                "backup: password file not found at {}, skipping",
                password_file.display()
            );
            return;
        }

        let repo = cfg.repository.clone();
        let pw = password_file.to_string_lossy().into_owned();
        let inc = includes_path.to_string_lossy().into_owned();
        let excludes: Vec<String> = cfg.exclude.clone();
        let keep_within = cfg.keep_within.clone();
        let env: std::collections::HashMap<String, String> = cfg.env.clone();
        let dispatcher = self.dispatcher.clone();

        tracing::info!("backup: starting restic backup");
        sentry_ext::breadcrumb("backup", "starting restic backup", &[("repository", &repo)]);

        let start = std::time::Instant::now();

        // Run in a blocking task — restic can take a long time.
        let result = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            use std::process::Command;

            // 1. Backup
            let mut cmd = Command::new("restic");
            cmd.args(["backup", "--files-from", &inc])
                .args(["--repo", &repo])
                .args(["--password-file", &pw])
                .args(["--json"])
                .envs(&env);
            for pattern in &excludes {
                cmd.args(["--exclude", pattern]);
            }

            let backup_timeout = std::time::Duration::from_secs(3600);
            let output = crate::cmd::output_with_timeout(&mut cmd, backup_timeout)
                .context("failed to run restic backup")?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("restic backup failed: {}", stderr.trim());
            }

            // Extract snapshot_id from the JSON summary line.
            let stdout = String::from_utf8_lossy(&output.stdout);
            let snapshot_id = stdout
                .lines()
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                .find(|j| j.get("message_type").and_then(|v| v.as_str()) == Some("summary"))
                .and_then(|j| {
                    j.get("snapshot_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .unwrap_or_else(|| "unknown".to_string());

            // 2. Forget + prune
            let forget_output = crate::cmd::output_with_timeout(
                Command::new("restic")
                    .args(["forget", "--prune", "--keep-within", &keep_within])
                    .args(["--repo", &repo])
                    .args(["--password-file", &pw])
                    .envs(&env),
                backup_timeout,
            );
            if let Ok(out) = forget_output {
                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    tracing::warn!("restic forget/prune failed: {}", stderr.trim());
                }
            }

            Ok(snapshot_id)
        })
        .await;

        let elapsed = start.elapsed().as_secs();

        match result {
            Ok(Ok(snapshot_id)) => {
                tracing::info!("backup completed: snapshot {snapshot_id} in {elapsed}s");
                sentry_ext::breadcrumb(
                    "backup",
                    &format!("backup completed: {snapshot_id}"),
                    &[("duration_secs", &elapsed.to_string())],
                );
                dispatcher.dispatch(&DaemonEvent::BackupCompleted {
                    snapshot_id,
                    duration_secs: elapsed,
                });
            }
            Ok(Err(e)) => {
                tracing::error!("backup failed: {e:#}");
                sentry_ext::breadcrumb("backup", &format!("backup failed: {e}"), &[]);
                dispatcher.dispatch(&DaemonEvent::BackupFailed {
                    error: format!("{e:#}"),
                });
            }
            Err(e) => {
                tracing::error!("backup task panicked: {e}");
                dispatcher.dispatch(&DaemonEvent::BackupFailed {
                    error: format!("task panicked: {e}"),
                });
            }
        }
    }

    fn handle_shutdown(&self, signal: &str) {
        tracing::info!("received {signal}, shutting down");
        sentry_ext::breadcrumb("daemon", &format!("{signal} received, shutting down"), &[]);
        self.dispatcher.dispatch(&DaemonEvent::DaemonStopped);
    }

    async fn send_heartbeat(
        &self,
        relay_proxy_hostname: Option<String>,
        relay_proxy_url: Option<String>,
    ) {
        if let (Some(url), Some(token)) = (&self.server_url, &self.server_token) {
            #[cfg(feature = "services")]
            let services = self.svc_mgr.collect_statuses();
            #[cfg(not(feature = "services"))]
            let services: Vec<serde_json::Value> = vec![];

            let tunnels_enabled = self.current_cfg.relay.tunnels_enabled;

            #[cfg(feature = "services")]
            let tunnels: Vec<serde_json::Value> = if tunnels_enabled {
                self.svc_mgr
                    .collect_tunnels()
                    .iter()
                    .map(|t| serde_json::json!({ "name": t.name, "port": t.tcp_port }))
                    .collect()
            } else {
                vec![]
            };
            #[cfg(not(feature = "services"))]
            let tunnels: Vec<serde_json::Value> = vec![];

            #[cfg(feature = "services")]
            let file_tunnels: Vec<serde_json::Value> = if tunnels_enabled {
                self.svc_mgr
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
                        if let crate::managed_service::FileTunnelDef::Folder { include, .. } =
                            &ft.def
                        {
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
                    .collect()
            } else {
                vec![]
            };
            #[cfg(not(feature = "services"))]
            let file_tunnels: Vec<serde_json::Value> = vec![];

            #[cfg(feature = "services")]
            let shell_tunnels: Vec<serde_json::Value> = if tunnels_enabled {
                self.svc_mgr
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
                    .collect()
            } else {
                vec![]
            };
            #[cfg(not(feature = "services"))]
            let shell_tunnels: Vec<serde_json::Value> = vec![];

            let sample = self.assessor.latest_sample_snapshot();
            let services_extended = self.assessor.latest_probes_snapshot();

            #[cfg(feature = "services")]
            let service_samples = self.svc_mgr.collect_service_samples().await;
            #[cfg(not(feature = "services"))]
            let service_samples = Vec::new();

            self.metrics
                .assessment
                .update_service_samples(&service_samples);

            // Host-level failure signals: resource pressure from the sample plus
            // drift signals from the supervisor. Critical ones drive the healer.
            let mut failure_signals = match &sample {
                Some(s) => mac_mgmt_agent::signals::evaluate_sample_signals(
                    s,
                    &mac_mgmt_agent::signals::SignalThresholds::default(),
                    chrono::Utc::now().timestamp(),
                ),
                None => Vec::new(),
            };
            #[cfg(feature = "services")]
            failure_signals.extend(self.svc_mgr.drift_signals());

            let url = url.clone();
            let token = token.clone();
            let iid = self.instance_id.clone();
            let hk = Arc::clone(&self.host_key);
            let pending = Arc::clone(&self.initial_assessment_pending);
            let assessor = Arc::clone(&self.assessor);
            let metrics = Arc::clone(&self.metrics);
            let lock = Arc::clone(&self.heartbeat_lock);
            tokio::spawn(async move {
                // Serialize POSTs from this daemon. Without this, two
                // overlapping spawns can land at the server in the wrong
                // order, leaving the row with stale data.
                let _guard = lock.lock().await;
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
                    service_samples,
                    failure_signals,
                )
                .await;

                if ok {
                    metrics.record_heartbeat_success();
                } else {
                    metrics.record_heartbeat_failure();
                }

                // First successful heartbeat creates the parent row for
                // assessments + probes via migration 031's FK. Only fire
                // the initial inventory send once, and only after we know
                // the parent is there. swap(false) is a compare-and-set
                // so concurrent heartbeat sends at startup don't race.
                if ok && pending.swap(false, Ordering::Relaxed) {
                    // Initial send — service inventories/security will be
                    // collected on the first full 6h inventory tick.
                    assessor
                        .send_inventory(&url, &token, &iid, &hk, Vec::new(), Vec::new())
                        .await;
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
    /// Service inventories/security are collected synchronously before spawning.
    async fn send_assessment_inventory(&self) {
        if let (Some(url), Some(token)) = (&self.server_url, &self.server_token) {
            #[cfg(feature = "services")]
            let (si, ss) = tokio::join!(
                self.svc_mgr.collect_service_inventories(),
                self.svc_mgr.collect_service_security(),
            );
            #[cfg(not(feature = "services"))]
            let (si, ss): (
                Vec<mac_mgmt_common::ServiceInventory>,
                Vec<mac_mgmt_common::ServiceSecurity>,
            ) = (Vec::new(), Vec::new());

            self.metrics.assessment.update_service_inventories(&si);
            self.metrics.assessment.update_service_security(&ss);

            let u = url.clone();
            let t = token.clone();
            let iid = self.instance_id.clone();
            let hk = Arc::clone(&self.host_key);
            let assessor = Arc::clone(&self.assessor);
            tokio::spawn(async move {
                assessor.send_inventory(&u, &t, &iid, &hk, si, ss).await;
            });
        }
    }

    /// Fire-and-forget: run all deep probes.
    fn run_assessment_probes(&self, kind: Option<crate::assessment::probes::ProbeKind>) {
        if let (Some(url), Some(token)) = (&self.server_url, &self.server_token) {
            let u = url.clone();
            let t = token.clone();
            let iid = self.instance_id.clone();
            let hk = Arc::clone(&self.host_key);
            let assessor = Arc::clone(&self.assessor);
            tokio::spawn(async move {
                assessor.run_probes_filtered(&u, &t, &iid, &hk, kind).await;
            });
        }
    }

    /// Respond to `PushCommand::RequestAssessment` — runs inventory + probes now.
    async fn handle_request_assessment(&self) {
        if let (Some(url), Some(token)) = (&self.server_url, &self.server_token) {
            #[cfg(feature = "services")]
            let (si, ss) = tokio::join!(
                self.svc_mgr.collect_service_inventories(),
                self.svc_mgr.collect_service_security(),
            );
            #[cfg(not(feature = "services"))]
            let (si, ss): (
                Vec<mac_mgmt_common::ServiceInventory>,
                Vec<mac_mgmt_common::ServiceSecurity>,
            ) = (Vec::new(), Vec::new());
            let u = url.clone();
            let t = token.clone();
            let iid = self.instance_id.clone();
            let hk = Arc::clone(&self.host_key);
            let assessor = Arc::clone(&self.assessor);
            tokio::spawn(async move {
                assessor.request(&u, &t, &iid, &hk, si, ss).await;
            });
        }
    }

    async fn handle_health_tick(&mut self) {
        #[cfg(feature = "services")]
        if tokio::time::timeout(
            std::time::Duration::from_secs(30),
            self.svc_mgr.health_tick(&self.metrics, self.upgrade_gate()),
        )
        .await
        .is_err()
        {
            tracing::warn!("health tick timed out (30s), continuing");
        }

        #[cfg(feature = "healer")]
        self.maybe_trigger_healer().await;
    }

    /// Auto-trigger a local healer session when services are persistently unhealthy.
    #[cfg(feature = "healer")]
    async fn maybe_trigger_healer(&mut self) {
        let healer_cfg = &self.current_cfg.healer;
        if !healer_cfg.auto_trigger.unwrap_or(false) {
            return;
        }

        let probes = self.assessor.latest_probes_snapshot();
        let has_unhealthy = probes.iter().any(|s| !s.healthy);

        if has_unhealthy {
            self.healer_unhealthy_counter += 1;
        } else {
            self.healer_unhealthy_counter = 0;
            return;
        }

        // Default threshold: 10 consecutive unhealthy ticks (~10 minutes at 1m interval).
        let threshold = 10u32;
        if self.healer_unhealthy_counter < threshold {
            return;
        }
        self.healer_unhealthy_counter = 0;

        let sample = self.assessor.latest_sample_snapshot();
        let sample_json = sample.as_ref().and_then(|s| serde_json::to_value(s).ok());

        let mut failure_signals = match &sample {
            Some(s) => mac_mgmt_agent::signals::evaluate_sample_signals(
                s,
                &mac_mgmt_agent::signals::SignalThresholds::default(),
                chrono::Utc::now().timestamp(),
            ),
            None => Vec::new(),
        };
        failure_signals.extend(self.svc_mgr.drift_signals());

        let file_tunnels =
            serde_json::to_value(self.svc_mgr.collect_file_tunnels()).unwrap_or_default();
        let shell_tunnels =
            serde_json::to_value(self.svc_mgr.collect_shell_tunnels()).unwrap_or_default();

        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_default();

        // Build instance access from the session factory (uses the same
        // tunnel registries the relay SSH module manages).
        let dummy_session = mac_mgmt_healer::session::HealerSession {
            id: uuid::Uuid::nil(),
            cluster_id: uuid::Uuid::nil(),
            instance_id: self.instance_id.clone(),
            state: mac_mgmt_healer::session::SessionState::Created,
            state_data: serde_json::json!({}),
            created_by: String::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            completed_at: None,
            error_message: None,
            initial_issues: serde_json::json!([]),
            provider: None,
            model: None,
            label: None,
        };
        let access = match self
            .healer
            .session_factory()
            .build_access(&dummy_session)
            .await
        {
            Ok(a) => a,
            Err(e) => {
                tracing::debug!(err = %e, "healer auto-trigger: failed to build access");
                return;
            }
        };

        let req = mac_mgmt_healer::SpawnRequest {
            cluster_id: uuid::Uuid::nil(),
            instance_id: self.instance_id.clone(),
            created_by: "auto:local-unhealthy".to_string(),
            user_message: None,
            instance_access: access.instance,
            cluster_access: access.cluster,
            metrics_url: access.metrics_url,
            services_extended: probes,
            failure_signals,
            sample: sample_json,
            file_tunnels,
            shell_tunnels,
            cluster_instances: Vec::new(),
            cluster_name: String::new(),
            hostname,
            skip_cooldown: false,
            provider: None,
            model: None,
            label: Some("auto-triggered (local)".to_string()),
            token_budget: None,
            proxy_expires: None,
            auto_approve: healer_cfg.auto_approve.unwrap_or(true),
            fix_provider: healer_cfg.fix_provider.clone(),
            fix_model: healer_cfg.fix_model.clone(),
            validator_provider: None,
            validator_model: None,
            ml_hints: None,
        };

        match self.healer.spawn_session(req).await {
            Ok(sid) => {
                tracing::info!(session_id = %sid, "auto-triggered local healer session");
            }
            Err(e) => {
                tracing::debug!(err = %e, "local healer auto-trigger skipped");
            }
        }
    }

    /// Handle a push command. Returns true if SSH keys should be synced.
    /// Note: SyncConfig is handled directly in the event loop (needs interval refs).
    async fn handle_push_cmd(&mut self, cmd: crate::server_push::PushCommand) -> bool {
        use crate::server_push::PushCommand;
        match cmd {
            PushCommand::Ping => unreachable!("Ping filtered in SSE parser"),
            PushCommand::Targeted { .. } => unreachable!("Targeted unwrapped in SSE parser"),
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
                        if let Err(e) = crate::packages::sync_packages(&u, &t).await {
                            tracing::warn!("push package sync failed: {e}");
                        }
                    });
                }
                false
            }
            PushCommand::SyncPackages => {
                if let (Some(url), Some(token)) = (&self.server_url, &self.server_token) {
                    let u = url.clone();
                    let t = token.clone();
                    tokio::spawn(async move {
                        if let Err(e) = crate::packages::sync_packages(&u, &t).await {
                            tracing::warn!("push package sync failed: {e}");
                        }
                    });
                }
                false
            }
            PushCommand::SyncSshKeys => true,
            PushCommand::SelfUpdate => {
                tracing::info!("server push: self-update requested");
                if let (Some(url), Some(token)) = (&self.server_url, &self.server_token) {
                    fetch_target_version(url, token).await;
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
                    fetch_nixpkgs_pin(url, token).await;
                }
                #[cfg(feature = "services")]
                self.svc_mgr.check_upgrades();
                #[cfg(feature = "services")]
                self.svc_mgr.retry_failed_installs();
                false
            }
            PushCommand::RequestAssessment => {
                tracing::info!("server push: system assessment requested");
                self.handle_request_assessment().await;
                false
            }
        }
    }

    #[cfg(feature = "services")]
    async fn refresh_schemas(&self) {
        let mut validators = Vec::new();
        if self.current_cfg.opencode.enabled {
            validators.push(crate::services::opencode::VALIDATOR.clone());
        }
        if self.current_cfg.openclaw.enabled {
            validators.push(crate::services::openclaw::VALIDATOR.clone());
        }
        validators.push(crate::services::mcporter::VALIDATOR.clone());
        crate::validator::refresh_all(&validators).await;
    }

    fn handle_local_sync(&self) {
        tracing::info!("local sync requested");
        self.spawn_sync_skills_and_mcp();
    }

    #[cfg(all(feature = "services", feature = "relay"))]
    fn update_relay_tunnel_defs(&self, relay_mgr: &crate::remote_ssh::RemoteSshState) {
        if !self.current_cfg.relay.tunnels_enabled {
            relay_mgr.update_tunnel_defs(vec![]);
            relay_mgr.update_tunnel_overrides(std::collections::HashMap::new());
            return;
        }
        let td = self.svc_mgr.collect_tunnels();
        relay_mgr.update_tunnel_defs(td);
        let overrides = self.svc_mgr.collect_tunnel_overrides();
        relay_mgr.update_tunnel_overrides(overrides);
    }

    #[cfg(all(feature = "services", feature = "relay"))]
    fn update_relay_file_tunnel_defs(&self, relay_mgr: &crate::remote_ssh::RemoteSshState) {
        if !self.current_cfg.relay.tunnels_enabled {
            relay_mgr.update_file_tunnel_defs(vec![]);
            return;
        }
        let fd = self.svc_mgr.collect_file_tunnels();
        relay_mgr.update_file_tunnel_defs(fd);
    }

    #[cfg(all(feature = "services", feature = "relay"))]
    fn update_relay_shell_tunnel_defs(&self, relay_mgr: &crate::remote_ssh::RemoteSshState) {
        if !self.current_cfg.relay.tunnels_enabled {
            relay_mgr.update_shell_tunnel_defs(vec![]);
            return;
        }
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

    // Ensure /run/opengl-driver symlink exists if the driver is mounted at
    // /var/lib/opengl-driver (Incus disk device). The /run tmpfs loses the
    // symlink on reboot, so recreate it every daemon start. Linux/Incus-only.
    #[cfg(unix)]
    {
        let opengl_var = std::path::Path::new("/var/lib/opengl-driver");
        if opengl_var.exists() {
            let link = std::path::Path::new("/run/opengl-driver");
            if !link.exists() {
                if let Err(e) = std::os::unix::fs::symlink(opengl_var, link) {
                    tracing::warn!("failed to create /run/opengl-driver symlink: {e}");
                } else {
                    tracing::info!("created /run/opengl-driver -> /var/lib/opengl-driver symlink");
                }
            }
        }
    }

    let mut cfg = config::load().await?;
    let mut current_cfg = cfg.clone();

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
    let server_token = cfg.server.token.as_ref().map(|s| s.expose().to_string());
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

    // Set the server URL for nixpkgs archive downloads and load any
    // cached pin so the daemon can operate even if the server is
    // unreachable on this boot.
    crate::nix::set_nixpkgs_server_url(server_url.clone());
    crate::nix::load_cached_nixpkgs_pin();
    crate::nix::load_cached_nix_caches();

    // Fetch the cluster's nixpkgs pin and nix caches before ServiceManager::init runs.
    if let (Some(url), Some(token)) = (&server_url, &server_token) {
        fetch_nixpkgs_pin(url, token).await;
        fetch_nix_caches(url, token).await;
    }

    // Pre-fetch JSON schemas used for config validation so they're
    // available without network delay during service setup.
    #[cfg(feature = "services")]
    {
        if cfg.opencode.enabled {
            crate::services::opencode::VALIDATOR.prefetch().await;
        }
        if cfg.openclaw.enabled {
            crate::services::openclaw::VALIDATOR.prefetch().await;
        }
        crate::services::mcporter::VALIDATOR.prefetch().await;
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

    // Spawn the AI API proxy if enabled.
    #[cfg(feature = "services")]
    let ai_proxy_handle = if current_cfg.ai_proxy.enabled {
        let usage_path = crate::config::config_dir().join("ai-proxy-usage.jsonl");
        let usage_tracker =
            std::sync::Arc::new(crate::ai_proxy::usage::UsageTracker::new(usage_path));
        // Generate an in-memory probe token with no budget for assessment probes.
        use rand::Rng;
        let probe_token: String = rand::rng()
            .sample_iter(&rand::distr::Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();
        let probe_key_hash = crate::ai_proxy::multihash_key(&probe_token);
        current_cfg.ai_proxy.probe_token = Some(probe_token);

        let mut state = crate::ai_proxy::AiProxyState::new(
            &current_cfg.ai_proxy,
            &current_cfg.ollama,
            &current_cfg.unsloth,
            std::sync::Arc::clone(&usage_tracker),
        );
        // Inject the probe key into the state so it survives config reloads.
        state.probe_key_hash = Some(probe_key_hash.clone());
        state.keys.write().await.push(crate::ai_proxy::KeyEntry {
            key_hash: probe_key_hash,
            name: "__probe__".into(),
            token_budget: 0,
            budget_window: std::time::Duration::from_secs(86400),
            enabled: true,
        });

        let state = std::sync::Arc::new(state);
        let handle = crate::ai_proxy::AiProxyHandle::new(std::sync::Arc::clone(&state));
        let proxy_port = current_cfg.ai_proxy.port;
        let proxy_host = current_cfg.ai_proxy.host.clone();

        // Periodic usage flush (every 30s)
        let flush_tracker = std::sync::Arc::clone(&usage_tracker);
        tokio::spawn(async move {
            let mut interval = time::interval(std::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                flush_tracker.flush();
            }
        });

        tokio::spawn(async move {
            if let Err(e) = crate::ai_proxy::server::build_rocket(state, proxy_port, &proxy_host)
                .launch()
                .await
            {
                tracing::error!("AI proxy server failed: {e}");
                sentry_ext::capture_error(&format!("AI proxy server failed: {e}"), &[]);
            }
        });
        tracing::info!("AI proxy started on port {proxy_port}");
        Some(handle)
    } else {
        None
    };

    let mut update_tick = time::interval(update_interval);
    let mut health_tick = time::interval(health_interval);
    let mut heartbeat_tick = time::interval(health_interval);
    // Poll remote config on the same cadence as updates — catches server-side
    // config changes even when the SSE SyncConfig push is missed or unavailable.
    let mut config_poll_tick = time::interval(update_interval);
    let mut assessment_inventory_tick = time::interval(assessment::DEFAULT_INVENTORY_INTERVAL);
    let mut liveness_probe_tick = time::interval(assessment::jittered(
        assessment::DEFAULT_LIVENESS_INTERVAL,
        10,
    ));
    let mut functional_probe_tick = time::interval(assessment::jittered(
        assessment::DEFAULT_FUNCTIONAL_INTERVAL,
        120,
    ));
    let backup_interval = if cfg.backup.enabled {
        humantime::parse_duration(&cfg.backup.interval)
            .unwrap_or(std::time::Duration::from_secs(6 * 3600))
    } else {
        // Effectively never — backup is disabled.
        std::time::Duration::from_secs(365 * 24 * 3600)
    };
    let mut backup_tick = time::interval(backup_interval);

    let nix_gc_interval = humantime::parse_duration(&cfg.nix_gc.interval)
        .unwrap_or(std::time::Duration::from_secs(86400));
    let mut nix_gc_tick = time::interval(nix_gc_interval);
    // Track whether a disk-pressure GC is already running to avoid piling up.
    let nix_gc_running = Arc::new(AtomicBool::new(false));

    let mut sigterm = crate::platform::ShutdownSignal::terminate()
        .context("failed to register SIGTERM handler")?;
    let mut sigint = crate::platform::ShutdownSignal::interrupt()
        .context("failed to register SIGINT handler")?;

    // Services are registered incrementally by health_tick as they
    // transition from Installing → Stopped (background install).

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
    let (mut relay_mgr, mut relay_heartbeat_rx) = crate::remote_ssh::RemoteSshState::new(
        server_url.clone(),
        server_token.clone(),
        cfg.relay.remote_ssh_enabled,
    );

    // Register swarm/relay integrated services before P2P init so they
    // report unhealthy (not missing) when the swarm fails to start.
    // The shared AtomicBools default to false; the P2pManager sets them
    // to true when the swarm listens / relay registers.
    #[cfg(all(feature = "relay", feature = "services"))]
    let p2p_swarm_listening = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    #[cfg(all(feature = "relay", feature = "services"))]
    let p2p_relay_registered = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    #[cfg(all(feature = "relay", feature = "services"))]
    if cfg.relay.relay_multiaddr.is_some() || cfg.relay.mdns_enabled {
        svc_mgr.add_integrated_service(std::sync::Arc::new(
            crate::services::swarm_svc::SwarmService::new(p2p_swarm_listening.clone()),
        ));
        if cfg.relay.relay_multiaddr.is_some() {
            svc_mgr.add_integrated_service(std::sync::Arc::new(
                crate::services::relay_svc::RelayService::new(p2p_relay_registered.clone()),
            ));
        }
    }

    // Initialize memvault BEFORE the p2p swarm so its store, identity, and
    // sync wiring can be composed into the cluster swarm (one swarm, one
    // identity). The peer_id is the libp2p PeerId bytes derived from the same
    // host key the swarm uses (design A-1: node key = libp2p key) — NOT a
    // separate hash — so join/attestation peer matching lines up.
    #[cfg(feature = "memvault")]
    let memvault_handle = if cfg.memvault.enabled {
        match crate::p2p::identity::keypair_from_russh(&host_key) {
            Ok(kp) => {
                let memvault_peer_id = kp.public().to_peer_id().to_bytes();
                match crate::memvault::MemvaultHandle::init(&cfg.memvault, memvault_peer_id).await {
                    Ok(h) => {
                        tracing::info!("memvault subsystem active");
                        Some(h)
                    }
                    Err(e) => {
                        tracing::error!("failed to initialize memvault: {e:#}");
                        None
                    }
                }
            }
            Err(e) => {
                tracing::error!("memvault: failed to derive libp2p peer_id: {e:#}");
                None
            }
        }
    } else {
        None
    };

    // Start the libp2p P2P manager if relay_multiaddr is configured, or if
    // memvault sync is enabled (it rides the same swarm and needs it running
    // to dial bootstrap peers).
    #[cfg(all(feature = "relay", feature = "memvault"))]
    let memvault_wants_swarm = memvault_handle.is_some();
    #[cfg(all(feature = "relay", not(feature = "memvault")))]
    let memvault_wants_swarm = false;
    #[cfg(feature = "relay")]
    let mut _p2p_mgr =
        if cfg.relay.relay_multiaddr.is_some() || cfg.relay.mdns_enabled || memvault_wants_swarm {
            let relay_multiaddr = cfg
                .relay
                .relay_multiaddr
                .as_deref()
                .and_then(|s| s.parse::<libp2p::Multiaddr>().ok());
            let handler_state = std::sync::Arc::new(crate::p2p::handler::HandlerState {
                ssh_allowed: relay_mgr.ssh_allowed.clone(),
                tunnel_defs: relay_mgr.tunnel_defs.clone(),
                tunnel_overrides: relay_mgr.tunnel_overrides.clone(),
                file_tunnel_registry: relay_mgr.file_tunnel_registry(),
                shell_tunnel_registry: relay_mgr.shell_tunnel_registry(),
                metrics_port,
                fake_origin_local: cfg.relay.fake_origin_local,
                client: reqwest::Client::new(),
                server_ssh_keys: relay_mgr.server_ssh_keys.clone(),
                relay_ssh_key: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
            });
            // Fetch cluster_id from server before p2p init so gossipsub subscribes immediately.
            let cluster_id = match (&server_url, &server_token) {
                (Some(url), Some(token)) => crate::p2p::fetch_cluster_id(url, token).await,
                _ => None,
            };
            let p2p_config = crate::p2p::P2pConfig {
                instance_id: instance_id.clone(),
                cluster_psk: cfg
                    .relay
                    .cluster_psk
                    .as_ref()
                    .and_then(|s| hex::decode(s.expose()).ok()),
                relay_multiaddr,
                mdns_enabled: cfg.relay.mdns_enabled,
                p2p_port: cfg.relay.p2p_port,
                ai_proxy_distribution: cfg.relay.ai_proxy_distribution,
                server_token: server_token.clone(),
                cluster_id,
                handler_state: Some(handler_state),
                #[cfg(feature = "services")]
                swarm_listening: Some(p2p_swarm_listening.clone()),
                #[cfg(not(feature = "services"))]
                swarm_listening: None,
                #[cfg(feature = "services")]
                relay_registered: Some(p2p_relay_registered.clone()),
                #[cfg(not(feature = "services"))]
                relay_registered: None,
                // Compose memvault sync into this swarm when enabled.
                #[cfg(feature = "memvault")]
                memvault: memvault_handle.as_ref().and_then(|h| h.p2p_sync(&host_key)),
            };
            match crate::p2p::P2pManager::new(&host_key, p2p_config).await {
                Ok(mgr) => {
                    tracing::info!(peer_id = %mgr.local_peer_id, "p2p swarm started");
                    Some(mgr)
                }
                Err(e) => {
                    tracing::error!("failed to start p2p swarm: {e:#}");
                    None
                }
            }
        } else {
            None
        };

    // Start server push WebSocket if server is configured.
    let mut push_rx = if let (Some(url), Some(token)) = (&server_url, &server_token) {
        let (_handle, rx) = crate::server_push::start(url, token, &instance_id);
        Some(rx)
    } else {
        None
    };

    // Initialize the healer subsystem (local mode — JSON file store, local tunnels).
    #[cfg(feature = "healer")]
    let healer = {
        let store_dir = config::config_dir().join("healer");
        let store: mac_mgmt_healer::store::DynStore = std::sync::Arc::new(
            mac_mgmt_healer::store::json_file::JsonFileStore::open(&store_dir)
                .context("failed to open healer store")?,
        );
        let instance_data: mac_mgmt_healer::DynInstanceData =
            std::sync::Arc::new(crate::healer_bridge::LocalInstanceDataSource::new(
                Arc::clone(&assessor),
                instance_id.clone(),
            ));
        let cloud_anthropic = cfg
            .cloud
            .iter()
            .find(|c| c.enabled && c.provider == mac_mgmt_common::CloudProvider::Anthropic);
        let cloud_openrouter = cfg
            .cloud
            .iter()
            .find(|c| c.enabled && c.provider == mac_mgmt_common::CloudProvider::Openrouter);
        let connector_config = mac_mgmt_healer::connector::ConnectorConfig {
            ollama_url: Some(format!("http://{}:{}", cfg.ollama.host, cfg.ollama.port)),
            ollama_model: Some(cfg.ollama.default_model.clone()),
            anthropic_api_key: cloud_anthropic
                .and_then(|c| c.api_key.as_ref().map(|s| s.expose().to_string())),
            anthropic_model: None,
            openrouter_api_key: cloud_openrouter
                .and_then(|c| c.api_key.as_ref().map(|s| s.expose().to_string())),
            openrouter_model: None,
            openai_sources: Vec::new(),
            token_budget: 200_000,
            context7_api_key: None,
            validator_provider: None,
            validator_model: None,
            fine_tuned_model: cfg.healer.fine_tuned_model.clone(),
        };
        let session_factory = std::sync::Arc::new(crate::healer_bridge::LocalSessionFactory::new(
            relay_mgr.file_tunnel_registry(),
            relay_mgr.shell_tunnel_registry(),
            log_buf.clone(),
        ));
        Arc::new(mac_mgmt_healer::HealerState::new(
            store,
            instance_data,
            session_factory,
            connector_config,
        ))
    };

    // Build the Daemon struct with all long-lived state.
    // (memvault was initialized earlier, before the p2p swarm.)
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
        #[cfg(feature = "services")]
        ai_proxy_handle,
        #[cfg(feature = "healer")]
        healer,
        #[cfg(feature = "healer")]
        healer_unhealthy_counter: 0,
        #[cfg(feature = "memvault")]
        memvault: memvault_handle,
        first_update: true,
        heartbeat_lock: Arc::new(tokio::sync::Mutex::new(())),
    };

    // Fetch target version before attempting self-update so check_and_apply
    // sees the server's target.
    if let (Some(url), Some(token)) = (&daemon.server_url, &daemon.server_token) {
        fetch_target_version(url, token).await;
    }

    #[cfg(feature = "self-update")]
    if daemon.in_upgrade_window() {
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(120),
            tokio::task::spawn_blocking(crate::self_update::check_and_apply),
        )
        .await;
    } else {
        tracing::info!("outside upgrade window, skipping initial self-update");
    }

    daemon.spawn_sync_skills_and_mcp();

    // The initial assessment snapshot is emitted by send_heartbeat() once
    // the first 2xx ack lands — the heartbeat INSERT creates the parent
    // row that migration 031's FK requires on the assessments insert.

    #[cfg(feature = "relay")]
    relay_mgr.sync_ssh_keys();

    // Register tunnel definitions immediately so they're available as soon
    // as the relay WebSocket connects, rather than waiting for the first
    // health_tick to fire.
    #[cfg(all(feature = "services", feature = "relay"))]
    daemon.update_relay_tunnel_defs(&relay_mgr);
    #[cfg(all(feature = "services", feature = "relay"))]
    daemon.update_relay_file_tunnel_defs(&relay_mgr);
    #[cfg(all(feature = "services", feature = "relay"))]
    daemon.update_relay_shell_tunnel_defs(&relay_mgr);

    // Helper macros purely for cfg-gated relay proxy access.
    macro_rules! relay_proxy_hostname {
        () => {{
            #[cfg(feature = "relay")]
            {
                None::<String>
            }
            #[cfg(not(feature = "relay"))]
            {
                None::<String>
            }
        }};
    }
    macro_rules! relay_proxy_url {
        () => {{
            #[cfg(all(feature = "relay", not(feature = "sim")))]
            {
                _p2p_mgr.as_ref().and_then(|m| m.relay_proxy_url())
            }
            #[cfg(any(not(feature = "relay"), feature = "sim"))]
            {
                None::<String>
            }
        }};
    }

    // ── Main event loop ──────────────────────────────────────────────

    #[cfg(feature = "self-update")]
    let mut restart_exec_bin: Option<std::path::PathBuf> = None;

    loop {
        tokio::select! {
            _ = sigterm.recv() => { daemon.handle_shutdown("SIGTERM"); break; }
            _ = sigint.recv() => { daemon.handle_shutdown("SIGINT"); break; }

            _ = update_tick.tick() => {
                daemon.handle_update().await;
                #[cfg(feature = "self-update")]
                {
                    crate::self_update::check_binary_changed();
                    if let Some(bin) = crate::self_update::take_restart_exec() {
                        restart_exec_bin = Some(bin);
                        daemon.handle_shutdown("self-update");
                        break;
                    }
                }
                #[cfg(feature = "relay")]
                relay_mgr.sync_ssh_keys();
            }

            _ = health_tick.tick() => {
                // Run connectors BEFORE health tick so that pre-start
                // connectors patch config files and connector env vars
                // are collected before health_tick registers new services
                // with the supervisor.
                #[cfg(feature = "services")]
                tokio::task::block_in_place(|| daemon.svc_mgr.run_connectors_tick());
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
                // Check disk pressure and trigger GC if needed.
                if !nix_gc_running.load(Ordering::Relaxed)
                    && disk_threshold_exceeded(
                        &daemon.assessor,
                        daemon.current_cfg.nix_gc.disk_threshold_percent,
                    )
                {
                    nix_gc_running.store(true, Ordering::Relaxed);
                    let flag = Arc::clone(&nix_gc_running);
                    let assessor = Arc::clone(&daemon.assessor);
                    let threshold = daemon.current_cfg.nix_gc.disk_threshold_percent;
                    tokio::spawn(async move {
                        run_nix_gc("disk pressure", &assessor, threshold).await;
                        flag.store(false, Ordering::Relaxed);
                    });
                }
                daemon.send_heartbeat(relay_proxy_hostname!(), relay_proxy_url!()).await;
                #[cfg(feature = "memvault")]
                if let Some(ref mv) = daemon.memvault {
                    mv.tick().await;
                }
            }

            _ = heartbeat_tick.tick() => {
                daemon.refresh_assessment_sample().await;
                daemon.send_heartbeat(relay_proxy_hostname!(), relay_proxy_url!()).await;
            }

            _install_update = async {
                #[cfg(feature = "services")]
                {
                    match &mut daemon.svc_mgr.install_rx {
                        Some(rx) => rx.recv().await,
                        None => std::future::pending().await,
                    }
                }
                #[cfg(not(feature = "services"))]
                {
                    std::future::pending::<Option<()>>().await
                }
            } => {
                #[cfg(feature = "services")]
                {
                    match _install_update {
                        Some(u) => daemon.svc_mgr.handle_install_update(u),
                        None => daemon.svc_mgr.finish_installs(),
                    }
                    #[cfg(feature = "relay")]
                    daemon.update_relay_tunnel_defs(&relay_mgr);
                    #[cfg(feature = "relay")]
                    daemon.update_relay_file_tunnel_defs(&relay_mgr);
                    #[cfg(feature = "relay")]
                    daemon.update_relay_shell_tunnel_defs(&relay_mgr);
                    daemon.send_heartbeat(relay_proxy_hostname!(), relay_proxy_url!()).await;
                }
            }

            _ = assessment_inventory_tick.tick() => {
                daemon.send_assessment_inventory().await;
            }

            _ = liveness_probe_tick.tick() => {
                daemon.run_assessment_probes(Some(assessment::probes::ProbeKind::Liveness));
            }

            _ = functional_probe_tick.tick() => {
                daemon.run_assessment_probes(Some(assessment::probes::ProbeKind::Functional));
            }

            _ = backup_tick.tick() => {
                #[cfg(feature = "services")]
                daemon.handle_backup().await;
            }

            _ = nix_gc_tick.tick() => {
                if !nix_gc_running.swap(true, Ordering::Relaxed) {
                    let flag = Arc::clone(&nix_gc_running);
                    let assessor = Arc::clone(&daemon.assessor);
                    let threshold = daemon.current_cfg.nix_gc.disk_threshold_percent;
                    tokio::spawn(async move {
                        // Build a temporary helper to call handle_nix_gc-style logic.
                        run_nix_gc("scheduled", &assessor, threshold).await;
                        flag.store(false, Ordering::Relaxed);
                    });
                }
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
                #[cfg(feature = "self-update")]
                if let Some(bin) = crate::self_update::take_restart_exec() {
                    restart_exec_bin = Some(bin);
                    daemon.handle_shutdown("self-update");
                    break;
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
                daemon.send_heartbeat(relay_proxy_hostname!(), relay_proxy_url!()).await;
            }
            // P2p event: relay proxy URL acquired → send heartbeat immediately.
            _p2p_evt = async {
                #[cfg(all(feature = "relay", not(feature = "sim")))]
                {
                    if let Some(mgr) = _p2p_mgr.as_mut() {
                        mgr.recv_event().await
                    } else {
                        std::future::pending().await
                    }
                }
                #[cfg(any(not(feature = "relay"), feature = "sim"))]
                { std::future::pending::<Option<()>>().await }
            } => {
                #[cfg(all(feature = "relay", not(feature = "sim")))]
                if matches!(_p2p_evt, Some(crate::p2p::P2pEvent::RelayProxyUrlAcquired)) {
                    tracing::info!("relay proxy URL acquired, sending immediate heartbeat");
                    daemon.send_heartbeat(relay_proxy_hostname!(), relay_proxy_url!()).await;
                }
            }
        }
    }

    #[cfg(feature = "services")]
    daemon.shutdown().await;

    #[cfg(feature = "relay")]
    relay_mgr.cleanup();

    sentry_ext::breadcrumb("daemon", "daemon shutdown complete", &[]);
    tracing::info!("daemon shutdown complete");

    // If a self-update completed, exec the new binary now that cleanup is done.
    #[cfg(feature = "self-update")]
    if restart_exec_bin.is_some() {
        use std::os::unix::process::CommandExt;
        // Use argv[0] (the symlink) — it was already updated to point to
        // the new binary. This way exec goes through the symlink and picks
        // up the new version, rather than exec'ing the resolved store path.
        let argv0 = std::env::args()
            .next()
            .unwrap_or_else(|| "mac-mgmt".to_string());
        let args: Vec<String> = std::env::args().skip(1).collect();
        tracing::info!("exec'ing via {argv0}");
        let err = std::process::Command::new(&argv0).args(&args).exec();
        tracing::error!("failed to exec {argv0}: {err}");
    }

    Ok(())
}

// ── Free functions ───────────────────────────────────────────────────

/// GET a JSON endpoint on the server with a 10s timeout.
/// Returns `None` on any failure (timeout, non-2xx, parse error), logging at debug level.
async fn fetch_server_json<T: serde::de::DeserializeOwned>(
    server_url: &str,
    server_token: &str,
    path: &str,
    label: &str,
) -> Option<T> {
    let client = reqwest::Client::new();
    let url = format!("{server_url}{path}");
    match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.get(&url).bearer_auth(server_token).send(),
    )
    .await
    {
        Ok(Ok(resp)) if resp.status().is_success() => resp.json::<T>().await.ok(),
        Ok(Ok(resp)) => {
            tracing::debug!("{label} fetch returned {}", resp.status());
            None
        }
        Ok(Err(e)) => {
            tracing::debug!("{label} fetch failed: {e}");
            None
        }
        Err(_) => {
            tracing::debug!("{label} fetch timed out");
            None
        }
    }
}

/// Fetch the target version from the server and set it for self-update.
async fn fetch_target_version(server_url: &str, server_token: &str) {
    let system = crate::nix::current_system().unwrap_or("");
    let path = format!("/api/update?system={system}");
    if let Some(info) = fetch_server_json::<mac_mgmt_common::UpdateTarget>(
        server_url,
        server_token,
        &path,
        "update target",
    )
    .await
    {
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

/// Fetch nix binary cache URLs and public keys from the server and apply them.
async fn fetch_nix_caches(server_url: &str, server_token: &str) {
    #[derive(serde::Deserialize)]
    struct NixCachesResponse {
        caches: Vec<NixCacheEntry>,
    }
    #[derive(serde::Deserialize)]
    struct NixCacheEntry {
        url: String,
        public_key: String,
    }
    if let Some(resp) = fetch_server_json::<NixCachesResponse>(
        server_url,
        server_token,
        "/api/nix-caches",
        "nix caches",
    )
    .await
    {
        let caches: Vec<(String, String)> = resp
            .caches
            .into_iter()
            .map(|c| (c.url, c.public_key))
            .collect();
        tracing::info!("server returned {} nix cache(s)", caches.len());
        crate::nix::set_nix_caches(caches);
    }
}

/// Fetch the cluster's nixpkgs commit pin from the server and apply it.
async fn fetch_nixpkgs_pin(server_url: &str, server_token: &str) {
    if let Some(pin) = fetch_server_json::<mac_mgmt_common::NixpkgsPin>(
        server_url,
        server_token,
        "/api/nixpkgs",
        "nixpkgs pin",
    )
    .await
    {
        tracing::info!("server nixpkgs pin: {:?}", pin.commit);
        crate::nix::set_nixpkgs_commit(pin.commit);
    }
}

/// Thin wrapper over [`mac_mgmt_agent::heartbeat::do_send_heartbeat`]
/// supplying this crate's build identity (version / environment / git sha).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn do_send_heartbeat(
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
    service_samples: Vec<mac_mgmt_common::ServiceSample>,
    failure_signals: Vec<mac_mgmt_common::FailureSignal>,
) -> bool {
    let identity = mac_mgmt_agent::heartbeat::HeartbeatIdentity {
        version: CURRENT_VERSION.to_string(),
        environment: ENVIRONMENT.to_string(),
        git_sha: crate::GIT_SHA.to_string(),
    };
    mac_mgmt_agent::heartbeat::do_send_heartbeat(
        &identity,
        server_url,
        server_token,
        instance_id,
        host_key,
        services,
        tunnels,
        file_tunnels,
        shell_tunnels,
        relay_proxy_hostname,
        relay_proxy_url,
        sample,
        services_extended,
        service_samples,
        failure_signals,
    )
    .await
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

    let server_url = cfg.server.url.clone();
    let server_token = cfg.server.token.as_ref().map(|s| s.expose().to_string());
    let skills_dir = std::path::PathBuf::from("/tmp/sim-skills");

    let dispatcher = Arc::new(Dispatcher::new(Vec::new(), None));

    // Services: create a minimal ServiceManager with zero services.
    #[cfg(feature = "services")]
    let svc_mgr = crate::service_mgmt::ServiceManager::sim_init(
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
    let mut liveness_probe_tick = time::interval(assessment::jittered(
        assessment::DEFAULT_LIVENESS_INTERVAL,
        10,
    ));
    let mut functional_probe_tick = time::interval(assessment::jittered(
        assessment::DEFAULT_FUNCTIONAL_INTERVAL,
        120,
    ));

    let instance_id = crate::host_keys::fingerprint_hex(&host_key);
    tracing::info!("sim daemon started, instance ID: {instance_id}");

    #[cfg(feature = "relay")]
    let (mut relay_mgr, mut relay_heartbeat_rx) = crate::remote_ssh::RemoteSshState::new(
        server_url.clone(),
        server_token.clone(),
        cfg.relay.remote_ssh_enabled,
    );

    // Start server push SSE if server is configured.
    let mut push_rx = if let (Some(url), Some(token)) = (&server_url, &server_token) {
        let (_handle, rx) = crate::server_push::start(url, token, &instance_id);
        Some(rx)
    } else {
        None
    };

    let assessor = Arc::new(Assessor::new());
    assessor.update_config(current_cfg.clone()).await;
    assessor.attach_metrics(Arc::clone(&metrics)).await;

    #[cfg(feature = "healer")]
    let healer_sim = {
        let ft = std::sync::Arc::new(tokio::sync::RwLock::new(
            crate::file_tunnels::FileTunnelRegistry::new(),
        ));
        let st = std::sync::Arc::new(tokio::sync::RwLock::new(
            crate::shell_tunnels::ShellTunnelRegistry::new(),
        ));
        Arc::new(mac_mgmt_healer::HealerState::new(
            std::sync::Arc::new(
                mac_mgmt_healer::store::json_file::JsonFileStore::open("/tmp/healer-sim").unwrap(),
            ),
            std::sync::Arc::new(crate::healer_bridge::LocalInstanceDataSource::new(
                Arc::clone(&assessor),
                instance_id.clone(),
            )),
            std::sync::Arc::new(crate::healer_bridge::LocalSessionFactory::new(
                ft,
                st,
                crate::log_buffer::LogBuffer::new(),
            )),
            mac_mgmt_healer::connector::ConnectorConfig::default(),
        ))
    };

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
        #[cfg(feature = "services")]
        ai_proxy_handle: None,
        #[cfg(feature = "healer")]
        healer: healer_sim,
        #[cfg(feature = "healer")]
        healer_unhealthy_counter: 0,
        #[cfg(feature = "memvault")]
        memvault: None,
        first_update: true,
        heartbeat_lock: Arc::new(tokio::sync::Mutex::new(())),
    };

    if let (Some(url), Some(token)) = (&daemon.server_url, &daemon.server_token) {
        fetch_target_version(url, token).await;
    }

    daemon.spawn_sync_skills_and_mcp();

    #[cfg(feature = "relay")]
    relay_mgr.sync_ssh_keys();

    // Re-use the relay macros from the production event loop.
    macro_rules! relay_proxy_hostname {
        () => {{
            #[cfg(feature = "relay")]
            {
                None::<String>
            }
            #[cfg(not(feature = "relay"))]
            {
                None::<String>
            }
        }};
    }
    macro_rules! relay_proxy_url {
        () => {{
            #[cfg(all(feature = "relay", not(feature = "sim")))]
            {
                _p2p_mgr.as_ref().and_then(|m| m.relay_proxy_url())
            }
            #[cfg(any(not(feature = "relay"), feature = "sim"))]
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
                // In sim mode, call connectors directly (block_in_place panics
                // on current_thread runtime used by #[tokio::test]).
                // Run connectors BEFORE health tick so connector env vars
                // are collected before services are registered.
                #[cfg(feature = "services")]
                daemon.svc_mgr.run_connectors_tick();
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
                daemon.send_heartbeat(relay_proxy_hostname!(), relay_proxy_url!()).await;
            }

            _ = heartbeat_tick.tick() => {
                daemon.refresh_assessment_sample().await;
                daemon.send_heartbeat(relay_proxy_hostname!(), relay_proxy_url!()).await;
            }

            _ = assessment_inventory_tick.tick() => {
                daemon.send_assessment_inventory().await;
            }

            _ = liveness_probe_tick.tick() => {
                daemon.run_assessment_probes(Some(assessment::probes::ProbeKind::Liveness));
            }

            _ = functional_probe_tick.tick() => {
                daemon.run_assessment_probes(Some(assessment::probes::ProbeKind::Functional));
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
                daemon.send_heartbeat(relay_proxy_hostname!(), relay_proxy_url!()).await;
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
    mock_services: Vec<Arc<dyn crate::managed_service::ManagedService>>,
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

    let server_url = cfg.server.url.clone();
    let server_token = cfg.server.token.as_ref().map(|s| s.expose().to_string());
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
    let mut liveness_probe_tick = time::interval(assessment::jittered(
        assessment::DEFAULT_LIVENESS_INTERVAL,
        10,
    ));
    let mut functional_probe_tick = time::interval(assessment::jittered(
        assessment::DEFAULT_FUNCTIONAL_INTERVAL,
        120,
    ));

    let instance_id = crate::host_keys::fingerprint_hex(&host_key);
    tracing::info!("sim daemon (with supervisor) started, instance ID: {instance_id}");

    #[cfg(feature = "relay")]
    let (mut relay_mgr, mut relay_heartbeat_rx) = crate::remote_ssh::RemoteSshState::new(
        server_url.clone(),
        server_token.clone(),
        cfg.relay.remote_ssh_enabled,
    );

    let mut push_rx = if let (Some(url), Some(token)) = (&server_url, &server_token) {
        let (_handle, rx) = crate::server_push::start(url, token, &instance_id);
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
                None::<String>
            }
            #[cfg(not(feature = "relay"))]
            {
                None::<String>
            }
        }};
    }
    macro_rules! relay_proxy_url {
        () => {{
            #[cfg(all(feature = "relay", not(feature = "sim")))]
            {
                _p2p_mgr.as_ref().and_then(|m| m.relay_proxy_url())
            }
            #[cfg(any(not(feature = "relay"), feature = "sim"))]
            {
                None::<String>
            }
        }};
    }

    #[cfg(feature = "healer")]
    let healer = Arc::new(mac_mgmt_healer::HealerState::new(
        std::sync::Arc::new(
            mac_mgmt_healer::store::json_file::JsonFileStore::open(
                config::config_dir().join("healer"),
            )
            .unwrap(),
        ),
        std::sync::Arc::new(crate::healer_bridge::LocalInstanceDataSource::new(
            Arc::clone(&assessor),
            instance_id.clone(),
        )),
        std::sync::Arc::new(crate::healer_bridge::LocalSessionFactory::new(
            relay_mgr.file_tunnel_registry(),
            relay_mgr.shell_tunnel_registry(),
            log_buf.clone(),
        )),
        mac_mgmt_healer::connector::ConnectorConfig::default(),
    ));

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
        ai_proxy_handle: None,
        #[cfg(feature = "healer")]
        healer,
        #[cfg(feature = "healer")]
        healer_unhealthy_counter: 0,
        #[cfg(feature = "memvault")]
        memvault: None,
        first_update: true,
        heartbeat_lock: Arc::new(tokio::sync::Mutex::new(())),
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
                // Run connectors BEFORE health tick so connector env vars
                // are collected before services are registered.
                daemon.svc_mgr.run_connectors_tick();
                daemon.handle_health_tick().await;
                #[cfg(feature = "relay")]
                daemon.update_relay_tunnel_defs(&relay_mgr);
                #[cfg(feature = "relay")]
                daemon.update_relay_file_tunnel_defs(&relay_mgr);
                #[cfg(feature = "relay")]
                daemon.update_relay_shell_tunnel_defs(&relay_mgr);
                daemon.update_relay_config(relay_proxy_hostname!());
                daemon.refresh_assessment_sample().await;
                daemon.send_heartbeat(relay_proxy_hostname!(), relay_proxy_url!()).await;
            }

            _ = heartbeat_tick.tick() => {
                daemon.refresh_assessment_sample().await;
                daemon.send_heartbeat(relay_proxy_hostname!(), relay_proxy_url!()).await;
            }

            _ = assessment_inventory_tick.tick() => {
                daemon.send_assessment_inventory().await;
            }

            _ = liveness_probe_tick.tick() => {
                daemon.run_assessment_probes(Some(assessment::probes::ProbeKind::Liveness));
            }

            _ = functional_probe_tick.tick() => {
                daemon.run_assessment_probes(Some(assessment::probes::ProbeKind::Functional));
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
                daemon.send_heartbeat(relay_proxy_hostname!(), relay_proxy_url!()).await;
            }
        }
    }

    daemon.shutdown().await;
    tracing::info!("sim daemon (with supervisor) shutdown complete");
    Ok(())
}

// ── Nix garbage collection (free functions, usable from spawned tasks) ───

/// Resolve which mount point backs `/nix/store` by stat'ing the path and
/// matching its device ID against the tracked mounts.  Falls back to `/`
/// if `/nix/store` doesn't exist or can't be matched.
fn nix_store_mount() -> String {
    // Device-ID matching needs Unix metadata (`st_dev`); nix-store disk-pressure
    // GC is a Unix/NixOS concern anyway. Non-Unix falls back to "/".
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let nix_dev = std::fs::metadata("/nix/store").ok().map(|m| m.dev());
        if let Some(dev) = nix_dev {
            // Walk the tracked mounts and pick the longest prefix whose dev matches.
            for candidate in &["/nix/store", "/nix", "/"] {
                if let Ok(m) = std::fs::metadata(candidate) {
                    if m.dev() == dev {
                        return candidate.to_string();
                    }
                }
            }
        }
    }
    "/".to_string()
}

/// Check whether the mount backing `/nix/store` exceeds the disk usage threshold.
fn disk_threshold_exceeded(assessor: &Assessor, threshold: u8) -> bool {
    if threshold == 0 {
        return false;
    }
    let Some(sample) = assessor.latest_sample_snapshot() else {
        return false;
    };
    let target_mount = nix_store_mount();
    // Find the sample entry for the target mount, falling back to "/" if the
    // exact mount isn't in the sample (e.g. /nix/store on the root partition
    // is reported as "/" by sysinfo).
    let df = sample
        .disk_free
        .iter()
        .find(|df| df.mount == target_mount)
        .or_else(|| sample.disk_free.iter().find(|df| df.mount == "/"));
    let Some(df) = df else {
        return false;
    };
    if df.total_bytes == 0 {
        return false;
    }
    let used_pct = ((df.total_bytes - df.free_bytes) * 100) / df.total_bytes;
    if used_pct >= threshold as u64 {
        tracing::info!(
            "nix_gc: mount {} at {}% usage (threshold {}%)",
            df.mount,
            used_pct,
            threshold,
        );
        return true;
    }
    false
}

/// Run `nix-collect-garbage --delete-old` and log the results.
async fn run_nix_gc(reason: &str, _assessor: &Assessor, _threshold: u8) {
    tracing::info!("nix_gc: starting garbage collection ({reason})");
    let start = std::time::Instant::now();

    let result = tokio::task::spawn_blocking(|| -> anyhow::Result<String> {
        use std::process::Command;

        let output = crate::cmd::output_with_timeout(
            Command::new("nix-collect-garbage").arg("--delete-old"),
            std::time::Duration::from_secs(600),
        )?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            anyhow::bail!("exit {}: {}", output.status, stderr.trim());
        }

        // nix-collect-garbage prints freed-space info to stderr
        let summary = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else if !stdout.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            "completed (no output)".to_string()
        };
        Ok(summary)
    })
    .await;

    let elapsed = start.elapsed().as_secs();

    match result {
        Ok(Ok(summary)) => {
            tracing::info!("nix_gc: finished in {elapsed}s — {summary}");
        }
        Ok(Err(e)) => {
            tracing::error!("nix_gc: failed in {elapsed}s — {e:#}");
        }
        Err(e) => {
            tracing::error!("nix_gc: task panicked — {e}");
        }
    }
}
