use anyhow::{Context, Result};
use russh::keys::PublicKey;
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::RwLock;

use super::host_keys;
use super::ssh_server::{self, SshSession};
use super::ws_stream::WsStream;
use crate::ws_reconnect::{self, WsClientConfig};

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum ControlMessage {
    Registered { ssh_port: u16 },
    SessionRequest { session_id: String },
    MetricsRequest { request_id: String, path: String },
}

pub async fn run(
    relay_url: &str,
    token: &str,
    instance_id: &str,
    agent_name: Option<&str>,
    server_ssh_keys: Arc<RwLock<Vec<PublicKey>>>,
    ssh_allowed: Arc<AtomicBool>,
    metrics_port: u16,
) -> Result<()> {
    // Load SSH config once
    let host_key = host_keys::load_or_generate()?;
    let russh_config = Arc::new(russh::server::Config {
        keys: vec![host_key],
        ..Default::default()
    });

    let agent_name_param = agent_name
        .map(|n| format!("&agent_name={}", urlencoding::encode(n)))
        .unwrap_or_default();

    let hostname_param = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .map(|h| format!("&hostname={}", urlencoding::encode(&h)))
        .unwrap_or_default();

    let ws_url = format!(
        "{relay_url}/api/daemon/register?instance_id={instance_id}{agent_name_param}{hostname_param}"
    );
    tracing::info!("relay client connecting to {relay_url} as {instance_id}");

    let (incoming_tx, mut incoming_rx) = mpsc::channel(64);
    let (outgoing_tx, outgoing_rx) = mpsc::channel(64);

    let ws_config = WsClientConfig {
        url: ws_url,
        auth_token: token.to_string(),
        ..Default::default()
    };

    let _ws_handle = ws_reconnect::spawn_reconnecting(ws_config, incoming_tx, outgoing_rx);

    while let Some(text) = incoming_rx.recv().await {
        let control: ControlMessage = match serde_json::from_str(&text) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("invalid relay control message ({e}): {text}");
                continue;
            }
        };

        match control {
            ControlMessage::Registered { ssh_port } => {
                tracing::info!(
                    "relay registered: instance={instance_id} ssh_port={ssh_port}"
                );
            }
            ControlMessage::SessionRequest { session_id } => {
                if !ssh_allowed.load(Ordering::Relaxed) {
                    tracing::info!("session request {session_id} denied (remote SSH disabled)");
                    continue;
                }
                tracing::info!("session request {session_id} accepted, opening data WS");
                let relay_url = relay_url.to_string();
                let token = token.to_string();
                let config = Arc::clone(&russh_config);
                let ssh_keys = Arc::clone(&server_ssh_keys);
                tokio::spawn(async move {
                    match handle_session(&relay_url, &token, &session_id, config, ssh_keys).await {
                        Ok(()) => tracing::info!("session {session_id} completed"),
                        Err(e) => tracing::error!("session {session_id} failed: {e:#}"),
                    }
                });
            }
            ControlMessage::MetricsRequest { request_id, path } => {
                tracing::debug!("metrics request {request_id}: {path}");
                let out_tx = outgoing_tx.clone();
                let port = metrics_port;
                tokio::spawn(async move {
                    handle_metrics_request(&out_tx, &request_id, &path, port).await;
                });
            }
        }
    }

    tracing::warn!("relay control channel closed");
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
    let host = ws_reconnect::extract_host(relay_url)?;

    let request = tokio_tungstenite::tungstenite::http::Request::builder()
        .uri(ws_url.parse::<tokio_tungstenite::tungstenite::http::Uri>()?)
        .header("Authorization", format!("Bearer {token}"))
        .header(
            "Sec-WebSocket-Key",
            tokio_tungstenite::tungstenite::handshake::client::generate_key(),
        )
        .header("Sec-WebSocket-Version", "13")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Host", &host)
        .body(())?;

    let (ws, _) = tokio_tungstenite::connect_async(request)
        .await
        .context("session WS connect failed")?;
    tracing::debug!("session {session_id} data WS connected");

    let stream = WsStream::new(ws);
    let mut authorized_keys = server_ssh_keys.read().await.clone();
    let local_count = ssh_server::load_authorized_keys().len();
    authorized_keys.extend(ssh_server::load_authorized_keys());
    tracing::debug!(
        "session {session_id} authorized keys: {} server + {} local",
        authorized_keys.len() - local_count,
        local_count
    );
    let handler = SshSession::new(authorized_keys);

    let session = russh::server::run_stream(config, stream, handler)
        .await
        .context("russh session setup failed")?;

    session.await.context("russh session failed")?;

    Ok(())
}

async fn handle_metrics_request(
    out_tx: &mpsc::Sender<String>,
    request_id: &str,
    path: &str,
    metrics_port: u16,
) {
    let url = format!("http://[::1]:{metrics_port}{path}");

    let (status, content_type, body) = match reqwest::Client::new()
        .get(&url)
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let ct = resp
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("text/plain")
                .to_string();
            let body = resp.text().await.unwrap_or_default();
            (status, ct, body)
        }
        Err(e) => {
            tracing::warn!("metrics fetch from local port {metrics_port} failed: {e}");
            (
                502,
                "text/plain".to_string(),
                format!("metrics fetch failed: {e}"),
            )
        }
    };
    tracing::debug!("metrics response {request_id} status={status} ({} bytes)", body.len());

    let msg = serde_json::json!({
        "type": "metrics_response",
        "request_id": request_id,
        "status": status,
        "content_type": content_type,
        "body": body
    });

    let _ = out_tx.send(msg.to_string()).await;
}
