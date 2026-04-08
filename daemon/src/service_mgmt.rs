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
use crate::sentry_ext;

struct ServiceState {
    service: Box<dyn ManagedService>,
    child: Option<std::process::Child>,
    upgrade_pending: bool,
    restart_pending: bool,
    skip_health_check: bool,
    post_start_done: bool,
    consecutive_crashes: u32,
    was_unhealthy: bool,
    log_task: Option<JoinHandle<()>>,
}

struct ConnectorState {
    connector: Box<dyn Connector>,
    done: bool,
}

pub struct ServiceManager {
    states: Vec<ServiceState>,
    install_only: Vec<Box<dyn ManagedService>>,
    connectors: Vec<ConnectorState>,
    dispatcher: Arc<Dispatcher>,
    log_buf: LogBuffer,
}

impl ServiceManager {
    /// Install and set up all services. Does NOT spawn any processes.
    /// Call `spawn_all()` after registering signal handlers.
    pub fn init(cfg: &mut crate::config::Config, dispatcher: Arc<Dispatcher>, log_buf: LogBuffer) -> Result<Self> {
        let global_cfg = std::mem::take(&mut cfg.global);
        let openclaw_cfg = std::mem::take(&mut cfg.openclaw);
        let ollama_cfg = std::mem::take(&mut cfg.ollama);
        let nexa_cfg = std::mem::take(&mut cfg.nexa);
        let lms_cfg = std::mem::take(&mut cfg.lms);
        let cloud_cfg = std::mem::take(&mut cfg.cloud);

        let connectors = connectors::build_connectors(&global_cfg, &ollama_cfg, &nexa_cfg, &lms_cfg, &cloud_cfg);

        let services = connectors::build_services(
            &global_cfg,
            openclaw_cfg,
            ollama_cfg,
            nexa_cfg,
            lms_cfg,
        );

        let mut states: Vec<ServiceState> = Vec::new();
        let mut install_only: Vec<Box<dyn ManagedService>> = Vec::new();

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
            states.push(ServiceState {
                service,
                child: None,
                upgrade_pending: false,
                restart_pending: false,
                skip_health_check: true,
                post_start_done: false,
                consecutive_crashes: 0,
                was_unhealthy: false,
                log_task: None,
            });
        }

        let connectors = connectors
            .into_iter()
            .map(|c| ConnectorState { connector: c, done: false })
            .collect();

        Ok(Self {
            states,
            install_only,
            connectors,
            dispatcher,
            log_buf,
        })
    }

    /// Spawn all managed services. Call after signal handlers are registered.
    pub fn spawn_all(&mut self) {
        for state in &mut self.states {
            let name = state.service.name();
            match state.service.spawn() {
                Ok(mut child) => {
                    let log_task = crate::log_capture::capture(name, &mut child, &self.log_buf);
                    tracing::info!("{name} spawned (pid: {})", child.id());
                    sentry_ext::breadcrumb(
                        "service",
                        &format!("{name} spawned"),
                        &[(("service", &name)), ("pid", &child.id().to_string())],
                    );
                    state.child = Some(child);
                    state.log_task = Some(log_task);
                }
                Err(e) => {
                    tracing::error!("{name} spawn failed: {e}");
                    sentry_ext::capture_error(
                        &format!("{name} spawn failed: {e}"),
                        &[(("service", &name))],
                    );
                }
            }
        }
    }

    /// Check for upgrades on all services (called on the update interval).
    pub fn check_upgrades(&mut self) {
        for svc in &self.install_only {
            let name = svc.name();
            sentry_ext::set_tag("service", &name);
            match svc.check_and_upgrade() {
                Ok(true) => {
                    tracing::info!("{name} upgraded (install-only)");
                    sentry_ext::breadcrumb(
                        "upgrade",
                        &format!("{name} upgraded (install-only)"),
                        &[(("service", &name))],
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
                        &[(("service", &name))],
                    );
                    self.dispatcher.dispatch(&DaemonEvent::UpgradeFailed {
                        service: name.to_string(),
                        error: e.to_string(),
                    });
                }
            }
        }

        for state in &mut self.states {
            if !state.upgrade_pending {
                let name = state.service.name();
                sentry_ext::set_tag("service", &name);
                match state.service.check_and_upgrade() {
                    Ok(true) => {
                        state.upgrade_pending = true;
                        sentry_ext::breadcrumb(
                            "upgrade",
                            &format!("{name} upgrade pending"),
                            &[(("service", &name))],
                        );
                    }
                    Ok(false) => {}
                    Err(e) => {
                        tracing::warn!("{name} upgrade check failed: {e}");
                        sentry_ext::capture_error(
                            &format!("{name} upgrade check failed: {e}"),
                            &[(("service", &name))],
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

    /// Kill the current child (if any) and spawn a fresh one.
    fn respawn(state: &mut ServiceState, log_buf: &LogBuffer) -> bool {
        if let Some(ref mut child) = state.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(task) = state.log_task.take() {
            task.abort();
        }
        let name = state.service.name();
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
                    &[(("service", &name))],
                );
                false
            }
        }
    }

    /// Run health checks, restart crashed services, apply pending upgrades.
    pub fn health_tick(&mut self, metrics: &Arc<Metrics>, in_upgrade_window: bool) {
        for state in &mut self.states {
            let name = state.service.name().to_string();
            sentry_ext::set_tag("service", &name);

            // Skip services that failed to spawn
            if state.child.is_none() {
                continue;
            }

            // Restart if exited
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
                        &[(("service", &name)), ("exit_code", &code)],
                    );

                    self.log_buf.push(format!("[{name}] crashed with {status} (#{crashes})", crashes = state.consecutive_crashes));
                    self.dispatcher.dispatch(&DaemonEvent::ServiceCrashed {
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
                                &[(("service", &name))],
                            );
                        }
                    }

                    if Self::respawn(state, &self.log_buf) {
                        state.upgrade_pending = false;
                    }
                }
                Ok(None) => {}
                Err(e) => tracing::error!("failed to check {name} status: {e}"),
            }

            // Apply pending restart (config change) when idle
            if state.restart_pending {
                match state.service.is_busy() {
                    Ok(false) => {
                        tracing::info!("{name} is idle, restarting for config change");
                        if Self::respawn(state, &self.log_buf) {
                            state.restart_pending = false;
                            state.upgrade_pending = false;
                        }
                    }
                    Ok(true) => tracing::info!("{name} is busy, deferring restart"),
                    Err(e) => tracing::warn!("{name} busy check for restart failed: {e}"),
                }
            }

            // Apply pending upgrade when idle
            let busy = if state.upgrade_pending && in_upgrade_window {
                match state.service.is_busy() {
                    Ok(false) => {
                        tracing::info!("{name} is idle, restarting to apply upgrade");
                        if Self::respawn(state, &self.log_buf) {
                            state.upgrade_pending = false;
                            sentry_ext::breadcrumb(
                                "upgrade",
                                &format!("{name} restarted for upgrade"),
                                &[(("service", &name))],
                            );
                            self.dispatcher.dispatch(&DaemonEvent::UpgradeInstalled {
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

            // Health check
            let healthy = if state.skip_health_check {
                tracing::info!("skipping health check, {name} recently started");
                state.skip_health_check = false;
                true
            } else {
                match state.service.check_health() {
                    Ok(true) => {
                        tracing::info!("{name} is healthy");
                        self.log_buf.push(format!("[{name}] healthy"));
                        state.consecutive_crashes = 0;
                        if state.was_unhealthy {
                            state.was_unhealthy = false;
                            self.dispatcher.dispatch(&DaemonEvent::ServiceRecovered {
                                service: name.to_string(),
                            });
                        }
                        if !state.post_start_done {
                            if let Err(e) = state.service.post_start() {
                                tracing::error!("{name} post_start failed: {e}");
                                sentry_ext::capture_error(
                                    &format!("{name} post_start failed: {e}"),
                                    &[(("service", &name))],
                                );
                            }
                            state.post_start_done = true;
                        }
                        true
                    }
                    Ok(false) => {
                        tracing::warn!("{name} is unhealthy, attempting repair");
                        self.log_buf.push(format!("[{name}] unhealthy, attempting repair"));
                        sentry_ext::breadcrumb(
                            "health",
                            &format!("{name} unhealthy, repairing"),
                            &[(("service", &name))],
                        );
                        if !state.was_unhealthy {
                            state.was_unhealthy = true;
                            self.dispatcher.dispatch(&DaemonEvent::ServiceUnhealthy {
                                service: name.to_string(),
                            });
                        }
                        if let Err(e) = state.service.repair() {
                            tracing::error!("{name} repair failed: {e}");
                            sentry_ext::capture_error(
                                &format!("{name} repair failed: {e}"),
                                &[(("service", &name))],
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

            metrics
                .service_healthy
                .with_label_values(&[&name])
                .set(if healthy { 1 } else { 0 });
            metrics
                .service_upgrade_pending
                .with_label_values(&[&name])
                .set(if state.upgrade_pending { 1 } else { 0 });
            metrics
                .service_busy
                .with_label_values(&[&name])
                .set(if busy { 1 } else { 0 });
        }

        // Run each connector once its dependencies have completed post_start
        for cs in &mut self.connectors {
            if cs.done {
                continue;
            }
            let deps_ready = cs.connector.depends_on().iter().all(|dep| {
                self.states
                    .iter()
                    .find(|s| s.service.name() == *dep)
                    .is_some_and(|s| s.post_start_done)
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

    /// Schedule a restart for all managed services (e.g., after config change).
    /// Services will be restarted when idle, similar to upgrade_pending.
    pub fn schedule_restart(&mut self) {
        for state in &mut self.states {
            state.restart_pending = true;
            tracing::info!("{} restart pending (config change)", state.service.name());
        }
    }

    /// Collect current service statuses for heartbeat reporting.
    pub fn collect_statuses(&self) -> Vec<serde_json::Value> {
        self.states
            .iter()
            .map(|state| {
                let name = state.service.name();
                serde_json::json!({
                    "name": name,
                    "healthy": !state.was_unhealthy,
                    "upgrade_pending": state.upgrade_pending,
                    "busy": false,
                })
            })
            .collect()
    }

    /// Graceful shutdown: SIGTERM all services, then SIGKILL after 10 s.
    pub async fn shutdown(&mut self) {
        for state in &mut self.states {
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
        for state in &mut self.states {
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
}
