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

/// Verify client certificate: must be present and authorized.
async fn verify_cert(
    cert_info: &Option<axum::Extension<ClientCertInfo>>,
    server_api_url: &str,
) -> Result<(), Response> {
    let Some(cert) = cert_info else {
        return Err((
            axum::http::StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({
                "error": "client certificate required"
            })),
        )
            .into_response());
    };

    if let Err(_status) = crate::auth::validate_cert(server_api_url, &cert.fingerprint_sha256).await
    {
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

    Ok(())
}

/// Serve the xterm.js terminal page.
async fn ssh_page(
    Path(instance_id): Path<String>,
    State(state): State<WebSshState>,
    cert_info: Option<axum::Extension<ClientCertInfo>>,
) -> Response {
    if let Err(resp) = verify_cert(&cert_info, &state.server_api_url).await {
        return resp;
    }
    Html(terminal_html(&instance_id)).into_response()
}

/// WebSocket handler: bridges to daemon SSH via the relay's SSH key.
async fn ssh_ws(
    ws: WebSocketUpgrade,
    Path(instance_id): Path<String>,
    State(state): State<WebSshState>,
    cert_info: Option<axum::Extension<ClientCertInfo>>,
) -> Response {
    if let Err(resp) = verify_cert(&cert_info, &state.server_api_url).await {
        return resp;
    }
    ws.on_upgrade(move |socket| handle_ssh_ws(socket, instance_id, state))
}

async fn handle_ssh_ws(socket: WebSocket, instance_id: String, state: WebSshState) {
    if let Err(e) = do_ssh_bridge(socket, &instance_id, &state).await {
        tracing::warn!("SSH WS bridge for {instance_id} failed: {e}");
    }
}

async fn do_ssh_bridge(
    socket: WebSocket,
    instance_id: &str,
    state: &WebSshState,
) -> anyhow::Result<()> {
    use futures_util::AsyncWriteExt;
    use tokio_util::compat::FuturesAsyncReadCompatExt;

    // Resolve instance to a connected daemon.
    let full_id = state
        .registry
        .resolve_prefix(instance_id)
        .ok_or_else(|| anyhow::anyhow!("instance {instance_id} not found"))?;

    let peer_id = state
        .registry
        .get_peer_id(&full_id)
        .ok_or_else(|| anyhow::anyhow!("no peer_id for {full_id}"))?;

    // Open libp2p tunnel with SSH handshake.
    let mut tunnel = tokio::time::timeout(
        OPEN_STREAM_TIMEOUT,
        state.relay_swarm.open_tunnel_stream(peer_id),
    )
    .await
    .map_err(|_| anyhow::anyhow!("timeout opening tunnel to {full_id}"))??;

    let handshake = serde_json::json!({ "type": "ssh" });
    let data = serde_json::to_vec(&handshake)?;
    tunnel
        .write_all(&(data.len() as u32).to_be_bytes())
        .await?;
    tunnel.write_all(&data).await?;
    tunnel.flush().await?;

    // Run russh client over the tunnel, authenticating with the relay's SSH key.
    let compat_stream = tunnel.compat();

    let config = Arc::new(russh::client::Config::default());
    let client_handler = SshClientHandler;
    let mut session =
        russh::client::connect_stream(config, compat_stream, client_handler).await?;

    // Authenticate as "root" (the daemon doesn't care about the username,
    // only the public key).
    let key_with_alg = russh::keys::PrivateKeyWithHashAlg::new(
        Arc::new(state.ssh_identity.private_key.clone()),
        None,
    );
    let auth_result = session
        .authenticate_publickey("root", key_with_alg)
        .await?;
    if !matches!(auth_result, russh::client::AuthResult::Success) {
        anyhow::bail!("SSH public key auth rejected by daemon");
    }

    // Open a session channel.
    let channel = session.channel_open_session().await?;

    // Request PTY + shell.
    channel
        .request_pty(false, "xterm-256color", 80, 24, 0, 0, &[])
        .await?;
    channel.request_shell(false).await?;

    // Bridge WebSocket <-> SSH channel.
    let (ws_tx, ws_rx) = socket.split();
    let ws_tx = Arc::new(Mutex::new(ws_tx));

    // Split the channel into read/write halves.
    let (channel_read, channel_write) = channel.split();

    // SSH channel data -> WebSocket (binary)
    let ws_tx_for_ssh = Arc::clone(&ws_tx);
    let mut channel_read = channel_read;
    let ssh_to_ws = tokio::spawn(async move {
        while let Some(msg) = channel_read.wait().await {
            match msg {
                russh::ChannelMsg::Data { data } => {
                    let mut tx = ws_tx_for_ssh.lock().await;
                    if tx.send(Message::Binary(data.to_vec().into())).await.is_err() {
                        break;
                    }
                }
                russh::ChannelMsg::ExtendedData { data, .. } => {
                    let mut tx = ws_tx_for_ssh.lock().await;
                    if tx.send(Message::Binary(data.to_vec().into())).await.is_err() {
                        break;
                    }
                }
                russh::ChannelMsg::Eof | russh::ChannelMsg::Close => break,
                russh::ChannelMsg::ExitStatus { .. } => break,
                _ => {}
            }
        }
    });

    // WebSocket -> SSH channel
    let mut ws_rx = ws_rx;
    let ws_to_ssh = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Binary(data) => {
                    if channel_write.data(&data[..]).await.is_err() {
                        break;
                    }
                }
                Message::Text(text) => {
                    // JSON control messages (e.g., terminal resize).
                    if let Ok(ctrl) = serde_json::from_str::<serde_json::Value>(&text) {
                        if ctrl["type"].as_str() == Some("resize") {
                            let cols = ctrl["cols"].as_u64().unwrap_or(80) as u32;
                            let rows = ctrl["rows"].as_u64().unwrap_or(24) as u32;
                            let _ = channel_write
                                .window_change(cols, rows, 0, 0)
                                .await;
                        }
                    } else {
                        // Treat plain text as terminal input.
                        if channel_write
                            .data(text.as_bytes())
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either direction to finish.
    tokio::select! {
        _ = ssh_to_ws => {}
        _ = ws_to_ssh => {}
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

function connect() {{
  const ws = new WebSocket(wsUrl);
  ws.binaryType = 'arraybuffer';

  ws.onopen = () => {{
    hideOverlay();
    // Send initial size.
    ws.send(JSON.stringify({{ type: 'resize', cols: term.cols, rows: term.rows }}));
  }};

  ws.onmessage = (ev) => {{
    if (ev.data instanceof ArrayBuffer) {{
      term.write(new Uint8Array(ev.data));
    }} else {{
      term.write(ev.data);
    }}
  }};

  ws.onclose = () => {{
    showOverlay('Disconnected. Reconnecting in 3s...');
    setTimeout(connect, 3000);
  }};

  ws.onerror = () => ws.close();

  term.onData((data) => {{
    if (ws.readyState === WebSocket.OPEN) {{
      // Send terminal input as binary.
      ws.send(new TextEncoder().encode(data));
    }}
  }});

  window.addEventListener('resize', () => {{
    fit.fit();
    if (ws.readyState === WebSocket.OPEN) {{
      ws.send(JSON.stringify({{ type: 'resize', cols: term.cols, rows: term.rows }}));
    }}
  }});
}}

connect();
</script>
</body>
</html>"#,
        instance_id = instance_id
    )
}
