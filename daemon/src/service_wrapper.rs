use anyhow::{Context, Result};
use std::time::Duration;

use crate::log_buffer::LogBuffer;
use crate::managed_service::{ManagedService, ServiceMode};
use crate::service_ipc::listener::{spawn_listener, IpcListener, NotificationSender};
use crate::service_ipc::protocol::{IpcNotification, IpcRequest, IpcResponse};

struct WrapperState {
    service_name: String,
    service: Box<dyn ManagedService>,
    child: Option<std::process::Child>,
    log_task: Option<tokio::task::JoinHandle<()>>,
    log_buf: LogBuffer,
    healthy: bool,
    busy: bool,
    upgrade_pending: bool,
    post_start_done: bool,
    consecutive_crashes: u32,
    was_unhealthy: bool,
    notif_tx: NotificationSender,
}

impl WrapperState {
    fn child_pid(&self) -> Option<u32> {
        self.child.as_ref().map(|c| c.id())
    }

    fn kill_child(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(task) = self.log_task.take() {
            task.abort();
        }
        self.child = None;
    }

    fn spawn_child(&mut self) -> bool {
        match self.service.spawn() {
            Ok(mut child) => {
                // Capture logs and also forward them as IPC notifications.
                let log_task = capture_and_forward(
                    &self.service_name,
                    &mut child,
                    &self.log_buf,
                    &self.notif_tx,
                );
                tracing::info!("{} spawned (pid: {})", self.service_name, child.id());
                self.child = Some(child);
                self.log_task = Some(log_task);
                self.post_start_done = false;
                true
            }
            Err(e) => {
                tracing::error!("{} spawn failed: {e}", self.service_name);
                false
            }
        }
    }

    fn respawn(&mut self) -> bool {
        self.kill_child();
        self.spawn_child()
    }

    fn run_health_check(&mut self) {
        let name = &self.service_name;
        match self.service.check_health() {
            Ok(true) => {
                if !self.healthy || self.was_unhealthy {
                    tracing::info!("{name} is healthy");
                    self.notif_tx.send(IpcNotification::Healthy {
                        service: name.clone(),
                    });
                }
                self.healthy = true;
                self.was_unhealthy = false;
                self.consecutive_crashes = 0;

                if !self.post_start_done {
                    if let Err(e) = self.service.post_start() {
                        tracing::error!("{name} post_start failed: {e}");
                    }
                    self.post_start_done = true;
                }
            }
            Ok(false) => {
                tracing::warn!("{name} is unhealthy");
                self.healthy = false;
                if !self.was_unhealthy {
                    self.was_unhealthy = true;
                    self.notif_tx.send(IpcNotification::Unhealthy {
                        service: name.clone(),
                    });
                }
                if let Err(e) = self.service.repair() {
                    tracing::error!("{name} repair failed: {e}");
                }
            }
            Err(e) => {
                tracing::warn!("{name} health check failed: {e}");
                self.healthy = false;
            }
        }

        self.busy = self.service.is_busy().unwrap_or(false);
    }

    /// Check if the child exited. Returns true if it crashed (and was respawned).
    fn check_child_exit(&mut self) -> bool {
        let Some(ref mut child) = self.child else {
            return false;
        };
        match child.try_wait() {
            Ok(Some(status)) => {
                self.consecutive_crashes += 1;
                let name = &self.service_name;
                tracing::warn!(
                    "{name} exited with {status} (crash #{})",
                    self.consecutive_crashes
                );
                self.notif_tx.send(IpcNotification::Crashed {
                    service: name.clone(),
                    exit_code: status.code(),
                });
                if let Some(task) = self.log_task.take() {
                    task.abort();
                }
                self.child = None;
                self.healthy = false;
                self.post_start_done = false;

                if self.consecutive_crashes >= 2 {
                    tracing::warn!(
                        "{name} crashed {} times, attempting repair",
                        self.consecutive_crashes
                    );
                    if let Err(e) = self.service.repair() {
                        tracing::error!("{name} repair failed: {e}");
                    }
                }

                self.spawn_child();
                true
            }
            Ok(None) => false,
            Err(e) => {
                tracing::error!("failed to check child status: {e}");
                false
            }
        }
    }

    fn build_status_response(&self) -> IpcResponse {
        IpcResponse::Status {
            service: self.service_name.clone(),
            healthy: self.healthy,
            busy: self.busy,
            upgrade_pending: self.upgrade_pending,
            pid: self.child_pid(),
            post_start_done: self.post_start_done,
            supports_hot_reload: self.service.supports_hot_reload(),
        }
    }
}

fn find_service(service_name: &str, cfg: &crate::config::Config) -> Result<Box<dyn ManagedService>> {
    let services = crate::connectors::build_services(
        &cfg.global,
        cfg.openclaw.clone(),
        cfg.ollama.clone(),
        cfg.nexa.clone(),
        cfg.lms.clone(),
    );
    services
        .into_iter()
        .find(|s| s.name() == service_name)
        .with_context(|| format!("service {service_name} not found in current config"))
}

/// Parse config JSON and build the named service. Returns an IpcResponse on failure.
fn parse_config_and_build(
    service_name: &str,
    config_json: &str,
) -> Result<Box<dyn ManagedService>, IpcResponse> {
    let cfg: crate::config::Config = serde_json::from_str(config_json).map_err(|e| {
        IpcResponse::Error {
            command: "set_config".into(),
            message: format!("invalid config JSON: {e}"),
        }
    })?;
    find_service(service_name, &cfg).map_err(|e| IpcResponse::Error {
        command: "set_config".into(),
        message: e.to_string(),
    })
}

/// Entry point for `mac-mgmt daemon-service-launch <service>`.
pub async fn run(service_name: &str, log_buf: LogBuffer) -> Result<()> {
    tracing::info!("service wrapper starting for {service_name}");

    // Bind the IPC socket first — the daemon will connect and send config.
    let socket_path = crate::service_ipc::socket_path(service_name);
    let listener = IpcListener::bind(&socket_path)
        .with_context(|| format!("bind IPC socket at {}", socket_path.display()))?;
    tracing::info!("IPC socket bound at {}, waiting for config from daemon", socket_path.display());

    let (mut req_rx, notif_tx) = spawn_listener(listener);

    // Wait for the daemon to send SetConfig with the full DaemonConfig.
    let service = loop {
        match req_rx.recv().await {
            Some((IpcRequest::SetConfig { config_json }, resp_tx)) => {
                match parse_config_and_build(service_name, &config_json) {
                    Ok(svc) if svc.service_mode() == ServiceMode::InstallOnly => {
                        let _ = resp_tx.send(IpcResponse::Error {
                            command: "set_config".into(),
                            message: format!("{service_name} is install-only"),
                        }).await;
                        anyhow::bail!("{service_name} is install-only");
                    }
                    Ok(svc) => {
                        let _ = resp_tx.send(IpcResponse::Ack { command: "set_config".into() }).await;
                        break svc;
                    }
                    Err(resp) => { let _ = resp_tx.send(resp).await; }
                }
            }
            Some((_other, resp_tx)) => {
                let _ = resp_tx.send(IpcResponse::Error {
                    command: "set_config".into(),
                    message: "wrapper not yet configured, send SetConfig first".into(),
                }).await;
            }
            None => anyhow::bail!("IPC channel closed before receiving config"),
        }
    };

    tracing::info!("{service_name}: configure");
    service.configure()?;
    tracing::info!("{service_name}: preflight");
    service.preflight()?;

    let mut state = WrapperState {
        service_name: service_name.to_string(),
        service,
        child: None,
        log_task: None,
        log_buf,
        healthy: false,
        busy: false,
        upgrade_pending: false,
        post_start_done: false,
        consecutive_crashes: 0,
        was_unhealthy: false,
        notif_tx,
    };

    // Initial spawn.
    state.spawn_child();

    let mut health_tick = tokio::time::interval(Duration::from_secs(60));
    health_tick.tick().await; // consume immediate tick
    let mut skip_first_health = true;

    let mut child_check = tokio::time::interval(Duration::from_secs(2));

    let mut sigterm =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .context("SIGTERM handler")?;
    let mut sigint =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
            .context("SIGINT handler")?;

    let mut pending_update_self = false;

    loop {
        tokio::select! {
            _ = sigterm.recv() => {
                tracing::info!("received SIGTERM, shutting down");
                break;
            }
            _ = sigint.recv() => {
                tracing::info!("received SIGINT, shutting down");
                break;
            }
            _ = child_check.tick() => {
                state.check_child_exit();
            }
            _ = health_tick.tick() => {
                if skip_first_health {
                    skip_first_health = false;
                    continue;
                }
                if state.child.is_some() {
                    state.run_health_check();
                }
            }
            Some((req, resp_tx)) = req_rx.recv() => {
                let is_update_self = matches!(req, IpcRequest::UpdateSelf);
                let is_shutdown = matches!(req, IpcRequest::Shutdown);

                let resp = handle_request(&mut state, req).await;
                let _ = resp_tx.send(resp).await;

                if is_update_self {
                    pending_update_self = true;
                    break;
                }
                if is_shutdown {
                    break;
                }
            }
        }
    }

    // Cleanup.
    state.kill_child();
    std::fs::remove_file(&socket_path).ok();

    if pending_update_self {
        tracing::info!("re-execing wrapper with new binary");
        do_update_self();
        // If exec fails, we fall through and exit. launchd/systemd will restart us.
        tracing::error!("exec failed, exiting (service manager will restart)");
    }

    tracing::info!("{service_name} wrapper shutdown complete");
    Ok(())
}

async fn handle_request(state: &mut WrapperState, req: IpcRequest) -> IpcResponse {
    let name = state.service_name.clone();
    match req {
        IpcRequest::SetConfig { config_json } => {
            tracing::info!("{name}: set_config received");
            match parse_config_and_build(&name, &config_json) {
                Ok(new_svc) => {
                    state.service = new_svc;
                    IpcResponse::Ack { command: "set_config".into() }
                }
                Err(resp) => resp,
            }
        }
        IpcRequest::Health => state.build_status_response(),
        IpcRequest::Configure => {
            tracing::info!("{name}: configure requested");
            match state.service.configure() {
                Ok(()) => IpcResponse::Ack { command: "configure".into() },
                Err(e) => IpcResponse::Error {
                    command: "configure".into(),
                    message: e.to_string(),
                },
            }
        }
        IpcRequest::Restart => {
            tracing::info!("{name}: restart requested");
            if let Err(e) = state.service.configure() {
                tracing::warn!("{name}: configure before restart failed: {e}");
            }
            if state.respawn() {
                state.upgrade_pending = false;
                IpcResponse::Ack { command: "restart".into() }
            } else {
                IpcResponse::Error {
                    command: "restart".into(),
                    message: "spawn failed".into(),
                }
            }
        }
        IpcRequest::Upgrade => {
            // The main daemon handles the actual nix upgrade. The wrapper
            // just restarts the service to pick up the new binary.
            tracing::info!("{name}: upgrade restart requested");
            if let Err(e) = state.service.configure() {
                tracing::warn!("{name}: configure before upgrade restart failed: {e}");
            }
            if state.respawn() {
                state.upgrade_pending = false;
                IpcResponse::Ack { command: "upgrade".into() }
            } else {
                IpcResponse::Error {
                    command: "upgrade".into(),
                    message: "spawn failed".into(),
                }
            }
        }
        IpcRequest::Shutdown => {
            tracing::info!("{name}: shutdown requested");
            state.kill_child();
            IpcResponse::Ack { command: "shutdown".into() }
        }
        IpcRequest::UpdateSelf => {
            tracing::info!("{name}: update-self requested");
            state.kill_child();
            IpcResponse::Ack { command: "update_self".into() }
            // The main loop breaks after this and calls do_update_self().
        }
    }
}

/// Like `log_capture::capture` but also sends each line as an IPC Log notification.
fn capture_and_forward(
    service_name: &str,
    child: &mut std::process::Child,
    buf: &LogBuffer,
    notif_tx: &NotificationSender,
) -> tokio::task::JoinHandle<()> {
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let name = service_name.to_string();
    let buf = buf.clone();
    let notif_tx = notif_tx.clone();

    tokio::task::spawn_blocking(move || {
        let stderr_thread = stderr.map(|stderr| {
            let name = name.clone();
            let buf = buf.clone();
            let notif_tx = notif_tx.clone();
            std::thread::spawn(move || {
                drain_and_forward(std::io::BufReader::new(stderr), &name, true, &buf, &notif_tx);
            })
        });

        if let Some(stdout) = stdout {
            drain_and_forward(std::io::BufReader::new(stdout), &name, false, &buf, &notif_tx);
        }

        if let Some(t) = stderr_thread {
            let _ = t.join();
        }
    })
}

fn drain_and_forward(
    reader: impl std::io::BufRead,
    name: &str,
    is_stderr: bool,
    buf: &LogBuffer,
    notif_tx: &NotificationSender,
) {
    for line in reader.lines() {
        let Ok(line) = line else { break };
        let clean = strip_ansi_escapes::strip(&line);
        let clean = String::from_utf8(clean).unwrap_or_else(|_| line.clone());
        if is_stderr {
            tracing::warn!(target: "service", "[{name}] {clean}");
        } else {
            tracing::info!(target: "service", "[{name}] {clean}");
        }
        buf.push(format!("[{name}] {clean}"));
        notif_tx.send(IpcNotification::Log {
            service: name.to_string(),
            line: clean,
            is_stderr,
        });
    }
}

/// Replace the current process image with a fresh exec of ourselves.
fn do_update_self() {
    use std::os::unix::process::CommandExt;

    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            tracing::error!("cannot determine binary path: {e}");
            return;
        }
    };

    let args: Vec<String> = std::env::args().skip(1).collect();
    tracing::info!("exec {exe:?} {args:?}");

    let err = std::process::Command::new(&exe).args(&args).exec();
    // exec() only returns on error.
    tracing::error!("exec failed: {err}");
}
