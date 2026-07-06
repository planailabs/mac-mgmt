use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

use crate::procutil;
use crate::transport::{self, IpcStream};
use tokio::sync::{Mutex, broadcast, mpsc};
use tokio::task::JoinHandle;

use serde::{Deserialize, Serialize};

use crate::protocol::{Message, Notification, Request, Response, ServiceStatus, SpawnSpec};

/// Persisted state for seamless reexec. Written before exec, read on startup.
#[derive(Serialize, Deserialize)]
struct SavedState {
    services: Vec<SavedService>,
}

#[derive(Serialize, Deserialize)]
struct SavedService {
    name: String,
    spec: SpawnSpec,
    pid: u32,
    resolved_program: Option<String>,
}

/// Path to the state file used for reexec handoff.
fn reexec_state_path(socket_path: &Path) -> std::path::PathBuf {
    socket_path.with_extension("reexec-state.json")
}

/// Delay between failed spawns before retrying.
const RESPAWN_DELAY: Duration = Duration::from_secs(2);

/// Return the log directory for service stdout/stderr files.
fn log_dir(socket_path: &Path) -> std::path::PathBuf {
    socket_path
        .parent()
        .unwrap_or_else(|| Path::new("/tmp"))
        .join("service-logs")
}

/// Return the log file path for a service.
fn service_log_path(socket_path: &Path, name: &str) -> std::path::PathBuf {
    log_dir(socket_path).join(format!("{name}.log"))
}
/// Time to wait after SIGTERM before sending SIGKILL.
const STOP_GRACE: Duration = Duration::from_secs(10);

/// Entry point for the supervisor.
///
/// Binds a Unix socket at `socket_path`, then runs until the process is
/// signalled or a `Shutdown` request arrives.
pub async fn run(socket_path: &Path) -> Result<bool> {
    tracing::info!("supervisor starting on {}", socket_path.display());

    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let listener = transport::bind(socket_path)?;

    // Ensure log directory exists.
    let logdir = log_dir(socket_path);
    std::fs::create_dir_all(&logdir).ok();

    let state = Arc::new(SupervisorState::new(socket_path.to_path_buf()));
    let (notif_tx, _) = broadcast::channel::<Notification>(256);

    // Adopt children from a previous supervisor that reexec'd.
    let state_file = reexec_state_path(socket_path);
    if state_file.exists() {
        if let Ok(json) = std::fs::read_to_string(&state_file) {
            std::fs::remove_file(&state_file).ok();
            if let Ok(saved) = serde_json::from_str::<SavedState>(&json) {
                tracing::info!(
                    "supervisor: adopting {} child(ren) from previous instance",
                    saved.services.len()
                );
                for svc in saved.services {
                    state
                        .adopt(
                            svc.name,
                            svc.spec,
                            svc.pid,
                            svc.resolved_program,
                            notif_tx.clone(),
                        )
                        .await;
                }
            }
        } else {
            std::fs::remove_file(&state_file).ok();
        }
    }
    let (req_tx, mut req_rx) = mpsc::channel::<(Request, mpsc::Sender<Response>)>(64);

    {
        let req_tx = req_tx.clone();
        let notif_tx = notif_tx.clone();
        tokio::spawn(async move {
            loop {
                match transport::accept(&listener).await {
                    Ok(stream) => {
                        let req_tx = req_tx.clone();
                        let notif_rx = notif_tx.subscribe();
                        tokio::spawn(handle_client(stream, req_tx, notif_rx));
                    }
                    Err(e) => {
                        tracing::warn!("supervisor accept failed: {e}");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });
    }

    // OS shutdown signal, registered once on a background task (cross-platform:
    // SIGTERM/SIGINT on unix, Ctrl-C/console-close on windows).
    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
    tokio::spawn(async move {
        wait_for_shutdown_signal().await;
        let _ = shutdown_tx.send(()).await;
    });

    let mut reexec = false;

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                tracing::info!("supervisor received shutdown signal");
                break;
            }
            Some((req, resp_tx)) = req_rx.recv() => {
                match req {
                    Request::Shutdown => {
                        tracing::info!("supervisor: shutdown requested");
                        let _ = resp_tx.send(Response::Ok).await;
                        break;
                    }
                    Request::UpdateSelf => {
                        if reexec_would_change_binary() {
                            tracing::info!("supervisor: update-self requested, saving state for reexec");
                            state.save_state_for_reexec(&reexec_state_path(socket_path));
                            let _ = resp_tx.send(Response::Ok).await;
                            reexec = true;
                            break;
                        }
                        tracing::info!(
                            "supervisor: update-self requested, binary unchanged — skipping reexec"
                        );
                        let _ = resp_tx.send(Response::NoChange).await;
                    }
                    other => {
                        let resp = state.handle(other, &notif_tx).await;
                        let _ = resp_tx.send(resp).await;
                    }
                }
            }
        }
    }

    if reexec {
        // On reexec, leave children running — they keep the same parent PID
        // after exec(). State was saved by save_state_for_reexec().
        // Abort all supervisor tasks so they release Child handles without
        // triggering stop_child (the stop_tx channel is not used here).
        tracing::info!("supervisor: leaving children running for reexec");
        state.abort_all_tasks().await;
    } else {
        tracing::info!("supervisor tearing down children");
        state.shutdown_all().await;
    }
    std::fs::remove_file(socket_path).ok();

    Ok(reexec)
}

/// Resolve once the OS asks us to shut down.
#[cfg(unix)]
async fn wait_for_shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};
    let mut term = match signal(SignalKind::terminate()) {
        Ok(s) => s,
        Err(e) => { tracing::warn!("SIGTERM handler: {e}"); return std::future::pending().await; }
    };
    let mut int = match signal(SignalKind::interrupt()) {
        Ok(s) => s,
        Err(e) => { tracing::warn!("SIGINT handler: {e}"); return std::future::pending().await; }
    };
    tokio::select! {
        _ = term.recv() => {}
        _ = int.recv() => {}
    }
}

/// Resolve once the OS asks us to shut down (windows: Ctrl-C / console close).
#[cfg(windows)]
async fn wait_for_shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

/// True when argv[0] (the symlink the supervisor was launched through) now
/// resolves to a different binary than the running process image — i.e. a
/// self-update repointed the symlink and a reexec would actually pick up new
/// code. Errs on the side of `true` (reexec) when either path can't be
/// resolved (bare argv[0], deleted store path, dev build).
fn reexec_would_change_binary() -> bool {
    let argv0 = std::env::args()
        .next()
        .unwrap_or_else(|| "mac-mgmt".to_string());
    let target = std::fs::canonicalize(&argv0);
    let running = std::env::current_exe().and_then(std::fs::canonicalize);
    match (target, running) {
        (Ok(t), Ok(r)) => t != r,
        _ => true,
    }
}

/// Re-exec the current binary with its original argv. Call after
/// `run()` returns `Ok(true)`. Child processes survive the exec because
/// the PID stays the same — the new process image adopts them via
/// the state file written by `save_state_for_reexec`.
pub fn reexec_self() -> ! {
    // Use argv[0] (the symlink path) instead of current_exe() (which
    // resolves symlinks). After self-update the symlink points to the
    // new binary, so re-exec through it picks up the new version.
    let argv0 = std::env::args()
        .next()
        .unwrap_or_else(|| "mac-mgmt".to_string());
    let args: Vec<String> = std::env::args().skip(1).collect();
    tracing::info!("supervisor reexec {argv0:?} {args:?}");
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // execv: the PID stays the same, so children keep their parent.
        let err = std::process::Command::new(&argv0).args(&args).exec();
        tracing::error!("supervisor exec failed: {err}");
        std::process::exit(1);
    }
    #[cfg(windows)]
    {
        // No execv on windows: spawn a fresh copy (it adopts the running
        // children via the saved state file) then exit. The children are
        // separate processes and survive the parent exiting.
        match std::process::Command::new(&argv0).args(&args).spawn() {
            Ok(_) => std::process::exit(0),
            Err(e) => {
                tracing::error!("supervisor respawn failed: {e}");
                std::process::exit(1);
            }
        }
    }
}

// ── Per-connection handler ───────────────────────────────────────────

async fn handle_client(
    stream: IpcStream,
    req_tx: mpsc::Sender<(Request, mpsc::Sender<Response>)>,
    mut notif_rx: broadcast::Receiver<Notification>,
) {
    let (reader, mut writer) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    tracing::info!("supervisor: daemon connected");

    loop {
        line.clear();
        tokio::select! {
            result = reader.read_line(&mut line) => {
                match result {
                    Ok(0) => {
                        tracing::info!("supervisor: daemon disconnected");
                        break;
                    }
                    Ok(_) => {
                        let msg: Message = match serde_json::from_str(line.trim()) {
                            Ok(m) => m,
                            Err(e) => {
                                tracing::warn!("supervisor: invalid message: {e}");
                                continue;
                            }
                        };
                        let Message::Request(req) = msg else {
                            tracing::warn!("supervisor: unexpected non-request");
                            continue;
                        };
                        let (resp_tx, mut resp_rx) = mpsc::channel(1);
                        if req_tx.send((req, resp_tx)).await.is_err() {
                            break;
                        }
                        let Some(resp) = resp_rx.recv().await else { break };
                        if write_msg(&mut writer, Message::Response(resp)).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::warn!("supervisor: read error: {e}");
                        break;
                    }
                }
            }
            notif = notif_rx.recv() => {
                match notif {
                    Ok(n) => {
                        if write_msg(&mut writer, Message::Notification(n)).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("supervisor: daemon notif lag, dropped {n}");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}

async fn write_msg(writer: &mut tokio::io::WriteHalf<IpcStream>, msg: Message) -> Result<()> {
    let mut line = serde_json::to_string(&msg).context("serialize")?;
    line.push('\n');
    writer.write_all(line.as_bytes()).await.context("write")?;
    writer.flush().await.context("flush")?;
    Ok(())
}

// ── Supervisor state ─────────────────────────────────────────────────

struct SupervisorState {
    services: Mutex<HashMap<String, Entry>>,
    socket_path: std::path::PathBuf,
}

struct Entry {
    spec: SpawnSpec,
    /// `None` when the service is stopped-but-registered.
    supervisor: Option<JoinHandle<()>>,
    /// `None` when the service is stopped-but-registered.
    stop_tx: Option<mpsc::Sender<()>>,
    /// Live PID of the currently running child (0 when none).
    pid: Arc<AtomicU32>,
    /// Canonical path of `spec.program` resolved at spawn time.
    resolved_program: Option<String>,
    /// True when the service was explicitly stopped (not crashed).
    stopped: bool,
}

impl SupervisorState {
    fn new(socket_path: std::path::PathBuf) -> Self {
        Self {
            services: Mutex::new(HashMap::new()),
            socket_path,
        }
    }

    async fn handle(
        self: &Arc<Self>,
        req: Request,
        notif_tx: &broadcast::Sender<Notification>,
    ) -> Response {
        match req {
            Request::Register { name, spec } => {
                self.register(name, spec, notif_tx.clone()).await;
                Response::Ok
            }
            Request::Unregister { name } => {
                self.unregister(&name).await;
                Response::Ok
            }
            Request::Stop { name } => self.stop_svc(&name).await,
            Request::Start { name } => self.start_svc(&name, notif_tx.clone()).await,
            Request::Restart { name } => self.restart_svc(&name, notif_tx.clone()).await,
            Request::Kill { name, signal } => self.kill_svc(&name, signal).await,
            Request::List => {
                let map = self.services.lock().await;
                let statuses: Vec<ServiceStatus> = map
                    .iter()
                    .map(|(name, entry)| {
                        let pid = entry.pid.load(Ordering::Relaxed);
                        let pid = if pid == 0 { None } else { Some(pid) };
                        let exe = pid.and_then(resolve_exe);
                        ServiceStatus {
                            name: name.clone(),
                            pid,
                            exe,
                            resolved_program: entry.resolved_program.clone(),
                            spec: Some(entry.spec.clone()),
                            stopped: entry.stopped,
                        }
                    })
                    .collect();
                Response::services(statuses)
            }
            Request::Shutdown | Request::UpdateSelf => Response::Error {
                message: "handled by main loop".into(),
            },
        }
    }

    async fn register(
        self: &Arc<Self>,
        name: String,
        spec: SpawnSpec,
        notif_tx: broadcast::Sender<Notification>,
    ) {
        // Check for an existing entry.
        let existing = {
            let map = self.services.lock().await;
            map.get(&name).map(|e| (e.spec.clone(), e.stopped))
        };
        if let Some((old_spec, was_stopped)) = existing {
            if old_spec == spec {
                if was_stopped {
                    // Same spec but stopped — just start it.
                    tracing::info!("supervisor: {name} stopped with matching spec, starting");
                    self.start_svc(&name, notif_tx).await;
                    return;
                }
                tracing::debug!("supervisor: {name} already registered with matching spec");
                return;
            }
            tracing::info!("supervisor: {name} spec changed, respawning");
            self.unregister(&name).await;
        }

        tracing::info!("supervisor: registering {name}");
        let resolved_program = resolve_program(&spec.program);
        let (stop_tx, stop_rx) = mpsc::channel(1);
        let pid = Arc::new(AtomicU32::new(0));
        let task_name = name.clone();
        let task_spec = spec.clone();
        let task_pid = pid.clone();
        let task = tokio::spawn(run_service(
            task_name,
            task_spec,
            notif_tx,
            stop_rx,
            task_pid,
            self.socket_path.clone(),
        ));
        self.services.lock().await.insert(
            name,
            Entry {
                spec,
                supervisor: Some(task),
                stop_tx: Some(stop_tx),
                pid,
                resolved_program,
                stopped: false,
            },
        );
    }

    /// Adopt an existing child process from a previous supervisor instance.
    async fn adopt(
        self: &Arc<Self>,
        name: String,
        spec: SpawnSpec,
        pid: u32,
        resolved_program: Option<String>,
        notif_tx: broadcast::Sender<Notification>,
    ) {
        // Check if the PID is still alive.
        let alive = procutil::is_alive(pid);
        if !alive {
            tracing::warn!("supervisor: cannot adopt {name} (pid {pid}): process not found");
            return;
        }
        tracing::info!("supervisor: adopting {name} (pid {pid})");

        let (stop_tx, stop_rx) = mpsc::channel(1);
        let pid_arc = Arc::new(AtomicU32::new(pid));
        let task_name = name.clone();
        let task_spec = spec.clone();
        let task_pid = pid_arc.clone();
        let task = tokio::spawn(monitor_adopted(
            task_name,
            task_spec,
            pid,
            notif_tx,
            stop_rx,
            task_pid,
            self.socket_path.clone(),
        ));
        self.services.lock().await.insert(
            name,
            Entry {
                spec,
                supervisor: Some(task),
                stop_tx: Some(stop_tx),
                pid: pid_arc,
                resolved_program,
                stopped: false,
            },
        );
    }

    async fn unregister(&self, name: &str) {
        let existing = self.services.lock().await.remove(name);
        if let Some(entry) = existing {
            tracing::info!("supervisor: unregistering {name}");
            if let Some(stop_tx) = entry.stop_tx {
                let _ = stop_tx.send(()).await;
            }
            if let Some(supervisor) = entry.supervisor {
                let _ = supervisor.await;
            }
        }
    }

    /// Stop a service but keep it registered so it can be started again.
    async fn stop_svc(self: &Arc<Self>, name: &str) -> Response {
        let mut map = self.services.lock().await;
        let Some(entry) = map.get_mut(name) else {
            return Response::Error {
                message: format!("unknown service '{name}'"),
            };
        };
        if entry.stopped {
            return Response::Ok; // already stopped
        }
        if let Some(stop_tx) = entry.stop_tx.take() {
            let _ = stop_tx.send(()).await;
        }
        if let Some(supervisor) = entry.supervisor.take() {
            // Drop the lock before awaiting the task to avoid deadlock.
            drop(map);
            let _ = supervisor.await;
            let mut map = self.services.lock().await;
            if let Some(entry) = map.get_mut(name) {
                entry.stopped = true;
            }
        } else {
            entry.stopped = true;
        }
        Response::Ok
    }

    /// Start a previously stopped service using its stored spec.
    async fn start_svc(
        self: &Arc<Self>,
        name: &str,
        notif_tx: broadcast::Sender<Notification>,
    ) -> Response {
        let mut map = self.services.lock().await;
        let Some(entry) = map.get_mut(name) else {
            return Response::Error {
                message: format!("unknown service '{name}'"),
            };
        };
        if !entry.stopped {
            return Response::Ok; // already running
        }

        tracing::info!("supervisor: starting stopped service {name}");
        let resolved_program = resolve_program(&entry.spec.program);
        let (stop_tx, stop_rx) = mpsc::channel(1);
        let pid = Arc::new(AtomicU32::new(0));
        let task = tokio::spawn(run_service(
            name.to_string(),
            entry.spec.clone(),
            notif_tx,
            stop_rx,
            pid.clone(),
            self.socket_path.clone(),
        ));
        entry.supervisor = Some(task);
        entry.stop_tx = Some(stop_tx);
        entry.pid = pid;
        entry.resolved_program = resolved_program;
        entry.stopped = false;
        Response::Ok
    }

    /// Stop then immediately re-start a named service.
    async fn restart_svc(
        self: &Arc<Self>,
        name: &str,
        notif_tx: broadcast::Sender<Notification>,
    ) -> Response {
        {
            let map = self.services.lock().await;
            if !map.contains_key(name) {
                return Response::Error {
                    message: format!("unknown service '{name}'"),
                };
            }
        }
        let resp = self.stop_svc(name).await;
        if !matches!(resp, Response::Ok) {
            return resp;
        }
        self.start_svc(name, notif_tx).await
    }

    /// Send a signal to the service's main process.
    async fn kill_svc(self: &Arc<Self>, name: &str, signal: i32) -> Response {
        let map = self.services.lock().await;
        let Some(entry) = map.get(name) else {
            return Response::Error {
                message: format!("unknown service '{name}'"),
            };
        };
        let pid = entry.pid.load(Ordering::Relaxed);
        if pid == 0 {
            return Response::Error {
                message: format!("service '{name}' has no running process"),
            };
        }
        match procutil::send_signal(pid, signal) {
            Ok(()) => Response::Ok,
            Err(err) => Response::Error {
                message: format!("kill({pid}, {signal}): {err}"),
            },
        }
    }

    /// Save current service state to a file so the new process can adopt children.
    fn save_state_for_reexec(&self, state_path: &Path) {
        // Use try_lock since we're on the event loop — block would deadlock.
        let map = match self.services.try_lock() {
            Ok(m) => m,
            Err(_) => {
                tracing::warn!("supervisor: could not lock services for reexec state save");
                return;
            }
        };
        let saved = SavedState {
            services: map
                .iter()
                .filter_map(|(name, entry)| {
                    let pid = entry.pid.load(Ordering::Relaxed);
                    if pid == 0 {
                        return None;
                    }
                    Some(SavedService {
                        name: name.clone(),
                        spec: entry.spec.clone(),
                        pid,
                        resolved_program: entry.resolved_program.clone(),
                    })
                })
                .collect(),
        };
        match serde_json::to_string(&saved) {
            Ok(json) => {
                if let Err(e) = std::fs::write(state_path, json) {
                    tracing::warn!("supervisor: failed to write reexec state: {e}");
                } else {
                    tracing::info!(
                        "supervisor: saved {} service(s) for reexec",
                        saved.services.len()
                    );
                }
            }
            Err(e) => tracing::warn!("supervisor: failed to serialize reexec state: {e}"),
        }
    }

    /// Abort all supervisor tasks without stopping children. Used before
    /// reexec so the tokio runtime can shut down without triggering the
    /// stop channel or dropping Child handles while children are alive.
    async fn abort_all_tasks(&self) {
        let mut map = self.services.lock().await;
        // Abort tasks first so they can't react to the stop_tx being dropped.
        for entry in map.values() {
            if let Some(ref supervisor) = entry.supervisor {
                supervisor.abort();
            }
        }
        map.clear();
    }

    async fn shutdown_all(&self) {
        let drained: Vec<(String, Entry)> = self.services.lock().await.drain().collect();
        for (name, entry) in drained {
            tracing::info!("supervisor: stopping {name}");
            if let Some(stop_tx) = entry.stop_tx {
                let _ = stop_tx.send(()).await;
            }
            if let Some(supervisor) = entry.supervisor {
                let _ = supervisor.await;
            }
        }
    }
}

// ── Per-service runner ──────────────────────────────────────────────

/// Monitor an adopted child process (from reexec). Re-attaches to the
/// child's stdout/stderr via /proc/<pid>/fd/{1,2} so log forwarding
/// continues after the supervisor reexec. Then waits for exit and
/// falls into the normal spawn-and-supervise loop.
async fn monitor_adopted(
    name: String,
    spec: SpawnSpec,
    adopted_pid: u32,
    notif_tx: broadcast::Sender<Notification>,
    mut stop_rx: mpsc::Receiver<()>,
    pid: Arc<AtomicU32>,
    socket_path: std::path::PathBuf,
) {
    tracing::info!("supervisor: monitoring adopted {name} (pid {adopted_pid})");

    // Re-attach log forwarding by tailing the service log file.
    // Since children write to log files (not pipes), the new supervisor
    // just tails the same file from the current position.
    let tail_handle = spawn_log_tailer(&name, &socket_path, notif_tx.clone());

    // Wait for the adopted process to exit by polling waitpid(WNOHANG).
    // We can't use kill(pid,0) because zombies still exist in the process
    // table — kill returns success for them, so we'd never detect the exit.
    // waitpid both detects AND reaps the zombie in one call.
    let exited = loop {
        // reap_if_exited detects AND reaps the zombie in one call (unix); on
        // windows it just checks liveness (no zombies).
        if procutil::reap_if_exited(adopted_pid) {
            break true;
        }
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(1)) => {}
            _ = stop_rx.recv() => {
                // Stop requested — kill the adopted child.
                tracing::info!("supervisor: stopping adopted {name} (pid {adopted_pid})");
                let _ = procutil::send_signal(adopted_pid, procutil::SIGTERM);
                tokio::time::sleep(Duration::from_secs(5)).await;
                if !procutil::reap_if_exited(adopted_pid) {
                    // Still running after grace period — force kill and reap.
                    let _ = procutil::send_signal(adopted_pid, procutil::SIGKILL);
                    procutil::reap_blocking(adopted_pid);
                }
                pid.store(0, Ordering::Relaxed);
                tail_handle.abort();
                return;
            }
        }
    };

    pid.store(0, Ordering::Relaxed);
    if exited {
        tracing::info!("supervisor: adopted {name} (pid {adopted_pid}) exited, respawning");
        let _ = notif_tx.send(Notification::Crashed {
            name: name.clone(),
            exit_code: None,
        });
        tokio::select! {
            _ = tokio::time::sleep(RESPAWN_DELAY) => {}
            _ = stop_rx.recv() => return,
        }
    }

    // Fall into normal spawn-and-supervise loop.
    tail_handle.abort();
    run_service(name, spec, notif_tx, stop_rx, pid, socket_path).await;
}

async fn run_service(
    name: String,
    spec: SpawnSpec,
    notif_tx: broadcast::Sender<Notification>,
    mut stop_rx: mpsc::Receiver<()>,
    pid: Arc<AtomicU32>,
    socket_path: std::path::PathBuf,
) {
    loop {
        let child = match spawn_child(&name, &spec, &socket_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("supervisor: {name} spawn failed: {e:#}");
                let _ = notif_tx.send(Notification::Crashed {
                    name: name.clone(),
                    exit_code: None,
                });
                tokio::select! {
                    _ = tokio::time::sleep(RESPAWN_DELAY) => continue,
                    _ = stop_rx.recv() => return,
                }
            }
        };
        pid.store(child.id().unwrap_or(0), Ordering::Relaxed);
        let exit = wait_child(&name, child, &notif_tx, &mut stop_rx, &socket_path).await;
        pid.store(0, Ordering::Relaxed);
        match exit {
            ChildExit::Stopped => return,
            ChildExit::Exited { code } => {
                let _ = notif_tx.send(Notification::Crashed {
                    name: name.clone(),
                    exit_code: code,
                });
                tokio::select! {
                    _ = tokio::time::sleep(RESPAWN_DELAY) => {}
                    _ = stop_rx.recv() => return,
                }
            }
        }
    }
}

enum ChildExit {
    Stopped,
    Exited { code: Option<i32> },
}

fn spawn_child(name: &str, spec: &SpawnSpec, socket_path: &Path) -> Result<Child> {
    let log_path = service_log_path(socket_path, name);
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("open log file {}", log_path.display()))?;
    let log_file2 = log_file
        .try_clone()
        .with_context(|| "clone log file handle")?;

    let mut cmd = Command::new(&spec.program);
    cmd.args(&spec.args)
        .envs(&spec.env)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(log_file2))
        .kill_on_drop(false);
    let child = cmd
        .spawn()
        .with_context(|| format!("spawn {} for {name}", spec.program))?;
    tracing::info!(
        "supervisor: {name} spawned pid={:?} ({} {}) log={}",
        child.id(),
        spec.program,
        spec.args.join(" "),
        log_path.display(),
    );
    Ok(child)
}

async fn wait_child(
    name: &str,
    mut child: Child,
    notif_tx: &broadcast::Sender<Notification>,
    stop_rx: &mut mpsc::Receiver<()>,
    socket_path: &Path,
) -> ChildExit {
    // Tail the log file for live forwarding to the daemon.
    let tail_handle = spawn_log_tailer(name, socket_path, notif_tx.clone());

    let exit = tokio::select! {
        status = child.wait() => {
            let code = status.ok().and_then(|s| s.code());
            tracing::warn!("supervisor: {name} exited code={code:?}");
            ChildExit::Exited { code }
        }
        _ = stop_rx.recv() => {
            tracing::info!("supervisor: {name} stop requested");
            stop_child(&mut child).await;
            ChildExit::Stopped
        }
    };

    tail_handle.abort();
    exit
}

async fn stop_child(child: &mut Child) {
    if let Some(pid) = child.id() {
        // Graceful stop: SIGTERM on unix, TerminateProcess on windows.
        let _ = procutil::send_signal(pid, procutil::SIGTERM);
        let deadline = tokio::time::Instant::now() + STOP_GRACE;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) if tokio::time::Instant::now() >= deadline => break,
                Ok(None) => tokio::time::sleep(Duration::from_millis(100)).await,
                Err(_) => return,
            }
        }
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
}

/// Spawn a task that tails a service log file from the current end,
/// forwarding new lines as notifications. Works across supervisor
/// reexec since it reads from a file, not a pipe.
fn spawn_log_tailer(
    name: &str,
    socket_path: &Path,
    notif_tx: broadcast::Sender<Notification>,
) -> JoinHandle<()> {
    let name = name.to_string();
    let log_path = service_log_path(socket_path, &name);
    tokio::spawn(async move {
        // Open the file and seek to end so we only forward new lines.
        let file = match tokio::fs::File::open(&log_path).await {
            Ok(f) => f,
            Err(e) => {
                tracing::debug!("supervisor: cannot tail {}: {e}", log_path.display());
                return;
            }
        };
        let mut reader = BufReader::new(file);
        // Seek to end.
        use tokio::io::AsyncSeekExt;
        if reader.seek(std::io::SeekFrom::End(0)).await.is_err() {
            return;
        }
        let mut lines = reader.lines();
        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    let clean = strip_ansi(&line);
                    tracing::info!(target: "service", "[{name}] {clean}");
                    let _ = notif_tx.send(Notification::Log {
                        name: name.clone(),
                        line: clean,
                        is_stderr: false,
                    });
                }
                Ok(None) => {
                    // EOF — file hasn't been written to yet. Wait and retry.
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    // Re-read without seeking — the reader position stays at the last read.
                }
                Err(_) => break,
            }
        }
    })
}

/// Resolve the executable of a running pid. Used by `List` so callers can
/// detect when the supervised process is an older binary than what's
/// currently installed. Only implemented on Linux (where `/proc/<pid>/exe`
/// is cheap and well-defined); returns `None` elsewhere.
#[cfg(target_os = "linux")]
fn resolve_exe(pid: u32) -> Option<String> {
    let path = std::fs::read_link(format!("/proc/{pid}/exe")).ok()?;
    Some(path.to_string_lossy().into_owned())
}

#[cfg(not(target_os = "linux"))]
fn resolve_exe(_pid: u32) -> Option<String> {
    None
}

/// Resolve `spec.program` to a canonical path at spawn time.  For bare
/// names (no `/`) this does a PATH lookup via the `which` crate; then
/// canonicalises the result.  The canonical path is what `binary_store_path`
/// on the daemon side produces, so comparing the two correctly detects
/// store-path drift even when the binary is a shebang wrapper script (where
/// `/proc/<pid>/exe` would point at the interpreter instead).
fn resolve_program(program: &str) -> Option<String> {
    let abs = if program.contains('/') {
        std::path::PathBuf::from(program)
    } else {
        which::which(program).ok()?
    };
    std::fs::canonicalize(abs)
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

/// Start the supervisor on a temporary socket, returning the socket path.
/// Used by tests to exercise the full supervisor ↔ client round-trip.
#[cfg(test)]
async fn start_test_supervisor() -> (std::path::PathBuf, tokio::task::JoinHandle<bool>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let sock = dir.keep().join("test.sock");
    let sock2 = sock.clone();
    let handle = tokio::spawn(async move { run(&sock2).await.unwrap() });
    // Wait briefly for the listener to bind.
    tokio::time::sleep(Duration::from_millis(100)).await;
    (sock, handle)
}

/// Strip common ANSI escape sequences without pulling in an extra dep.
fn strip_ansi(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == 0x1b && i + 1 < bytes.len() {
            // ESC sequence — skip until a terminating byte.
            i += 1;
            let next = bytes[i];
            if next == b'[' {
                i += 1;
                while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                    i += 1;
                }
                if i < bytes.len() {
                    i += 1;
                }
            } else if next == b']' {
                i += 1;
                while i < bytes.len() && bytes[i] != 0x07 && bytes[i] != 0x1b {
                    i += 1;
                }
                if i < bytes.len()
                    && bytes[i] == 0x1b
                    && i + 1 < bytes.len()
                    && bytes[i + 1] == b'\\'
                {
                    i += 2;
                } else if i < bytes.len() {
                    i += 1;
                }
            } else {
                i += 1;
            }
        } else {
            out.push(b);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| input.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Client;

    async fn setup() -> (std::path::PathBuf, tokio::task::JoinHandle<bool>) {
        start_test_supervisor().await
    }

    async fn client(sock: &Path) -> Client {
        Client::connect(sock, Duration::from_secs(5)).await.unwrap()
    }

    fn sleep_spec() -> SpawnSpec {
        SpawnSpec {
            program: "sleep".into(),
            args: vec!["3600".into()],
            env: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_update_self_unchanged_binary_skips_reexec() {
        // In tests argv[0] and current_exe both resolve to the test binary,
        // so the supervisor must report NoChange and keep running.
        let (sock, _sup) = setup().await;
        let mut c = client(&sock).await;

        let reexecing = c.update_self().await.unwrap();
        assert!(!reexecing, "unchanged binary must not trigger a reexec");

        // Supervisor is still alive and serving requests.
        let list = c.list().await.unwrap();
        assert!(list.is_empty());

        c.shutdown().await.ok();
    }

    #[tokio::test]
    async fn test_register_and_list() {
        let (sock, _sup) = setup().await;
        let mut c = client(&sock).await;

        c.register("test-svc", sleep_spec()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;

        let list = c.list().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "test-svc");
        assert!(list[0].pid.is_some(), "should have a PID");
        assert!(!list[0].stopped);

        c.shutdown().await.ok();
    }

    #[tokio::test]
    async fn test_stop_keeps_registration() {
        let (sock, _sup) = setup().await;
        let mut c = client(&sock).await;

        c.register("test-svc", sleep_spec()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;

        c.stop_service("test-svc").await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;

        let list = c.list().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "test-svc");
        assert!(list[0].stopped, "should be stopped");
        assert!(list[0].pid.is_none() || list[0].pid == Some(0));

        c.shutdown().await.ok();
    }

    #[tokio::test]
    async fn test_start_stopped_service() {
        let (sock, _sup) = setup().await;
        let mut c = client(&sock).await;

        c.register("test-svc", sleep_spec()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;

        c.stop_service("test-svc").await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;

        c.start_service("test-svc").await.unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;

        let list = c.list().await.unwrap();
        assert_eq!(list.len(), 1);
        assert!(!list[0].stopped, "should not be stopped");
        assert!(
            list[0].pid.is_some() && list[0].pid != Some(0),
            "should have a PID"
        );

        c.shutdown().await.ok();
    }

    #[tokio::test]
    async fn test_restart_service() {
        let (sock, _sup) = setup().await;
        let mut c = client(&sock).await;

        c.register("test-svc", sleep_spec()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;

        let list1 = c.list().await.unwrap();
        let pid1 = list1[0].pid;

        c.restart_service("test-svc").await.unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;

        let list2 = c.list().await.unwrap();
        assert_eq!(list2.len(), 1);
        assert!(!list2[0].stopped);
        assert!(list2[0].pid.is_some() && list2[0].pid != Some(0));
        // PID should have changed after restart.
        assert_ne!(list2[0].pid, pid1, "PID should change after restart");

        c.shutdown().await.ok();
    }

    #[tokio::test]
    async fn test_kill_service() {
        let (sock, _sup) = setup().await;
        let mut c = client(&sock).await;

        c.register("test-svc", sleep_spec()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;

        // SIGTERM — supervisor will respawn.
        c.kill_service("test-svc", crate::procutil::SIGTERM).await.unwrap();
        // Give supervisor time to detect exit and respawn.
        tokio::time::sleep(Duration::from_secs(3)).await;

        let list = c.list().await.unwrap();
        assert_eq!(list.len(), 1);
        assert!(
            list[0].pid.is_some() && list[0].pid != Some(0),
            "should be respawned"
        );

        c.shutdown().await.ok();
    }

    #[tokio::test]
    async fn test_stop_unknown_service() {
        let (sock, _sup) = setup().await;
        let mut c = client(&sock).await;

        let result = c.stop_service("nonexistent").await;
        assert!(result.is_err());

        c.shutdown().await.ok();
    }

    #[tokio::test]
    async fn test_start_already_running() {
        let (sock, _sup) = setup().await;
        let mut c = client(&sock).await;

        c.register("test-svc", sleep_spec()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Starting an already running service should be a no-op.
        c.start_service("test-svc").await.unwrap();

        let list = c.list().await.unwrap();
        assert_eq!(list.len(), 1);
        assert!(!list[0].stopped);

        c.shutdown().await.ok();
    }

    #[tokio::test]
    async fn test_unregister_stopped_service() {
        let (sock, _sup) = setup().await;
        let mut c = client(&sock).await;

        c.register("test-svc", sleep_spec()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;

        c.stop_service("test-svc").await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;

        c.unregister("test-svc").await.unwrap();

        let list = c.list().await.unwrap();
        assert!(list.is_empty());

        c.shutdown().await.ok();
    }
}
