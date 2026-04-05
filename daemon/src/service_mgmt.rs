use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;

use crate::connectors;
use crate::events::DaemonEvent;
use crate::managed_service::{ManagedService, ServiceMode};
use crate::metrics::Metrics;
use crate::notify::Dispatcher;
use crate::sentry_ext;

struct ServiceState {
    service: Box<dyn ManagedService>,
    child: std::process::Child,
    upgrade_pending: bool,
    skip_health_check: bool,
    post_start_done: bool,
    consecutive_crashes: u32,
    was_unhealthy: bool,
}

pub struct ServiceManager {
    states: Vec<ServiceState>,
    install_only: Vec<Box<dyn ManagedService>>,
    dispatcher: Arc<Dispatcher>,
}

impl ServiceManager {
    /// Create services from config, install, set up, and spawn them.
    pub fn init(cfg: &mut crate::config::Config, dispatcher: Arc<Dispatcher>) -> Result<Self> {
        let global_cfg = std::mem::take(&mut cfg.global);
        let openclaw_cfg = std::mem::take(&mut cfg.openclaw);
        let ollama_cfg = std::mem::take(&mut cfg.ollama);
        let nexa_cfg = std::mem::take(&mut cfg.nexa);

        let services = connectors::build_services(
            &global_cfg,
            openclaw_cfg,
            ollama_cfg,
            nexa_cfg,
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
            let child = service.spawn()?;
            sentry_ext::breadcrumb(
                "service",
                &format!("{name} initialized"),
                &[("service", &name), ("pid", &child.id().to_string())],
            );
            states.push(ServiceState {
                service,
                child,
                upgrade_pending: false,
                skip_health_check: true,
                post_start_done: false,
                consecutive_crashes: 0,
                was_unhealthy: false,
            });
        }

        Ok(Self {
            states,
            install_only,
            dispatcher,
        })
    }

    /// Check for upgrades on all services (called on the update interval).
    pub fn check_upgrades(&mut self) {
        for svc in &self.install_only {
            let name = svc.name();
            sentry_ext::set_tag("service", name);
            match svc.check_and_upgrade() {
                Ok(true) => {
                    tracing::info!("{name} upgraded (install-only)");
                    sentry_ext::breadcrumb(
                        "upgrade",
                        &format!("{name} upgraded (install-only)"),
                        &[("service", name)],
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
                        &[("service", name)],
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
                sentry_ext::set_tag("service", name);
                match state.service.check_and_upgrade() {
                    Ok(true) => {
                        state.upgrade_pending = true;
                        sentry_ext::breadcrumb(
                            "upgrade",
                            &format!("{name} upgrade pending"),
                            &[("service", name)],
                        );
                    }
                    Ok(false) => {}
                    Err(e) => {
                        tracing::warn!("{name} upgrade check failed: {e}");
                        sentry_ext::capture_error(
                            &format!("{name} upgrade check failed: {e}"),
                            &[("service", name)],
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

    /// Run health checks, restart crashed services, apply pending upgrades.
    pub fn health_tick(&mut self, metrics: &Arc<Metrics>) {
        for state in &mut self.states {
            let name = state.service.name();
            sentry_ext::set_tag("service", name);

            // Restart if exited
            match state.child.try_wait() {
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
                        &[("service", name), ("exit_code", &code)],
                    );

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
                                &[("service", name)],
                            );
                        }
                    }

                    match state.service.spawn() {
                        Ok(child) => {
                            state.child = child;
                            state.upgrade_pending = false;
                            state.skip_health_check = true;
                            state.post_start_done = false;
                        }
                        Err(e) => {
                            tracing::error!("{name} respawn failed: {e}");
                            sentry_ext::capture_error(
                                &format!("{name} respawn failed: {e}"),
                                &[("service", name)],
                            );
                        }
                    }
                }
                Ok(None) => {}
                Err(e) => tracing::error!("failed to check {name} status: {e}"),
            }

            // Apply pending upgrade when idle
            let busy = if state.upgrade_pending {
                match state.service.is_busy() {
                    Ok(false) => {
                        tracing::info!("{name} is idle, restarting to apply upgrade");
                        let _ = state.child.kill();
                        let _ = state.child.wait();
                        match state.service.spawn() {
                            Ok(child) => {
                                state.child = child;
                                state.upgrade_pending = false;
                                state.skip_health_check = true;
                                state.post_start_done = false;
                                sentry_ext::breadcrumb(
                                    "upgrade",
                                    &format!("{name} restarted for upgrade"),
                                    &[("service", name)],
                                );
                                self.dispatcher.dispatch(&DaemonEvent::UpgradeInstalled {
                                    service: name.to_string(),
                                });
                            }
                            Err(e) => {
                                tracing::error!("{name} upgrade respawn failed: {e}");
                                sentry_ext::capture_error(
                                    &format!("{name} upgrade respawn failed: {e}"),
                                    &[("service", name)],
                                );
                            }
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
                                    &[("service", name)],
                                );
                            }
                            state.post_start_done = true;
                        }
                        true
                    }
                    Ok(false) => {
                        tracing::warn!("{name} is unhealthy, attempting repair");
                        sentry_ext::breadcrumb(
                            "health",
                            &format!("{name} unhealthy, repairing"),
                            &[("service", name)],
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
                                &[("service", name)],
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
                .with_label_values(&[name])
                .set(if healthy { 1 } else { 0 });
            metrics
                .service_upgrade_pending
                .with_label_values(&[name])
                .set(if state.upgrade_pending { 1 } else { 0 });
            metrics
                .service_busy
                .with_label_values(&[name])
                .set(if busy { 1 } else { 0 });
        }
    }

    /// Graceful shutdown: SIGTERM all services, then SIGKILL after 10 s.
    pub async fn shutdown(&mut self) {
        for state in &mut self.states {
            let name = state.service.name();
            let pid = state.child.id();
            tracing::info!("sending SIGTERM to {name} (pid {pid})");
            unsafe {
                libc::kill(pid as i32, libc::SIGTERM);
            }
        }

        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        for state in &mut self.states {
            let name = state.service.name();
            loop {
                match state.child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) if tokio::time::Instant::now() >= deadline => {
                        tracing::warn!("{name} did not exit in time, sending SIGKILL");
                        let _ = state.child.kill();
                        let _ = state.child.wait();
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
            tracing::info!("{name} stopped");
        }
    }
}
