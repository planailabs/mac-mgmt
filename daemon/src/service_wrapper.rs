use anyhow::{Context, Result};
use std::time::Duration;

use crate::service_ipc::listener::{spawn_listener, IpcListener, NotificationSender};
use crate::service_ipc::protocol::{IpcNotification, IpcRequest, IpcResponse, SpawnSpec};

/// What the main loop should do after handling an IPC request.
enum Action {
    Continue,
    Shutdown,
    UpdateSelf,
}

/// Entry point for `mac-mgmt daemon-service-launch <service>`.
pub async fn run(service_name: &str) -> Result<()> {
    tracing::info!("wrapper starting for {service_name}");

    let socket_path = crate::service_ipc::socket_path(service_name);
    let listener = IpcListener::bind(&socket_path)
        .with_context(|| format!("bind {}", socket_path.display()))?;
    tracing::info!("waiting for Spawn on {}", socket_path.display());

    let (mut req_rx, notif_tx) = spawn_listener(listener);

    // Wait for the initial Spawn.
    let mut spec = loop {
        match req_rx.recv().await {
            Some((IpcRequest::Spawn(spec), resp_tx)) => {
                let _ = resp_tx.send(IpcResponse::Ok).await;
                break spec;
            }
            Some((_, resp_tx)) => {
                let _ = resp_tx
                    .send(IpcResponse::Error { message: "send Spawn first".into() })
                    .await;
            }
            None => anyhow::bail!("IPC closed before Spawn"),
        }
    };

    let mut child: Option<std::process::Child> = None;
    let mut log_task: Option<tokio::task::JoinHandle<()>> = None;
    let exe_mtime = exe_modified();

    spawn(&mut child, &mut log_task, service_name, &spec, &notif_tx);

    let mut child_tick = tokio::time::interval(Duration::from_secs(2));
    let mut self_tick = tokio::time::interval(Duration::from_secs(60));
    self_tick.tick().await; // skip immediate

    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .context("SIGTERM")?;
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .context("SIGINT")?;

    let mut do_update_self = false;

    loop {
        tokio::select! {
            _ = sigterm.recv() => break,
            _ = sigint.recv() => break,
            _ = child_tick.tick() => {
                check_child(&mut child, &mut log_task, service_name, &spec, &notif_tx);
            }
            _ = self_tick.tick() => {
                if exe_mtime != exe_modified() {
                    tracing::info!("{service_name}: binary changed, re-exec");
                    do_update_self = true;
                    break;
                }
            }
            Some((req, resp_tx)) = req_rx.recv() => {
                let (resp, action) = handle(
                    req,
                    &mut child,
                    &mut log_task,
                    &mut spec,
                    service_name,
                    &notif_tx,
                );
                let _ = resp_tx.send(resp).await;
                match action {
                    Action::Continue => {}
                    Action::Shutdown => break,
                    Action::UpdateSelf => { do_update_self = true; break; }
                }
            }
        }
    }

    kill(&mut child, &mut log_task);
    std::fs::remove_file(&socket_path).ok();

    if do_update_self {
        reexec();
    }

    Ok(())
}

// ── Child management ─────────────────────────────────────────────────

fn spawn(
    child: &mut Option<std::process::Child>,
    log_task: &mut Option<tokio::task::JoinHandle<()>>,
    name: &str,
    spec: &SpawnSpec,
    notif_tx: &NotificationSender,
) {
    kill(child, log_task);
    match std::process::Command::new(&spec.program)
        .args(&spec.args)
        .envs(&spec.env)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(mut c) => {
            *log_task = Some(forward_logs(name, &mut c, notif_tx));
            tracing::info!("{name} spawned (pid {})", c.id());
            *child = Some(c);
        }
        Err(e) => tracing::error!("{name} spawn failed: {e}"),
    }
}

fn kill(child: &mut Option<std::process::Child>, log_task: &mut Option<tokio::task::JoinHandle<()>>) {
    if let Some(c) = child.as_mut() {
        let _ = c.kill();
        let _ = c.wait();
    }
    if let Some(t) = log_task.take() {
        t.abort();
    }
    *child = None;
}

fn check_child(
    child: &mut Option<std::process::Child>,
    log_task: &mut Option<tokio::task::JoinHandle<()>>,
    name: &str,
    spec: &SpawnSpec,
    notif_tx: &NotificationSender,
) {
    let Some(c) = child.as_mut() else { return };
    match c.try_wait() {
        Ok(Some(status)) => {
            tracing::warn!("{name} exited ({status})");
            notif_tx.send(IpcNotification::Crashed { exit_code: status.code() });
            if let Some(t) = log_task.take() { t.abort(); }
            *child = None;
            spawn(child, log_task, name, spec, notif_tx);
        }
        Ok(None) => {}
        Err(e) => tracing::error!("{name} try_wait: {e}"),
    }
}

// ── IPC handler ──────────────────────────────────────────────────────

fn handle(
    req: IpcRequest,
    child: &mut Option<std::process::Child>,
    log_task: &mut Option<tokio::task::JoinHandle<()>>,
    spec: &mut SpawnSpec,
    name: &str,
    notif_tx: &NotificationSender,
) -> (IpcResponse, Action) {
    match req {
        IpcRequest::Spawn(new_spec) => {
            *spec = new_spec;
            spawn(child, log_task, name, spec, notif_tx);
            (IpcResponse::Ok, Action::Continue)
        }
        IpcRequest::Shutdown => {
            kill(child, log_task);
            (IpcResponse::Ok, Action::Shutdown)
        }
        IpcRequest::UpdateSelf => {
            kill(child, log_task);
            (IpcResponse::Ok, Action::UpdateSelf)
        }
    }
}

// ── Log forwarding ───────────────────────────────────────────────────

fn forward_logs(
    name: &str,
    child: &mut std::process::Child,
    notif_tx: &NotificationSender,
) -> tokio::task::JoinHandle<()> {
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let name = name.to_string();
    let tx = notif_tx.clone();

    tokio::task::spawn_blocking(move || {
        let stderr_thread = stderr.map(|r| {
            let name = name.clone();
            let tx = tx.clone();
            std::thread::spawn(move || drain(std::io::BufReader::new(r), &name, true, &tx))
        });
        if let Some(r) = stdout {
            drain(std::io::BufReader::new(r), &name, false, &tx);
        }
        if let Some(t) = stderr_thread {
            let _ = t.join();
        }
    })
}

fn drain(reader: impl std::io::BufRead, name: &str, is_stderr: bool, tx: &NotificationSender) {
    for line in reader.lines() {
        let Ok(raw) = line else { break };
        let clean = String::from_utf8(strip_ansi_escapes::strip(&raw)).unwrap_or(raw);
        if is_stderr {
            tracing::warn!(target: "service", "[{name}] {clean}");
        } else {
            tracing::info!(target: "service", "[{name}] {clean}");
        }
        tx.send(IpcNotification::Log { line: clean, is_stderr });
    }
}

// ── Self-update ──────────────────────────────────────────────────────

fn exe_modified() -> Option<std::time::SystemTime> {
    std::env::current_exe().ok().and_then(|p| std::fs::metadata(&p).ok()?.modified().ok())
}

fn reexec() {
    use std::os::unix::process::CommandExt;
    let Ok(exe) = std::env::current_exe() else { return };
    let args: Vec<String> = std::env::args().skip(1).collect();
    tracing::info!("exec {exe:?} {args:?}");
    let err = std::process::Command::new(&exe).args(&args).exec();
    tracing::error!("exec failed: {err}");
}
