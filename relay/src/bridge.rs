use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use tokio::net::TcpStream;

use axum::extract::ws::WebSocket;

const SESSION_TTL: Duration = Duration::from_secs(60);
const MAX_PENDING_PROXY_SESSIONS: usize = 1000;

struct PendingSession {
    stream: TcpStream,
    secret: String,
    created_at: Instant,
}

/// Pending sessions: SSH TCP streams waiting for the daemon's data channel.
static PENDING_SESSIONS: LazyLock<Mutex<HashMap<String, PendingSession>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Pending proxy session variants.
enum PendingProxySession {
    /// A browser WebSocket to be bridged with the daemon's data WS (for /proxy_ws).
    WebSocket {
        ws: WebSocket,
        secret: String,
        created_at: Instant,
    },
    /// A oneshot sender to deliver the daemon's data WS to the caller (for catchall proxy).
    Callback {
        tx: tokio::sync::oneshot::Sender<WebSocket>,
        secret: String,
        created_at: Instant,
    },
}

static PENDING_PROXY_SESSIONS: LazyLock<Mutex<HashMap<String, PendingProxySession>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn register_pending_proxy_session(session_id: String, secret: String, ws: WebSocket) -> bool {
    let mut sessions = PENDING_PROXY_SESSIONS.lock().unwrap();
    if sessions.len() >= MAX_PENDING_PROXY_SESSIONS {
        tracing::warn!("pending proxy session limit reached ({MAX_PENDING_PROXY_SESSIONS}), rejecting");
        return false;
    }
    sessions.insert(session_id.clone(), PendingProxySession::WebSocket {
        ws,
        secret,
        created_at: Instant::now(),
    });
    tracing::debug!("registered pending proxy session {session_id} (ws bridge)");
    true
}

pub fn register_pending_proxy_session_with_callback(
    session_id: String,
    secret: String,
    tx: tokio::sync::oneshot::Sender<WebSocket>,
) -> bool {
    let mut sessions = PENDING_PROXY_SESSIONS.lock().unwrap();
    if sessions.len() >= MAX_PENDING_PROXY_SESSIONS {
        tracing::warn!("pending proxy session limit reached ({MAX_PENDING_PROXY_SESSIONS}), rejecting");
        return false;
    }
    sessions.insert(session_id.clone(), PendingProxySession::Callback {
        tx,
        secret,
        created_at: Instant::now(),
    });
    tracing::debug!("registered pending proxy session {session_id} (callback)");
    true
}

/// Remove a pending proxy session (used for cleanup on failure).
pub fn remove_pending_proxy_session(session_id: &str) {
    PENDING_PROXY_SESSIONS.lock().unwrap().remove(session_id);
}

/// Check if a pending proxy session exists (without consuming it).
pub fn has_pending_proxy_session(session_id: &str) -> bool {
    PENDING_PROXY_SESSIONS.lock().unwrap().contains_key(session_id)
}

/// Complete a pending proxy session with the daemon's data WS.
/// For WS bridge sessions: bridges and returns true.
/// For callback sessions: sends the WS to the caller and returns true.
/// Returns false if session not found.
pub async fn complete_proxy_session(session_id: &str, secret: &str, daemon_ws: WebSocket) -> bool {
    let pending = {
        let mut sessions = PENDING_PROXY_SESSIONS.lock().unwrap();
        // Always remove the session to prevent orphans.
        let Some(pending) = sessions.remove(session_id) else { return false; };
        let (pending_secret, pending_time) = match &pending {
            PendingProxySession::WebSocket { secret: s, created_at, .. } => (s.as_str(), *created_at),
            PendingProxySession::Callback { secret: s, created_at, .. } => (s.as_str(), *created_at),
        };
        if pending_secret != secret {
            tracing::warn!("proxy session {session_id}: secret mismatch, dropping");
            return false;
        }
        if pending_time.elapsed() > SESSION_TTL {
            tracing::warn!("proxy session {session_id}: expired ({:.1}s old), dropping", pending_time.elapsed().as_secs_f64());
            return false;
        }
        Some(pending)
    };

    match pending {
        Some(PendingProxySession::WebSocket { ws: browser_ws, .. }) => {
            tracing::info!("bridging proxy session {session_id} (ws-ws)");
            bridge_ws_ws(browser_ws, daemon_ws).await;
            true
        }
        Some(PendingProxySession::Callback { tx, .. }) => {
            tracing::info!("delivering daemon WS for proxy session {session_id}");
            let _ = tx.send(daemon_ws);
            true
        }
        None => false,
    }
}

/// Bridge two WebSockets bidirectionally, forwarding close frames.
pub async fn bridge_ws_ws(ws_a: WebSocket, ws_b: WebSocket) {
    mac_mgmt_ws::bridge::axum_ws(ws_a, ws_b).await;
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
            let created_at = match s {
                PendingProxySession::WebSocket { created_at, .. } => *created_at,
                PendingProxySession::Callback { created_at, .. } => *created_at,
            };
            let expired = created_at.elapsed() > SESSION_TTL;
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
pub async fn bridge_tcp_ws(tcp: TcpStream, ws: WebSocket) {
    mac_mgmt_ws::bridge::tcp_ws(tcp, ws).await;
}
