//! Web terminal SSH endpoint.
//!
//! Serves an xterm.js terminal at `/ssh/{instance_id}` and a WebSocket
//! endpoint at `/ssh/{instance_id}/ws` that bridges to the daemon's SSH
//! server. The relay authenticates to the daemon using its own ephemeral
//! SSH key; the user is authenticated by their TLS client certificate.

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::Mutex;

use crate::daemon_registry::DaemonRegistry;
use crate::mtls::ClientCertInfo;
use crate::p2p::RelaySwarm;
use crate::ssh_identity::RelaySshIdentity;

const OPEN_STREAM_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Shared state for the web SSH handler.
#[derive(Clone)]
pub struct WebSshState {
    pub registry: Arc<DaemonRegistry>,
    pub relay_swarm: Arc<RelaySwarm>,
    pub ssh_identity: Arc<RelaySshIdentity>,
    pub server_api_url: String,
}

pub fn router(state: WebSshState) -> Router {
    Router::new()
        .route("/ssh/{instance_id}", get(ssh_page))
        .route("/ssh/{instance_id}/ws", get(ssh_ws))
        .with_state(state)
}

/// Verify access via client certificate OR Bearer token.
async fn verify_access(
    headers: &axum::http::HeaderMap,
    cert_info: &Option<axum::Extension<ClientCertInfo>>,
    server_api_url: &str,
) -> Result<(), Response> {
    // Try client certificate first.
    if let Some(cert) = cert_info {
        if crate::auth::validate_cert(server_api_url, &cert.fingerprint_sha256)
            .await
            .is_ok()
        {
            return Ok(());
        }
    }

    // Fall back to Bearer token.
    if let Some(token) = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        match crate::auth::validate_token(server_api_url, token).await {
            Ok(info)
                if matches!(
                    info.token_kind.as_str(),
                    "admin" | "setting" | "cert_admin" | "cert_cluster"
                ) =>
            {
                return Ok(());
            }
            _ => {}
        }
    }

    // Neither worked — return appropriate error.
    if let Some(cert) = cert_info {
        return Err((
            axum::http::StatusCode::FORBIDDEN,
            axum::Json(serde_json::json!({
                "error": "certificate not authorized",
                "cert_fingerprint": cert.fingerprint_sha256,
                "hint": "Add this fingerprint to cluster or admin certificate settings"
            })),
        )
            .into_response());
    }

    Err((
        axum::http::StatusCode::UNAUTHORIZED,
        axum::Json(serde_json::json!({
            "error": "authentication required (client certificate or Bearer token)"
        })),
    )
        .into_response())
}

/// Serve the xterm.js terminal page.
async fn ssh_page(
    Path(instance_id): Path<String>,
    State(state): State<WebSshState>,
    headers: axum::http::HeaderMap,
    cert_info: Option<axum::Extension<ClientCertInfo>>,
) -> Response {
    if let Err(resp) = verify_access(&headers, &cert_info, &state.server_api_url).await {
        return resp;
    }
    Html(terminal_html(&instance_id)).into_response()
}

/// WebSocket handler: bridges to daemon SSH via the relay's SSH key.
async fn ssh_ws(
    ws: WebSocketUpgrade,
    Path(instance_id): Path<String>,
    State(state): State<WebSshState>,
    headers: axum::http::HeaderMap,
    cert_info: Option<axum::Extension<ClientCertInfo>>,
) -> Response {
    if let Err(resp) = verify_access(&headers, &cert_info, &state.server_api_url).await {
        return resp;
    }
    ws.on_upgrade(move |socket| handle_ssh_ws(socket, instance_id, state))
}

async fn handle_ssh_ws(socket: WebSocket, instance_id: String, state: WebSshState) {
    let (ws_tx, ws_rx) = socket.split();
    let ws_tx = Arc::new(Mutex::new(ws_tx));

    if let Err(e) = do_ssh_bridge(Arc::clone(&ws_tx), ws_rx, &instance_id, &state).await {
        tracing::warn!("SSH WS bridge for {instance_id} failed: {e}");
        // Send error to client so they see WHY the connection failed.
        let msg = serde_json::json!({ "type": "error", "message": format!("{e:#}") });
        let mut tx = ws_tx.lock().await;
        let _ = tx.send(Message::Text(msg.to_string().into())).await;
        let _ = tx.close().await;
    }
}

/// Send a status update to the client overlay.
async fn send_status(ws_tx: &Mutex<futures_util::stream::SplitSink<WebSocket, Message>>, msg: &str) {
    let json = serde_json::json!({ "type": "status", "message": msg });
    let mut tx = ws_tx.lock().await;
    let _ = tx.send(Message::Text(json.to_string().into())).await;
}

async fn do_ssh_bridge(
    ws_tx: Arc<Mutex<futures_util::stream::SplitSink<WebSocket, Message>>>,
    ws_rx: futures_util::stream::SplitStream<WebSocket>,
    instance_id: &str,
    state: &WebSshState,
) -> anyhow::Result<()> {
    use futures_util::AsyncWriteExt;
    use tokio_util::compat::FuturesAsyncReadCompatExt;

    // Resolve instance to a connected daemon.
    send_status(&ws_tx, "Resolving instance...").await;
    let full_id = state
        .registry
        .resolve_prefix(instance_id)
        .ok_or_else(|| anyhow::anyhow!("instance '{instance_id}' not found — daemon may be offline"))?;

    let peer_id = state
        .registry
        .get_peer_id(&full_id)
        .ok_or_else(|| anyhow::anyhow!("daemon '{full_id}' has no p2p connection to relay"))?;

    // Open libp2p tunnel with SSH handshake.
    send_status(&ws_tx, "Opening tunnel...").await;
    let mut tunnel = tokio::time::timeout(
        OPEN_STREAM_TIMEOUT,
        state.relay_swarm.open_tunnel_stream(peer_id),
    )
    .await
    .map_err(|_| anyhow::anyhow!("tunnel to '{full_id}' timed out — daemon may be unreachable"))??;

    let handshake = serde_json::json!({ "type": "ssh" });
    let data = serde_json::to_vec(&handshake)?;
    tunnel
        .write_all(&(data.len() as u32).to_be_bytes())
        .await?;
    tunnel.write_all(&data).await?;
    tunnel.flush().await?;

    // Run russh client over the tunnel, authenticating with the relay's SSH key.
    send_status(&ws_tx, "SSH handshake...").await;
    let compat_stream = tunnel.compat();

    let config = Arc::new(russh::client::Config {
        // No inactivity timeout — the WebSocket layer handles keepalives.
        inactivity_timeout: None,
        ..Default::default()
    });
    let client_handler = SshClientHandler;
    let mut session =
        russh::client::connect_stream(config, compat_stream, client_handler).await?;

    // Authenticate as "root" (the daemon doesn't care about the username,
    // only the public key).
    send_status(&ws_tx, "Authenticating...").await;
    let key_with_alg = russh::keys::PrivateKeyWithHashAlg::new(
        Arc::new(state.ssh_identity.private_key.clone()),
        None,
    );
    let auth_result = session
        .authenticate_publickey("root", key_with_alg)
        .await?;
    if !matches!(auth_result, russh::client::AuthResult::Success) {
        anyhow::bail!("SSH key auth rejected — relay key may not be authorized on daemon");
    }

    // Open a session channel.
    send_status(&ws_tx, "Starting shell...").await;
    let channel = session.channel_open_session().await?;

    // Request PTY + shell (want_reply=true so we know they succeeded).
    channel
        .request_pty(true, "xterm-256color", 80, 24, 0, 0, &[])
        .await?;
    channel.request_shell(true).await?;

    // Split the channel into read/write halves.
    let (channel_read, channel_write) = channel.split();

    // SSH channel data -> WebSocket (binary)
    let ws_tx_for_ssh = Arc::clone(&ws_tx);
    let instance_tag = full_id.clone();
    let mut channel_read = channel_read;
    let mut ssh_to_ws = tokio::spawn(async move {
        while let Some(msg) = channel_read.wait().await {
            match msg {
                russh::ChannelMsg::Data { data } => {
                    let mut tx = ws_tx_for_ssh.lock().await;
                    if tx.send(Message::Binary(data.to_vec().into())).await.is_err() {
                        tracing::debug!("[{instance_tag}] ssh→ws: WebSocket send failed");
                        break;
                    }
                }
                russh::ChannelMsg::ExtendedData { data, .. } => {
                    let mut tx = ws_tx_for_ssh.lock().await;
                    if tx.send(Message::Binary(data.to_vec().into())).await.is_err() {
                        tracing::debug!("[{instance_tag}] ssh→ws: WebSocket send failed");
                        break;
                    }
                }
                russh::ChannelMsg::Eof => {
                    tracing::debug!("[{instance_tag}] ssh→ws: channel EOF");
                    break;
                }
                russh::ChannelMsg::Close => {
                    tracing::debug!("[{instance_tag}] ssh→ws: channel closed");
                    break;
                }
                russh::ChannelMsg::ExitStatus { exit_status } => {
                    tracing::debug!("[{instance_tag}] ssh→ws: exit status {exit_status}");
                    break;
                }
                _ => {}
            }
        }
        tracing::debug!("[{instance_tag}] ssh→ws task finished");
    });

    // WebSocket -> SSH channel
    let instance_tag2 = full_id.clone();
    let mut ws_rx = ws_rx;
    let mut ws_to_ssh = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Binary(data) => {
                    if channel_write.data(&data[..]).await.is_err() {
                        tracing::debug!("[{instance_tag2}] ws→ssh: channel write failed");
                        break;
                    }
                }
                Message::Text(text) => {
                    // JSON control messages (e.g., terminal resize).
                    if let Ok(ctrl) = serde_json::from_str::<serde_json::Value>(&text) {
                        if ctrl["type"].as_str() == Some("resize") {
                            let cols = ctrl["cols"].as_u64().unwrap_or(80) as u32;
                            let rows = ctrl["rows"].as_u64().unwrap_or(24) as u32;
                            if let Err(e) = channel_write
                                .window_change(cols, rows, 0, 0)
                                .await
                            {
                                tracing::warn!("[{instance_tag2}] ws→ssh: window_change failed: {e}");
                            }
                        }
                    } else {
                        // Treat plain text as terminal input.
                        if channel_write
                            .data(text.as_bytes())
                            .await
                            .is_err()
                        {
                            tracing::debug!("[{instance_tag2}] ws→ssh: channel write failed");
                            break;
                        }
                    }
                }
                Message::Close(_) => {
                    tracing::debug!("[{instance_tag2}] ws→ssh: WebSocket close frame");
                    break;
                }
                _ => {}
            }
        }
        tracing::debug!("[{instance_tag2}] ws→ssh task finished");
    });

    // Wait for either direction to finish, then abort the other.
    tokio::select! {
        _ = &mut ssh_to_ws => {
            tracing::debug!("[{full_id}] ssh→ws finished first, aborting ws→ssh");
            ws_to_ssh.abort();
        }
        _ = &mut ws_to_ssh => {
            tracing::debug!("[{full_id}] ws→ssh finished first, aborting ssh→ws");
            ssh_to_ws.abort();
        }
    }

    let _ = session.disconnect(russh::Disconnect::ByApplication, "", "en").await;
    tracing::info!("SSH WS bridge for {instance_id} closed");
    Ok(())
}

/// Minimal russh client handler — accepts all host keys since the libp2p
/// tunnel is already authenticated.
struct SshClientHandler;

impl russh::client::Handler for SshClientHandler {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true) // Trust all — tunnel is already authenticated via libp2p.
    }
}

/// Generate the HTML page for the xterm.js terminal.
fn terminal_html(instance_id: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8"/>
<title>SSH — {instance_id}</title>
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@xterm/xterm@5/css/xterm.min.css"/>
<style>
  body {{ margin: 0; background: #1e1e1e; overflow: hidden; }}
  #terminal {{ width: 100vw; height: 100vh; }}
  #overlay {{
    display: none; position: fixed; inset: 0;
    background: rgba(0,0,0,0.7); color: #fff;
    font-family: monospace; font-size: 14px;
    justify-content: center; align-items: center;
    z-index: 10;
  }}
  #overlay.active {{ display: flex; }}
</style>
</head>
<body>
<div id="terminal"></div>
<div id="overlay"><span id="overlay-msg">Connecting...</span></div>
<script src="https://cdn.jsdelivr.net/npm/@xterm/xterm@5/lib/xterm.min.js"></script>
<script src="https://cdn.jsdelivr.net/npm/@xterm/addon-fit@0/lib/addon-fit.min.js"></script>
<script>
const term = new window.Terminal({{ cursorBlink: true, fontSize: 14 }});
const fit = new window.FitAddon.FitAddon();
term.loadAddon(fit);
term.open(document.getElementById('terminal'));
fit.fit();

const overlay = document.getElementById('overlay');
const overlayMsg = document.getElementById('overlay-msg');
function showOverlay(msg) {{ overlay.className = 'active'; overlayMsg.textContent = msg; }}
function hideOverlay() {{ overlay.className = ''; }}

showOverlay('Connecting...');

const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
const wsUrl = proto + '//' + location.host + '/ssh/{instance_id}/ws';

let currentWs = null;

// Register input/resize handlers once (not per connection).
term.onData((data) => {{
  if (currentWs && currentWs.readyState === WebSocket.OPEN) {{
    currentWs.send(new TextEncoder().encode(data));
  }}
}});

window.addEventListener('resize', () => {{
  fit.fit();
  if (currentWs && currentWs.readyState === WebSocket.OPEN) {{
    currentWs.send(JSON.stringify({{ type: 'resize', cols: term.cols, rows: term.rows }}));
  }}
}});

function connect() {{
  const ws = new WebSocket(wsUrl);
  ws.binaryType = 'arraybuffer';
  currentWs = ws;

  ws.onopen = () => {{
    hideOverlay();
    ws.send(JSON.stringify({{ type: 'resize', cols: term.cols, rows: term.rows }}));
  }};

  let gotError = false;

  ws.onmessage = (ev) => {{
    if (ev.data instanceof ArrayBuffer) {{
      hideOverlay();
      term.write(new Uint8Array(ev.data));
    }} else {{
      // Text frame — check for JSON control messages from relay.
      try {{
        const ctrl = JSON.parse(ev.data);
        if (ctrl.type === 'error') {{
          gotError = true;
          showOverlay('Error: ' + ctrl.message);
          return;
        }}
        if (ctrl.type === 'status') {{
          showOverlay(ctrl.message);
          return;
        }}
      }} catch (e) {{}}
      // Plain text terminal data.
      hideOverlay();
      term.write(ev.data);
    }}
  }};

  ws.onclose = () => {{
    if (currentWs === ws) currentWs = null;
    if (gotError) {{
      // Error already shown in overlay — retry with longer delay.
      setTimeout(connect, 10000);
    }} else {{
      showOverlay('Disconnected. Reconnecting in 3s...');
      setTimeout(connect, 3000);
    }}
  }};

  ws.onerror = () => ws.close();
}}

connect();
</script>
</body>
</html>"#,
        instance_id = instance_id
    )
}
