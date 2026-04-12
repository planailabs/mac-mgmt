use anyhow::{Context, Result};
use russh::keys::PublicKey;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::RwLock;

use super::ssh_server::{self, SshSession};
use super::ws_stream::WsStream;
use crate::ws_reconnect::{self, WsClientConfig};

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum ControlMessage {
    Registered {
        ssh_port: u16,
        #[serde(default)]
        proxy_hostname: Option<String>,
    },
    SessionRequest {
        session_id: String,
        session_secret: String,
    },
    MetricsRequest { request_id: String, path: String },
    ProxyRequest {
        request_id: String,
        tunnel_name: String,
        method: String,
        path: String,
        headers: Vec<(String, String)>,
        body: Option<String>,
    },
}

/// A tunnel definition used to map tunnel names to local host:port.
#[derive(Debug, Clone)]
pub struct TunnelTarget {
    pub host: String,
    pub port: u16,
}

pub async fn run(
    relay_url: &str,
    token: &str,
    instance_id: &str,
    agent_name: Option<&str>,
    host_key: russh::keys::PrivateKey,
    server_ssh_keys: Arc<RwLock<Vec<PublicKey>>>,
    ssh_allowed: Arc<AtomicBool>,
    metrics_port: u16,
    tunnel_defs: Arc<RwLock<HashMap<String, TunnelTarget>>>,
    relay_proxy_hostname: Arc<RwLock<Option<String>>>,
    ws_outgoing_tx: Arc<RwLock<Option<mpsc::Sender<String>>>>,
) -> Result<()> {
    let russh_config = Arc::new(russh::server::Config {
        keys: vec![host_key],
        // Only advertise public-key auth; password / keyboard-interactive
        // are explicitly disabled at the protocol level.
        methods: russh::MethodSet::from(&[russh::MethodKind::PublicKey][..]),
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

    // Share the outgoing channel so the Manager can send tunnel advertisements.
    *ws_outgoing_tx.write().await = Some(outgoing_tx.clone());

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
            ControlMessage::Registered { ssh_port, proxy_hostname } => {
                tracing::info!(
                    "relay registered: instance={instance_id} ssh_port={ssh_port} proxy_hostname={proxy_hostname:?}"
                );
                // Store the relay's proxy hostname for heartbeats.
                if let Some(ph) = proxy_hostname {
                    *relay_proxy_hostname.write().await = Some(ph);
                }
                // Advertise our tunnels to the relay.
                let tunnels: Vec<serde_json::Value> = {
                    let defs = tunnel_defs.read().await;
                    defs.iter()
                        .map(|(name, t)| serde_json::json!({ "name": name, "tcp_port": t.port }))
                        .collect()
                };
                let advert = serde_json::json!({
                    "type": "tunnel_advertisement",
                    "tunnels": tunnels,
                });
                let _ = outgoing_tx.send(advert.to_string()).await;
            }
            ControlMessage::SessionRequest { session_id, session_secret } => {
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
                    match handle_session(&relay_url, &token, &session_id, &session_secret, config, ssh_keys).await {
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
            ControlMessage::ProxyRequest { request_id, tunnel_name, method, path, headers, body } => {
                tracing::debug!("proxy request {request_id}: {tunnel_name}{path}");
                let target = {
                    let defs = tunnel_defs.read().await;
                    defs.get(&tunnel_name).cloned()
                };
                let out_tx = outgoing_tx.clone();
                tokio::spawn(async move {
                    handle_proxy_request(&out_tx, &request_id, target, &method, &path, headers, body).await;
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
    session_secret: &str,
    config: Arc<russh::server::Config>,
    server_ssh_keys: Arc<RwLock<Vec<PublicKey>>>,
) -> Result<()> {
    let ws_url = format!(
        "{relay_url}/api/daemon/session/{session_id}?session_secret={}",
        urlencoding::encode(session_secret),
    );
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

async fn handle_proxy_request(
    out_tx: &mpsc::Sender<String>,
    request_id: &str,
    target: Option<TunnelTarget>,
    method: &str,
    path: &str,
    headers: Vec<(String, String)>,
    body: Option<String>,
) {
    let Some(target) = target else {
        let msg = serde_json::json!({
            "type": "proxy_response",
            "request_id": request_id,
            "status": 404,
            "headers": [],
            "body": null,
        });
        let _ = out_tx.send(msg.to_string()).await;
        return;
    };

    let url = format!("http://{}:{}{path}", target.host, target.port);
    let client = reqwest::Client::new();

    let mut req = match method {
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        "PATCH" => client.patch(&url),
        "HEAD" => client.head(&url),
        _ => client.get(&url),
    };

    for (k, v) in &headers {
        // Skip hop-by-hop headers
        let lk = k.to_lowercase();
        if lk == "host" || lk == "connection" || lk == "transfer-encoding" {
            continue;
        }
        req = req.header(k.as_str(), v.as_str());
    }

    if let Some(b64) = body {
        use base64::Engine;
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&b64) {
            req = req.body(bytes);
        }
    }

    let (status, resp_headers, resp_body) = match req
        .timeout(Duration::from_secs(60))
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let hdrs: Vec<(String, String)> = resp
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();
            let bytes = resp.bytes().await.unwrap_or_default();
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            (status, hdrs, b64)
        }
        Err(e) => {
            tracing::warn!("proxy request to {url} failed: {e}");
            (502, vec![], String::new())
        }
    };

    tracing::debug!("proxy response {request_id} status={status}");

    let msg = serde_json::json!({
        "type": "proxy_response",
        "request_id": request_id,
        "status": status,
        "headers": resp_headers,
        "body": resp_body,
    });

    let _ = out_tx.send(msg.to_string()).await;
}
