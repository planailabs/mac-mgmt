use std::collections::HashMap;
use std::sync::Arc;

#[cfg(feature = "services")]
use crate::managed_service::ShellTunnel;

// ── Virtual command handler ─────────────────────────────────────────────

/// Return type for virtual shell command handlers.
#[cfg(feature = "services")]
pub struct VirtualOutput {
    pub lines: Vec<(String, String)>, // (stream, data)
    pub exit_code: i32,
}

/// A function that handles a virtual shell command instead of spawning a process.
/// Takes the user arg and returns output synchronously (called from async context
/// via block_in_place).
#[cfg(feature = "services")]
pub type VirtualHandler = Arc<dyn Fn(Option<&str>) -> VirtualOutput + Send + Sync>;

// ── Registry ────────────────────────────────────────────────────────────

/// Tracks the current set of shell command definitions advertised by services.
#[cfg(feature = "services")]
pub struct ShellTunnelRegistry {
    tunnels: HashMap<String, ShellTunnel>,
    virtual_handlers: HashMap<String, VirtualHandler>,
}

/// Stub when services feature is disabled — the relay client still needs the type.
#[cfg(not(feature = "services"))]
pub struct ShellTunnelRegistry;

#[cfg(not(feature = "services"))]
impl ShellTunnelRegistry {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(feature = "services")]
impl ShellTunnelRegistry {
    pub fn new() -> Self {
        Self {
            tunnels: HashMap::new(),
            virtual_handlers: HashMap::new(),
        }
    }

    pub fn update(&mut self, defs: Vec<ShellTunnel>) {
        self.tunnels.clear();
        for d in defs {
            self.tunnels.insert(d.def.name.clone(), d);
        }
    }

    pub fn get(&self, name: &str) -> Option<&ShellTunnel> {
        self.tunnels.get(name)
    }

    /// Register a virtual handler for a command name. When this command is
    /// executed, the handler is called instead of spawning a process.
    pub fn register_virtual(&mut self, name: &str, handler: VirtualHandler) {
        self.virtual_handlers.insert(name.to_string(), handler);
    }

    /// Get the virtual handler for a command, if any.
    pub fn get_virtual(&self, name: &str) -> Option<&VirtualHandler> {
        self.virtual_handlers.get(name)
    }

    /// Serialize all shell tunnels as JSON for relay advertisement.
    pub fn to_json(&self) -> Vec<serde_json::Value> {
        self.tunnels
            .values()
            .map(|st| {
                serde_json::json!({
                    "name": st.def.name,
                    "service": st.service,
                    "description": st.def.description,
                    "requires_arg": st.def.arg_template.is_some(),
                    "arg_label": st.def.arg_template.as_ref().map(|t| &t.label),
                    "arg_placeholder": st.def.arg_template.as_ref().map(|t| &t.placeholder),
                })
            })
            .collect()
    }
}

// ── Shell command execution (data session) ─────────────────────────────

/// Default maximum execution time for a shell command (5 minutes).
#[cfg(feature = "services")]
pub(crate) const DEFAULT_EXEC_SECS: u64 = 300;

// ── Shared helpers ─────────────────────────────────────────────────────

/// Validate the user argument against the tunnel's arg_template regex.
#[cfg(feature = "services")]
pub(crate) fn validate_args(tunnel: &ShellTunnel, user_arg: Option<&str>) -> Result<(), String> {
    if let Some(ref tmpl) = tunnel.def.arg_template {
        if let Some(arg) = user_arg {
            if let Some(ref pattern) = tmpl.validation {
                match regex::Regex::new(pattern) {
                    Ok(re) if !re.is_match(arg) => {
                        return Err(format!(
                            "argument does not match required pattern: {pattern}"
                        ));
                    }
                    Err(e) => {
                        tracing::warn!("invalid validation regex for {}: {e}", tunnel.def.name);
                    }
                    _ => {}
                }
            }
        }
    } else if user_arg.is_some() {
        return Err("this command does not accept arguments".into());
    }
    Ok(())
}

/// Build a tokio Command from a tunnel definition + optional user arg.
/// Stdout/stderr are piped, stdin is null.
#[cfg(feature = "services")]
pub(crate) fn build_command(
    tunnel: &ShellTunnel,
    user_arg: Option<&str>,
) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(&tunnel.def.command);
    cmd.args(&tunnel.def.args);
    if let Some(arg) = user_arg {
        if !arg.is_empty() {
            cmd.arg(arg);
        }
    }
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.stdin(std::process::Stdio::null());
    cmd
}

// ── Stream-based execution ───────────────────────────────────────────

/// Handle a shell command execution via a data stream session.
///
/// Protocol (length-prefixed framing):
/// 1. Daemon validates the command and spawns it.
/// 2. Daemon streams JSON messages `{ "stream": "stdout"|"stderr", "data": "..." }`
///    as output lines arrive.
/// 3. On process exit: daemon sends `{ "exit_code": N }` and end-of-stream.
/// 4. On timeout (5 min): daemon kills the process and sends `{ "exit_code": -1, "error": "timeout" }`.
#[cfg(feature = "services")]
pub async fn handle_exec_session<S>(
    tunnel: &ShellTunnel,
    user_arg: Option<&str>,
    stream: &mut S,
    virtual_handler: Option<&VirtualHandler>,
) where
    S: futures_util::AsyncRead + futures_util::AsyncWrite + Unpin + Send,
{
    use mac_mgmt_common::framing as stream_framing;
    use tokio::io::AsyncBufReadExt;

    macro_rules! send_error {
        ($error:expr) => {{
            let msg = serde_json::json!({ "exit_code": -1, "error": $error });
            let _ = stream_framing::write_json(stream, &msg).await;
            let _ = stream_framing::write_end(stream).await;
            return;
        }};
    }

    // Validate user argument
    if let Err(e) = validate_args(tunnel, user_arg) {
        send_error!(e);
    }

    // Virtual handler — run callback instead of spawning a process
    if let Some(handler) = virtual_handler {
        let output = handler(user_arg);
        for (strm, data) in &output.lines {
            let msg = serde_json::json!({ "stream": strm, "data": data });
            if stream_framing::write_json(stream, &msg).await.is_err() {
                return;
            }
        }
        let msg = serde_json::json!({ "exit_code": output.exit_code });
        let _ = stream_framing::write_json(stream, &msg).await;
        let _ = stream_framing::write_end(stream).await;
        tracing::info!(
            "virtual shell exec completed: {} (exit={})",
            tunnel.def.name,
            output.exit_code
        );
        return;
    }

    // Build the command
    let mut cmd = build_command(tunnel, user_arg);

    tracing::info!(
        "shell exec: {} {} {}",
        tunnel.def.command,
        tunnel.def.args.join(" "),
        user_arg.unwrap_or("")
    );

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            send_error!(format!("failed to spawn command: {e}"));
        }
    };

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let mut stdout_reader = tokio::io::BufReader::new(stdout).lines();
    let mut stderr_reader = tokio::io::BufReader::new(stderr).lines();

    let exec_timeout = tunnel.def.timeout_secs.unwrap_or(DEFAULT_EXEC_SECS);
    let timeout = tokio::time::sleep(std::time::Duration::from_secs(exec_timeout));
    tokio::pin!(timeout);

    // Stream output until process exits or timeout
    loop {
        tokio::select! {
            line = stdout_reader.next_line() => {
                match line {
                    Ok(Some(data)) => {
                        let msg = serde_json::json!({ "stream": "stdout", "data": data });
                        if stream_framing::write_json(stream, &msg).await.is_err() {
                            let _ = child.kill().await;
                            return;
                        }
                    }
                    Ok(None) => {
                        while let Ok(Some(data)) = stderr_reader.next_line().await {
                            let msg = serde_json::json!({ "stream": "stderr", "data": data });
                            if stream_framing::write_json(stream, &msg).await.is_err() {
                                let _ = child.kill().await;
                                return;
                            }
                        }
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("shell exec stdout read error: {e}");
                        break;
                    }
                }
            }
            line = stderr_reader.next_line() => {
                match line {
                    Ok(Some(data)) => {
                        let msg = serde_json::json!({ "stream": "stderr", "data": data });
                        if stream_framing::write_json(stream, &msg).await.is_err() {
                            let _ = child.kill().await;
                            return;
                        }
                    }
                    Ok(None) => {
                        while let Ok(Some(data)) = stdout_reader.next_line().await {
                            let msg = serde_json::json!({ "stream": "stdout", "data": data });
                            if stream_framing::write_json(stream, &msg).await.is_err() {
                                let _ = child.kill().await;
                                return;
                            }
                        }
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("shell exec stderr read error: {e}");
                        break;
                    }
                }
            }
            _ = &mut timeout => {
                tracing::warn!("shell exec timeout for {} ({}s)", tunnel.def.name, exec_timeout);
                let _ = child.kill().await;
                let msg = serde_json::json!({ "exit_code": -1, "error": format!("command timed out after {exec_timeout}s") });
                let _ = stream_framing::write_json(stream, &msg).await;
                let _ = stream_framing::write_end(stream).await;
                return;
            }
        }
    }

    // Wait for exit code
    let exit_code = match child.wait().await {
        Ok(status) => status.code().unwrap_or(-1),
        Err(e) => {
            tracing::warn!("shell exec wait error: {e}");
            -1
        }
    };

    let msg = serde_json::json!({ "exit_code": exit_code });
    let _ = stream_framing::write_json(stream, &msg).await;
    let _ = stream_framing::write_end(stream).await;
    tracing::info!(
        "shell exec completed: {} (exit={})",
        tunnel.def.name,
        exit_code
    );
}
