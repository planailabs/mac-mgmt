use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

use crate::connectors::{self, Connector};
use crate::events::DaemonEvent;
use crate::log_buffer::LogBuffer;
use crate::managed_service::{ManagedService, ServiceMode};
use crate::metrics::Metrics;
use crate::notify::Dispatcher;
use crate::service_ipc::client::ManagedClient;
use crate::service_ipc::protocol::{IpcRequest, IpcResponse, IpcNotification};
use crate::sentry_ext;

// ── Inline (child-process) backend state ─────────────────────────────

struct InlineServiceState {
    service: Box<dyn ManagedService>,
    child: Option<std::process::Child>,
    healthy: bool,
    upgrade_pending: bool,
    restart_pending: bool,
    skip_health_check: bool,
    post_start_done: bool,
    consecutive_crashes: u32,
    was_unhealthy: bool,
    log_task: Option<JoinHandle<()>>,
}

// ── External (IPC) backend state ─────────────────────────────────────

struct ExternalServiceState {
    service_name: String,
    service: Box<dyn ManagedService>,
    client: Option<ManagedClient>,
    healthy: bool,
    upgrade_pending: bool,
    update_self_pending: bool,
    restart_pending: bool,
    skip_health_check: bool,
    post_start_done: bool,
    consecutive_crashes: u32,
    was_unhealthy: bool,
    /// Store path of the service binary when last spawned.
    running_store_path: Option<String>,
}

struct ConnectorState {
    connector: Box<dyn Connector>,
    done: bool,
}

enum ServiceBackend {
    /// Original: daemon owns child processes directly.
    Inline(Vec<InlineServiceState>),
    /// New: services run as independent system services via IPC.
    External(Vec<ExternalServiceState>),
}

pub struct ServiceManager {
    backend: ServiceBackend,
    install_only: Vec<Box<dyn ManagedService>>,
    connectors: Vec<ConnectorState>,
    dispatcher: Arc<Dispatcher>,
    log_buf: LogBuffer,
}

impl ServiceManager {
    /// Install and set up all services. Does NOT spawn any processes.
    /// Call `spawn_all()` (inline) or `connect_all()` (external) after
    /// registering signal handlers.
    pub fn init(
        cfg: &mut crate::config::Config,
        dispatcher: Arc<Dispatcher>,
        log_buf: LogBuffer,
    ) -> Result<Self> {
        let external = cfg.global.external_processes;

        let global_cfg = std::mem::take(&mut cfg.global);
        let openclaw_cfg = std::mem::take(&mut cfg.openclaw);
        let ollama_cfg = std::mem::take(&mut cfg.ollama);
        let nexa_cfg = std::mem::take(&mut cfg.nexa);
        let lms_cfg = std::mem::take(&mut cfg.lms);
        let cloud_cfg = std::mem::take(&mut cfg.cloud);

        let connectors = connectors::build_connectors(
            &global_cfg, &ollama_cfg, &nexa_cfg, &lms_cfg, &cloud_cfg,
        );

        let services = connectors::build_services(
            &global_cfg, openclaw_cfg, ollama_cfg, nexa_cfg, lms_cfg,
        );

        let mut install_only: Vec<Box<dyn ManagedService>> = Vec::new();

        let backend = if external {
            tracing::info!("external_processes=true, using per-service system units");

            // Clean up stale units from previous runs.
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

                // Install and setup via nix (done in the daemon, not the wrapper).
                service.ensure_installed()?;
                service.ensure_setup()?;

                // Install and start the per-service system unit.
                if let Err(e) = crate::service::install_managed_service(&name) {
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
                    healthy: false,
                    upgrade_pending: false,
                    update_self_pending: false,
                    restart_pending: false,
                    skip_health_check: true,
                    post_start_done: false,
                    consecutive_crashes: 0,
                    was_unhealthy: false,
                    running_store_path: None,
                });
            }

            ServiceBackend::External(external_states)
        } else {
            // Clean up any stale per-service units from a previous external_processes=true run.
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
                    healthy: false,
                    upgrade_pending: false,
                    restart_pending: false,
                    skip_health_check: true,
                    post_start_done: false,
                    consecutive_crashes: 0,
                    was_unhealthy: false,
                    log_task: None,
                });
            }

            ServiceBackend::Inline(inline_states)
        };

        let connectors = connectors
            .into_iter()
            .map(|c| ConnectorState { connector: c, done: false })
            .collect();

        Ok(Self {
            backend,
            install_only,
            connectors,
            dispatcher,
            log_buf,
        })
    }

    // Notification receiver is reserved for future use when the daemon
    // event loop has a dedicated select arm for IPC notifications.

    // ── Spawn / Connect ──────────────────────────────────────────────

    /// Spawn all managed services (inline mode). No-op in external mode.
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
                    sentry_ext::breadcrumb(
                        "service",
                        &format!("{name} spawned"),
                        &[("service", &name), ("pid", &child.id().to_string())],
                    );
                    state.child = Some(child);
                    state.log_task = Some(log_task);
                }
                Err(e) => {
                    tracing::error!("{name} spawn failed: {e}");
                    sentry_ext::capture_error(
                        &format!("{name} spawn failed: {e}"),
                        &[("service", &name)],
                    );
                }
            }
        }
    }

    /// Connect to all service wrapper sockets (external mode). No-op in inline mode.
    /// Sends `Spawn` with the service's spawn spec to start the child process.
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
            Ok(mut client) => {
                tracing::info!("connected to {name} wrapper, sending Spawn");
                // Run configure + preflight before spawning.
                if let Err(e) = state.service.configure() {
                    tracing::warn!("{name} configure failed: {e}");
                }
                if let Err(e) = state.service.preflight() {
                    tracing::warn!("{name} preflight failed: {e}");
                }
                let spec = state.service.spawn_spec();
                state.running_store_path = crate::nix::binary_store_path(state.service.binary_name());
                match client.request(&IpcRequest::Spawn(spec)).await {
                    Ok(IpcResponse::Ok) => {
                        tracing::info!("{name} spawned via wrapper");
                        state.skip_health_check = true;
                    }
                    Ok(IpcResponse::Error { message }) => {
                        tracing::error!("{name} Spawn failed: {message}");
                    }
                    Err(e) => {
                        tracing::error!("{name} Spawn IPC failed: {e}");
                    }
                }
                state.client = Some(client);
            }
            Err(e) => {
                tracing::error!("failed to connect to {name} wrapper: {e}");
                sentry_ext::capture_error(
                    &format!("IPC connect to {name} failed: {e}"),
                    &[("service", name)],
                );
            }
        }
    }

    // ── Upgrades ─────────────────────────────────────────────────────

    /// Check for upgrades on all services (called on the update interval).
    pub fn check_upgrades(&mut self) {
        // Install-only services are always handled directly.
        for svc in &self.install_only {
            let name = svc.name();
            sentry_ext::set_tag("service", &name);
            match svc.check_and_upgrade() {
                Ok(true) => {
                    tracing::info!("{name} upgraded (install-only)");
                    sentry_ext::breadcrumb(
                        "upgrade",
                        &format!("{name} upgraded (install-only)"),
                        &[("service", &name)],
                    );
                    self.dispatcher.dispatch(&DaemonEvent::UpgradeInstalled {
                        service: name.to_string(),
                    });
                }
                Ok(false) => {}
                Err(e) => {
                    tracing::warn!("{name} upgrade check failed: {e}");
                    sentry_ext::capture_error(
                        &format!("{name} upgrade check failed: {e}"),
                        &[("service", &name)],
                    );
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
                    if !state.upgrade_pending {
                        let name = state.service.name();
                        sentry_ext::set_tag("service", &name);
                        match state.service.check_and_upgrade() {
                            Ok(true) => {
                                state.upgrade_pending = true;
                                sentry_ext::breadcrumb(
                                    "upgrade",
                                    &format!("{name} upgrade pending"),
                                    &[("service", &name)],
                                );
                            }
                            Ok(false) => {}
                            Err(e) => {
                                tracing::warn!("{name} upgrade check failed: {e}");
                                sentry_ext::capture_error(
                                    &format!("{name} upgrade check failed: {e}"),
                                    &[("service", &name)],
                                );
                                self.dispatcher.dispatch(&DaemonEvent::UpgradeFailed {
                                    service: name.to_string(),
                                    error: e.to_string(),
                                });
                            }
                        }
                    }
                }
            }
            ServiceBackend::External(states) => {
                // Daemon handles nix upgrades, then marks restart pending
                // so health_tick sends Upgrade (restart) to the wrapper.
                for state in states {
                    if state.upgrade_pending {
                        continue;
                    }
                    let name = state.service.name();
                    sentry_ext::set_tag("service", &name);
                    match state.service.check_and_upgrade() {
                        Ok(true) => {
                            state.upgrade_pending = true;
                            sentry_ext::breadcrumb(
                                "upgrade",
                                &format!("{name} upgrade pending"),
                                &[("service", &name)],
                            );
                        }
                        Ok(false) => {}
                        Err(e) => {
                            tracing::warn!("{name} upgrade check failed: {e}");
                            sentry_ext::capture_error(
                                &format!("{name} upgrade check failed: {e}"),
                                &[("service", &name)],
                            );
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

    // ── Inline respawn helper ────────────────────────────────────────

    fn respawn_inline(state: &mut InlineServiceState, log_buf: &LogBuffer) -> bool {
        if let Some(ref mut child) = state.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(task) = state.log_task.take() {
            task.abort();
        }
        let name = state.service.name();
        // Re-apply configuration before spawning.
        if let Err(e) = state.service.configure() {
            tracing::warn!("{name} configure before respawn failed: {e}");
        }
        match state.service.spawn() {
            Ok(mut child) => {
                state.log_task = Some(crate::log_capture::capture(name, &mut child, log_buf));
                state.child = Some(child);
                state.skip_health_check = true;
                state.post_start_done = false;
                true
            }
            Err(e) => {
                tracing::error!("{name} spawn failed: {e}");
                sentry_ext::capture_error(
                    &format!("{name} spawn failed: {e}"),
                    &[("service", &name)],
                );
                false
            }
        }
    }

    // ── Health tick ──────────────────────────────────────────────────

    /// Run health checks, restart crashed services, apply pending upgrades.
    pub async fn health_tick(&mut self, metrics: &Arc<Metrics>, in_upgrade_window: bool) {
        match &mut self.backend {
            ServiceBackend::Inline(states) => {
                Self::health_tick_inline(
                    states,
                    &self.log_buf,
                    &self.dispatcher,
                    metrics,
                    in_upgrade_window,
                );
            }
            ServiceBackend::External(states) => {
                Self::health_tick_external(
                    states,
                    &self.dispatcher,
                    &self.log_buf,
                    metrics,
                    in_upgrade_window,
                )
                .await;
            }
        }

        // Run connectors when dependencies are ready.
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

            if state.child.is_none() {
                continue;
            }

            // Restart if exited.
            let exit_status = state.child.as_mut().unwrap().try_wait();
            match exit_status {
                Ok(Some(status)) => {
                    state.consecutive_crashes += 1;
                    tracing::warn!(
                        "{name} exited with {status}, restarting (crash #{})",
                        state.consecutive_crashes
                    );
                    let code = status
                        .code()
                        .map(|c| c.to_string())
                        .unwrap_or("signal".to_string());
                    sentry_ext::capture_error(
                        &format!("{name} process exited unexpectedly"),
                        &[("service", &name), ("exit_code", &code)],
                    );

                    log_buf.push(format!(
                        "[{name}] crashed with {status} (#{crashes})",
                        crashes = state.consecutive_crashes
                    ));
                    dispatcher.dispatch(&DaemonEvent::ServiceCrashed {
                        service: name.to_string(),
                        exit_code: status.code(),
                    });

                    if state.consecutive_crashes >= 2 {
                        tracing::warn!(
                            "{name} crashed {} times, attempting repair before respawn",
                            state.consecutive_crashes
                        );
                        if let Err(e) = state.service.repair() {
                            tracing::error!("{name} repair failed: {e}");
                            sentry_ext::capture_error(
                                &format!("{name} repair failed: {e}"),
                                &[("service", &name)],
                            );
                        }
                    }

                    if Self::respawn_inline(state, log_buf) {
                        state.upgrade_pending = false;
                    }
                }
                Ok(None) => {}
                Err(e) => tracing::error!("failed to check {name} status: {e}"),
            }

            // Apply pending restart (config change) when idle.
            if state.restart_pending {
                match state.service.is_busy() {
                    Ok(false) => {
                        tracing::info!("{name} is idle, restarting for config change");
                        if Self::respawn_inline(state, log_buf) {
                            state.restart_pending = false;
                            state.upgrade_pending = false;
                        }
                    }
                    Ok(true) => tracing::info!("{name} is busy, deferring restart"),
                    Err(e) => tracing::warn!("{name} busy check for restart failed: {e}"),
                }
            }

            // Apply pending upgrade when idle.
            let busy = if state.upgrade_pending && in_upgrade_window {
                match state.service.is_busy() {
                    Ok(false) => {
                        tracing::info!("{name} is idle, restarting to apply upgrade");
                        if Self::respawn_inline(state, log_buf) {
                            state.upgrade_pending = false;
                            sentry_ext::breadcrumb(
                                "upgrade",
                                &format!("{name} restarted for upgrade"),
                                &[("service", &name)],
                            );
                            dispatcher.dispatch(&DaemonEvent::UpgradeInstalled {
                                service: name.to_string(),
                            });
                        }
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

            // Health check.
            state.healthy = if state.skip_health_check {
                tracing::info!("skipping health check, {name} recently started");
                state.skip_health_check = false;
                true
            } else {
                match state.service.check_health() {
                    Ok(true) => {
                        tracing::info!("{name} is healthy");
                        log_buf.push(format!("[{name}] healthy"));
                        state.consecutive_crashes = 0;
                        if state.was_unhealthy {
                            state.was_unhealthy = false;
                            dispatcher.dispatch(&DaemonEvent::ServiceRecovered {
                                service: name.to_string(),
                            });
                        }
                        if !state.post_start_done {
                            if let Err(e) = state.service.post_start() {
                                tracing::error!("{name} post_start failed: {e}");
                                sentry_ext::capture_error(
                                    &format!("{name} post_start failed: {e}"),
                                    &[("service", &name)],
                                );
                            }
                            state.post_start_done = true;
                        }
                        true
                    }
                    Ok(false) => {
                        tracing::warn!("{name} is unhealthy, attempting repair");
                        log_buf.push(format!("[{name}] unhealthy, attempting repair"));
                        sentry_ext::breadcrumb(
                            "health",
                            &format!("{name} unhealthy, repairing"),
                            &[("service", &name)],
                        );
                        if !state.was_unhealthy {
                            state.was_unhealthy = true;
                            dispatcher.dispatch(&DaemonEvent::ServiceUnhealthy {
                                service: name.to_string(),
                            });
                        }
                        if let Err(e) = state.service.repair() {
                            tracing::error!("{name} repair failed: {e}");
                            sentry_ext::capture_error(
                                &format!("{name} repair failed: {e}"),
                                &[("service", &name)],
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

            Self::update_metrics(metrics, &name, state.healthy, state.upgrade_pending, busy);
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

            // Ensure wrapper is connected.
            if state.client.is_none() {
                let path = crate::service_ipc::socket_path(&name);
                match ManagedClient::connect(&path, Duration::from_secs(5)).await {
                    Ok(c) => {
                        tracing::info!("reconnected to {name} wrapper");
                        state.client = Some(c);
                    }
                    Err(_) => {
                        tracing::warn!("{name} wrapper not reachable, trying to restart unit");
                        if let Err(e) = crate::service::start_managed_service(&name) {
                            tracing::error!("failed to start {name} unit: {e}");
                        }
                        continue;
                    }
                }
            }

            let client = state.client.as_mut().unwrap();

            // Drain log and crash notifications from the wrapper.
            while let Some(notif) = client.try_recv_notification() {
                match notif {
                    IpcNotification::Crashed { exit_code } => {
                        state.consecutive_crashes += 1;
                        tracing::warn!("{name} crashed (#{}, exit: {exit_code:?})", state.consecutive_crashes);
                        dispatcher.dispatch(&DaemonEvent::ServiceCrashed {
                            service: name.clone(),
                            exit_code,
                        });
                        state.healthy = false;
                        state.post_start_done = false;

                        if state.consecutive_crashes >= 2 {
                            if let Err(e) = state.service.repair() {
                                tracing::error!("{name} repair failed: {e}");
                            }
                        }
                    }
                    IpcNotification::Log { line, .. } => {
                        log_buf.push(format!("[{name}] {line}"));
                    }
                }
            }

            // Apply pending restart (config change) when idle.
            let busy = state.service.is_busy().unwrap_or(false);
            if state.restart_pending {
                if !busy || in_upgrade_window {
                    tracing::info!("{name} restarting for config change");
                    if let Err(e) = state.service.configure() {
                        tracing::warn!("{name} configure failed: {e}");
                    }
                    let spec = state.service.spawn_spec();
                    state.running_store_path = crate::nix::binary_store_path(state.service.binary_name());
                    match client.request(&IpcRequest::Spawn(spec)).await {
                        Ok(IpcResponse::Ok) => {
                            state.restart_pending = false;
                            state.upgrade_pending = false;
                            state.skip_health_check = true;
                        }
                        _ => {}
                    }
                } else {
                    tracing::info!("{name} is busy, deferring restart");
                }
            }

            // Apply pending upgrade when idle.
            if state.upgrade_pending && in_upgrade_window {
                if !busy || in_upgrade_window {
                    tracing::info!("{name} restarting for upgrade");
                    let spec = state.service.spawn_spec();
                    state.running_store_path = crate::nix::binary_store_path(state.service.binary_name());
                    match client.request(&IpcRequest::Spawn(spec)).await {
                        Ok(IpcResponse::Ok) => {
                            state.upgrade_pending = false;
                            state.skip_health_check = true;
                            dispatcher.dispatch(&DaemonEvent::UpgradeInstalled {
                                service: name.clone(),
                            });
                        }
                        Ok(IpcResponse::Error { message }) => {
                            dispatcher.dispatch(&DaemonEvent::UpgradeFailed {
                                service: name.clone(),
                                error: message,
                            });
                        }
                        _ => {}
                    }
                }
            }

            // Detect service binary store path drift.
            let current_store = crate::nix::binary_store_path(state.service.binary_name());
            if let (Some(old), Some(new)) = (&state.running_store_path, &current_store) {
                if old != new && (!busy || in_upgrade_window) {
                    tracing::info!("{name} binary changed ({old} → {new}), restarting");
                    let spec = state.service.spawn_spec();
                    state.running_store_path = current_store;
                    let _ = client.request(&IpcRequest::Spawn(spec)).await;
                    state.skip_health_check = true;
                }
            }

            // Health check (daemon calls it directly, not the wrapper).
            if state.skip_health_check {
                state.skip_health_check = false;
            } else {
                match state.service.check_health() {
                    Ok(true) => {
                        state.healthy = true;
                        state.consecutive_crashes = 0;
                        if state.was_unhealthy {
                            state.was_unhealthy = false;
                            dispatcher.dispatch(&DaemonEvent::ServiceRecovered {
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
                        state.healthy = false;
                        if !state.was_unhealthy {
                            state.was_unhealthy = true;
                            dispatcher.dispatch(&DaemonEvent::ServiceUnhealthy {
                                service: name.clone(),
                            });
                        }
                        if let Err(e) = state.service.repair() {
                            tracing::error!("{name} repair failed: {e}");
                        }
                    }
                    Err(e) => {
                        tracing::warn!("{name} health check failed: {e}");
                        state.healthy = false;
                    }
                }
            }

            // Retry deferred update-self when idle.
            if state.update_self_pending && !busy {
                tracing::info!("{name} is now idle, sending deferred update-self");
                Self::do_send_update_self(client, &name).await;
                state.update_self_pending = false;
            }

            Self::update_metrics(metrics, &name, state.healthy, state.upgrade_pending, busy);
        }
    }

    fn run_connectors(&mut self) {
        for cs in &mut self.connectors {
            if cs.done {
                continue;
            }
            let deps_ready = cs.connector.depends_on().iter().all(|dep| {
                match &self.backend {
                    ServiceBackend::Inline(states) => states
                        .iter()
                        .find(|s| s.service.name() == *dep)
                        .is_some_and(|s| s.post_start_done),
                    ServiceBackend::External(states) => states
                        .iter()
                        .find(|s| s.service_name == *dep)
                        .is_some_and(|s| s.post_start_done),
                }
            });
            if !deps_ready {
                continue;
            }
            let name = cs.connector.name();
            tracing::info!("running connector: {name}");
            if let Err(e) = cs.connector.connect() {
                tracing::error!("connector {name} failed: {e}");
                sentry_ext::capture_error(
                    &format!("connector {name} failed: {e}"),
                    &[("connector", name)],
                );
            }
            cs.done = true;
        }
    }

    fn update_metrics(metrics: &Metrics, name: &str, healthy: bool, upgrade_pending: bool, busy: bool) {
        metrics.service_healthy.with_label_values(&[name]).set(if healthy { 1 } else { 0 });
        metrics.service_upgrade_pending.with_label_values(&[name]).set(if upgrade_pending { 1 } else { 0 });
        metrics.service_busy.with_label_values(&[name]).set(if busy { 1 } else { 0 });
    }

    // ── Schedule restart ─────────────────────────────────────────────

    /// Schedule a restart for all managed services (e.g., after config change).
    /// Services that support hot-reload will have `configure()` called instead
    /// of being restarted.
    pub async fn schedule_restart(&mut self) {
        match &mut self.backend {
            ServiceBackend::Inline(states) => {
                for state in states {
                    let name = state.service.name().to_string();
                    if state.service.supports_hot_reload() {
                        tracing::info!("{name} supports hot reload, running configure");
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
                        tracing::info!("{name} supports hot reload, running configure");
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

    /// Collect current service statuses for heartbeat reporting.
    pub fn collect_statuses(&self) -> Vec<serde_json::Value> {
        match &self.backend {
            ServiceBackend::Inline(states) => states
                .iter()
                .map(|state| {
                    let name = state.service.name();
                    serde_json::json!({
                        "name": name,
                        "healthy": state.healthy,
                        "upgrade_pending": state.upgrade_pending,
                        "busy": false,
                    })
                })
                .collect(),
            ServiceBackend::External(states) => states
                .iter()
                .map(|state| {
                    serde_json::json!({
                        "name": state.service_name,
                        "healthy": state.healthy,
                        "upgrade_pending": state.upgrade_pending,
                        "busy": false,
                    })
                })
                .collect(),
        }
    }

    // ── Shutdown ─────────────────────────────────────────────────────

    /// Graceful shutdown. Inline services are killed (they're child processes).
    /// External services are left running — they survive daemon restarts.
    pub async fn shutdown(&mut self) {
        match &mut self.backend {
            ServiceBackend::Inline(states) => {
                // SIGTERM all, then SIGKILL after 10s.
                for state in states.iter() {
                    if let Some(ref child) = state.child {
                        let name = state.service.name();
                        let pid = child.id();
                        tracing::info!("sending SIGTERM to {name} (pid {pid})");
                        unsafe {
                            libc::kill(pid as i32, libc::SIGTERM);
                        }
                    }
                }

                let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
                for state in states.iter_mut() {
                    let Some(ref mut child) = state.child else {
                        continue;
                    };
                    let name = state.service.name();
                    loop {
                        match child.try_wait() {
                            Ok(Some(_)) => break,
                            Ok(None) if tokio::time::Instant::now() >= deadline => {
                                tracing::warn!("{name} did not exit in time, sending SIGKILL");
                                let _ = child.kill();
                                let _ = child.wait();
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
                    if let Some(task) = state.log_task.take() {
                        task.abort();
                    }
                    tracing::info!("{name} stopped");
                }
            }
            ServiceBackend::External(_) => {
                tracing::info!("leaving external services running (they survive daemon restarts)");
            }
        }
    }

    /// Request all external wrappers to re-exec with the new binary.
    /// Defers for busy services — they will be re-exec'd on the next
    /// health tick when idle. No-op in inline mode.
    #[allow(dead_code)]
    pub async fn send_update_self(&mut self) {
        let ServiceBackend::External(ref mut states) = self.backend else {
            return;
        };
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
