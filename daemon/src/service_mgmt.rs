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
    upgrade_pending: bool,
    update_self_pending: bool,
    post_start_done: bool,
    was_unhealthy: bool,
    last_healthy: bool,
    last_busy: bool,
    supports_hot_reload: bool,
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
    /// Serialized config JSON, sent to wrappers on connect.
    config_json: Option<String>,
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
        let config_json = if external {
            Some(serde_json::to_string(&*cfg).unwrap_or_default())
        } else {
            None
        };

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

                let hot_reload = service.supports_hot_reload();
                external_states.push(ExternalServiceState {
                    service_name: name,
                    service,
                    client: None,
                    upgrade_pending: false,
                    update_self_pending: false,
                    post_start_done: false,
                    was_unhealthy: false,
                    last_healthy: false,
                    last_busy: false,
                    supports_hot_reload: hot_reload,
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
            config_json,
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
    /// Sends `SetConfig` to each wrapper after connecting so it can build its
    /// service and start spawning.
    pub async fn connect_all(&mut self) {
        let ServiceBackend::External(ref mut states) = self.backend else {
            return;
        };
        let config_json = match &self.config_json {
            Some(j) => j.clone(),
            None => return,
        };
        for state in states {
            let name = &state.service_name;
            let path = crate::service_ipc::socket_path(name);
            tracing::info!("connecting to {name} wrapper at {}", path.display());
            match ManagedClient::connect(&path, Duration::from_secs(30)).await {
                Ok(mut client) => {
                    tracing::info!("connected to {name} wrapper, sending config");
                    match client
                        .request(&IpcRequest::SetConfig {
                            config_json: config_json.clone(),
                        })
                        .await
                    {
                        Ok(IpcResponse::Ack { .. }) => {
                            tracing::info!("{name} wrapper configured");
                        }
                        Ok(IpcResponse::Error { message, .. }) => {
                            tracing::error!("{name} wrapper rejected config: {message}");
                        }
                        Ok(_) => {}
                        Err(e) => {
                            tracing::error!("{name} SetConfig failed: {e}");
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
            let name = &state.service_name;
            sentry_ext::set_tag("service", name);

            let Some(ref mut client) = state.client else {
                // Not connected — try to reconnect.
                let path = crate::service_ipc::socket_path(name);
                match ManagedClient::connect(&path, Duration::from_secs(5)).await {
                    Ok(c) => {
                        tracing::info!("reconnected to {name} wrapper");
                        state.client = Some(c);
                    }
                    Err(_) => {
                        tracing::warn!("{name} wrapper not reachable, trying to restart unit");
                        if let Err(e) = crate::service::start_managed_service(name) {
                            tracing::error!("failed to start {name} unit: {e}");
                        }
                        continue;
                    }
                }
                continue;
            };

            // Drain any pending notifications.
            while let Some(notif) = client.try_recv_notification() {
                match &notif {
                    IpcNotification::Crashed { service, exit_code } => {
                        dispatcher.dispatch(&DaemonEvent::ServiceCrashed {
                            service: service.clone(),
                            exit_code: *exit_code,
                        });
                    }
                    IpcNotification::Healthy { service } => {
                        if state.was_unhealthy {
                            dispatcher.dispatch(&DaemonEvent::ServiceRecovered {
                                service: service.clone(),
                            });
                            state.was_unhealthy = false;
                        }
                    }
                    IpcNotification::Unhealthy { service } => {
                        if !state.was_unhealthy {
                            state.was_unhealthy = true;
                            dispatcher.dispatch(&DaemonEvent::ServiceUnhealthy {
                                service: service.clone(),
                            });
                        }
                    }
                    IpcNotification::Log { service, line, .. } => {
                        log_buf.push(format!("[{service}] {line}"));
                    }
                }
            }

            // Send upgrade command if pending and in window.
            if state.upgrade_pending && in_upgrade_window {
                match client.request(&IpcRequest::Upgrade).await {
                    Ok(IpcResponse::Ack { .. }) => {
                        state.upgrade_pending = false;
                        dispatcher.dispatch(&DaemonEvent::UpgradeInstalled {
                            service: name.to_string(),
                        });
                    }
                    Ok(IpcResponse::Error { message, .. }) => {
                        tracing::warn!("{name} upgrade failed: {message}");
                        dispatcher.dispatch(&DaemonEvent::UpgradeFailed {
                            service: name.to_string(),
                            error: message,
                        });
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!("{name} upgrade IPC failed: {e}");
                        state.client = None; // connection lost
                        continue;
                    }
                }
            }

            // Query health status.
            match client.request(&IpcRequest::Health).await {
                Ok(IpcResponse::Status {
                    healthy,
                    busy,
                    upgrade_pending,
                    post_start_done,
                    supports_hot_reload,
                    ..
                }) => {
                    state.last_healthy = healthy;
                    state.last_busy = busy;
                    state.post_start_done = post_start_done;
                    state.supports_hot_reload = supports_hot_reload;
                    if upgrade_pending {
                        state.upgrade_pending = true;
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!("{name} health IPC failed: {e}");
                    state.client = None;
                    state.last_healthy = false;
                    continue;
                }
            }

            // Retry deferred update-self when no longer busy.
            if state.update_self_pending && !state.last_busy {
                if let Some(ref mut client) = state.client {
                    tracing::info!("{name} is now idle, sending deferred update-self");
                    Self::do_send_update_self(client, name).await;
                    state.update_self_pending = false;
                }
            }

            Self::update_metrics(metrics, name, state.last_healthy, state.upgrade_pending, state.last_busy);
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
                    if let Some(ref mut client) = state.client {
                        // First try Configure. If the service supports hot
                        // reload, the wrapper will apply the config without
                        // restarting the process.
                        let use_configure = state.supports_hot_reload;
                        let cmd = if use_configure {
                            IpcRequest::Configure
                        } else {
                            IpcRequest::Restart
                        };
                        let cmd_name = if use_configure { "configure" } else { "restart" };
                        match client.request(&cmd).await {
                            Ok(IpcResponse::Ack { .. }) => {
                                tracing::info!("{name} {cmd_name} sent via IPC");
                            }
                            Ok(IpcResponse::Error { message, .. }) => {
                                tracing::warn!("{name} {cmd_name} failed: {message}");
                                // Fall back to restart if configure failed.
                                if use_configure {
                                    tracing::info!("{name} falling back to restart");
                                    let _ = client.request(&IpcRequest::Restart).await;
                                }
                            }
                            Ok(_) => {}
                            Err(e) => {
                                tracing::warn!("{name} {cmd_name} IPC failed: {e}");
                                state.client = None;
                            }
                        }
                    } else {
                        tracing::warn!("{name} not connected, restarting unit directly");
                        if let Err(e) = crate::service::stop_managed_service(name) {
                            tracing::warn!("stop {name}: {e}");
                        }
                        if let Err(e) = crate::service::start_managed_service(name) {
                            tracing::error!("start {name}: {e}");
                        }
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
                        "healthy": state.last_healthy,
                        "upgrade_pending": state.upgrade_pending,
                        "busy": state.last_busy,
                    })
                })
                .collect(),
        }
    }

    // ── Shutdown ─────────────────────────────────────────────────────

    /// Graceful shutdown.
    ///
    /// If `stop_services` is true, also stop the per-service system units
    /// (used by `mac-mgmt stop`). If false, leave them running (used by
    /// daemon self-update / restart where services should survive).
    pub async fn shutdown(&mut self, stop_services: bool) {
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
            ServiceBackend::External(states) => {
                if stop_services {
                    for state in states.iter_mut() {
                        let name = &state.service_name;
                        if let Some(ref mut client) = state.client {
                            let _ = client.request(&IpcRequest::Shutdown).await;
                        }
                        if let Err(e) = crate::service::stop_managed_service(name) {
                            tracing::warn!("stop {name} unit: {e}");
                        }
                    }
                } else {
                    tracing::info!("leaving per-service units running (daemon restart)");
                }
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
            let Some(ref mut client) = state.client else {
                state.update_self_pending = true;
                continue;
            };
            if state.last_busy {
                tracing::info!("{name} is busy, deferring update-self");
                state.update_self_pending = true;
                continue;
            }
            Self::do_send_update_self(client, name).await;
        }
    }

    async fn do_send_update_self(client: &mut ManagedClient, name: &str) {
        match client.request(&IpcRequest::UpdateSelf).await {
            Ok(IpcResponse::Ack { .. }) => {
                tracing::info!("{name} update-self sent");
            }
            Ok(IpcResponse::Error { message, .. }) => {
                tracing::warn!("{name} update-self failed: {message}");
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!("{name} update-self IPC failed: {e}");
            }
        }
    }
}
