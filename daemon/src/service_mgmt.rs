use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};

use mac_mgmt_services::{Client as SupervisorClient, Notification};

/// Environment variable that switches the supervisor to in-process mode.
/// When set to `1`, the daemon spawns the supervisor as a tokio task instead
/// of installing it as a separate OS unit, while still talking to it over
/// the usual Unix socket. Intended for development.
const INPROCESS_ENV: &str = "INPROCESS_SERVICE_MANAGER";

fn inprocess_enabled() -> bool {
    matches!(std::env::var(INPROCESS_ENV).ok().as_deref(), Some("1"))
}

use crate::config_providers::{ConfigStore, ConnectorSnapshot};
use crate::connectors::{self, Connector, ConnectorPhase};
use crate::events::DaemonEvent;
use crate::log_buffer::LogBuffer;
use crate::managed_service::{
    FileTunnel, FileTunnelDef, ManagedService, ServiceMode, ShellTunnel, TunnelDef,
};
use crate::metrics::Metrics;
use crate::notify::Dispatcher;
use crate::sentry_ext;

// ── Service lifecycle state machine ──────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServicePhase {
    /// Background install (`ensure_installed` + `ensure_setup`) in progress.
    Installing,
    /// Background install failed — kept around for retry and in case the
    /// service is still running from a previous daemon instance.
    InstallFailed,
    /// Not running / not registered with supervisor.
    Stopped,
    /// Just (re)registered, grace period before first real health check.
    Starting,
    /// Running, health checks passing.
    Healthy,
    /// Running, health checks failing.
    Unhealthy,
    /// Crashed — waiting for backoff to expire before attempting repair + restart.
    CrashBackoff,
}

impl ServicePhase {
    fn is_healthy(self) -> bool {
        self == Self::Healthy
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Installing => "installing",
            Self::InstallFailed => "install_failed",
            Self::Stopped => "stopped",
            Self::Starting => "starting",
            Self::Healthy => "healthy",
            Self::Unhealthy => "unhealthy",
            Self::CrashBackoff => "crash_backoff",
        }
    }
}

struct ServiceState {
    name: String,
    service: Arc<dyn ManagedService>,
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
    /// When `phase == CrashBackoff`, the earliest `Instant` at which the
    /// service may be repaired and restarted.
    restart_at: Option<Instant>,
    /// Environment variables collected from connectors (secrets, etc.).
    /// Merged into SpawnSpec at registration time.
    connector_env: std::collections::HashMap<String, String>,
    /// Whether connector env has been collected for this service since
    /// it left Installing/InstallFailed phase.
    connector_env_collected: bool,
}

pub(crate) struct InstallUpdate {
    name: String,
    result: anyhow::Result<()>,
}

struct ConnectorState {
    connector: Box<dyn Connector>,
    last_snapshot: ConnectorSnapshot,
    ran: bool,
}

pub struct ServiceManager {
    services: Vec<ServiceState>,
    install_only: Vec<Arc<dyn ManagedService>>,
    connectors: Vec<ConnectorState>,
    client: Option<SupervisorClient>,
    dispatcher: Arc<Dispatcher>,
    log_buf: LogBuffer,
    pub config_store: ConfigStore,
    /// `true` when the supervisor runs as a tokio task inside this daemon
    /// process. Skips OS-unit install and disables UpdateSelf RPC (a daemon
    /// self-update will recreate the supervisor anyway).
    inprocess: bool,
    /// Receives per-service install completions from the background task.
    pub install_rx: Option<tokio::sync::mpsc::Receiver<InstallUpdate>>,
    /// Full config as JSON from the last init/apply, for per-service change
    /// detection in `apply_config` (USB apply-on-save).
    last_config: serde_json::Value,
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
            install_rx: None,
            last_config: serde_json::Value::Null,
        }
    }

    /// Create a ServiceManager with mock services and an in-process supervisor.
    /// The supervisor runs as a tokio task; services are registered but use
    /// no-op install/setup (they don't call real nix commands).
    #[cfg(feature = "sim")]
    pub fn sim_init_with_supervisor(
        dispatcher: Arc<Dispatcher>,
        log_buf: LogBuffer,
        mock_services: Vec<Arc<dyn ManagedService>>,
    ) -> Self {
        spawn_inprocess_supervisor();

        let services: Vec<ServiceState> = mock_services
            .into_iter()
            .map(|svc| {
                let name = svc.name().to_string();
                ServiceState {
                    name,
                    service: svc,
                    phase: ServicePhase::Stopped,
                    upgrade_pending: false,
                    restart_pending: false,
                    post_start_done: false,
                    consecutive_crashes: 0,
                    running_store_path: None,
                    registered: false,
                    restart_at: None,
                    connector_env: std::collections::HashMap::new(),
                    connector_env_collected: true,
                }
            })
            .collect();

        Self {
            services,
            install_only: Vec::new(),
            connectors: Vec::new(),
            client: None,
            dispatcher,
            log_buf,
            config_store: ConfigStore::new(None),
            inprocess: true,
            install_rx: None,
            last_config: serde_json::Value::Null,
        }
    }

    pub fn init(
        cfg: &mut crate::config::Config,
        dispatcher: Arc<Dispatcher>,
        log_buf: LogBuffer,
    ) -> Result<Self> {
        // Snapshot the full config before build_services consumes its sections,
        // for per-service change detection in apply_config (USB apply-on-save).
        let last_config = serde_json::to_value(&*cfg).unwrap_or(serde_json::Value::Null);

        let inprocess = inprocess_enabled();
        if inprocess {
            tracing::info!("{INPROCESS_ENV}=1, launching services supervisor in-process");
            spawn_inprocess_supervisor();
        }
        // Otherwise the supervisor is expected to be running from a prior
        // `mac-mgmt install` (OS unit); the client retries until the socket
        // shows up.

        let cache_dir = crate::config::config_dir().join("config_providers.json");
        let mut config_store = ConfigStore::new(Some(cache_dir));

        if let Ok(v) = serde_json::to_value(&cfg.ollama) {
            config_store.set("ollama", v);
        }
        if let Ok(v) = serde_json::to_value(&cfg.lms) {
            config_store.set("lms", v);
        }
        if let Ok(v) = serde_json::to_value(&cfg.unsloth) {
            config_store.set("unsloth", v);
        }
        if let Ok(v) = serde_json::to_value(&cfg.litellm) {
            config_store.set("litellm", v);
        }
        if let Ok(v) = serde_json::to_value(&cfg.openclaw) {
            config_store.set("openclaw", v);
        }
        if let Ok(v) = serde_json::to_value(&cfg.opencode) {
            config_store.set("opencode", v);
        }
        if let Ok(v) = serde_json::to_value(&cfg.hermes) {
            config_store.set("hermes", v);
        }
        if let Ok(v) = serde_json::to_value(&cfg.cloud) {
            config_store.set("cloud", v);
        }
        if let Ok(v) = serde_json::to_value(&cfg.backup) {
            config_store.set("backup", v);
        }
        if let Ok(v) = serde_json::to_value(&cfg.memvault) {
            config_store.set("memvault", v);
        }

        let connectors = connectors::build_connectors(&cfg);

        let all_services = connectors::build_services(cfg);

        let mut install_only: Vec<Arc<dyn ManagedService>> = Vec::new();
        let mut services: Vec<ServiceState> = Vec::new();

        for svc in all_services {
            let name = svc.name().to_string();
            if svc.service_mode() == ServiceMode::InstallOnly {
                install_only.push(svc);
            } else {
                services.push(ServiceState {
                    name,
                    service: svc,
                    phase: ServicePhase::Installing,
                    upgrade_pending: false,
                    restart_pending: false,
                    post_start_done: false,
                    consecutive_crashes: 0,
                    running_store_path: None,
                    registered: false,
                    restart_at: None,
                    connector_env: std::collections::HashMap::new(),
                    connector_env_collected: false,
                });
            }
        }

        let connectors: Vec<ConnectorState> = connectors
            .into_iter()
            .map(|c| ConnectorState {
                connector: c,
                last_snapshot: ConnectorSnapshot::default(),
                ran: false,
            })
            .collect();

        // Spawn background task for serial installation. Connectors,
        // connector env, and backup paths are deferred — they run
        // progressively via run_connectors_tick as services finish
        // installing.
        let to_install = services
            .iter()
            .map(|s| Arc::clone(&s.service))
            .chain(install_only.iter().map(Arc::clone));
        let install_rx = Some(Self::spawn_install_task(to_install));

        Ok(Self {
            services,
            install_only,
            connectors,
            client: None,
            dispatcher,
            log_buf,
            config_store,
            inprocess,
            install_rx,
            last_config,
        })
    }

    /// Add an integrated service after init (e.g. p2p probe services that
    /// depend on state only available after the swarm has started).
    /// The service starts in `Healthy` phase and skips installation.
    pub fn add_integrated_service(&mut self, svc: Arc<dyn ManagedService>) {
        debug_assert_eq!(svc.service_mode(), ServiceMode::Integrated);
        let name = svc.name().to_string();
        tracing::info!("{name} registered (integrated, post-init)");
        self.services.push(ServiceState {
            name,
            service: svc,
            phase: ServicePhase::Starting,
            upgrade_pending: false,
            restart_pending: false,
            post_start_done: false,
            consecutive_crashes: 0,
            running_store_path: None,
            registered: false,
            restart_at: None,
            connector_env: std::collections::HashMap::new(),
            connector_env_collected: true,
        });
    }

    // ── Background install machinery ────────────────────────────────

    /// Spawn a background task to serially install the given services.
    /// Returns a receiver that yields one `InstallUpdate` per service.
    fn spawn_install_task(
        services: impl Iterator<Item = Arc<dyn ManagedService>>,
    ) -> tokio::sync::mpsc::Receiver<InstallUpdate> {
        let (tx, rx) = tokio::sync::mpsc::channel(4);
        let to_install: Vec<_> = services.collect();
        if !to_install.is_empty() {
            tokio::task::spawn_blocking(move || {
                for svc in to_install {
                    let name = svc.name().to_string();
                    tracing::info!("{name} starting install");
                    let result = svc.ensure_installed().and_then(|_| svc.ensure_setup());
                    if let Err(ref e) = result {
                        tracing::error!("{name} install failed: {e}");
                    }
                    let _ = tx.blocking_send(InstallUpdate { name, result });
                }
            });
        }
        rx
    }

    /// Future that resolves when the next install update arrives.
    /// Returns `None` when the install task is done (channel closed).
    pub async fn recv_install_update(&mut self) -> Option<InstallUpdate> {
        match &mut self.install_rx {
            Some(rx) => rx.recv().await,
            None => std::future::pending().await,
        }
    }

    /// Process a single install completion.
    pub fn handle_install_update(&mut self, update: InstallUpdate) {
        if let Err(ref e) = update.result {
            tracing::error!("{} background install failed: {e}", update.name);
            if let Some(s) = self.services.iter_mut().find(|s| s.name == update.name) {
                s.phase = ServicePhase::InstallFailed;
            }
            return;
        }

        // Managed services: preflight + transition.
        if let Some(s) = self.services.iter_mut().find(|s| s.name == update.name) {
            if s.service.service_mode() != ServiceMode::Integrated {
                if let Err(e) = s.service.preflight() {
                    tracing::warn!("{} preflight failed: {e}", update.name);
                }
            }
            let integrated = s.service.service_mode() == ServiceMode::Integrated;
            s.phase = if integrated {
                ServicePhase::Starting
            } else {
                ServicePhase::Stopped
            };
            s.registered = integrated;
            sentry_ext::breadcrumb(
                "service",
                &format!("{} installed and ready", update.name),
                &[("service", &update.name)],
            );
            tracing::info!("{} installed and ready", update.name);
            return;
        }

        // install_only: just log.
        sentry_ext::breadcrumb(
            "service",
            &format!("{} installed (install-only)", update.name),
            &[("service", &update.name)],
        );
        tracing::info!("{} installed (install-only)", update.name);
    }

    /// Called when `recv_install_update` returns `None` (channel closed / task done).
    pub fn finish_installs(&mut self) {
        self.install_rx = None;
    }

    /// Re-queue `InstallFailed` services for installation.
    /// Called when nixpkgs pin changes or config reloads.
    pub fn retry_failed_installs(&mut self) {
        // Don't start a new task if one is already running.
        if self.install_rx.is_some() {
            return;
        }
        let has_failed = self
            .services
            .iter()
            .any(|s| s.phase == ServicePhase::InstallFailed);
        if !has_failed {
            return;
        }
        for s in &mut self.services {
            if s.phase == ServicePhase::InstallFailed {
                tracing::info!("{} queued for install retry", s.name);
                s.phase = ServicePhase::Installing;
            }
        }
        let to_retry = self
            .services
            .iter()
            .filter(|s| s.phase == ServicePhase::Installing)
            .map(|s| Arc::clone(&s.service));
        self.install_rx = Some(Self::spawn_install_task(to_retry));
    }

    /// Returns `true` if a dependency key maps to a service that is still
    /// installing or failed to install.
    fn dep_blocked_by_install(&self, dep: &str) -> bool {
        self.services.iter().any(|s| {
            s.name == dep
                && matches!(
                    s.phase,
                    ServicePhase::Installing | ServicePhase::InstallFailed
                )
        })
    }

    /// Collect backup-worthy paths from all services and store them in the
    /// config store as the `"backup_paths"` provider. The BackupConnector
    /// reads these to write `restic-includes.txt`.
    fn update_backup_paths(
        config_store: &mut ConfigStore,
        services: &[ServiceState],
        install_only: &[Arc<dyn ManagedService>],
    ) {
        let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/root"));
        let mut paths: Vec<serde_json::Value> = Vec::new();
        for s in services {
            for dp in s.service.data_paths(&home) {
                paths.push(serde_json::json!({
                    "service": s.name,
                    "name": dp.name,
                    "path": dp.path.to_string_lossy(),
                    "backup": dp.backup,
                }));
            }
        }
        for svc in install_only {
            for dp in svc.data_paths(&home) {
                paths.push(serde_json::json!({
                    "service": svc.name(),
                    "name": dp.name,
                    "path": dp.path.to_string_lossy(),
                    "backup": dp.backup,
                }));
            }
        }
        if let Ok(v) = serde_json::to_value(&paths) {
            config_store.set("backup_paths", v);
        }
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

        // Check which services are already running in the supervisor
        // (e.g. adopted after a reexec). Adopt them without re-registering
        // to avoid killing running processes. If the spec changed, schedule
        // a restart via the daemon's busy/upgrade system instead.
        let running_services: std::collections::HashMap<String, mac_mgmt_services::ServiceStatus> =
            if let Some(client) = self.client.as_mut() {
                client
                    .list()
                    .await
                    .map(|statuses| {
                        statuses
                            .into_iter()
                            .filter(|s| s.pid.is_some())
                            .map(|s| (s.name.clone(), s))
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                std::collections::HashMap::new()
            };

        for i in 0..self.services.len() {
            let state = &mut self.services[i];
            // Integrated services are not registered with the supervisor.
            if state.service.service_mode() == ServiceMode::Integrated {
                continue;
            }
            // Skip services still installing — the binary may not exist yet.
            // health_tick will register them once install completes.
            if matches!(
                state.phase,
                ServicePhase::Installing | ServicePhase::InstallFailed
            ) {
                continue;
            }
            let name = &state.name;
            if let Some(status) = running_services.get(name.as_str()) {
                tracing::info!(
                    "{name} already running in supervisor (pid {:?}), adopting",
                    status.pid
                );
                state.registered = true;
                state.phase = ServicePhase::Starting;

                // Compare the full spawn spec (program, args, env) and
                // the resolved binary path. Schedule a graceful restart
                // if either changed (e.g. args changed, or nix upgrade
                // installed a new store path for the same program name).
                let mut desired_spec = state.service.spawn_spec();
                for (k, v) in &state.connector_env {
                    desired_spec
                        .env
                        .entry(k.clone())
                        .or_insert_with(|| v.clone());
                }
                if let Some(running_spec) = &status.spec {
                    if *running_spec != desired_spec {
                        tracing::info!("{name} spec changed, scheduling restart");
                        state.restart_pending = true;
                    }
                }
                if !state.restart_pending {
                    let desired_resolved = which::which(&desired_spec.program)
                        .ok()
                        .and_then(|p| std::fs::canonicalize(p).ok());
                    let running_resolved = status
                        .resolved_program
                        .as_deref()
                        .map(std::path::PathBuf::from);
                    if let (Some(desired), Some(running)) = (&desired_resolved, &running_resolved) {
                        if desired != running {
                            tracing::info!(
                                "{name} binary changed ({} -> {}), scheduling upgrade",
                                running.display(),
                                desired.display(),
                            );
                            state.upgrade_pending = true;
                        }
                    }
                }
            } else {
                self.register_service(i).await;
            }
        }
        self.refresh_running_store_paths().await;
    }

    async fn register_service(&mut self, i: usize) {
        let Some(client) = self.client.as_mut() else {
            return;
        };
        let state = &mut self.services[i];
        let name = state.name.clone();
        if let Err(e) = state.service.configure() {
            tracing::warn!("{name} configure failed: {e}");
        }
        let mut spec = state.service.spawn_spec();
        // Merge environment variables contributed by connectors (e.g. API keys).
        for (k, v) in &state.connector_env {
            spec.env.entry(k.clone()).or_insert_with(|| v.clone());
        }
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
        let Some(client) = self.client.as_mut() else {
            return;
        };
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
        let Some(client) = self.client.as_mut() else {
            return;
        };
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
                        s.post_start_done = false;
                        // Exponential backoff: 5s, 10s, 20s, … capped at 60s.
                        let delay =
                            Duration::from_secs(5u64.saturating_mul(
                                1 << s.consecutive_crashes.min(4).saturating_sub(1),
                            ))
                            .min(Duration::from_secs(60));
                        s.restart_at = Some(Instant::now() + delay);
                        s.phase = ServicePhase::CrashBackoff;
                        tracing::info!(
                            "{name} entering crash backoff ({} crashes, delay {delay:?})",
                            s.consecutive_crashes,
                        );
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
        let has_managed = self
            .services
            .iter()
            .any(|s| s.service.service_mode() == ServiceMode::Managed);
        let client_ok = if has_managed {
            self.ensure_client().await
        } else {
            true
        };
        if !client_ok {
            for s in &mut self.services {
                Self::update_metrics(metrics, &s.name, false, s.upgrade_pending, false, s.phase);
            }
            return;
        }

        // Ensure every managed service has been registered at least once.
        // Skip services still installing or failed to install.
        for i in 0..self.services.len() {
            if !self.services[i].registered
                && self.services[i].service.service_mode() == ServiceMode::Managed
                && !matches!(
                    self.services[i].phase,
                    ServicePhase::Installing | ServicePhase::InstallFailed
                )
            {
                self.register_service(i).await;
            }
        }

        // Refresh running_store_path from the supervisor so that a restart
        // between upgrade-install and upgrade-apply is still detected.
        if has_managed {
            self.refresh_running_store_paths().await;
            self.drain_notifications();
        }

        // Phase 0: handle crash backoff expiry.
        // The supervisor auto-restarts crashed processes, so we don't
        // reregister here — that would kill the already-restarted process.
        // We just run repair (if needed) and transition back to Starting so
        // the next tick promotes to Healthy and resumes health checks.
        let now = Instant::now();
        for state in self.services.iter_mut() {
            // Integrated services don't crash-backoff via the supervisor.
            if state.service.service_mode() == ServiceMode::Integrated {
                continue;
            }
            if state.phase == ServicePhase::CrashBackoff {
                let ready = state.restart_at.map_or(true, |t| now >= t);
                if !ready {
                    continue;
                }
                let name = &state.name;
                if state.consecutive_crashes >= 2 {
                    tracing::info!("{name} crash backoff expired, attempting repair");
                    if let Err(e) = state.service.repair() {
                        tracing::error!("{name} repair failed: {e}");
                    }
                }
                state.restart_at = None;
                state.phase = ServicePhase::Starting;
                state.post_start_done = false;
            }
        }

        // Phase 1: lifecycle management.
        let mut busy_flags = vec![false; self.services.len()];
        let mut pending_reregisters: Vec<usize> = Vec::new();

        for (i, state) in self.services.iter_mut().enumerate() {
            let name = state.name.clone();
            sentry_ext::set_tag("service", &name);

            // Services in crash backoff skip normal lifecycle processing.
            if state.phase == ServicePhase::CrashBackoff {
                continue;
            }

            let integrated = state.service.service_mode() == ServiceMode::Integrated;

            let busy = state.service.is_busy().unwrap_or(false);
            busy_flags[i] = busy;

            // Supervisor-managed lifecycle (restart, upgrade, binary drift)
            // does not apply to integrated services.
            if !integrated {
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

                // Detect binary store-path drift — treated as an upgrade so
                // the restart waits for the upgrade window.
                if !state.upgrade_pending {
                    let current_store = crate::nix::binary_store_path(state.service.binary_name());
                    if let (Some(old), Some(new)) = (&state.running_store_path, &current_store) {
                        if old != new {
                            tracing::info!(
                                "{name} binary changed ({old} → {new}), scheduling upgrade"
                            );
                            state.upgrade_pending = true;
                        }
                    }
                }

                if state.upgrade_pending && in_upgrade_window {
                    pending_reregisters.push(i);
                    state.upgrade_pending = false;
                    continue;
                }
            }

            if state.phase == ServicePhase::Starting {
                state.phase = ServicePhase::Healthy;
            }
        }

        if !pending_reregisters.is_empty() {
            for i in pending_reregisters {
                let name = self.services[i].name.clone();
                self.reregister_service(i).await;
                self.dispatcher
                    .dispatch(&DaemonEvent::UpgradeInstalled { service: name });
            }
            // Refresh running_store_path immediately so the drift check on the
            // next tick sees the newly spawned binary, not the stale pre-reregister
            // value.  Without this, a transient supervisor-list failure on the next
            // tick would leave running_store_path stale, causing a false drift
            // detection and a spurious re-upgrade.
            if has_managed {
                self.refresh_running_store_paths().await;
            }
        }

        // Phase 2: concurrent health checks with timeout.
        use std::future::Future;
        use std::pin::Pin;
        let check_results: Vec<(usize, ServicePhase, Result<bool>)> = {
            let mut futs: Vec<
                Pin<Box<dyn Future<Output = (usize, ServicePhase, Result<bool>)> + Send + '_>>,
            > = Vec::new();
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
                        self.dispatcher.dispatch(&DaemonEvent::ServiceRecovered {
                            service: name.clone(),
                        });
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
                        self.dispatcher.dispatch(&DaemonEvent::ServiceUnhealthy {
                            service: name.clone(),
                        });
                    }
                    // Integrated services have no external process to repair.
                    if state.service.service_mode() != ServiceMode::Integrated {
                        if let Err(e) = state.service.repair() {
                            tracing::error!("{name} repair failed: {e}");
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("{name} health check failed: {e}");
                    // Treat errors (including timeouts) as unhealthy so the
                    // phase doesn't silently stay Healthy while checks fail.
                    if prev_phase != ServicePhase::Unhealthy {
                        state.phase = ServicePhase::Unhealthy;
                        self.dispatcher.dispatch(&DaemonEvent::ServiceUnhealthy {
                            service: name.clone(),
                        });
                    }
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
                state.phase,
            );
        }
    }

    pub fn run_connectors_tick(&mut self) {
        self.run_prestart_connectors();
        self.collect_connector_env_for_new();
        self.run_connectors();
        Self::update_backup_paths(&mut self.config_store, &self.services, &self.install_only);
    }

    /// Run pre-start connectors (config-patching). Skips connectors whose
    /// dependencies map to services still in `Installing` or `InstallFailed`.
    fn run_prestart_connectors(&mut self) {
        for i in 0..self.connectors.len() {
            if self.connectors[i].connector.phase() != ConnectorPhase::PreStart {
                continue;
            }
            let deps: Vec<&str> = self.connectors[i].connector.depends_on().to_vec();
            let deps_ready = deps.iter().all(|dep| {
                self.config_store.get(dep).is_some() && !self.dep_blocked_by_install(dep)
            });
            if !deps_ready {
                continue;
            }
            let should_run = !self.connectors[i].ran
                || self
                    .config_store
                    .any_changed(&deps, &self.connectors[i].last_snapshot);
            if !should_run {
                continue;
            }
            let name = self.connectors[i].connector.name();
            let configs = self.config_store.values_for(&deps);
            tracing::info!("running pre-start connector: {name}");
            if let Err(e) = self.connectors[i].connector.connect(&configs) {
                tracing::error!("pre-start connector {name} failed: {e}");
            }
            self.connectors[i].last_snapshot = self.config_store.snapshot(&deps);
            self.connectors[i].ran = true;
        }
    }

    /// Collect connector env vars for services that have left
    /// `Installing`/`InstallFailed` and haven't been collected yet.
    fn collect_connector_env_for_new(&mut self) {
        for state in &mut self.services {
            if state.connector_env_collected {
                continue;
            }
            if matches!(
                state.phase,
                ServicePhase::Installing | ServicePhase::InstallFailed
            ) {
                continue;
            }
            let mut env = std::collections::HashMap::new();
            for cs in &self.connectors {
                let deps = cs.connector.depends_on();
                let configs = self.config_store.values_for(deps);
                let vars = cs.connector.service_env(&state.name, &configs);
                env.extend(vars);
            }
            if !env.is_empty() {
                tracing::debug!(
                    "{}: {} connector env var(s) collected",
                    state.name,
                    env.len()
                );
            }
            state.connector_env = env;
            state.connector_env_collected = true;
        }
    }

    /// Run post-start connectors (need running services). Skips connectors
    /// whose dependencies map to services still installing.
    fn run_connectors(&mut self) {
        for i in 0..self.connectors.len() {
            if self.connectors[i].connector.phase() != ConnectorPhase::PostStart {
                continue;
            }
            let deps: Vec<&str> = self.connectors[i].connector.depends_on().to_vec();

            let deps_ready = deps.iter().all(|dep| {
                if self.dep_blocked_by_install(dep) {
                    return false;
                }
                if self.config_store.get(dep).is_some() {
                    return true;
                }
                self.services
                    .iter()
                    .find(|s| s.name == *dep)
                    .is_some_and(|s| s.post_start_done)
            });
            if !deps_ready {
                continue;
            }

            let should_run = !self.connectors[i].ran
                || self
                    .config_store
                    .any_changed(&deps, &self.connectors[i].last_snapshot);
            if !should_run {
                continue;
            }

            let name = self.connectors[i].connector.name();
            let configs = self.config_store.values_for(&deps);
            tracing::info!("running connector: {name}");
            if let Err(e) = self.connectors[i].connector.connect(&configs) {
                tracing::error!("connector {name} failed: {e}");
            }
            self.connectors[i].last_snapshot = self.config_store.snapshot(&deps);
            self.connectors[i].ran = true;
        }
    }

    fn update_metrics(
        metrics: &Metrics,
        name: &str,
        healthy: bool,
        upgrade_pending: bool,
        busy: bool,
        phase: ServicePhase,
    ) {
        metrics
            .service_healthy
            .with_label_values(&[name])
            .set(if healthy { 1 } else { 0 });
        metrics
            .service_upgrade_pending
            .with_label_values(&[name])
            .set(if upgrade_pending { 1 } else { 0 });
        metrics
            .service_busy
            .with_label_values(&[name])
            .set(if busy { 1 } else { 0 });
        metrics
            .service_phase
            .with_label_values(&[name])
            .set(match phase {
                ServicePhase::Installing => 5,
                ServicePhase::InstallFailed => 6,
                ServicePhase::Stopped => 0,
                ServicePhase::Starting => 1,
                ServicePhase::Healthy => 2,
                ServicePhase::Unhealthy => 3,
                ServicePhase::CrashBackoff => 4,
            });
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

    /// Update the config store and rebuild connectors after a config reload.
    ///
    /// This ensures connectors see new cloud/ollama/lms settings and that the
    /// connector list matches the current `default_llm` / `default_agent`.
    pub fn reload_connectors(&mut self, cfg: &crate::config::Config) {
        // Update config store entries so change detection works.
        if let Ok(v) = serde_json::to_value(&cfg.ollama) {
            self.config_store.set("ollama", v);
        }
        if let Ok(v) = serde_json::to_value(&cfg.lms) {
            self.config_store.set("lms", v);
        }
        if let Ok(v) = serde_json::to_value(&cfg.unsloth) {
            self.config_store.set("unsloth", v);
        }
        if let Ok(v) = serde_json::to_value(&cfg.litellm) {
            self.config_store.set("litellm", v);
        }
        if let Ok(v) = serde_json::to_value(&cfg.openclaw) {
            self.config_store.set("openclaw", v);
        }
        if let Ok(v) = serde_json::to_value(&cfg.opencode) {
            self.config_store.set("opencode", v);
        }
        if let Ok(v) = serde_json::to_value(&cfg.hermes) {
            self.config_store.set("hermes", v);
        }
        if let Ok(v) = serde_json::to_value(&cfg.cloud) {
            self.config_store.set("cloud", v);
        }
        if let Ok(v) = serde_json::to_value(&cfg.backup) {
            self.config_store.set("backup", v);
        }
        if let Ok(v) = serde_json::to_value(&cfg.memvault) {
            self.config_store.set("memvault", v);
        }

        // Refresh backup paths in the config store.
        Self::update_backup_paths(&mut self.config_store, &self.services, &self.install_only);

        // Rebuild the connector list from current config.
        let new_connectors = connectors::build_connectors(&cfg);
        self.connectors = new_connectors
            .into_iter()
            .map(|c| ConnectorState {
                connector: c,
                last_snapshot: ConnectorSnapshot::default(),
                ran: false,
            })
            .collect();
        tracing::info!(
            "connectors rebuilt: {}",
            self.connectors
                .iter()
                .map(|c| c.connector.name())
                .collect::<Vec<_>>()
                .join(", ")
        );

        // Run pre-start connectors so config patches are applied before
        // schedule_restart() restarts services, and re-collect connector env.
        self.run_prestart_connectors();
        self.collect_connector_env_for_new();
    }

    /// Apply an edited config at runtime (USB apply-on-save). Reconciles the
    /// running service set against the new config — installs + starts
    /// newly-enabled services, stops + unregisters ones that were disabled —
    /// and rebuilds connectors. No process restart required.
    ///
    /// Caller should run `retry_failed_installs()` + `schedule_restart()`
    /// afterwards to bring the new services up.
    pub async fn apply_config(&mut self, cfg: &mut crate::config::Config) {
        use std::collections::HashSet;

        // Snapshot for per-service change detection, before build_services
        // consumes the per-provider sections.
        let new_config = serde_json::to_value(&*cfg).unwrap_or(serde_json::Value::Null);
        let old_config = std::mem::replace(&mut self.last_config, new_config.clone());

        // Update the config store + connectors first — this reads the per-
        // provider sections, which `build_services` below then consumes.
        self.reload_connectors(cfg);

        // Desired vs current service sets (by name).
        let desired = connectors::build_services(cfg);
        let desired_names: HashSet<String> =
            desired.iter().map(|s| s.name().to_string()).collect();
        let current_names: HashSet<String> = self
            .services
            .iter()
            .map(|s| s.name.clone())
            .chain(self.install_only.iter().map(|s| s.name().to_string()))
            .collect();

        // ── Stop + unregister services that are no longer enabled.
        let to_remove: Vec<String> =
            current_names.difference(&desired_names).cloned().collect();
        if !to_remove.is_empty() && self.ensure_client().await {
            if let Some(client) = self.client.as_mut() {
                for name in &to_remove {
                    tracing::info!("apply_config: stopping disabled service {name}");
                    let _ = client.stop_service(name).await;
                    let _ = client.unregister(name).await;
                }
            }
        }
        self.services.retain(|s| !to_remove.contains(&s.name));
        self.install_only
            .retain(|s| !to_remove.contains(&s.name().to_string()));

        // ── Add newly-enabled services and install them.
        let mut added: Vec<Arc<dyn ManagedService>> = Vec::new();
        for svc in desired {
            if current_names.contains(svc.name()) {
                continue; // already present; config-only change handled above
            }
            let name = svc.name().to_string();
            tracing::info!("apply_config: enabling service {name}");
            if svc.service_mode() == ServiceMode::InstallOnly {
                self.install_only.push(Arc::clone(&svc));
            } else {
                self.services.push(ServiceState {
                    name,
                    service: Arc::clone(&svc),
                    phase: ServicePhase::Installing,
                    upgrade_pending: false,
                    restart_pending: false,
                    post_start_done: false,
                    consecutive_crashes: 0,
                    running_store_path: None,
                    registered: false,
                    restart_at: None,
                    connector_env: std::collections::HashMap::new(),
                    connector_env_collected: false,
                });
            }
            added.push(svc);
        }
        if !added.is_empty() {
            // The install runs in this detached task even though we drop the
            // update receiver (the USB loop tracks phases best-effort); the
            // restart_pending set below starts them once installed.
            let _ = Self::spawn_install_task(added.into_iter());
        }

        // ── Selective restart — only what actually changed:
        //   • newly-added running services → start them;
        //   • existing services whose config section changed → restart (or
        //     hot-reload). Unchanged services keep running untouched.
        for state in &mut self.services {
            if !current_names.contains(&state.name) {
                state.restart_pending = true; // newly added → bring up
                continue;
            }
            // Sub-services (e.g. `hermes-dashboard`) follow their parent
            // section (`hermes`).
            let section = state.name.split('-').next().unwrap_or(&state.name);
            if old_config.get(section) == new_config.get(section) {
                continue; // unchanged — leave it running
            }
            if state.service.supports_hot_reload() {
                match state.service.configure() {
                    Ok(()) => tracing::info!(
                        "apply_config: {} reconfigured (hot reload, no restart)",
                        state.name
                    ),
                    Err(e) => {
                        tracing::warn!(
                            "apply_config: {} configure failed, restarting: {e}",
                            state.name
                        );
                        state.restart_pending = true;
                    }
                }
            } else {
                tracing::info!("apply_config: {} config changed, restart pending", state.name);
                state.restart_pending = true;
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
                    "phase": s.phase.as_str(),
                })
            })
            .collect()
    }

    pub fn collect_tunnels(&self) -> Vec<TunnelDef> {
        self.services
            .iter()
            .flat_map(|s| s.service.expose_tunnels())
            .collect()
    }

    pub fn collect_tunnel_overrides(
        &self,
    ) -> std::collections::HashMap<String, Vec<crate::p2p::proxy_helpers::TunnelOverride>> {
        let mut all = std::collections::HashMap::new();
        for s in &self.services {
            for (name, overrides) in s.service.tunnel_overrides() {
                all.entry(name).or_insert_with(Vec::new).extend(overrides);
            }
        }
        all
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

    // ── Per-service assessments ─────────────────────────────────────

    /// Collect dynamic samples from all healthy managed services. Called per heartbeat.
    pub async fn collect_service_samples(&self) -> Vec<mac_mgmt_common::ServiceSample> {
        use mac_mgmt_common::ServiceSample;

        let futs: Vec<_> = self
            .services
            .iter()
            .filter(|s| s.phase.is_healthy())
            .map(|s| {
                let name = s.name.clone();
                let fut = s.service.service_sample();
                async move {
                    let entries =
                        tokio::time::timeout(std::time::Duration::from_secs(5), fut).await;
                    let entries = entries.unwrap_or_default();
                    if entries.is_empty() {
                        None
                    } else {
                        Some(ServiceSample {
                            service: name,
                            entries,
                        })
                    }
                }
            })
            .collect();
        futures_util::future::join_all(futs)
            .await
            .into_iter()
            .flatten()
            .collect()
    }

    /// Collect static inventory from all managed + install-only services.
    /// Called at the ~6h inventory cadence.
    pub async fn collect_service_inventories(&self) -> Vec<mac_mgmt_common::ServiceInventory> {
        use mac_mgmt_common::ServiceInventory;

        let timeout = std::time::Duration::from_secs(30);
        let mut futs: Vec<
            std::pin::Pin<Box<dyn std::future::Future<Output = Option<ServiceInventory>> + Send>>,
        > = Vec::new();

        for s in &self.services {
            let name = s.name.clone();
            let fut = s.service.service_inventory();
            futs.push(Box::pin(async move {
                let entries = tokio::time::timeout(timeout, fut).await.unwrap_or_default();
                if entries.is_empty() {
                    None
                } else {
                    Some(ServiceInventory {
                        service: name,
                        entries,
                    })
                }
            }));
        }
        for svc in &self.install_only {
            let name = svc.name().to_string();
            let fut = svc.service_inventory();
            futs.push(Box::pin(async move {
                let entries = tokio::time::timeout(timeout, fut).await.unwrap_or_default();
                if entries.is_empty() {
                    None
                } else {
                    Some(ServiceInventory {
                        service: name,
                        entries,
                    })
                }
            }));
        }

        futures_util::future::join_all(futs)
            .await
            .into_iter()
            .flatten()
            .collect()
    }

    /// Collect security findings from all managed + install-only services.
    /// Called at the ~6h inventory cadence.
    pub async fn collect_service_security(&self) -> Vec<mac_mgmt_common::ServiceSecurity> {
        use mac_mgmt_common::ServiceSecurity;

        let timeout = std::time::Duration::from_secs(30);
        let mut futs: Vec<
            std::pin::Pin<Box<dyn std::future::Future<Output = Option<ServiceSecurity>> + Send>>,
        > = Vec::new();

        for s in &self.services {
            let name = s.name.clone();
            let fut = s.service.service_security();
            futs.push(Box::pin(async move {
                let findings = tokio::time::timeout(timeout, fut).await.unwrap_or_default();
                if findings.is_empty() {
                    None
                } else {
                    Some(ServiceSecurity {
                        service: name,
                        findings,
                    })
                }
            }));
        }
        for svc in &self.install_only {
            let name = svc.name().to_string();
            let fut = svc.service_security();
            futs.push(Box::pin(async move {
                let findings = tokio::time::timeout(timeout, fut).await.unwrap_or_default();
                if findings.is_empty() {
                    None
                } else {
                    Some(ServiceSecurity {
                        service: name,
                        findings,
                    })
                }
            }));
        }

        futures_util::future::join_all(futs)
            .await
            .into_iter()
            .flatten()
            .collect()
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
    use crate::managed_service::FileTunnel;
    use crate::validator::Validator;
    vec![FileTunnel {
        def: FileTunnelDef::Folder {
            name: "daemon-config".into(),
            path: crate::config::config_dir().to_string_lossy().into(),
            writable: true,
            allow_write: Vec::new(),
            include: Some(vec!["config.toml".into(), "ollama-env".into()]),
            validators: vec![Validator::toml("config.toml")],
            description: "Daemon configuration directory".into(),
        },
        service: "daemon".into(),
    }]
}

/// System-level shell commands (not tied to a ManagedService).
fn daemon_system_shell_tunnels() -> Vec<ShellTunnel> {
    use crate::managed_service::{ShellArgTemplate, ShellCommandDef};
    vec![
        ShellTunnel {
            def: ShellCommandDef {
                name: "nix-profile-list".into(),
                command: "nix".into(),
                args: vec!["profile".into(), "list".into()],
                description: "List installed nix packages".into(),
                arg_template: None,
                timeout_secs: None,
            },
            service: "daemon".into(),
        },
        ShellTunnel {
            def: ShellCommandDef {
                name: "nix-collect-garbage".into(),
                command: "nix-collect-garbage".into(),
                args: vec!["--delete-old".into()],
                description: "Delete old nix generations and collect garbage".into(),
                arg_template: None,
                timeout_secs: None,
            },
            service: "daemon".into(),
        },
        ShellTunnel {
            def: ShellCommandDef {
                name: "nix-store-gc-print".into(),
                command: "nix-store".into(),
                args: vec!["--gc".into(), "--print-dead".into()],
                description: "Show reclaimable nix store space (dry run)".into(),
                arg_template: None,
                timeout_secs: None,
            },
            service: "daemon".into(),
        },
        ShellTunnel {
            def: ShellCommandDef {
                name: "df".into(),
                command: "df".into(),
                args: vec!["-h".into()],
                description: "Disk usage (human-readable)".into(),
                arg_template: None,
                timeout_secs: None,
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
                timeout_secs: None,
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
                timeout_secs: None,
            },
            service: "daemon".into(),
        },
        ShellTunnel {
            def: ShellCommandDef {
                name: "service-journal".into(),
                command: "journalctl".into(),
                args: vec!["-n".into(), "100".into(), "--no-pager".into(), "-u".into()],
                description: "System journal for a service unit (e.g. ollama, openclaw)".into(),
                arg_template: Some(ShellArgTemplate {
                    label: "Service unit name".into(),
                    placeholder: "ollama".into(),
                    validation: Some(r"^[a-zA-Z0-9._@-]+$".into()),
                }),
                timeout_secs: None,
            },
            service: "daemon".into(),
        },
        ShellTunnel {
            def: ShellCommandDef {
                name: "systemctl-status".into(),
                command: "systemctl".into(),
                args: vec!["status".into()],
                description: "Status of a systemd service unit".into(),
                arg_template: Some(ShellArgTemplate {
                    label: "Service unit name".into(),
                    placeholder: "ollama".into(),
                    validation: Some(r"^[a-zA-Z0-9._@-]+$".into()),
                }),
                timeout_secs: None,
            },
            service: "daemon".into(),
        },
        // Virtual commands — handled by callbacks, not spawned processes.
        // The actual handlers are registered via register_virtual_handlers().
        ShellTunnel {
            def: ShellCommandDef {
                name: "service-restart".into(),
                command: String::new(), // virtual — not spawned
                args: Vec::new(),
                description: "Restart a managed service via the supervisor (e.g. ollama, openclaw)"
                    .into(),
                arg_template: Some(ShellArgTemplate {
                    label: "Service name".into(),
                    placeholder: "ollama".into(),
                    validation: Some(r"^[a-zA-Z0-9._-]+$".into()),
                }),
                timeout_secs: None,
            },
            service: "daemon".into(),
        },
        ShellTunnel {
            def: ShellCommandDef {
                name: "restart-daemon".into(),
                command: String::new(), // virtual — not spawned
                args: Vec::new(),
                description:
                    "Stop the mac-mgmt daemon (the service manager will restart it automatically)"
                        .into(),
                arg_template: None,
                timeout_secs: None,
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
