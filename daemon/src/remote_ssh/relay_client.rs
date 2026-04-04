use anyhow::{Context, Result};
use futures_util::StreamExt;
use russh::keys::PublicKey;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::http::Uri;
use tokio_tungstenite::tungstenite::Message;

use super::host_keys;
use super::ssh_server::{self, SshSession};
use super::ws_stream::WsStream;

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum ControlMessage {
    Registered { ssh_port: u16 },
    SessionRequest { session_id: String },
}

pub async fn run(
    relay_url: &str,
    token: &str,
    instance_id: &str,
    agent_name: Option<&str>,
    server_ssh_keys: Arc<RwLock<Vec<PublicKey>>>,
) -> Result<()> {
    let mut backoff = Duration::from_secs(1);
    let max_backoff = Duration::from_secs(60);

    loop {
        tracing::info!("connecting to relay at {relay_url}");
        match connect_and_serve(relay_url, token, instance_id, agent_name, Arc::clone(&server_ssh_keys)).await {
            Ok(()) => {
                tracing::info!("relay connection closed normally");
                return Ok(());
            }
            Err(e) => {
                tracing::warn!("relay connection failed: {e:#}, reconnecting in {backoff:?}");
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(max_backoff);
            }
        }
    }
}

async fn connect_and_serve(
    relay_url: &str,
    token: &str,
    instance_id: &str,
    agent_name: Option<&str>,
    server_ssh_keys: Arc<RwLock<Vec<PublicKey>>>,
) -> Result<()> {
    let agent_name_param = agent_name
        .map(|n| format!("&agent_name={}", urlencoding::encode(n)))
        .unwrap_or_default();

    let ws_url = format!(
        "{relay_url}/api/daemon/register?instance_id={instance_id}{agent_name_param}"
    );

    let host = extract_host(relay_url)?;

    let request = tokio_tungstenite::tungstenite::http::Request::builder()
        .uri(ws_url.parse::<Uri>()?)
        .header("Authorization", format!("Bearer {token}"))
        .header("Sec-WebSocket-Key", tokio_tungstenite::tungstenite::handshake::client::generate_key())
        .header("Sec-WebSocket-Version", "13")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Host", &host)
        .body(())?;

    let (mut ws, _) = tokio_tungstenite::connect_async(request)
        .await
        .context("WebSocket connect failed")?;

    tracing::info!("connected to relay, entering control loop");

    // Load SSH config
    let host_key = host_keys::load_or_generate()?;
    let russh_config = Arc::new(russh::server::Config {
        keys: vec![host_key],
        ..Default::default()
    });

    while let Some(msg) = ws.next().await {
        let msg = msg.context("WS read error")?;
        match msg {
            Message::Text(text) => {
                let control: ControlMessage = serde_json::from_str(&text)
                    .with_context(|| format!("invalid control message: {text}"))?;

                match control {
                    ControlMessage::Registered { ssh_port } => {
                        tracing::info!("registered with relay, SSH port: {ssh_port}");
                    }
                    ControlMessage::SessionRequest { session_id } => {
                        tracing::info!("session request: {session_id}");
                        let relay_url = relay_url.to_string();
                        let token = token.to_string();
                        let config = Arc::clone(&russh_config);
                        let ssh_keys = Arc::clone(&server_ssh_keys);
                        tokio::spawn(async move {
                            if let Err(e) = handle_session(
                                &relay_url,
                                &token,
                                &session_id,
                                config,
                                ssh_keys,
                            )
                            .await
                            {
                                tracing::error!("session {session_id} failed: {e:#}");
                            }
                        });
                    }
                }
            }
            Message::Close(_) => {
                tracing::info!("relay sent close frame");
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

async fn handle_session(
    relay_url: &str,
    token: &str,
    session_id: &str,
    config: Arc<russh::server::Config>,
    server_ssh_keys: Arc<RwLock<Vec<PublicKey>>>,
) -> Result<()> {
    let ws_url = format!("{relay_url}/api/daemon/session/{session_id}");
    let host = extract_host(relay_url)?;

    let request = tokio_tungstenite::tungstenite::http::Request::builder()
        .uri(ws_url.parse::<Uri>()?)
        .header("Authorization", format!("Bearer {token}"))
        .header("Sec-WebSocket-Key", tokio_tungstenite::tungstenite::handshake::client::generate_key())
        .header("Sec-WebSocket-Version", "13")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Host", &host)
        .body(())?;

    let (ws, _) = tokio_tungstenite::connect_async(request)
        .await
        .context("session WS connect failed")?;

    let stream = WsStream::new(ws);
    let mut authorized_keys = server_ssh_keys.read().await.clone();
    authorized_keys.extend(ssh_server::load_authorized_keys());
    let handler = SshSession::new(authorized_keys);

    let session = russh::server::run_stream(config, stream, handler)
        .await
        .context("russh session setup failed")?;

    session.await.context("russh session failed")?;

    Ok(())
}

fn extract_host(url: &str) -> Result<String> {
    let url = url
        .strip_prefix("wss://")
        .or_else(|| url.strip_prefix("ws://"))
        .or_else(|| url.strip_prefix("https://"))
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let host = url.split('/').next().unwrap_or(url);
    Ok(host.to_string())
}
