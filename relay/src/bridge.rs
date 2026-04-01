use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use axum::extract::ws::{Message, WebSocket};

/// Pending sessions: SSH TCP streams waiting for the daemon's data channel.
static PENDING_SESSIONS: LazyLock<Mutex<HashMap<String, TcpStream>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn register_pending_session(session_id: String, stream: TcpStream) {
    PENDING_SESSIONS
        .lock()
        .unwrap()
        .insert(session_id, stream);
}

pub fn take_pending_session(session_id: &str) -> Option<TcpStream> {
    PENDING_SESSIONS
        .lock()
        .unwrap()
        .remove(session_id)
}

/// Bridge bidirectional data between a TCP stream and a WebSocket.
pub async fn bridge_tcp_ws(mut tcp: TcpStream, ws: WebSocket) {
    let (mut ws_sink, mut ws_stream) = ws.split();
    let (mut tcp_read, mut tcp_write) = tcp.split();

    let ws_to_tcp = async {
        while let Some(Ok(msg)) = ws_stream.next().await {
            match msg {
                Message::Binary(data) => {
                    if tcp_write.write_all(&data).await.is_err() {
                        break;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    };

    let tcp_to_ws = async {
        let mut buf = [0u8; 8192];
        loop {
            match tcp_read.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if ws_sink
                        .send(Message::Binary(buf[..n].to_vec().into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    };

    tokio::select! {
        _ = ws_to_tcp => {}
        _ = tcp_to_ws => {}
    }

    tracing::debug!("bridge closed");
}
