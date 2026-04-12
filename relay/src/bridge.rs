use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use axum::extract::ws::{Message, WebSocket};

const SESSION_TTL: Duration = Duration::from_secs(30);

struct PendingSession {
    stream: TcpStream,
    secret: String,
    created_at: Instant,
}

/// Pending sessions: SSH TCP streams waiting for the daemon's data channel.
static PENDING_SESSIONS: LazyLock<Mutex<HashMap<String, PendingSession>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Pending proxy session: a browser WebSocket waiting for the daemon's data channel.
struct PendingProxySession {
    ws: WebSocket,
    secret: String,
    created_at: Instant,
}

static PENDING_PROXY_SESSIONS: LazyLock<Mutex<HashMap<String, PendingProxySession>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn register_pending_proxy_session(session_id: String, secret: String, ws: WebSocket) {
    let mut sessions = PENDING_PROXY_SESSIONS.lock().unwrap();
    sessions.insert(session_id.clone(), PendingProxySession {
        ws,
        secret,
        created_at: Instant::now(),
    });
    tracing::debug!("registered pending proxy session {session_id}");
}

pub fn take_pending_proxy_session(session_id: &str, secret: &str) -> Option<WebSocket> {
    let mut sessions = PENDING_PROXY_SESSIONS.lock().unwrap();
    if let Some(pending) = sessions.get(session_id) {
        if pending.secret != secret {
            tracing::warn!("proxy session {session_id}: secret mismatch");
            return None;
        }
        if pending.created_at.elapsed() > SESSION_TTL {
            sessions.remove(session_id);
            return None;
        }
        return sessions.remove(session_id).map(|p| p.ws);
    }
    None
}

/// Bridge two WebSockets bidirectionally.
pub async fn bridge_ws_ws(ws_a: WebSocket, ws_b: WebSocket) {
    let (mut a_sink, mut a_stream) = ws_a.split();
    let (mut b_sink, mut b_stream) = ws_b.split();

    let a_to_b = async {
        while let Some(msg) = a_stream.next().await {
            match msg {
                Ok(msg @ (Message::Binary(_) | Message::Text(_))) => {
                    if b_sink.send(msg).await.is_err() { break; }
                }
                Ok(Message::Close(_)) | Err(_) => break,
                _ => {}
            }
        }
    };

    let b_to_a = async {
        while let Some(msg) = b_stream.next().await {
            match msg {
                Ok(msg @ (Message::Binary(_) | Message::Text(_))) => {
                    if a_sink.send(msg).await.is_err() { break; }
                }
                Ok(Message::Close(_)) | Err(_) => break,
                _ => {}
            }
        }
    };

    tokio::select! {
        _ = a_to_b => {}
        _ = b_to_a => {}
    }
    tracing::debug!("ws-ws bridge closed");
}

pub fn register_pending_session(session_id: String, secret: String, stream: TcpStream) {
    let count = {
        let mut sessions = PENDING_SESSIONS.lock().unwrap();
        sessions.insert(
            session_id.clone(),
            PendingSession {
                stream,
                secret,
                created_at: Instant::now(),
            },
        );
        sessions.len()
    };
    tracing::debug!("registered pending session {session_id} (pending now: {count})");
}

pub fn take_pending_session(session_id: &str, secret: &str) -> Option<TcpStream> {
    let mut sessions = PENDING_SESSIONS.lock().unwrap();
    if let Some(pending) = sessions.get(session_id) {
        if pending.secret != secret {
            tracing::warn!("session {session_id}: secret mismatch, rejecting");
            return None;
        }
        if pending.created_at.elapsed() > SESSION_TTL {
            tracing::warn!("session {session_id}: expired, removing");
            sessions.remove(session_id);
            return None;
        }
        return sessions.remove(session_id).map(|p| p.stream);
    }
    tracing::warn!("no pending session found for {session_id}");
    None
}

/// Remove sessions that have been pending longer than the TTL.
pub fn cleanup_expired() {
    {
        let mut sessions = PENDING_SESSIONS.lock().unwrap();
        let before = sessions.len();
        sessions.retain(|id, s| {
            let expired = s.created_at.elapsed() > SESSION_TTL;
            if expired { tracing::info!("expiring stale pending session {id}"); }
            !expired
        });
        let removed = before - sessions.len();
        if removed > 0 { tracing::debug!("cleaned up {removed} expired pending session(s)"); }
    }
    {
        let mut sessions = PENDING_PROXY_SESSIONS.lock().unwrap();
        sessions.retain(|id, s| {
            let expired = s.created_at.elapsed() > SESSION_TTL;
            if expired { tracing::info!("expiring stale pending proxy session {id}"); }
            !expired
        });
    }
}

/// Spawn a background task that periodically cleans up expired sessions.
pub fn spawn_cleanup_task() {
    tokio::spawn(async {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop {
            interval.tick().await;
            cleanup_expired();
        }
    });
}

/// Bridge bidirectional data between a TCP stream and a WebSocket.
pub async fn bridge_tcp_ws(mut tcp: TcpStream, ws: WebSocket) {
    let (mut ws_sink, mut ws_stream) = ws.split();
    let (mut tcp_read, mut tcp_write) = tcp.split();
    let mut ws_to_tcp_bytes: u64 = 0;
    let mut tcp_to_ws_bytes: u64 = 0;

    let ws_to_tcp = async {
        let mut bytes: u64 = 0;
        while let Some(msg) = ws_stream.next().await {
            match msg {
                Ok(Message::Binary(data)) => {
                    if let Err(e) = tcp_write.write_all(&data).await {
                        tracing::debug!("ws->tcp write failed: {e}");
                        break;
                    }
                    bytes += data.len() as u64;
                }
                Ok(Message::Close(_)) => {
                    tracing::debug!("ws closed by peer");
                    break;
                }
                Err(e) => {
                    tracing::debug!("ws read error: {e}");
                    break;
                }
                _ => {}
            }
        }
        bytes
    };

    let tcp_to_ws = async {
        let mut buf = [0u8; 8192];
        let mut bytes: u64 = 0;
        loop {
            match tcp_read.read(&mut buf).await {
                Ok(0) => {
                    tracing::debug!("tcp closed by peer");
                    break;
                }
                Ok(n) => {
                    if let Err(e) = ws_sink
                        .send(Message::Binary(buf[..n].to_vec().into()))
                        .await
                    {
                        tracing::debug!("tcp->ws send failed: {e}");
                        break;
                    }
                    bytes += n as u64;
                }
                Err(e) => {
                    tracing::debug!("tcp read error: {e}");
                    break;
                }
            }
        }
        bytes
    };

    tokio::select! {
        n = ws_to_tcp => { ws_to_tcp_bytes = n; }
        n = tcp_to_ws => { tcp_to_ws_bytes = n; }
    }

    tracing::info!(
        "bridge closed (ws->tcp: {ws_to_tcp_bytes}B, tcp->ws: {tcp_to_ws_bytes}B)"
    );
}
