use anyhow::{Context, Result};
use mac_mgmt_ws::tungstenite;
use russh::keys::PublicKey;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::RwLock;

use super::ssh_server::{self, SshSession};
use crate::file_tunnels::FileTunnelRegistry;
use crate::shell_tunnels::ShellTunnelRegistry;
use mac_mgmt_ws::{WsClientConfig, WsConnect, WsStream};

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum ControlMessage {
    Registered {
        ssh_port: u16,
        #[serde(default)]
        proxy_hostname: Option<String>,
        #[serde(default)]
        proxy_url: Option<String>,
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
    /// Streaming proxy request — response sent as multiple messages on the control channel.
    ProxyStreamRequest {
        request_id: String,
        tunnel_name: String,
        method: String,
        path: String,
        headers: serde_json::Value,
        body: Option<String>,
    },
    ProxySessionRequest {
        session_id: String,
        session_secret: String,
        tunnel_name: String,
        mode: String,
        path: String,
    },
    /// List files in a file tunnel (response sent on the control channel).
    FileListRequest {
        request_id: String,
        tunnel_name: String,
        path: Option<String>,
    },
    /// Start a data session for file read or write.
    FileSessionRequest {
        session_id: String,
        session_secret: String,
        tunnel_name: String,
        mode: String,
        path: Option<String>,
        expected_mtime: Option<i64>,
    },
    /// Start a data session for shell command execution.
    ShellSessionRequest {
        session_id: String,
        session_secret: String,
        command_name: String,
        user_arg: Option<String>,
    },
}

/// A tunnel definition used to map tunnel names to local host:port.
#[derive(Debug, Clone)]
pub struct TunnelTarget {
    pub host: String,
    pub port: u16,
}

// ── Shared proxy helpers ─────────────────────────────────────────────

/// Build a reqwest request for the given HTTP method against a tunnel target.
fn build_proxy_request(
    client: &reqwest::Client,
    target: &TunnelTarget,
    method: &str,
    path: &str,
) -> reqwest::RequestBuilder {
    let url = format!("http://{}:{}{path}", target.host, target.port);
    let mut req = match method {
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        "PATCH" => client.patch(&url),
        "HEAD" => client.head(&url),
        _ => client.get(&url),
    };
    req = req.header("host", format!("{}:{}", target.host, target.port));
    req
}

/// Apply request headers from a Vec, filtering hop-by-hop headers.
fn apply_headers_vec(
    mut req: reqwest::RequestBuilder,
    headers: &[(String, String)],
) -> reqwest::RequestBuilder {
    for (k, v) in headers {
        let lk = k.to_lowercase();
        if lk == "connection" || lk == "transfer-encoding" || lk == "host" {
            continue;
        }
        req = req.header(k.as_str(), v.as_str());
    }
    req
}

/// Apply request headers from a JSON object, filtering hop-by-hop headers.
fn apply_headers_json(
    mut req: reqwest::RequestBuilder,
    headers: &serde_json::Value,
) -> reqwest::RequestBuilder {
    if let Some(hdrs) = headers.as_object() {
        for (k, v) in hdrs {
            let lk = k.to_lowercase();
            if lk == "connection" || lk == "transfer-encoding" || lk == "host" {
                continue;
            }
            if let Some(val) = v.as_str() {
                req = req.header(k.as_str(), val);
            }
        }
    }
    req
}

/// Decode a base64-encoded body and attach it to the request.
fn apply_body_b64(req: reqwest::RequestBuilder, body_b64: Option<String>) -> reqwest::RequestBuilder {
    if let Some(b64) = body_b64 {
        use base64::Engine;
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&b64) {
            return req.body(bytes);
        }
    }
    req
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
    relay_proxy_url: Arc<RwLock<Option<String>>>,
    ws_outgoing_tx: Arc<RwLock<Option<mpsc::Sender<String>>>>,
    file_tunnel_registry: Arc<RwLock<FileTunnelRegistry>>,
    shell_tunnel_registry: Arc<RwLock<ShellTunnelRegistry>>,
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

    let _ws_handle = mac_mgmt_ws::spawn_reconnecting(ws_config, incoming_tx, outgoing_rx);

    while let Some(text) = incoming_rx.recv().await {
        let control: ControlMessage = match serde_json::from_str(&text) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("invalid relay control message ({e}): {text}");
                continue;
            }
        };

        match control {
            ControlMessage::Registered { ssh_port, proxy_hostname, proxy_url } => {
                tracing::info!(
                    "relay registered: instance={instance_id} ssh_port={ssh_port} proxy_hostname={proxy_hostname:?} proxy_url={proxy_url:?}"
                );
                // Store the relay's proxy hostname and URL for heartbeats.
                if let Some(ph) = proxy_hostname {
                    *relay_proxy_hostname.write().await = Some(ph);
                }
                if let Some(pu) = proxy_url {
                    *relay_proxy_url.write().await = Some(pu);
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
            ControlMessage::ProxyStreamRequest { request_id, tunnel_name, method, path, headers, body } => {
                tracing::debug!("proxy stream {request_id}: {method} {tunnel_name}{path}");
                let target = {
                    let defs = tunnel_defs.read().await;
                    defs.get(&tunnel_name).cloned()
                };
                let out_tx = outgoing_tx.clone();
                tokio::spawn(async move {
                    handle_proxy_stream_request(&out_tx, &request_id, target, &method, &path, headers, body).await;
                });
            }
            ControlMessage::ProxySessionRequest { session_id, session_secret, tunnel_name, mode, path } => {
                tracing::info!("proxy session {session_id}: {mode} {tunnel_name}{path}");
                let target = {
                    let defs = tunnel_defs.read().await;
                    defs.get(&tunnel_name).cloned()
                };
                let relay = relay_url.to_string();
                let tok = token.to_string();
                tokio::spawn(async move {
                    if let Err(e) = handle_proxy_session(
                        &relay, &tok, &session_id, &session_secret, target, &mode, &path,
                    ).await {
                        tracing::error!("proxy session {session_id} failed: {e:#}");
                    }
                });
            }
            ControlMessage::FileListRequest { request_id, tunnel_name, path } => {
                tracing::debug!("file list {request_id}: {tunnel_name}");
                #[cfg(feature = "services")]
                {
                    let registry = file_tunnel_registry.read().await;
                    let tunnel = registry.get(&tunnel_name).cloned();
                    drop(registry);
                    let out_tx = outgoing_tx.clone();
                    tokio::spawn(async move {
                        let (status, body) = match tunnel {
                            Some(t) => crate::file_tunnels::handle_list(&t, path.as_deref()),
                            None => (404, serde_json::json!({ "error": "file tunnel not found" })),
                        };
                        let msg = serde_json::json!({
                            "type": "file_response",
                            "request_id": request_id,
                            "status": status,
                            "body": body.to_string(),
                        });
                        let _ = out_tx.send(msg.to_string()).await;
                    });
                }
                #[cfg(not(feature = "services"))]
                {
                    let out_tx = outgoing_tx.clone();
                    tokio::spawn(async move {
                        let msg = serde_json::json!({
                            "type": "file_response",
                            "request_id": request_id,
                            "status": 404,
                            "body": serde_json::json!({ "error": "file tunnels not available" }).to_string(),
                        });
                        let _ = out_tx.send(msg.to_string()).await;
                    });
                }
            }
            ControlMessage::FileSessionRequest { session_id, session_secret, tunnel_name, mode, path, expected_mtime } => {
                #[cfg(feature = "services")]
                {
                    tracing::info!("file session {session_id}: {mode} {tunnel_name}");
                    let registry = file_tunnel_registry.read().await;
                    let tunnel = registry.get(&tunnel_name).cloned();
                    drop(registry);
                    let relay = relay_url.to_string();
                    let tok = token.to_string();
                    tokio::spawn(async move {
                        if let Err(e) = handle_file_session(
                            &relay, &tok, &session_id, &session_secret,
                            tunnel, &mode, path.as_deref(), expected_mtime,
                        ).await {
                            tracing::error!("file session {session_id} failed: {e:#}");
                        }
                    });
                }
                #[cfg(not(feature = "services"))]
                {
                    let _ = (&session_id, &session_secret, &tunnel_name, &mode, &path, &expected_mtime);
                }
            }
            ControlMessage::ShellSessionRequest { session_id, session_secret, command_name, user_arg } => {
                #[cfg(feature = "services")]
                {
                    tracing::info!("shell session {session_id}: {command_name}");
                    let registry = shell_tunnel_registry.read().await;
                    let tunnel = registry.get(&command_name).cloned();
                    drop(registry);
                    let relay = relay_url.to_string();
                    let tok = token.to_string();
                    tokio::spawn(async move {
                        if let Err(e) = handle_shell_session(
                            &relay, &tok, &session_id, &session_secret,
                            tunnel, user_arg.as_deref(),
                        ).await {
                            tracing::error!("shell session {session_id} failed: {e:#}");
                        }
                    });
                }
                #[cfg(not(feature = "services"))]
                {
                    let _ = (&session_id, &session_secret, &command_name, &user_arg);
                }
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

    let ws = WsConnect::new(&ws_url)
        .bearer_auth(token)
        .connect()
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

    let client = reqwest::Client::new();
    let req = build_proxy_request(&client, &target, method, path);
    let req = apply_headers_vec(req, &headers);
    let req = apply_body_b64(req, body);

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
            tracing::warn!("proxy request to {}:{}{path} failed: {e}", target.host, target.port);
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

/// Streaming proxy request handler — sends response as multiple messages
/// on the control channel (headers, base64 body chunks, end).
async fn handle_proxy_stream_request(
    out_tx: &mpsc::Sender<String>,
    request_id: &str,
    target: Option<TunnelTarget>,
    method: &str,
    path: &str,
    headers_json: serde_json::Value,
    body_b64: Option<String>,
) {
    let Some(target) = target else {
        let msg = serde_json::json!({
            "type": "proxy_stream_headers",
            "request_id": request_id,
            "status": 404,
            "headers": [],
        });
        let _ = out_tx.send(msg.to_string()).await;
        let _ = out_tx.send(serde_json::json!({
            "type": "proxy_stream_end", "request_id": request_id
        }).to_string()).await;
        return;
    };

    let client = reqwest::Client::new();
    let req = build_proxy_request(&client, &target, method, path);
    let req = apply_headers_json(req, &headers_json);
    let req = apply_body_b64(req, body_b64);

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("proxy stream {request_id} to {}:{}{path} failed: {e}", target.host, target.port);
            let _ = out_tx.send(serde_json::json!({
                "type": "proxy_stream_headers",
                "request_id": request_id,
                "status": 502,
                "headers": [],
            }).to_string()).await;
            let _ = out_tx.send(serde_json::json!({
                "type": "proxy_stream_end", "request_id": request_id
            }).to_string()).await;
            return;
        }
    };

    let status = resp.status().as_u16();
    let resp_headers: Vec<(String, String)> = resp
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    // Send response headers.
    let _ = out_tx.send(serde_json::json!({
        "type": "proxy_stream_headers",
        "request_id": request_id,
        "status": status,
        "headers": resp_headers,
    }).to_string()).await;

    // Stream body chunks as base64.
    use futures_util::StreamExt;
    let mut body_stream = resp.bytes_stream();
    while let Some(chunk) = body_stream.next().await {
        match chunk {
            Ok(bytes) => {
                use base64::Engine;
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                let _ = out_tx.send(serde_json::json!({
                    "type": "proxy_stream_chunk",
                    "request_id": request_id,
                    "data": b64,
                }).to_string()).await;
            }
            Err(e) => {
                tracing::warn!("proxy stream {request_id} chunk error: {e}");
                break;
            }
        }
    }

    // Signal response complete.
    let _ = out_tx.send(serde_json::json!({
        "type": "proxy_stream_end", "request_id": request_id
    }).to_string()).await;
}

/// Maximum chunk size for streaming over data WS (must be under relay's WS limit).
const STREAM_CHUNK_SIZE: usize = 1024 * 1024; // 1 MB

/// Handle a proxy session: connect data WS to relay, then either bridge
/// to a local WebSocket or stream an HTTP request/response.
async fn handle_proxy_session(
    relay_url: &str,
    token: &str,
    session_id: &str,
    session_secret: &str,
    target: Option<TunnelTarget>,
    mode: &str,
    path: &str,
) -> anyhow::Result<()> {
    let Some(target) = target else {
        anyhow::bail!("tunnel not found");
    };

    // Connect data WS to relay
    let ws_url = format!(
        "{relay_url}/api/daemon/session/{session_id}?session_secret={}",
        urlencoding::encode(session_secret),
    );

    let data_ws = WsConnect::new(&ws_url)
        .bearer_auth(token)
        .connect()
        .await
        .context("proxy session data WS connect failed")?;

    tracing::debug!("proxy session {session_id} data WS connected");

    match mode {
        "websocket" => proxy_session_websocket(data_ws, &target, path).await,
        "stream" => proxy_session_stream(data_ws, &target, path).await,
        _ => anyhow::bail!("unknown proxy session mode: {mode}"),
    }
}

/// Bridge relay data WS ↔ local service WS.
async fn proxy_session_websocket(
    data_ws: mac_mgmt_ws::ClientWs,
    target: &TunnelTarget,
    path: &str,
) -> anyhow::Result<()> {
    let local_url = format!("ws://{}:{}{path}", target.host, target.port);
    let local_ws = WsConnect::new(&local_url)
        .connect()
        .await
        .context("local WS connect failed")?;

    tracing::info!("proxy WS session bridging to {local_url}");
    mac_mgmt_ws::bridge::client_ws(data_ws, local_ws).await;
    tracing::info!("proxy WS session ended");
    Ok(())
}

/// Stream HTTP response from local service through relay data WS.
///
/// Protocol:
/// 1. Daemon reads first text message from relay: JSON `{ method, path, headers, body? }`
/// 2. If body present, relay sends binary chunks (request body)
/// 3. Relay sends a text message `"end_request"` to signal request body is complete
/// 4. Daemon sends text message: JSON `{ status, headers }` (response headers)
/// 5. Daemon sends binary messages: response body chunks (≤ STREAM_CHUNK_SIZE)
/// 6. Daemon closes the WS
async fn proxy_session_stream(
    data_ws: mac_mgmt_ws::ClientWs,
    target: &TunnelTarget,
    path: &str,
) -> anyhow::Result<()> {
    use futures_util::{SinkExt, StreamExt};

    let (mut sink, mut stream) = data_ws.split();

    // Read the request details from the first message
    let first_msg = stream.next().await
        .ok_or_else(|| anyhow::anyhow!("data WS closed before request"))?
        .context("data WS read error")?;

    let req_json: serde_json::Value = match first_msg {
        tungstenite::Message::Text(t) => serde_json::from_str(&t)?,
        _ => anyhow::bail!("expected text message with request details"),
    };

    let method = req_json["method"].as_str().unwrap_or("GET");
    let req_path = req_json["path"].as_str().unwrap_or(path);

    let client = reqwest::Client::new();
    let mut req = build_proxy_request(&client, &target, method, req_path);
    req = apply_headers_json(req, &req_json["headers"]);

    // Collect request body chunks from data WS until "end_request"
    let has_body = req_json.get("has_body").and_then(|v| v.as_bool()).unwrap_or(false);
    if has_body {
        let mut body_bytes = Vec::new();
        while let Some(Ok(msg)) = stream.next().await {
            match msg {
                tungstenite::Message::Binary(chunk) => body_bytes.extend_from_slice(&chunk),
                tungstenite::Message::Text(t) if t.as_str() == "end_request" => break,
                _ => break,
            }
        }
        req = req.body(body_bytes);
    }

    // Make the request
    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            let err = serde_json::json!({ "status": 502, "headers": [] });
            let _ = sink.send(tungstenite::Message::Text(err.to_string().into())).await;
            anyhow::bail!("local request failed: {e}");
        }
    };

    let status = resp.status().as_u16();
    let headers: Vec<(String, String)> = resp
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    // Send response headers
    let header_msg = serde_json::json!({ "status": status, "headers": headers });
    sink.send(tungstenite::Message::Text(header_msg.to_string().into()))
        .await
        .context("failed to send response headers")?;

    // Stream response body in chunks respecting WS size limit
    let mut body_stream = resp.bytes_stream();
    let mut buf = Vec::new();
    while let Some(chunk) = body_stream.next().await {
        match chunk {
            Ok(bytes) => {
                buf.extend_from_slice(&bytes);
                while buf.len() >= STREAM_CHUNK_SIZE {
                    let chunk: Vec<u8> = buf.drain(..STREAM_CHUNK_SIZE).collect();
                    if sink.send(tungstenite::Message::Binary(chunk.into())).await.is_err() {
                        return Ok(());
                    }
                }
            }
            Err(e) => {
                tracing::warn!("stream chunk error: {e}");
                break;
            }
        }
    }
    // Flush remaining
    if !buf.is_empty() {
        let _ = sink.send(tungstenite::Message::Binary(buf.into())).await;
    }

    let _ = sink.send(tungstenite::Message::Close(None)).await;
    tracing::debug!("proxy stream session ended");
    Ok(())
}

/// Handle a file session: connect data WS to relay, then dispatch to
/// the appropriate file tunnel handler (read or write).
#[cfg(feature = "services")]
async fn handle_file_session(
    relay_url: &str,
    token: &str,
    session_id: &str,
    session_secret: &str,
    tunnel: Option<crate::managed_service::FileTunnel>,
    mode: &str,
    path: Option<&str>,
    expected_mtime: Option<i64>,
) -> anyhow::Result<()> {
    let Some(tunnel) = tunnel else {
        anyhow::bail!("file tunnel not found");
    };

    // Connect data WS to relay
    let ws_url = format!(
        "{relay_url}/api/daemon/session/{session_id}?session_secret={}",
        urlencoding::encode(session_secret),
    );

    let data_ws = WsConnect::new(&ws_url)
        .bearer_auth(token)
        .connect()
        .await
        .context("file session data WS connect failed")?;

    tracing::debug!("file session {session_id} data WS connected");

    match mode {
        "read" => {
            crate::file_tunnels::handle_read_session(&tunnel, path, data_ws).await;
        }
        "write" => {
            crate::file_tunnels::handle_write_session(&tunnel, path, expected_mtime, data_ws)
                .await;
        }
        _ => anyhow::bail!("unknown file session mode: {mode}"),
    }

    Ok(())
}

/// Handle a shell command session: connect data WS to relay, then dispatch to
/// the shell tunnel handler for command execution.
#[cfg(feature = "services")]
async fn handle_shell_session(
    relay_url: &str,
    token: &str,
    session_id: &str,
    session_secret: &str,
    tunnel: Option<crate::managed_service::ShellTunnel>,
    user_arg: Option<&str>,
) -> anyhow::Result<()> {
    let Some(tunnel) = tunnel else {
        anyhow::bail!("shell command not found");
    };

    let ws_url = format!(
        "{relay_url}/api/daemon/session/{session_id}?session_secret={}",
        urlencoding::encode(session_secret),
    );

    let data_ws = WsConnect::new(&ws_url)
        .bearer_auth(token)
        .connect()
        .await
        .context("shell session data WS connect failed")?;

    tracing::debug!("shell session {session_id} data WS connected");

    crate::shell_tunnels::handle_exec_session(&tunnel, user_arg, data_ws).await;

    Ok(())
}
