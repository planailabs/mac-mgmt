//! Bridges between axum WebSocket connections and libp2p substreams.
//!
//! Two use cases:
//! 1. **End-user proxy**: Browser opens WS to the relay proxy subdomain,
//!    relay opens a libp2p tunnel substream to the daemon, bridges the two.
//! 2. **libp2p-over-WS**: Daemon connects via WS on the main HTTP port,
//!    relay bridges the raw WS connection to a TCP connection to the local
//!    libp2p WS listener so the daemon gets a full libp2p connection.

use axum::extract::ws::{Message, WebSocket};
use futures_util::{AsyncReadExt, AsyncWriteExt, SinkExt, StreamExt};

/// Bridge an axum WebSocket to a libp2p `Stream` (tunnel substream).
///
/// WS binary messages are forwarded as raw bytes on the substream.
/// WS text messages are forwarded as UTF-8 bytes.
/// Runs until either side closes or errors.
pub async fn bridge_ws_to_stream(ws: WebSocket, mut stream: libp2p::Stream) {
    let (mut ws_sink, mut ws_stream) = ws.split();
    let mut buf = vec![0u8; 64 * 1024];

    loop {
        tokio::select! {
            ws_msg = ws_stream.next() => {
                match ws_msg {
                    Some(Ok(Message::Binary(data))) => {
                        if stream.write_all(&data).await.is_err() { break; }
                    }
                    Some(Ok(Message::Text(text))) => {
                        if stream.write_all(text.as_bytes()).await.is_err() { break; }
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    _ => {}
                }
            }
            read_result = stream.read(&mut buf) => {
                match read_result {
                    Ok(0) => break,
                    Ok(n) => {
                        if ws_sink.send(Message::Binary(buf[..n].to_vec().into())).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    }

    let _ = stream.close().await;
    let _ = ws_sink.send(Message::Close(None)).await;
}

/// Bridge an axum WebSocket to a TCP connection to the local libp2p
/// WS listener, allowing daemons to establish libp2p connections
/// through the main HTTP port.
///
/// Forwards raw WebSocket frames bidirectionally between the incoming
/// axum WS and a new WS connection to `localhost:{p2p_port}`.
pub async fn bridge_ws_to_libp2p_listener(ws: WebSocket, p2p_port: u16) {
    use tokio_tungstenite::tungstenite;

    let url = format!("ws://[::1]:{p2p_port}");
    let connect_result = tokio_tungstenite::connect_async(&url).await;
    let (upstream, _) = match connect_result {
        Ok(pair) => pair,
        Err(e) => {
            tracing::warn!("failed to connect to local libp2p WS listener: {e}");
            return;
        }
    };

    let (mut up_sink, mut up_stream) = upstream.split();
    let (mut ws_sink, mut ws_stream) = ws.split();

    // client WS → libp2p WS
    let client_to_upstream = async {
        while let Some(msg) = ws_stream.next().await {
            let tung_msg = match msg {
                Ok(Message::Binary(data)) => tungstenite::Message::Binary(data),
                Ok(Message::Text(text)) => {
                    tungstenite::Message::text(text.as_str())
                }
                Ok(Message::Ping(data)) => tungstenite::Message::Ping(data.to_vec().into()),
                Ok(Message::Pong(data)) => tungstenite::Message::Pong(data.to_vec().into()),
                Ok(Message::Close(_)) => {
                    let _ = up_sink.send(tungstenite::Message::Close(None)).await;
                    break;
                }
                Err(_) => break,
            };
            if up_sink.send(tung_msg).await.is_err() {
                break;
            }
        }
    };

    // libp2p WS → client WS
    let upstream_to_client = async {
        while let Some(msg) = up_stream.next().await {
            let axum_msg = match msg {
                Ok(tungstenite::Message::Binary(data)) => Message::Binary(data),
                Ok(tungstenite::Message::Text(text)) => {
                    Message::Text(text.as_str().into())
                }
                Ok(tungstenite::Message::Ping(data)) => Message::Ping(data.to_vec().into()),
                Ok(tungstenite::Message::Pong(data)) => Message::Pong(data.to_vec().into()),
                Ok(tungstenite::Message::Close(_)) => {
                    let _ = ws_sink.send(Message::Close(None)).await;
                    break;
                }
                _ => continue,
            };
            if ws_sink.send(axum_msg).await.is_err() {
                break;
            }
        }
    };

    tokio::select! {
        _ = client_to_upstream => {}
        _ = upstream_to_client => {}
    }
}
