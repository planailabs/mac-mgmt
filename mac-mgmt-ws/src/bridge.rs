use futures_util::{SinkExt, StreamExt};

/// Bridge two tokio-tungstenite client WebSockets bidirectionally.
///
/// Forwards text, binary, ping, and pong frames. Propagates close frames
/// and sends a close to the other side when one direction finishes.
#[cfg(feature = "client")]
pub async fn client_ws(ws_a: crate::ClientWs, ws_b: crate::ClientWs) {
    use tokio_tungstenite::tungstenite::Message;

    let (mut a_sink, mut a_stream) = ws_a.split();
    let (mut b_sink, mut b_stream) = ws_b.split();

    let a_to_b = async {
        while let Some(msg) = a_stream.next().await {
            match msg {
                Ok(
                    msg @ (Message::Binary(_)
                    | Message::Text(_)
                    | Message::Ping(_)
                    | Message::Pong(_)),
                ) => {
                    if b_sink.send(msg).await.is_err() {
                        break;
                    }
                }
                Ok(Message::Close(frame)) => {
                    let _ = b_sink.send(Message::Close(frame)).await;
                    break;
                }
                Err(_) => {
                    let _ = b_sink.send(Message::Close(None)).await;
                    break;
                }
                _ => {}
            }
        }
    };

    let b_to_a = async {
        while let Some(msg) = b_stream.next().await {
            match msg {
                Ok(
                    msg @ (Message::Binary(_)
                    | Message::Text(_)
                    | Message::Ping(_)
                    | Message::Pong(_)),
                ) => {
                    if a_sink.send(msg).await.is_err() {
                        break;
                    }
                }
                Ok(Message::Close(frame)) => {
                    let _ = a_sink.send(Message::Close(frame)).await;
                    break;
                }
                Err(_) => {
                    let _ = a_sink.send(Message::Close(None)).await;
                    break;
                }
                _ => {}
            }
        }
    };

    tokio::select! {
        _ = a_to_b => {
            let _ = a_sink.send(Message::Close(None)).await;
        }
        _ = b_to_a => {
            let _ = b_sink.send(Message::Close(None)).await;
        }
    }
    tracing::debug!("client ws-ws bridge closed");
}

/// Bridge two axum WebSockets bidirectionally, forwarding close frames.
#[cfg(feature = "axum")]
pub async fn axum_ws(
    ws_a: axum::extract::ws::WebSocket,
    ws_b: axum::extract::ws::WebSocket,
) {
    use axum::extract::ws::Message;

    let (mut a_sink, mut a_stream) = ws_a.split();
    let (mut b_sink, mut b_stream) = ws_b.split();

    let a_to_b = async {
        while let Some(msg) = a_stream.next().await {
            match msg {
                Ok(
                    msg @ (Message::Binary(_)
                    | Message::Text(_)
                    | Message::Ping(_)
                    | Message::Pong(_)),
                ) => {
                    if b_sink.send(msg).await.is_err() {
                        break;
                    }
                }
                Ok(Message::Close(frame)) => {
                    let _ = b_sink.send(Message::Close(frame)).await;
                    break;
                }
                Err(_) => {
                    let _ = b_sink.send(Message::Close(None)).await;
                    break;
                }
            }
        }
    };

    let b_to_a = async {
        while let Some(msg) = b_stream.next().await {
            match msg {
                Ok(
                    msg @ (Message::Binary(_)
                    | Message::Text(_)
                    | Message::Ping(_)
                    | Message::Pong(_)),
                ) => {
                    if a_sink.send(msg).await.is_err() {
                        break;
                    }
                }
                Ok(Message::Close(frame)) => {
                    let _ = a_sink.send(Message::Close(frame)).await;
                    break;
                }
                Err(_) => {
                    let _ = a_sink.send(Message::Close(None)).await;
                    break;
                }
            }
        }
    };

    tokio::select! {
        _ = a_to_b => {
            let _ = a_sink.send(Message::Close(None)).await;
        }
        _ = b_to_a => {
            let _ = b_sink.send(Message::Close(None)).await;
        }
    }
    tracing::debug!("axum ws-ws bridge closed");
}

/// Bridge a TCP stream and an axum WebSocket bidirectionally.
///
/// Returns `(ws_to_tcp_bytes, tcp_to_ws_bytes)`.
#[cfg(feature = "axum")]
pub async fn tcp_ws(
    mut tcp: tokio::net::TcpStream,
    ws: axum::extract::ws::WebSocket,
) -> (u64, u64) {
    use axum::extract::ws::Message;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let (mut ws_sink, mut ws_stream) = ws.split();
    let (mut tcp_read, mut tcp_write) = tcp.split();

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
                    if ws_sink
                        .send(Message::Binary(buf[..n].to_vec().into()))
                        .await
                        .is_err()
                    {
                        tracing::debug!("tcp->ws send failed");
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

    let mut ws_to_tcp_bytes: u64 = 0;
    let mut tcp_to_ws_bytes: u64 = 0;

    tokio::select! {
        n = ws_to_tcp => { ws_to_tcp_bytes = n; }
        n = tcp_to_ws => { tcp_to_ws_bytes = n; }
    }

    tracing::info!(
        "bridge closed (ws->tcp: {ws_to_tcp_bytes}B, tcp->ws: {tcp_to_ws_bytes}B)"
    );

    (ws_to_tcp_bytes, tcp_to_ws_bytes)
}
