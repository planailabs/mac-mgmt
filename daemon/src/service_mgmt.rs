use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

use crate::connectors::{self, Connector};
use crate::events::DaemonEvent;
use crate::log_buffer::LogBuffer;
use crate::config_providers::{ConfigStore, ConnectorSnapshot};
use crate::managed_service::{ManagedService, ServiceMode, TunnelDef};
use crate::metrics::Metrics;
use crate::notify::Dispatcher;
use crate::service_ipc::client::ManagedClient;
use crate::service_ipc::protocol::{IpcRequest, IpcResponse, IpcNotification};
use crate::sentry_ext;

fn hash_current_exe() -> Option<Vec<u8>> {
    use sha2::{Digest, Sha256};
    let exe = std::env::current_exe().ok()?;
    let bytes = std::fs::read(&exe).ok()?;
    Some(Sha256::digest(&bytes).to_vec())
}

// ── Service lifecycle state machine ──────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServicePhase {
    /// Not spawned / not connected.
    Stopped,
    /// Just spawned, grace period before first real health check.
    Starting,
    /// Running, health checks passing.
    Healthy,
    /// Running, health checks failing.
    Unhealthy,
    /// Process exited unexpectedly, awaiting respawn.
    Crashed,
}

impl ServicePhase {
    fn is_healthy(self) -> bool {
        self == Self::Healthy
    }
}

// ── Inline (child-process) backend state ─────────────────────────────

struct InlineServiceState {
    service: Box<dyn ManagedService>,
    child: Option<std::process::Child>,
    phase: ServicePhase,
    upgrade_pending: bool,
    restart_pending: bool,
    post_start_done: bool,
    consecutive_crashes: u32,
    log_task: Option<JoinHandle<()>>,
}

// ── External (IPC) backend state ─────────────────────────────────────

struct ExternalServiceState {
    service_name: String,
    service: Box<dyn ManagedService>,
    client: Option<ManagedClient>,
    phase: ServicePhase,
    upgrade_pending: bool,
    update_self_pending: bool,
    restart_pending: bool,
    post_start_done: bool,
    consecutive_crashes: u32,
    running_store_path: Option<String>,
    /// Hash of the mac-mgmt binary when the wrapper was last connected/spawned.
    /// Compared on each health tick to detect stale wrappers after self-update.
    wrapper_binary_hash: Option<Vec<u8>>,
}

impl ExternalServiceState {
    /// Send a request to the wrapper. On IPC error, disconnects the client
    /// so the next health tick will reconnect.
    async fn send(&mut self, req: &IpcRequest) -> Option<IpcResponse> {
        let Some(ref mut client) = self.client else {
            return None;
        };
        match client.request(req).await {
            Ok(resp) => Some(resp),
            Err(e) => {
                tracing::warn!("{}: IPC error, disconnecting: {e}", self.service_name);
                self.client = None;
                None
            }
        }
    }

    /// Drain all pending notifications. Disconnects on channel close.
    fn drain_notifications(&mut self, name: &str, log_buf: &LogBuffer, dispatcher: &Dispatcher) {
        let Some(ref mut client) = self.client else { return };
        loop {
            match client.try_recv_notification() {
                Some(IpcNotification::Crashed { exit_code }) => {
                    self.consecutive_crashes += 1;
                    tracing::warn!("{name} crashed (#{}, exit: {exit_code:?})", self.consecutive_crashes);
                    dispatcher.dispatch(&DaemonEvent::ServiceCrashed {
                        service: name.to_string(),
                        exit_code,
                    });
                    // External services are respawned by the wrapper, so from
                    // the daemon's perspective a crash is just "unhealthy until
                    // the wrapper brings it back."  Using Unhealthy (not Crashed)
                    // keeps the health-check loop running so recovery is detected.
                    self.phase = ServicePhase::Unhealthy;
                    self.post_start_done = false;

                    if self.consecutive_crashes >= 2 {
                        if let Err(e) = self.service.repair() {
                            tracing::error!("{name} repair failed: {e}");
                        }
                    }
                }
                Some(IpcNotification::Log { line, .. }) => {
                    log_buf.push(format!("[{name}] {line}"));
                }
                None => break,
            }
        }
    }

    /// Send Spawn to the wrapper and update state.
    async fn spawn_via_wrapper(&mut self) {
        let name = self.service_name.clone();
        let spec = self.service.spawn_spec();
        self.running_store_path = crate::nix::binary_store_path(self.service.binary_name());
        match self.send(&IpcRequest::Spawn(spec)).await {
            Some(IpcResponse::Ok) => {
                tracing::info!("{name} spawned via wrapper");
                self.phase = ServicePhase::Starting;
                self.post_start_done = false;
            }
            Some(IpcResponse::Error { message }) => {
                tracing::error!("{name} Spawn failed: {message}");
            }
            None => {} // disconnected, will reconnect next tick
        }
    }
}

struct ConnectorState {
    connector: Box<dyn Connector>,
    /// Snapshot of config provider versions when the connector last ran.
    last_snapshot: ConnectorSnapshot,
    /// Whether the connector has run at least once.
    ran: bool,
}

enum ServiceBackend {
    Inline(Vec<InlineServiceState>),
    External(Vec<ExternalServiceState>),
}

pub struct ServiceManager {
    backend: ServiceBackend,
    install_only: Vec<Box<dyn ManagedService>>,
    connectors: Vec<ConnectorState>,
    dispatcher: Arc<Dispatcher>,
    log_buf: LogBuffer,
    /// Reactive config store for connector dependencies.
    pub config_store: ConfigStore,
}

impl ServiceManager {
    pub fn init(
        cfg: &mut crate::config::Config,
        dispatcher: Arc<Dispatcher>,
        log_buf: LogBuffer,
    ) -> Result<Self> {
        let mut external = cfg.global.external_processes;

        // Auto-detect: if external processes are requested but the systemd
        // user bus isn't available (no login session), fall back to inline.
        if external {
            let uid = unsafe { libc::getuid() };
            let bus_path = format!("/run/user/{uid}/bus");
            if !std::path::Path::new(&bus_path).exists() {
                tracing::warn!(
                    "external_processes=true but systemd user bus not found ({bus_path}), falling back to inline"
                );
                external = false;
            }
        }

        let global_cfg = std::mem::take(&mut cfg.global);
        let openclaw_cfg = std::mem::take(&mut cfg.openclaw);
        let ollama_cfg = std::mem::take(&mut cfg.ollama);
        let nexa_cfg = std::mem::take(&mut cfg.nexa);
        let lms_cfg = std::mem::take(&mut cfg.lms);
        let cloud_cfg = std::mem::take(&mut cfg.cloud);

        // Initialize config store with disk cache for relay/persistent state.
        let cache_dir = crate::config::config_dir().join("config_providers.json");
        let mut config_store = ConfigStore::new(Some(cache_dir));

        // Register service configs as providers.
        if let Ok(v) = serde_json::to_value(&ollama_cfg) {
            config_store.set("ollama", v);
        }
        if let Ok(v) = serde_json::to_value(&nexa_cfg) {
            config_store.set("nexa", v);
        }
        if let Ok(v) = serde_json::to_value(&lms_cfg) {
            config_store.set("lms", v);
        }
        if let Ok(v) = serde_json::to_value(&openclaw_cfg) {
            config_store.set("openclaw", v);
        }
        if let Ok(v) = serde_json::to_value(&cloud_cfg) {
            config_store.set("cloud", v);
        }

        let connectors = connectors::build_connectors(
            &global_cfg, &ollama_cfg, &nexa_cfg, &lms_cfg, &cloud_cfg,
        );

        let services = connectors::build_services(
            &global_cfg, openclaw_cfg, ollama_cfg, nexa_cfg, lms_cfg,
        );

        let mut install_only: Vec<Box<dyn ManagedService>> = Vec::new();

        let backend = if external {
            tracing::info!("external_processes=true, using per-service system units");

            let desired_names: Vec<String> = services
                .iter()
                .filter(|s| s.service_mode() != ServiceMode::InstallOnly)
                .map(|s| s.name().to_string())
                .collect();

            if let Ok(existing) = crate::service::list_managed_service_units() {
                for name in &existing {
                    if !desired_names.contains(name) {
                        tracing::info!("cleaning up stale managed service unit: {name}");
                        if let Err(e) = crate::service::cleanup_managed_service(name) {
                            tracing::warn!("cleanup of {name} failed: {e}");
                        }
                    }
                }
            }

            let mut external_states = Vec::new();

            for service in services {
                let name = service.name().to_string();

                if service.service_mode() == ServiceMode::InstallOnly {
                    service.ensure_installed()?;
                    service.ensure_setup()?;
                    tracing::info!("{name} is install-only, handled directly");
                    install_only.push(service);
                    continue;
                }

                service.ensure_installed()?;
                service.ensure_setup()?;

                if crate::service::is_managed_service_installed(&name) {
                    tracing::info!("{name} service unit already installed, ensuring running");
                    if let Err(e) = crate::service::start_managed_service(&name) {
                        tracing::warn!("{name} start failed, reinstalling: {e}");
                        if let Err(e) = crate::service::install_managed_service(&name) {
                            tracing::error!("failed to install managed service unit for {name}: {e}");
                            continue;
                        }
                    }
                } else if let Err(e) = crate::service::install_managed_service(&name) {
                    tracing::error!("failed to install managed service unit for {name}: {e}");
                    sentry_ext::capture_error(
                        &format!("failed to install managed service {name}: {e}"),
                        &[("service", &name)],
                    );
                    continue;
                }

                external_states.push(ExternalServiceState {
                    service_name: name,
                    service,
                    client: None,
                    phase: ServicePhase::Stopped,
                    upgrade_pending: false,
                    update_self_pending: false,
                    restart_pending: false,
                    post_start_done: false,
                    consecutive_crashes: 0,
                    running_store_path: None,
                    wrapper_binary_hash: None,
                });
            }

            ServiceBackend::External(external_states)
        } else {
            if let Ok(existing) = crate::service::list_managed_service_units() {
                for name in &existing {
                    tracing::info!("cleaning up stale managed service unit (external_processes=false): {name}");
                    if let Err(e) = crate::service::cleanup_managed_service(name) {
                        tracing::warn!("cleanup of {name} failed: {e}");
                    }
                }
            }

            let mut inline_states = Vec::new();

            for service in services {
                let name = service.name().to_string();
                service.ensure_installed()?;
                service.ensure_setup()?;

                if service.service_mode() == ServiceMode::InstallOnly {
                    tracing::info!("{name} is install-only, skipping spawn");
                    sentry_ext::breadcrumb(
                        "service",
                        &format!("{name} installed (install-only)"),
                        &[("service", &name)],
                    );
                    install_only.push(service);
                    continue;
                }

                service.preflight()?;
                sentry_ext::breadcrumb(
                    "service",
                    &format!("{name} installed and ready"),
                    &[("service", &name)],
                );
                inline_states.push(InlineServiceState {
                    service,
                    child: None,
                    phase: ServicePhase::Stopped,
                    upgrade_pending: false,
                    restart_pending: false,
                    post_start_done: false,
                    consecutive_crashes: 0,
                    log_task: None,
                });
            }

            ServiceBackend::Inline(inline_states)
        };

        let connectors = connectors
            .into_iter()
            .map(|c| ConnectorState {
                connector: c,
                last_snapshot: ConnectorSnapshot::default(),
                ran: false,
            })
            .collect();

        Ok(Self {
            backend,
            install_only,
            connectors,
            dispatcher,
            log_buf,
            config_store,
        })
    }

    /// Register service-specific Prometheus metrics with the given Metrics instance.
    /// Call once after init, before the first health tick.
    pub fn register_metrics(&self, metrics: &Metrics) {
        let services: Vec<&dyn ManagedService> = match &self.backend {
            ServiceBackend::Inline(states) => states.iter().map(|s| s.service.as_ref()).collect(),
            ServiceBackend::External(states) => states.iter().map(|s| s.service.as_ref()).collect(),
        };
        for svc in services {
            for collector in svc.metric_collectors() {
                if let Err(e) = metrics.register_collector(collector) {
                    tracing::warn!("{}: failed to register metric: {e}", svc.name());
                }
            }
        }
    }

    // ── Spawn / Connect ──────────────────────────────────────────────

    pub fn spawn_all(&mut self) {
        let ServiceBackend::Inline(ref mut states) = self.backend else {
            return;
        };
        for state in states {
            let name = state.service.name();
            match state.service.spawn() {
                Ok(mut child) => {
                    let log_task = crate::log_capture::capture(name, &mut child, &self.log_buf);
                    tracing::info!("{name} spawned (pid: {})", child.id());
                    state.child = Some(child);
                    state.log_task = Some(log_task);
                    state.phase = ServicePhase::Starting;
                }
                Err(e) => {
                    tracing::error!("{name} spawn failed: {e}");
                }
            }
        }
    }

    pub async fn connect_all(&mut self) {
        let ServiceBackend::External(ref mut states) = self.backend else {
            return;
        };
        for state in states {
            Self::connect_and_spawn(state).await;
        }
    }

    async fn connect_and_spawn(state: &mut ExternalServiceState) {
        let name = &state.service_name;
        let path = crate::service_ipc::socket_path(name);
        tracing::info!("connecting to {name} wrapper at {}", path.display());
        match ManagedClient::connect(&path, Duration::from_secs(30)).await {
            Ok(client) => {
                tracing::info!("connected to {name} wrapper");
                state.client = Some(client);
                state.wrapper_binary_hash = hash_current_exe();
                if let Err(e) = state.service.configure() {
                    tracing::warn!("{name} configure failed: {e}");
                }
                if let Err(e) = state.service.preflight() {
                    tracing::warn!("{name} preflight failed: {e}");
                }
                state.spawn_via_wrapper().await;
            }
            Err(e) => {
                tracing::error!("failed to connect to {name} wrapper: {e}");
            }
        }
    }

    // ── Upgrades ─────────────────────────────────────────────────────

    pub fn check_upgrades(&mut self) {
        for svc in &self.install_only {
            let name = svc.name();
            sentry_ext::set_tag("service", &name);
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

        match &mut self.backend {
            ServiceBackend::Inline(states) => {
                for state in states {
                    if state.upgrade_pending {
                        continue;
                    }
                    let name = state.service.name();
                    sentry_ext::set_tag("service", &name);
                    match state.service.check_and_upgrade() {
                        Ok(true) => { state.upgrade_pending = true; }
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
            }
            ServiceBackend::External(states) => {
                for state in states {
                    if state.upgrade_pending {
                        continue;
                    }
                    let name = state.service.name();
                    sentry_ext::set_tag("service", &name);
                    match state.service.check_and_upgrade() {
                        Ok(true) => { state.upgrade_pending = true; }
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
            }
        }
    }

    // ── Inline respawn ───────────────────────────────────────────────

    fn respawn_inline(state: &mut InlineServiceState, log_buf: &LogBuffer) -> bool {
        if let Some(ref mut child) = state.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(task) = state.log_task.take() {
            task.abort();
        }
        let name = state.service.name();
        if let Err(e) = state.service.configure() {
            tracing::warn!("{name} configure before respawn failed: {e}");
        }
        match state.service.spawn() {
            Ok(mut child) => {
                state.log_task = Some(crate::log_capture::capture(name, &mut child, log_buf));
                state.child = Some(child);
                state.phase = ServicePhase::Starting;
                state.post_start_done = false;
                true
            }
            Err(e) => {
                tracing::error!("{name} spawn failed: {e}");
                false
            }
        }
    }

    // ── Health tick ──────────────────────────────────────────────────

    pub async fn health_tick(&mut self, metrics: &Arc<Metrics>, in_upgrade_window: bool) {
        match &mut self.backend {
            ServiceBackend::Inline(states) => {
                Self::health_tick_inline(states, &self.log_buf, &self.dispatcher, metrics, in_upgrade_window);
            }
            ServiceBackend::External(states) => {
                Self::health_tick_external(states, &self.dispatcher, &self.log_buf, metrics, in_upgrade_window).await;
            }
        }
        self.run_connectors();
    }

    fn health_tick_inline(
        states: &mut [InlineServiceState],
        log_buf: &LogBuffer,
        dispatcher: &Dispatcher,
        metrics: &Metrics,
        in_upgrade_window: bool,
    ) {
        for state in states {
            let name = state.service.name().to_string();
            sentry_ext::set_tag("service", &name);

            if state.phase == ServicePhase::Stopped { continue; }

            // ── Detect crash (process exited) ────────────────────────
            if let Some(ref mut child) = state.child {
                if let Ok(Some(status)) = child.try_wait() {
                    state.consecutive_crashes += 1;
                    tracing::warn!("{name} exited with {status} (crash #{})", state.consecutive_crashes);
                    log_buf.push(format!("[{name}] crashed with {status} (#{crashes})", crashes = state.consecutive_crashes));
                    dispatcher.dispatch(&DaemonEvent::ServiceCrashed {
                        service: name.to_string(),
                        exit_code: status.code(),
                    });
                    state.phase = ServicePhase::Crashed;

                    if state.consecutive_crashes >= 2 {
                        if let Err(e) = state.service.repair() {
                            tracing::error!("{name} repair failed: {e}");
                        }
                    }

                    // Immediate respawn attempt.
                    if Self::respawn_inline(state, log_buf) {
                        state.upgrade_pending = false;
                    }
                    // phase is now Starting (respawn) or still Crashed (failed)
                }
            }

            // ── Check if service needs a restart due to external changes
            if !state.restart_pending && state.phase.is_healthy() && state.service.needs_restart() {
                tracing::info!("{name} needs restart (external change detected)");
                state.restart_pending = true;
            }

            // ── Pending restart when idle ─────────────────────────────
            if state.restart_pending {
                let busy = state.service.is_busy().unwrap_or(false);
                if !busy || in_upgrade_window {
                    if Self::respawn_inline(state, log_buf) {
                        state.restart_pending = false;
                        state.upgrade_pending = false;
                    }
                }
            }

            // ── Pending upgrade when idle ─────────────────────────────
            let busy = if state.upgrade_pending && in_upgrade_window {
                let busy = state.service.is_busy().unwrap_or(false);
                if !busy {
                    if Self::respawn_inline(state, log_buf) {
                        state.upgrade_pending = false;
                        dispatcher.dispatch(&DaemonEvent::UpgradeInstalled { service: name.to_string() });
                    }
                }
                busy
            } else { false };

            // ── Phase transitions (health check) ─────────────────────
            let prev_phase = state.phase;
            match state.phase {
                ServicePhase::Starting => {
                    // Grace period: assume healthy, real check next tick.
                    state.phase = ServicePhase::Healthy;
                }
                ServicePhase::Healthy | ServicePhase::Unhealthy => {
                    match state.service.check_health() {
                        Ok(true) => {
                            state.consecutive_crashes = 0;
                            state.phase = ServicePhase::Healthy;
                            if prev_phase == ServicePhase::Unhealthy {
                                dispatcher.dispatch(&DaemonEvent::ServiceRecovered { service: name.to_string() });
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
                                dispatcher.dispatch(&DaemonEvent::ServiceUnhealthy { service: name.to_string() });
                            }
                            if let Err(e) = state.service.repair() {
                                tracing::error!("{name} repair failed: {e}");
                            }
                        }
                        Err(e) => {
                            tracing::warn!("{name} health check failed: {e}");
                            state.phase = ServicePhase::Unhealthy;
                            if prev_phase != ServicePhase::Unhealthy {
                                dispatcher.dispatch(&DaemonEvent::ServiceUnhealthy { service: name.to_string() });
                            }
                        }
                    }
                }
                ServicePhase::Crashed | ServicePhase::Stopped => {}
            }

            state.service.collect_metrics();
            Self::update_metrics(metrics, &name, state.phase.is_healthy(), state.upgrade_pending, busy);
        }
    }

    async fn health_tick_external(
        states: &mut [ExternalServiceState],
        dispatcher: &Dispatcher,
        log_buf: &LogBuffer,
        metrics: &Metrics,
        in_upgrade_window: bool,
    ) {
        for state in states {
            let name = state.service_name.clone();
            sentry_ext::set_tag("service", &name);

            // ── Reconnect if disconnected, re-send Spawn ─────────────
            if state.client.is_none() {
                let path = crate::service_ipc::socket_path(&name);
                match ManagedClient::connect(&path, Duration::from_secs(5)).await {
                    Ok(client) => {
                        tracing::info!("reconnected to {name} wrapper");
                        state.client = Some(client);
                        state.wrapper_binary_hash = hash_current_exe();
                        if let Err(e) = state.service.configure() {
                            tracing::warn!("{name} configure on reconnect: {e}");
                        }
                        state.spawn_via_wrapper().await;
                    }
                    Err(_) => {
                        tracing::warn!("{name} wrapper not reachable, restarting unit");
                        let _ = crate::service::start_managed_service(&name);
                        state.phase = ServicePhase::Stopped;
                        Self::update_metrics(metrics, &name, false, state.upgrade_pending, false);
                        continue;
                    }
                }
            }

            // ── Drain notifications (logs, crashes) ──────────────────
            state.drain_notifications(&name, log_buf, dispatcher);

            // Check wrapper is still alive by peeking at the notification channel.
            if state.client.as_mut().is_some_and(|c| c.is_disconnected()) {
                tracing::warn!("{name} wrapper connection lost");
                state.client = None;
                state.phase = ServicePhase::Stopped;
                Self::update_metrics(metrics, &name, false, state.upgrade_pending, false);
                continue;
            }

            let busy = state.service.is_busy().unwrap_or(false);

            // ── Check if service needs a restart due to external changes
            if !state.restart_pending && state.phase.is_healthy() && state.service.needs_restart() {
                tracing::info!("{name} needs restart (external change detected)");
                state.restart_pending = true;
            }

            // ── Pending restart ──────────────────────────────────────
            if state.restart_pending && (!busy || in_upgrade_window) {
                if let Err(e) = state.service.configure() {
                    tracing::warn!("{name} configure failed: {e}");
                }
                state.spawn_via_wrapper().await;
                state.restart_pending = false;
                state.upgrade_pending = false;
            }

            // ── Pending upgrade ──────────────────────────────────────
            if state.upgrade_pending && in_upgrade_window && (!busy || in_upgrade_window) {
                state.spawn_via_wrapper().await;
                if state.phase == ServicePhase::Starting { // spawn succeeded
                    state.upgrade_pending = false;
                    dispatcher.dispatch(&DaemonEvent::UpgradeInstalled { service: name.clone() });
                }
            }

            // ── Binary store path drift ──────────────────────────────
            let current_store = crate::nix::binary_store_path(state.service.binary_name());
            if let (Some(old), Some(new)) = (&state.running_store_path, &current_store) {
                if old != new && (!busy || in_upgrade_window) {
                    tracing::info!("{name} binary changed ({old} → {new}), restarting");
                    state.spawn_via_wrapper().await;
                }
            }

            // ── Phase transitions (health check) ─────────────────────
            let prev_phase = state.phase;
            match state.phase {
                ServicePhase::Starting => {
                    // Grace period: assume healthy, real check next tick.
                    state.phase = ServicePhase::Healthy;
                }
                ServicePhase::Healthy | ServicePhase::Unhealthy => {
                    match state.service.check_health() {
                        Ok(true) => {
                            state.consecutive_crashes = 0;
                            state.phase = ServicePhase::Healthy;
                            if prev_phase == ServicePhase::Unhealthy {
                                dispatcher.dispatch(&DaemonEvent::ServiceRecovered { service: name.clone() });
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
                                dispatcher.dispatch(&DaemonEvent::ServiceUnhealthy { service: name.clone() });
                            }
                            if let Err(e) = state.service.repair() {
                                tracing::error!("{name} repair failed: {e}");
                            }
                        }
                        Err(e) => {
                            tracing::warn!("{name} health check failed: {e}");
                            state.phase = ServicePhase::Unhealthy;
                            if prev_phase != ServicePhase::Unhealthy {
                                dispatcher.dispatch(&DaemonEvent::ServiceUnhealthy { service: name.clone() });
                            }
                        }
                    }
                }
                ServicePhase::Crashed | ServicePhase::Stopped => {}
            }

            // ── Detect wrapper running an outdated mac-mgmt binary ───
            if !state.update_self_pending {
                let current_hash = hash_current_exe();
                if state.wrapper_binary_hash != current_hash {
                    tracing::info!("{name} wrapper is running an outdated binary");
                    state.update_self_pending = true;
                }
            }

            // ── Deferred update-self when idle ───────────────────────
            if state.update_self_pending && !busy {
                if let Some(ref mut client) = state.client {
                    Self::do_send_update_self(client, &name).await;
                    state.wrapper_binary_hash = hash_current_exe();
                }
                state.update_self_pending = false;
            }

            state.service.collect_metrics();
            Self::update_metrics(metrics, &name, state.phase.is_healthy(), state.upgrade_pending, busy);
        }
    }

    fn run_connectors(&mut self) {
        for cs in &mut self.connectors {
            let deps = cs.connector.depends_on();

            // All deps must be satisfied: either a running service or a config provider.
            let deps_ready = deps.iter().all(|dep| {
                if self.config_store.get(dep).is_some() {
                    return true;
                }
                match &self.backend {
                    ServiceBackend::Inline(states) => states.iter()
                        .find(|s| s.service.name() == *dep)
                        .is_some_and(|s| s.post_start_done),
                    ServiceBackend::External(states) => states.iter()
                        .find(|s| s.service_name == *dep)
                        .is_some_and(|s| s.post_start_done),
                }
            });
            if !deps_ready { continue; }

            // Re-run if never ran, or if any config provider dependency changed.
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
        match &mut self.backend {
            ServiceBackend::Inline(states) => {
                for state in states {
                    let name = state.service.name().to_string();
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
            ServiceBackend::External(states) => {
                for state in states {
                    let name = &state.service_name;
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
        }
    }

    // ── Status collection ────────────────────────────────────────────

    pub fn collect_statuses(&self) -> Vec<serde_json::Value> {
        match &self.backend {
            ServiceBackend::Inline(states) => states.iter().map(|s| {
                serde_json::json!({
                    "name": s.service.name(),
                    "healthy": s.phase.is_healthy(),
                    "upgrade_pending": s.upgrade_pending,
                    "busy": false,
                })
            }).collect(),
            ServiceBackend::External(states) => states.iter().map(|s| {
                serde_json::json!({
                    "name": s.service_name,
                    "healthy": s.phase.is_healthy(),
                    "upgrade_pending": s.upgrade_pending,
                    "busy": false,
                })
            }).collect(),
        }
    }

    // ── Tunnel collection ────────────────────────────────────────────

    pub fn collect_tunnels(&self) -> Vec<TunnelDef> {
        match &self.backend {
            ServiceBackend::Inline(states) => states
                .iter()
                .filter(|s| s.phase.is_healthy())
                .flat_map(|s| s.service.expose_tunnels())
                .collect(),
            ServiceBackend::External(states) => states
                .iter()
                .filter(|s| s.phase.is_healthy())
                .flat_map(|s| s.service.expose_tunnels())
                .collect(),
        }
    }

    // ── Shutdown ─────────────────────────────────────────────────────

    pub async fn shutdown(&mut self) {
        match &mut self.backend {
            ServiceBackend::Inline(states) => {
                for state in states.iter() {
                    if let Some(ref child) = state.child {
                        let pid = child.id();
                        tracing::info!("sending SIGTERM to {} (pid {pid})", state.service.name());
                        unsafe { libc::kill(pid as i32, libc::SIGTERM); }
                    }
                }
                let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
                for state in states.iter_mut() {
                    let Some(ref mut child) = state.child else { continue };
                    let name = state.service.name();
                    loop {
                        match child.try_wait() {
                            Ok(Some(_)) => break,
                            Ok(None) if tokio::time::Instant::now() >= deadline => {
                                tracing::warn!("{name} did not exit, SIGKILL");
                                let _ = child.kill();
                                let _ = child.wait();
                                break;
                            }
                            Ok(None) => tokio::time::sleep(Duration::from_millis(100)).await,
                            Err(_) => break,
                        }
                    }
                    if let Some(task) = state.log_task.take() { task.abort(); }
                }
            }
            ServiceBackend::External(_) => {
                tracing::info!("leaving external services running");
            }
        }
    }

    #[allow(dead_code)]
    pub async fn send_update_self(&mut self) {
        let ServiceBackend::External(ref mut states) = self.backend else { return };
        for state in states {
            let name = &state.service_name;
            let busy = state.service.is_busy().unwrap_or(false);
            let Some(ref mut client) = state.client else {
                state.update_self_pending = true;
                continue;
            };
            if busy {
                tracing::info!("{name} is busy, deferring update-self");
                state.update_self_pending = true;
                continue;
            }
            Self::do_send_update_self(client, name).await;
        }
    }

    async fn do_send_update_self(client: &mut ManagedClient, name: &str) {
        match client.request(&IpcRequest::UpdateSelf).await {
            Ok(IpcResponse::Ok) => tracing::info!("{name} update-self sent"),
            Ok(IpcResponse::Error { message }) => tracing::warn!("{name} update-self failed: {message}"),
            Err(e) => tracing::warn!("{name} update-self IPC failed: {e}"),
        }
    }
}
