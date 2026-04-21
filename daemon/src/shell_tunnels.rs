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
pub type VirtualHandler =
    Arc<dyn Fn(Option<&str>) -> VirtualOutput + Send + Sync>;

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
}

// ── Shell command execution (data session) ─────────────────────────────

/// Default maximum execution time for a shell command (5 minutes).
#[cfg(feature = "services")]
const DEFAULT_EXEC_SECS: u64 = 300;

/// Handle a shell command execution via a dedicated data WebSocket session.
///
/// Protocol:
/// 1. Relay opens data WS to daemon.
/// 2. Daemon validates the command and spawns it.
/// 3. Daemon streams text messages `{ "stream": "stdout"|"stderr", "data": "..." }`
///    as output lines arrive.
/// 4. On process exit: daemon sends `{ "exit_code": N }` and closes WS.
/// 5. On timeout (5 min): daemon kills the process and sends `{ "exit_code": -1, "error": "timeout" }`.
#[cfg(feature = "services")]
pub async fn handle_exec_session(
    tunnel: &ShellTunnel,
    user_arg: Option<&str>,
    ws: mac_mgmt_ws::ClientWs,
    virtual_handler: Option<&VirtualHandler>,
) {
    use futures_util::{SinkExt, StreamExt};
    use mac_mgmt_ws::tungstenite;
    use tokio::io::AsyncBufReadExt;

    let (mut sink, _stream) = ws.split();

    macro_rules! send_error {
        ($error:expr) => {{
            let msg = serde_json::json!({ "exit_code": -1, "error": $error });
            let _ = sink.send(tungstenite::Message::Text(msg.to_string().into())).await;
            let _ = sink.send(tungstenite::Message::Close(None)).await;
            return;
        }};
    }

    // Validate user argument against regex if required
    if let Some(ref tmpl) = tunnel.def.arg_template {
        if let Some(arg) = user_arg {
            if let Some(ref pattern) = tmpl.validation {
                match regex::Regex::new(pattern) {
                    Ok(re) => {
                        if !re.is_match(arg) {
                            send_error!(format!(
                                "argument does not match required pattern: {pattern}"
                            ));
                        }
                    }
                    Err(e) => {
                        tracing::warn!("invalid validation regex for {}: {e}", tunnel.def.name);
                    }
                }
            }
        }
    } else if user_arg.is_some() {
        send_error!("this command does not accept arguments");
    }

    // Virtual handler — run callback instead of spawning a process
    if let Some(handler) = virtual_handler {
        let output = handler(user_arg);
        for (stream, data) in &output.lines {
            let msg = serde_json::json!({ "stream": stream, "data": data });
            if sink
                .send(tungstenite::Message::Text(msg.to_string().into()))
                .await
                .is_err()
            {
                return;
            }
        }
        let msg = serde_json::json!({ "exit_code": output.exit_code });
        let _ = sink
            .send(tungstenite::Message::Text(msg.to_string().into()))
            .await;
        let _ = sink.send(tungstenite::Message::Close(None)).await;
        tracing::info!(
            "virtual shell exec completed: {} (exit={})",
            tunnel.def.name,
            output.exit_code
        );
        return;
    }

    // Build the command
    let mut cmd = tokio::process::Command::new(&tunnel.def.command);
    cmd.args(&tunnel.def.args);
    if let Some(arg) = user_arg {
        if !arg.is_empty() {
            cmd.arg(arg);
        }
    }

    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    // Don't inherit stdin
    cmd.stdin(std::process::Stdio::null());

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
                        if sink.send(tungstenite::Message::Text(msg.to_string().into())).await.is_err() {
                            let _ = child.kill().await;
                            return;
                        }
                    }
                    Ok(None) => {
                        // stdout closed — drain stderr then wait for exit
                        while let Ok(Some(data)) = stderr_reader.next_line().await {
                            let msg = serde_json::json!({ "stream": "stderr", "data": data });
                            if sink.send(tungstenite::Message::Text(msg.to_string().into())).await.is_err() {
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
                        if sink.send(tungstenite::Message::Text(msg.to_string().into())).await.is_err() {
                            let _ = child.kill().await;
                            return;
                        }
                    }
                    Ok(None) => {
                        // stderr closed — drain stdout then wait for exit
                        while let Ok(Some(data)) = stdout_reader.next_line().await {
                            let msg = serde_json::json!({ "stream": "stdout", "data": data });
                            if sink.send(tungstenite::Message::Text(msg.to_string().into())).await.is_err() {
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
                let _ = sink.send(tungstenite::Message::Text(msg.to_string().into())).await;
                let _ = sink.send(tungstenite::Message::Close(None)).await;
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
    let _ = sink
        .send(tungstenite::Message::Text(msg.to_string().into()))
        .await;
    let _ = sink.send(tungstenite::Message::Close(None)).await;
    tracing::info!(
        "shell exec completed: {} (exit={})",
        tunnel.def.name,
        exit_code
    );
}

// ── Direct (non-WebSocket) execution ──────────────────────────────────

/// Output from `exec_direct`, matching the structure the healer expects.
#[cfg(feature = "services")]
pub struct DirectExecOutput {
    pub lines: Vec<(String, String)>, // (stream, data)
    pub exit_code: Option<i32>,
    pub error: Option<String>,
}

/// Execute a shell command directly (no WebSocket). Returns structured output.
#[cfg(feature = "services")]
pub async fn exec_direct(
    tunnel: &ShellTunnel,
    user_arg: Option<&str>,
    virtual_handler: Option<&VirtualHandler>,
) -> Result<DirectExecOutput, String> {
    use tokio::io::AsyncBufReadExt;

    // Validate user argument against regex if required
    if let Some(ref tmpl) = tunnel.def.arg_template {
        if let Some(arg) = user_arg {
            if let Some(ref pattern) = tmpl.validation {
                match regex::Regex::new(pattern) {
                    Ok(re) => {
                        if !re.is_match(arg) {
                            return Err(format!(
                                "argument does not match required pattern: {pattern}"
                            ));
                        }
                    }
                    Err(e) => {
                        tracing::warn!("invalid validation regex for {}: {e}", tunnel.def.name);
                    }
                }
            }
        }
    } else if user_arg.is_some() {
        return Err("this command does not accept arguments".to_string());
    }

    // Virtual handler — run callback instead of spawning a process
    if let Some(handler) = virtual_handler {
        let output = handler(user_arg);
        return Ok(DirectExecOutput {
            lines: output.lines,
            exit_code: Some(output.exit_code),
            error: None,
        });
    }

    // Build the command
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

    tracing::info!(
        "shell exec (direct): {} {} {}",
        tunnel.def.command,
        tunnel.def.args.join(" "),
        user_arg.unwrap_or("")
    );

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return Err(format!("failed to spawn command: {e}"));
        }
    };

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let mut stdout_reader = tokio::io::BufReader::new(stdout).lines();
    let mut stderr_reader = tokio::io::BufReader::new(stderr).lines();

    let exec_timeout = tunnel.def.timeout_secs.unwrap_or(DEFAULT_EXEC_SECS);
    let timeout = tokio::time::sleep(std::time::Duration::from_secs(exec_timeout));
    tokio::pin!(timeout);

    let mut lines = Vec::new();

    // Stream output until process exits or timeout
    loop {
        tokio::select! {
            line = stdout_reader.next_line() => {
                match line {
                    Ok(Some(data)) => {
                        lines.push(("stdout".to_string(), data));
                    }
                    Ok(None) => {
                        // stdout closed — drain stderr
                        while let Ok(Some(data)) = stderr_reader.next_line().await {
                            lines.push(("stderr".to_string(), data));
                        }
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("shell exec (direct) stdout read error: {e}");
                        break;
                    }
                }
            }
            line = stderr_reader.next_line() => {
                match line {
                    Ok(Some(data)) => {
                        lines.push(("stderr".to_string(), data));
                    }
                    Ok(None) => {
                        // stderr closed — drain stdout
                        while let Ok(Some(data)) = stdout_reader.next_line().await {
                            lines.push(("stdout".to_string(), data));
                        }
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("shell exec (direct) stderr read error: {e}");
                        break;
                    }
                }
            }
            _ = &mut timeout => {
                tracing::warn!("shell exec (direct) timeout for {} ({}s)", tunnel.def.name, exec_timeout);
                let _ = child.kill().await;
                return Ok(DirectExecOutput {
                    lines,
                    exit_code: Some(-1),
                    error: Some(format!("command timed out after {exec_timeout}s")),
                });
            }
        }
    }

    // Wait for exit code
    let exit_code = match child.wait().await {
        Ok(status) => status.code().unwrap_or(-1),
        Err(e) => {
            tracing::warn!("shell exec (direct) wait error: {e}");
            -1
        }
    };

    tracing::info!(
        "shell exec (direct) completed: {} (exit={})",
        tunnel.def.name,
        exit_code
    );

    Ok(DirectExecOutput {
        lines,
        exit_code: Some(exit_code),
        error: None,
    })
}
