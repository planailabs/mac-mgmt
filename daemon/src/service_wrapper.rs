use anyhow::{Context, Result};
use std::time::Duration;

use crate::log_buffer::LogBuffer;
use crate::service_ipc::listener::{spawn_listener, IpcListener, NotificationSender};
use crate::service_ipc::protocol::{IpcNotification, IpcRequest, IpcResponse, SpawnSpec};

struct WrapperState {
    service_name: String,
    child: Option<std::process::Child>,
    log_task: Option<tokio::task::JoinHandle<()>>,
    log_buf: LogBuffer,
    notif_tx: NotificationSender,
    /// Hash of the mac-mgmt binary at startup, for self-update detection.
    own_binary_hash: Option<Vec<u8>>,
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

    fn spawn_child(&mut self, spec: &SpawnSpec) -> bool {
        match std::process::Command::new(&spec.program)
            .args(&spec.args)
            .envs(&spec.env)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(mut child) => {
                let log_task = capture_and_forward(
                    &self.service_name,
                    &mut child,
                    &self.log_buf,
                    &self.notif_tx,
                );
                tracing::info!("{} spawned (pid: {})", self.service_name, child.id());
                self.child = Some(child);
                self.log_task = Some(log_task);
                true
            }
            Err(e) => {
                tracing::error!("{} spawn failed: {e}", self.service_name);
                false
            }
        }
    }

    fn respawn(&mut self, spec: &SpawnSpec) -> bool {
        self.kill_child();
        self.spawn_child(spec)
    }

    /// Check if the child exited. Returns true if it crashed.
    fn check_child_exit(&mut self, spec: &SpawnSpec) -> bool {
        let Some(ref mut child) = self.child else {
            return false;
        };
        match child.try_wait() {
            Ok(Some(status)) => {
                tracing::warn!("{} exited with {status}", self.service_name);
                self.notif_tx.send(IpcNotification::Crashed {
                    exit_code: status.code(),
                });
                if let Some(task) = self.log_task.take() {
                    task.abort();
                }
                self.child = None;
                self.spawn_child(spec);
                true
            }
            Ok(None) => false,
            Err(e) => {
                tracing::error!("failed to check child status: {e}");
                false
            }
        }
    }

    /// Check if our own mac-mgmt binary has changed since startup.
    fn own_binary_changed(&self) -> bool {
        let current = hash_current_exe();
        match (&self.own_binary_hash, &current) {
            (Some(old), Some(new)) if old != new => {
                tracing::info!("{}: mac-mgmt binary changed, need re-exec", self.service_name);
                true
            }
            _ => false,
        }
    }
}

/// Entry point for `mac-mgmt daemon-service-launch <service>`.
pub async fn run(service_name: &str, log_buf: LogBuffer) -> Result<()> {
    tracing::info!("service wrapper starting for {service_name}");

    // Bind the IPC socket first — the daemon will connect and send Spawn.
    let socket_path = crate::service_ipc::socket_path(service_name);
    let listener = IpcListener::bind(&socket_path)
        .with_context(|| format!("bind IPC socket at {}", socket_path.display()))?;
    tracing::info!("IPC socket bound at {}, waiting for Spawn from daemon", socket_path.display());

    let (mut req_rx, notif_tx) = spawn_listener(listener);

    // Wait for the daemon to send the initial Spawn command.
    let mut spec = loop {
        match req_rx.recv().await {
            Some((IpcRequest::Spawn(spec), resp_tx)) => {
                let _ = resp_tx.send(IpcResponse::Ack { command: "spawn".into() }).await;
                break spec;
            }
            Some((_other, resp_tx)) => {
                let _ = resp_tx.send(IpcResponse::Error {
                    command: "spawn".into(),
                    message: "wrapper not yet spawned, send Spawn first".into(),
                }).await;
            }
            None => anyhow::bail!("IPC channel closed before receiving Spawn"),
        }
    };

    let mut state = WrapperState {
        service_name: service_name.to_string(),
        child: None,
        log_task: None,
        log_buf,
        notif_tx,
        own_binary_hash: hash_current_exe(),
    };

    state.spawn_child(&spec);

    let mut child_check = tokio::time::interval(Duration::from_secs(2));
    let mut self_check = tokio::time::interval(Duration::from_secs(60));
    self_check.tick().await; // consume immediate tick

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
                state.check_child_exit(&spec);
            }
            _ = self_check.tick() => {
                if state.own_binary_changed() {
                    pending_update_self = true;
                    break;
                }
            }
            Some((req, resp_tx)) = req_rx.recv() => {
                let is_update_self = matches!(req, IpcRequest::UpdateSelf);
                let is_shutdown = matches!(req, IpcRequest::Shutdown);

                let resp = handle_request(&mut state, &mut spec, req);
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

    state.kill_child();
    std::fs::remove_file(&socket_path).ok();

    if pending_update_self {
        tracing::info!("re-execing wrapper with new binary");
        do_update_self();
        tracing::error!("exec failed, exiting (service manager will restart)");
    }

    tracing::info!("{service_name} wrapper shutdown complete");
    Ok(())
}

fn handle_request(state: &mut WrapperState, spec: &mut SpawnSpec, req: IpcRequest) -> IpcResponse {
    match req {
        IpcRequest::Spawn(new_spec) => {
            tracing::info!("{}: respawn with new spec", state.service_name);
            *spec = new_spec;
            if state.respawn(spec) {
                IpcResponse::Ack { command: "spawn".into() }
            } else {
                IpcResponse::Error {
                    command: "spawn".into(),
                    message: "spawn failed".into(),
                }
            }
        }
        IpcRequest::Status => IpcResponse::Status {
            pid: state.child_pid(),
            running: state.child.is_some(),
        },
        IpcRequest::Shutdown => {
            tracing::info!("{}: shutdown requested", state.service_name);
            state.kill_child();
            IpcResponse::Ack { command: "shutdown".into() }
        }
        IpcRequest::UpdateSelf => {
            tracing::info!("{}: update-self requested", state.service_name);
            state.kill_child();
            IpcResponse::Ack { command: "update_self".into() }
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
            line: clean,
            is_stderr,
        });
    }
}

/// SHA-256 hash of the currently running mac-mgmt binary.
fn hash_current_exe() -> Option<Vec<u8>> {
    use sha2::{Digest, Sha256};
    let exe = std::env::current_exe().ok()?;
    let bytes = std::fs::read(&exe).ok()?;
    Some(Sha256::digest(&bytes).to_vec())
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
    tracing::error!("exec failed: {err}");
}
