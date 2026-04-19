use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, broadcast, mpsc};
use tokio::task::JoinHandle;

use crate::protocol::{Message, Notification, Request, Response, ServiceStatus, SpawnSpec};

/// Delay between failed spawns before retrying.
const RESPAWN_DELAY: Duration = Duration::from_secs(2);
/// Time to wait after SIGTERM before sending SIGKILL.
const STOP_GRACE: Duration = Duration::from_secs(10);

/// Entry point for the supervisor.
///
/// Binds a Unix socket at `socket_path`, then runs until the process is
/// signalled, a `Shutdown` request arrives, or an `UpdateSelf` request
/// arrives. On `UpdateSelf` the function returns `Ok(true)` so the caller
/// can re-exec the current binary via [`reexec_self`].
pub async fn run(socket_path: &Path) -> Result<bool> {
    tracing::info!("supervisor starting on {}", socket_path.display());

    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    if socket_path.exists() {
        std::fs::remove_file(socket_path).ok();
    }
    let listener = UnixListener::bind(socket_path)
        .with_context(|| format!("bind {}", socket_path.display()))?;

    let state = Arc::new(SupervisorState::new());
    let (notif_tx, _) = broadcast::channel::<Notification>(256);
    let (req_tx, mut req_rx) = mpsc::channel::<(Request, mpsc::Sender<Response>)>(64);

    {
        let req_tx = req_tx.clone();
        let notif_tx = notif_tx.clone();
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
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

    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .context("SIGTERM handler")?;
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .context("SIGINT handler")?;

    let mut reexec = false;

    loop {
        tokio::select! {
            _ = sigterm.recv() => {
                tracing::info!("supervisor received SIGTERM");
                break;
            }
            _ = sigint.recv() => {
                tracing::info!("supervisor received SIGINT");
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
                        tracing::info!("supervisor: update-self requested");
                        let _ = resp_tx.send(Response::Ok).await;
                        reexec = true;
                        break;
                    }
                    other => {
                        let resp = state.handle(other, &notif_tx).await;
                        let _ = resp_tx.send(resp).await;
                    }
                }
            }
        }
    }

    tracing::info!("supervisor tearing down children");
    state.shutdown_all().await;
    std::fs::remove_file(socket_path).ok();

    Ok(reexec)
}

/// Re-exec the current binary with its original argv. Intended for use after
/// `run()` returns `Ok(true)`.
pub fn reexec_self() -> ! {
    use std::os::unix::process::CommandExt;
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("supervisor reexec: current_exe: {e}");
            std::process::exit(1);
        }
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    tracing::info!("supervisor exec {exe:?} {args:?}");
    let err = std::process::Command::new(&exe).args(&args).exec();
    tracing::error!("supervisor exec failed: {err}");
    std::process::exit(1);
}

// ── Per-connection handler ───────────────────────────────────────────

async fn handle_client(
    stream: UnixStream,
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

async fn write_msg(writer: &mut tokio::io::WriteHalf<UnixStream>, msg: Message) -> Result<()> {
    let mut line = serde_json::to_string(&msg).context("serialize")?;
    line.push('\n');
    writer.write_all(line.as_bytes()).await.context("write")?;
    writer.flush().await.context("flush")?;
    Ok(())
}

// ── Supervisor state ─────────────────────────────────────────────────

struct SupervisorState {
    services: Mutex<HashMap<String, Entry>>,
}

struct Entry {
    spec: SpawnSpec,
    supervisor: JoinHandle<()>,
    stop_tx: mpsc::Sender<()>,
    /// Live PID of the currently running child (0 when none).
    pid: Arc<AtomicU32>,
    /// Canonical path of `spec.program` resolved at spawn time.
    /// Stored so `List` can return it for store-path drift detection
    /// even when the binary is a shebang wrapper script.
    resolved_program: Option<String>,
}

impl SupervisorState {
    fn new() -> Self {
        Self {
            services: Mutex::new(HashMap::new()),
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
        // Stop any existing entry whose spec doesn't match.
        let existing = {
            let map = self.services.lock().await;
            map.get(&name).map(|e| e.spec.clone())
        };
        if let Some(old) = existing {
            if old == spec {
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
            task_name, task_spec, notif_tx, stop_rx, task_pid,
        ));
        self.services.lock().await.insert(
            name,
            Entry {
                spec,
                supervisor: task,
                stop_tx,
                pid,
                resolved_program,
            },
        );
    }

    async fn unregister(&self, name: &str) {
        let existing = self.services.lock().await.remove(name);
        if let Some(entry) = existing {
            tracing::info!("supervisor: unregistering {name}");
            let _ = entry.stop_tx.send(()).await;
            let _ = entry.supervisor.await;
        }
    }

    async fn shutdown_all(&self) {
        let drained: Vec<(String, Entry)> = self.services.lock().await.drain().collect();
        for (name, entry) in drained {
            tracing::info!("supervisor: stopping {name}");
            let _ = entry.stop_tx.send(()).await;
            let _ = entry.supervisor.await;
        }
    }
}

// ── Per-service runner ──────────────────────────────────────────────

async fn run_service(
    name: String,
    spec: SpawnSpec,
    notif_tx: broadcast::Sender<Notification>,
    mut stop_rx: mpsc::Receiver<()>,
    pid: Arc<AtomicU32>,
) {
    loop {
        let child = match spawn_child(&name, &spec) {
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
        let exit = wait_child(&name, child, &notif_tx, &mut stop_rx).await;
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

fn spawn_child(name: &str, spec: &SpawnSpec) -> Result<Child> {
    let mut cmd = Command::new(&spec.program);
    cmd.args(&spec.args)
        .envs(&spec.env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let child = cmd
        .spawn()
        .with_context(|| format!("spawn {} for {name}", spec.program))?;
    tracing::info!(
        "supervisor: {name} spawned pid={:?} ({} {})",
        child.id(),
        spec.program,
        spec.args.join(" ")
    );
    Ok(child)
}

async fn wait_child(
    name: &str,
    mut child: Child,
    notif_tx: &broadcast::Sender<Notification>,
    stop_rx: &mut mpsc::Receiver<()>,
) -> ChildExit {
    if let Some(stdout) = child.stdout.take() {
        tokio::spawn(forward_lines(
            name.to_string(),
            stdout,
            false,
            notif_tx.clone(),
        ));
    }
    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(forward_lines(
            name.to_string(),
            stderr,
            true,
            notif_tx.clone(),
        ));
    }

    tokio::select! {
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
    }
}

async fn stop_child(child: &mut Child) {
    if let Some(pid) = child.id() {
        // SAFETY: libc::kill is always safe to call with an i32 pid/signum.
        unsafe {
            libc::kill(pid as i32, libc::SIGTERM);
        }
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

async fn forward_lines<R>(
    name: String,
    stream: R,
    is_stderr: bool,
    notif_tx: broadcast::Sender<Notification>,
) where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    let mut reader = BufReader::new(stream).lines();
    while let Ok(Some(line)) = reader.next_line().await {
        let clean = strip_ansi(&line);
        if is_stderr {
            tracing::warn!(target: "service", "[{name}] {clean}");
        } else {
            tracing::info!(target: "service", "[{name}] {clean}");
        }
        let _ = notif_tx.send(Notification::Log {
            name: name.clone(),
            line: clean,
            is_stderr,
        });
    }
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
