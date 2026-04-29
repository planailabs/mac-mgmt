//! Shared helpers for opening tunnel substreams and reading framed responses.
//!
//! Used by both proxy_handler.rs and api.rs to avoid duplicating the
//! stream_framing read/write protocol (tag + length-prefixed frames).

use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use futures_util::{AsyncReadExt, AsyncWriteExt};

use crate::p2p::RelaySwarm;

const MAX_FRAME: usize = 16 * 1024 * 1024;

/// Open a tunnel substream to a peer, send a JSON handshake, return the stream.
pub async fn open(
    swarm: &Arc<RelaySwarm>,
    peer_id: libp2p::PeerId,
    handshake: &serde_json::Value,
    timeout: Duration,
) -> Result<libp2p::Stream, StatusCode> {
    let mut tunnel = tokio::time::timeout(timeout, swarm.open_tunnel_stream(peer_id))
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    let data = serde_json::to_vec(handshake).unwrap_or_default();
    tunnel
        .write_all(&(data.len() as u32).to_be_bytes())
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    tunnel
        .write_all(&data)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    tunnel.flush().await.map_err(|_| StatusCode::BAD_GATEWAY)?;

    Ok(tunnel)
}

/// Read one JSON frame (tag 0x01 + length-prefixed payload).
pub async fn read_json_frame(
    tunnel: &mut libp2p::Stream,
) -> Result<serde_json::Value, StatusCode> {
    let mut tag = [0u8; 1];
    tunnel
        .read_exact(&mut tag)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    if tag[0] != 0x01 {
        return Err(StatusCode::BAD_GATEWAY);
    }
    let mut len_buf = [0u8; 4];
    tunnel
        .read_exact(&mut len_buf)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME {
        return Err(StatusCode::BAD_GATEWAY);
    }
    let mut buf = vec![0u8; len];
    tunnel
        .read_exact(&mut buf)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    serde_json::from_slice(&buf).map_err(|_| StatusCode::BAD_GATEWAY)
}

/// Read binary body chunks (tag 0x02) until end marker (tag 0x03).
pub async fn read_binary_body(tunnel: &mut libp2p::Stream) -> Vec<u8> {
    let mut body = Vec::new();
    loop {
        let mut tag = [0u8; 1];
        if tunnel.read_exact(&mut tag).await.is_err() {
            break;
        }
        match tag[0] {
            0x02 => {
                let mut len_buf = [0u8; 4];
                if tunnel.read_exact(&mut len_buf).await.is_err() {
                    break;
                }
                let chunk_len = u32::from_be_bytes(len_buf) as usize;
                if chunk_len > MAX_FRAME {
                    break;
                }
                let mut chunk = vec![0u8; chunk_len];
                if tunnel.read_exact(&mut chunk).await.is_err() {
                    break;
                }
                body.extend_from_slice(&chunk);
            }
            0x03 => break,
            _ => break,
        }
    }
    body
}

/// Open tunnel, send handshake, read a single JSON response frame + end.
pub async fn open_and_read_json(
    swarm: &Arc<RelaySwarm>,
    peer_id: libp2p::PeerId,
    handshake: serde_json::Value,
    timeout: Duration,
) -> Result<serde_json::Value, StatusCode> {
    let mut tunnel = open(swarm, peer_id, &handshake, timeout).await?;
    read_json_frame(&mut tunnel).await
}

/// Open tunnel, send handshake, read JSON header + binary body.
/// Returns (status, content_type, body_string).
pub async fn open_and_read_response(
    swarm: &Arc<RelaySwarm>,
    peer_id: libp2p::PeerId,
    handshake: serde_json::Value,
    timeout: Duration,
) -> Result<(u16, String, String), StatusCode> {
    let mut tunnel = open(swarm, peer_id, &handshake, timeout).await?;
    let hdr = read_json_frame(&mut tunnel).await?;

    let status = hdr["status"].as_u64().unwrap_or(502) as u16;
    let content_type = hdr["content_type"]
        .as_str()
        .unwrap_or("text/plain")
        .to_string();

    let body_data = read_binary_body(&mut tunnel).await;
    let body = String::from_utf8_lossy(&body_data).to_string();

    Ok((status, content_type, body))
}

/// Read SSE-style framed JSON messages (tag 0x01 frames until 0x03 end).
/// Returns each frame as it arrives via an async stream, suitable for
/// axum SSE responses.
pub fn read_json_frames_stream(
    mut tunnel: libp2p::Stream,
) -> impl futures_util::Stream<Item = Result<serde_json::Value, std::io::Error>> {
    async_stream::stream! {
        loop {
            let mut tag = [0u8; 1];
            if tunnel.read_exact(&mut tag).await.is_err() { break; }
            match tag[0] {
                0x01 => {
                    let mut len_buf = [0u8; 4];
                    if tunnel.read_exact(&mut len_buf).await.is_err() { break; }
                    let len = u32::from_be_bytes(len_buf) as usize;
                    if len > MAX_FRAME { break; }
                    let mut buf = vec![0u8; len];
                    if tunnel.read_exact(&mut buf).await.is_err() { break; }
                    match serde_json::from_slice(&buf) {
                        Ok(val) => yield Ok(val),
                        Err(e) => yield Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
                    }
                }
                0x03 => break,
                _ => break,
            }
        }
    }
}
