use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;

use mac_mgmt_services::{Client as SupervisorClient, Notification};

/// Environment variable that switches the supervisor to in-process mode.
/// When set to `1`, the daemon spawns the supervisor as a tokio task instead
/// of installing it as a separate OS unit, while still talking to it over
/// the usual Unix socket. Intended for development.
const INPROCESS_ENV: &str = "INPROCESS_SERVICE_MANAGER";

fn inprocess_enabled() -> bool {
    matches!(std::env::var(INPROCESS_ENV).ok().as_deref(), Some("1"))
}

use crate::connectors::{self, Connector};
use crate::events::DaemonEvent;
use crate::log_buffer::LogBuffer;
use crate::config_providers::{ConfigStore, ConnectorSnapshot};
use crate::managed_service::{FileTunnel, FileTunnelDef, ManagedService, ServiceMode, ShellTunnel, TunnelDef};
use crate::metrics::Metrics;
use crate::notify::Dispatcher;
use crate::sentry_ext;

// ── Service lifecycle state machine ──────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServicePhase {
    /// Not running / not registered with supervisor.
    Stopped,
    /// Just (re)registered, grace period before first real health check.
    Starting,
    /// Running, health checks passing.
    Healthy,
    /// Running, health checks failing.
    Unhealthy,
}

impl ServicePhase {
    fn is_healthy(self) -> bool {
        self == Self::Healthy
    }
}

struct ServiceState {
    name: String,
    service: Box<dyn ManagedService>,
    phase: ServicePhase,
    upgrade_pending: bool,
    restart_pending: bool,
    post_start_done: bool,
    consecutive_crashes: u32,
    /// Nix store path of the binary at the time of last (re)spawn — used to
    /// detect when the installed version changes under the running process.
    running_store_path: Option<String>,
    /// Whether we've already sent an initial Register for this service on the
    /// current client connection.
    registered: bool,
}

struct ConnectorState {
    connector: Box<dyn Connector>,
    last_snapshot: ConnectorSnapshot,
    ran: bool,
}

pub struct ServiceManager {
    services: Vec<ServiceState>,
    install_only: Vec<Box<dyn ManagedService>>,
    connectors: Vec<ConnectorState>,
    client: Option<SupervisorClient>,
    dispatcher: Arc<Dispatcher>,
    log_buf: LogBuffer,
    pub config_store: ConfigStore,
    /// `true` when the supervisor runs as a tokio task inside this daemon
    /// process. Skips OS-unit install and disables UpdateSelf RPC (a daemon
    /// self-update will recreate the supervisor anyway).
    inprocess: bool,
}

impl ServiceManager {
    /// Create a minimal ServiceManager for simulation testing.
    /// No services, no supervisor connection, no nix calls.
    #[cfg(feature = "sim")]
    pub fn sim_init(dispatcher: Arc<Dispatcher>, log_buf: LogBuffer) -> Self {
        Self {
            services: Vec::new(),
            install_only: Vec::new(),
            connectors: Vec::new(),
            client: None,
            dispatcher,
            log_buf,
            config_store: ConfigStore::new(None),
            inprocess: false,
        }
    }

    pub fn init(
        cfg: &mut crate::config::Config,
        dispatcher: Arc<Dispatcher>,
        log_buf: LogBuffer,
    ) -> Result<Self> {
        let inprocess = inprocess_enabled();
        if inprocess {
            tracing::info!(
                "{INPROCESS_ENV}=1, launching services supervisor in-process"
            );
            spawn_inprocess_supervisor();
        }
        // Otherwise the supervisor is expected to be running from a prior
        // `mac-mgmt install` (OS unit); the client retries until the socket
        // shows up.

        let global_cfg = std::mem::take(&mut cfg.global);
        let openclaw_cfg = std::mem::take(&mut cfg.openclaw);
        let opencode_cfg = std::mem::take(&mut cfg.opencode);
        let ollama_cfg = std::mem::take(&mut cfg.ollama);
        let lms_cfg = std::mem::take(&mut cfg.lms);
        let cloud_cfgs = std::mem::take(&mut cfg.cloud);

        let cache_dir = crate::config::config_dir().join("config_providers.json");
        let mut config_store = ConfigStore::new(Some(cache_dir));

        if let Ok(v) = serde_json::to_value(&ollama_cfg) {
            config_store.set("ollama", v);
        }
        if let Ok(v) = serde_json::to_value(&lms_cfg) {
            config_store.set("lms", v);
        }
        if let Ok(v) = serde_json::to_value(&openclaw_cfg) {
            config_store.set("openclaw", v);
        }
        if let Ok(v) = serde_json::to_value(&opencode_cfg) {
            config_store.set("opencode", v);
        }
        if let Ok(v) = serde_json::to_value(&cloud_cfgs) {
            config_store.set("cloud", v);
        }

        let connectors = connectors::build_connectors(
            &global_cfg, &ollama_cfg, &lms_cfg, &cloud_cfgs,
        );

        let all_services = connectors::build_services(
            &global_cfg, openclaw_cfg, opencode_cfg, ollama_cfg, lms_cfg,
        );

        let mut install_only: Vec<Box<dyn ManagedService>> = Vec::new();
        let mut services: Vec<ServiceState> = Vec::new();

        for svc in all_services {
            let name = svc.name().to_string();
            svc.ensure_installed()?;
            svc.ensure_setup()?;

            if svc.service_mode() == ServiceMode::InstallOnly {
                sentry_ext::breadcrumb(
                    "service",
                    &format!("{name} installed (install-only)"),
                    &[("service", &name)],
                );
                install_only.push(svc);
                continue;
            }

            if let Err(e) = svc.preflight() {
                tracing::warn!("{name} preflight failed: {e}");
            }
            sentry_ext::breadcrumb(
                "service",
                &format!("{name} installed and ready"),
                &[("service", &name)],
            );
            services.push(ServiceState {
                name,
                service: svc,
                phase: ServicePhase::Stopped,
                upgrade_pending: false,
                restart_pending: false,
                post_start_done: false,
                consecutive_crashes: 0,
                running_store_path: None,
                registered: false,
            });
        }

        let connectors = connectors
            .into_iter()
            .map(|c| ConnectorState {
                connector: c,
                last_snapshot: ConnectorSnapshot::default(),
                ran: false,
            })
            .collect();

        Ok(Self {
            services,
            install_only,
            connectors,
            client: None,
            dispatcher,
            log_buf,
            config_store,
            inprocess,
        })
    }

    /// Register service-specific Prometheus metrics with the given Metrics instance.
    pub fn register_metrics(&self, metrics: &Metrics) {
        for s in &self.services {
            for collector in s.service.metric_collectors() {
                if let Err(e) = metrics.register_collector(collector) {
                    tracing::warn!("{}: failed to register metric: {e}", s.name);
                }
            }
        }
    }

    // ── Supervisor connection ────────────────────────────────────────

    async fn ensure_client(&mut self) -> bool {
        if self.client.as_ref().is_some_and(|c| !c.is_disconnected()) {
            return true;
        }
        if self.client.as_ref().is_some_and(|c| c.is_disconnected()) {
            tracing::warn!("services supervisor disconnected, reconnecting");
            self.client = None;
            for s in &mut self.services {
                s.registered = false;
                if s.phase != ServicePhase::Stopped {
                    s.phase = ServicePhase::Stopped;
                }
            }
        }
        let socket = mac_mgmt_services::default_socket_path();
        tracing::info!("connecting to services supervisor at {}", socket.display());
        match SupervisorClient::connect(&socket, Duration::from_secs(30)).await {
            Ok(c) => {
                self.client = Some(c);
                tracing::info!("connected to services supervisor");
                true
            }
            Err(e) => {
                tracing::warn!("services supervisor connect failed: {e}");
                false
            }
        }
    }

    /// Connect (if needed) and (re)register every managed service.
    pub async fn connect_all(&mut self) {
        if !self.ensure_client().await {
            return;
        }
        for i in 0..self.services.len() {
            self.register_service(i).await;
        }
        self.refresh_running_store_paths().await;
    }

    async fn register_service(&mut self, i: usize) {
        let Some(client) = self.client.as_mut() else { return };
        let state = &mut self.services[i];
        let name = state.name.clone();
        if let Err(e) = state.service.configure() {
            tracing::warn!("{name} configure failed: {e}");
        }
        let spec = state.service.spawn_spec();
        // `running_store_path` is populated from the supervisor's view of the
        // actual running binary via `refresh_running_store_paths` below, so
        // don't speculatively set it from `which` here — that was masking
        // drift when a daemon restart happened between upgrade-install and
        // upgrade-apply.
        match client.register(&name, spec).await {
            Ok(()) => {
                tracing::info!("{name} registered with supervisor");
                state.registered = true;
                state.phase = ServicePhase::Starting;
                state.post_start_done = false;
            }
            Err(e) => {
                tracing::warn!("{name} register failed: {e}");
            }
        }
    }

    /// Ask the supervisor for its current view of the running children and
    /// set each service's `running_store_path` to the nix store prefix of
    /// the spawned program.  Prefers `resolved_program` (the canonical path
    /// of `spec.program` captured at spawn time) over `exe` (`/proc/pid/exe`)
    /// because for shebang wrapper scripts the latter points at the
    /// interpreter, not the wrapper itself.
    async fn refresh_running_store_paths(&mut self) {
        let Some(client) = self.client.as_mut() else { return };
        let statuses = match client.list().await {
            Ok(s) => s,
            Err(e) => {
                tracing::debug!("supervisor list failed: {e}");
                return;
            }
        };
        let mut by_name: std::collections::HashMap<String, Option<String>> =
            std::collections::HashMap::with_capacity(statuses.len());
        for status in statuses {
            // Prefer `resolved_program` (always the script/binary itself)
            // over `exe` (which may be the interpreter for scripts).
            let prefix = status
                .resolved_program
                .as_deref()
                .and_then(crate::nix::store_path_prefix)
                .or_else(|| {
                    status
                        .exe
                        .as_deref()
                        .and_then(crate::nix::store_path_prefix)
                });
            by_name.insert(status.name, prefix);
        }
        for state in &mut self.services {
            if let Some(prefix) = by_name.remove(&state.name) {
                state.running_store_path = prefix;
            }
        }
    }

    async fn reregister_service(&mut self, i: usize) {
        // Force a fresh spawn by sending an Unregister followed by Register.
        let name = self.services[i].name.clone();
        if let Some(client) = self.client.as_mut() {
            if let Err(e) = client.unregister(&name).await {
                tracing::warn!("{name} unregister failed: {e}");
            }
        }
        self.register_service(i).await;
    }

    fn drain_notifications(&mut self) {
        let Some(client) = self.client.as_mut() else { return };
        while let Some(notif) = client.try_recv_notification() {
            match notif {
                Notification::Log { name, line, .. } => {
                    self.log_buf.push(format!("[{name}] {line}"));
                }
                Notification::Crashed { name, exit_code } => {
                    tracing::warn!("{name} crashed (exit={exit_code:?})");
                    self.dispatcher.dispatch(&DaemonEvent::ServiceCrashed {
                        service: name.clone(),
                        exit_code,
                    });
                    if let Some(s) = self.services.iter_mut().find(|s| s.name == name) {
                        s.consecutive_crashes += 1;
                        s.phase = ServicePhase::Unhealthy;
                        s.post_start_done = false;
                        if s.consecutive_crashes >= 2 {
                            if let Err(e) = s.service.repair() {
                                tracing::error!("{name} repair failed: {e}");
                            }
                        }
                    }
                }
            }
        }
    }

    // ── Upgrades ─────────────────────────────────────────────────────

    pub fn check_upgrades(&mut self) {
        for svc in &self.install_only {
            let name = svc.name();
            sentry_ext::set_tag("service", name);
            match svc.check_and_upgrade() {
                Ok(true) => {
                    tracing::info!("{name} upgraded (install-only)");
                    self.dispatcher.dispatch(&DaemonEvent::UpgradeInstalled {
                        service: name.to_string(),
                    });
                }
                Ok(false) => {}
                Err(e) => {
                    tracing::warn!("{name} upgrade check failed: {e}");
                    self.dispatcher.dispatch(&DaemonEvent::UpgradeFailed {
                        service: name.to_string(),
                        error: e.to_string(),
                    });
                }
            }
        }

        for state in &mut self.services {
            if state.upgrade_pending {
                continue;
            }
            let name = &state.name;
            sentry_ext::set_tag("service", name);
            match state.service.check_and_upgrade() {
                Ok(true) => state.upgrade_pending = true,
                Ok(false) => {}
                Err(e) => {
                    tracing::warn!("{name} upgrade check failed: {e}");
                    self.dispatcher.dispatch(&DaemonEvent::UpgradeFailed {
                        service: name.clone(),
                        error: e.to_string(),
                    });
                }
            }
        }
    }

    // ── Health tick ──────────────────────────────────────────────────

    const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(10);

    pub async fn health_tick(&mut self, metrics: &Arc<Metrics>, in_upgrade_window: bool) {
        // No services → nothing to health-check or connect to.
        if self.services.is_empty() {
            return;
        }
        if !self.ensure_client().await {
            for s in &mut self.services {
                Self::update_metrics(metrics, &s.name, false, s.upgrade_pending, false);
            }
            return;
        }

        // Ensure every service has been registered at least once this connection.
        for i in 0..self.services.len() {
            if !self.services[i].registered {
                self.register_service(i).await;
            }
        }

        // Refresh running_store_path from the supervisor so that a restart
        // between upgrade-install and upgrade-apply is still detected.
        self.refresh_running_store_paths().await;

        self.drain_notifications();

        // Phase 1: lifecycle management.
        let mut busy_flags = vec![false; self.services.len()];
        let mut pending_reregisters: Vec<usize> = Vec::new();

        for (i, state) in self.services.iter_mut().enumerate() {
            let name = state.name.clone();
            sentry_ext::set_tag("service", &name);

            let busy = state.service.is_busy().unwrap_or(false);
            busy_flags[i] = busy;

            if !state.restart_pending
                && state.phase.is_healthy()
                && state.service.needs_restart()
            {
                tracing::info!("{name} needs restart (external change detected)");
                state.restart_pending = true;
            }

            if state.restart_pending && (!busy || in_upgrade_window) {
                pending_reregisters.push(i);
                state.restart_pending = false;
                state.upgrade_pending = false;
                continue;
            }

            if state.upgrade_pending && in_upgrade_window {
                pending_reregisters.push(i);
                state.upgrade_pending = false;
                continue;
            }

            // Detect binary store-path drift.
            let current_store = crate::nix::binary_store_path(state.service.binary_name());
            if let (Some(old), Some(new)) = (&state.running_store_path, &current_store) {
                if old != new && (!busy || in_upgrade_window) {
                    tracing::info!("{name} binary changed ({old} → {new}), restarting");
                    pending_reregisters.push(i);
                }
            }

            if state.phase == ServicePhase::Starting {
                state.phase = ServicePhase::Healthy;
            }
        }

        for i in pending_reregisters {
            let name = self.services[i].name.clone();
            self.reregister_service(i).await;
            self.dispatcher.dispatch(&DaemonEvent::UpgradeInstalled { service: name });
        }

        // Phase 2: concurrent health checks with timeout.
        use std::pin::Pin;
        use std::future::Future;
        let check_results: Vec<(usize, ServicePhase, Result<bool>)> = {
            let mut futs: Vec<Pin<Box<dyn Future<Output = (usize, ServicePhase, Result<bool>)> + Send + '_>>> =
                Vec::new();
            for (i, state) in self.services.iter().enumerate() {
                if matches!(state.phase, ServicePhase::Healthy | ServicePhase::Unhealthy) {
                    let prev = state.phase;
                    let fut = state.service.check_health_async();
                    futs.push(Box::pin(async move {
                        let result = tokio::time::timeout(Self::HEALTH_CHECK_TIMEOUT, fut).await;
                        let result = match result {
                            Ok(r) => r,
                            Err(_) => Err(anyhow::anyhow!("health check timed out")),
                        };
                        (i, prev, result)
                    }));
                }
            }
            futures_util::future::join_all(futs).await
        };

        // Phase 3: apply results + metrics.
        for (i, prev_phase, result) in check_results {
            let state = &mut self.services[i];
            let name = state.name.clone();
            match result {
                Ok(true) => {
                    state.consecutive_crashes = 0;
                    state.phase = ServicePhase::Healthy;
                    if prev_phase == ServicePhase::Unhealthy {
                        self.dispatcher
                            .dispatch(&DaemonEvent::ServiceRecovered { service: name.clone() });
                    }
                    if !state.post_start_done {
                        if let Err(e) = state.service.post_start() {
                            tracing::error!("{name} post_start failed: {e}");
                        }
                        state.post_start_done = true;
                    }
                }
                Ok(false) => {
                    state.phase = ServicePhase::Unhealthy;
                    if prev_phase != ServicePhase::Unhealthy {
                        self.dispatcher
                            .dispatch(&DaemonEvent::ServiceUnhealthy { service: name.clone() });
                    }
                    if let Err(e) = state.service.repair() {
                        tracing::error!("{name} repair failed: {e}");
                    }
                }
                Err(e) => {
                    tracing::warn!("{name} health check failed: {e}");
                }
            }
        }

        for (i, state) in self.services.iter().enumerate() {
            state.service.collect_metrics();
            Self::update_metrics(
                metrics,
                &state.name,
                state.phase.is_healthy(),
                state.upgrade_pending,
                busy_flags[i],
            );
        }
    }

    pub fn run_connectors_tick(&mut self) {
        self.run_connectors();
    }

    fn run_connectors(&mut self) {
        for cs in &mut self.connectors {
            let deps = cs.connector.depends_on();

            let deps_ready = deps.iter().all(|dep| {
                if self.config_store.get(dep).is_some() {
                    return true;
                }
                self.services
                    .iter()
                    .find(|s| s.name == *dep)
                    .is_some_and(|s| s.post_start_done)
            });
            if !deps_ready { continue; }

            let should_run = !cs.ran || self.config_store.any_changed(deps, &cs.last_snapshot);
            if !should_run { continue; }

            let name = cs.connector.name();
            let configs = self.config_store.values_for(deps);
            tracing::info!("running connector: {name}");
            if let Err(e) = cs.connector.connect(&configs) {
                tracing::error!("connector {name} failed: {e}");
            }
            cs.last_snapshot = self.config_store.snapshot(deps);
            cs.ran = true;
        }
    }

    fn update_metrics(metrics: &Metrics, name: &str, healthy: bool, upgrade_pending: bool, busy: bool) {
        metrics.service_healthy.with_label_values(&[name]).set(if healthy { 1 } else { 0 });
        metrics.service_upgrade_pending.with_label_values(&[name]).set(if upgrade_pending { 1 } else { 0 });
        metrics.service_busy.with_label_values(&[name]).set(if busy { 1 } else { 0 });
    }

    // ── Schedule restart ─────────────────────────────────────────────

    pub async fn schedule_restart(&mut self) {
        for state in &mut self.services {
            let name = &state.name;
            if state.service.supports_hot_reload() {
                match state.service.configure() {
                    Ok(()) => tracing::info!("{name} configured (no restart needed)"),
                    Err(e) => {
                        tracing::warn!("{name} configure failed, scheduling restart: {e}");
                        state.restart_pending = true;
                    }
                }
            } else {
                state.restart_pending = true;
                tracing::info!("{name} restart pending (config change)");
            }
        }
    }

    // ── Status collection ────────────────────────────────────────────

    pub fn collect_statuses(&self) -> Vec<serde_json::Value> {
        self.services
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "healthy": s.phase.is_healthy(),
                    "upgrade_pending": s.upgrade_pending,
                    "busy": false,
                })
            })
            .collect()
    }

    pub fn collect_tunnels(&self) -> Vec<TunnelDef> {
        self.services
            .iter()
            .filter(|s| s.phase.is_healthy())
            .flat_map(|s| s.service.expose_tunnels())
            .collect()
    }

    pub fn collect_shell_tunnels(&self) -> Vec<ShellTunnel> {
        let mut shell_tunnels: Vec<ShellTunnel> = self
            .services
            .iter()
            .flat_map(|s| {
                s.service
                    .expose_shell_commands()
                    .into_iter()
                    .map(|def| ShellTunnel {
                        def,
                        service: s.name.clone(),
                    })
            })
            .collect();
        for svc in &self.install_only {
            let svc_name = svc.name().to_string();
            shell_tunnels.extend(
                svc.expose_shell_commands()
                    .into_iter()
                    .map(|def| ShellTunnel {
                        def,
                        service: svc_name.clone(),
                    }),
            );
        }
        shell_tunnels.extend(daemon_system_shell_tunnels());
        shell_tunnels
    }

    pub fn collect_file_tunnels(&self) -> Vec<FileTunnel> {
        let mut file_tunnels: Vec<FileTunnel> = self
            .services
            .iter()
            .flat_map(|s| {
                s.service.expose_files().into_iter().map(|def| FileTunnel {
                    def,
                    service: s.name.clone(),
                })
            })
            .collect();
        for svc in &self.install_only {
            let svc_name = svc.name().to_string();
            file_tunnels.extend(svc.expose_files().into_iter().map(|def| FileTunnel {
                def,
                service: svc_name.clone(),
            }));
        }
        file_tunnels.extend(daemon_config_file_tunnels());
        // Only announce tunnels whose path exists on disk.
        file_tunnels.retain(|ft| std::path::Path::new(ft.path()).exists());
        file_tunnels
    }

    // ── Shutdown ─────────────────────────────────────────────────────

    pub async fn shutdown(&mut self) {
        // The supervisor keeps running; its children stay up across daemon
        // restarts. Just drop the client.
        self.client = None;
        tracing::info!("leaving services supervisor running");
    }

    /// Tell the supervisor to re-exec itself, picking up the new binary.
    #[allow(dead_code)]
    pub async fn send_update_self(&mut self) {
        if self.inprocess {
            // The supervisor lives inside this daemon process, so the
            // daemon's own self-update will restart it. Nothing to do.
            return;
        }
        let Some(client) = self.client.as_mut() else {
            return;
        };
        if let Err(e) = client.update_self().await {
            tracing::warn!("supervisor update-self failed: {e}");
        } else {
            tracing::info!("supervisor update-self sent");
        }
        // The supervisor will tear down and re-exec; drop our client so the
        // next health tick reconnects and re-registers every service.
        self.client = None;
        for s in &mut self.services {
            s.registered = false;
            s.phase = ServicePhase::Stopped;
        }
    }
}

/// File tunnels for the daemon's own config directory (not a ManagedService).
fn daemon_config_file_tunnels() -> Vec<FileTunnel> {
    use crate::managed_service::{FileTunnel, FileValidator};
    vec![FileTunnel {
        def: FileTunnelDef::Folder {
            name: "daemon-config".into(),
            path: crate::config::config_dir().to_string_lossy().into(),
            writable: true,
            allow_write: Vec::new(),
            include: Some(vec!["config.toml".into(), "ollama-env".into()]),
            validators: vec![FileValidator {
                glob: "config.toml".into(),
                command: vec!["mac-mgmt".into(), "check-config".into()],
            }],
            description: "Daemon configuration directory".into(),
        },
        service: "daemon".into(),
    }]
}

/// System-level shell commands (not tied to a ManagedService).
fn daemon_system_shell_tunnels() -> Vec<ShellTunnel> {
    use crate::managed_service::ShellCommandDef;
    vec![
        ShellTunnel {
            def: ShellCommandDef {
                name: "nix-profile-list".into(),
                command: "nix".into(),
                args: vec!["profile".into(), "list".into()],
                description: "List installed nix packages".into(),
                arg_template: None,
            },
            service: "daemon".into(),
        },
        ShellTunnel {
            def: ShellCommandDef {
                name: "daemon-status".into(),
                command: "systemctl".into(),
                args: vec!["status".into(), "mac-mgmt".into()],
                description: "Daemon systemd status".into(),
                arg_template: None,
            },
            service: "daemon".into(),
        },
        ShellTunnel {
            def: ShellCommandDef {
                name: "daemon-journal".into(),
                command: "journalctl".into(),
                args: vec![
                    "-u".into(),
                    "mac-mgmt".into(),
                    "-n".into(),
                    "100".into(),
                    "--no-pager".into(),
                ],
                description: "Daemon journal (last 100 lines)".into(),
                arg_template: None,
            },
            service: "daemon".into(),
        },
    ]
}

/// Launch the services supervisor as a tokio task inside the current process.
/// The task logs any error and exits; next health tick will fail to connect
/// and retry, which also surfaces the problem.
fn spawn_inprocess_supervisor() {
    let socket = mac_mgmt_services::default_socket_path();
    tokio::spawn(async move {
        match mac_mgmt_services::server::run(&socket).await {
            Ok(true) => {
                // UpdateSelf was requested. In-process we can't re-exec just
                // the supervisor, so log it and exit — the next reconnect
                // will fail and the daemon will surface the problem.
                tracing::warn!(
                    "in-process supervisor asked to re-exec; not supported, exiting task"
                );
            }
            Ok(false) => {
                tracing::info!("in-process supervisor exited cleanly");
            }
            Err(e) => {
                tracing::error!("in-process supervisor failed: {e:#}");
            }
        }
    });
}
