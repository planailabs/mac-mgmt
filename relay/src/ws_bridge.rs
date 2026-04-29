//! Bridges between axum WebSocket connections and libp2p substreams.
//!
//! Uses a simple framing protocol on the substream to preserve WS
//! message types:
//!   Tag 0x01 + length-prefixed payload = Text message
//!   Tag 0x02 + length-prefixed payload = Binary message
//!   Tag 0x03 = Close
//!
//! Ping/Pong are handled locally at each end (not forwarded).

use axum::extract::ws::{Message, WebSocket};
use futures_util::{AsyncReadExt, AsyncWriteExt, SinkExt, StreamExt};

const TAG_TEXT: u8 = 0x01;
const TAG_BINARY: u8 = 0x02;
const TAG_CLOSE: u8 = 0x03;

/// Bridge an axum WebSocket to a libp2p `Stream` (tunnel substream).
pub async fn bridge_ws_to_stream(ws: WebSocket, mut stream: libp2p::Stream) {
    let (mut ws_sink, mut ws_stream) = ws.split();
    let mut read_buf = vec![0u8; 64 * 1024];

    loop {
        tokio::select! {
            ws_msg = ws_stream.next() => {
                match ws_msg {
                    Some(Ok(Message::Text(text))) => {
                        let bytes = text.as_bytes();
                        let header = [TAG_TEXT];
                        let len = (bytes.len() as u32).to_be_bytes();
                        if stream.write_all(&header).await.is_err()
                            || stream.write_all(&len).await.is_err()
                            || stream.write_all(bytes).await.is_err()
                        { break; }
                        let _ = stream.flush().await;
                    }
                    Some(Ok(Message::Binary(data))) => {
                        let header = [TAG_BINARY];
                        let len = (data.len() as u32).to_be_bytes();
                        if stream.write_all(&header).await.is_err()
                            || stream.write_all(&len).await.is_err()
                            || stream.write_all(&data).await.is_err()
                        { break; }
                        let _ = stream.flush().await;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        // Respond to ping locally.
                        let _ = ws_sink.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Pong(_))) => {} // ignore
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => {
                        let _ = stream.write_all(&[TAG_CLOSE]).await;
                        break;
                    }
                }
            }
            // Read framed messages from the substream.
            read_result = stream.read_exact(&mut read_buf[..1]) => {
                if read_result.is_err() { break; }
                match read_buf[0] {
                    TAG_TEXT => {
                        let mut len_buf = [0u8; 4];
                        if stream.read_exact(&mut len_buf).await.is_err() { break; }
                        let len = u32::from_be_bytes(len_buf) as usize;
                        if len > 16 * 1024 * 1024 { break; }
                        let mut data = vec![0u8; len];
                        if stream.read_exact(&mut data).await.is_err() { break; }
                        let text = String::from_utf8_lossy(&data);
                        if ws_sink.send(Message::Text(text.into_owned().into())).await.is_err() { break; }
                    }
                    TAG_BINARY => {
                        let mut len_buf = [0u8; 4];
                        if stream.read_exact(&mut len_buf).await.is_err() { break; }
                        let len = u32::from_be_bytes(len_buf) as usize;
                        if len > 16 * 1024 * 1024 { break; }
                        let mut data = vec![0u8; len];
                        if stream.read_exact(&mut data).await.is_err() { break; }
                        if ws_sink.send(Message::Binary(data.into())).await.is_err() { break; }
                    }
                    TAG_CLOSE => break,
                    _ => break,
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
