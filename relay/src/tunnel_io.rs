//! Shared helpers for opening tunnel substreams and reading framed responses.
//!
//! Uses the framing primitives from `mac_mgmt_common::framing`.

use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use mac_mgmt_common::framing;

use crate::p2p::RelaySwarm;

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

    framing::write_lp_json(&mut tunnel, handshake)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    Ok(tunnel)
}

/// Read one JSON frame (tag 0x01 + length-prefixed payload).
pub async fn read_json_frame(tunnel: &mut libp2p::Stream) -> Result<serde_json::Value, StatusCode> {
    match framing::read_tagged_frame(tunnel).await {
        Ok(Some(framing::TaggedFrame::Json(val))) => Ok(val),
        _ => Err(StatusCode::BAD_GATEWAY),
    }
}

/// Read binary body chunks (tag 0x02) until end marker (tag 0x03).
pub async fn read_binary_body(tunnel: &mut libp2p::Stream) -> Vec<u8> {
    let mut body = Vec::new();
    loop {
        match framing::read_tagged_frame(tunnel).await {
            Ok(Some(framing::TaggedFrame::Binary(data))) => body.extend_from_slice(&data),
            Ok(Some(framing::TaggedFrame::End)) | Ok(None) => break,
            _ => break,
        }
    }
    body
}

/// Open tunnel, send handshake, read a single JSON response frame.
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

/// Async stream of binary body chunks (tag 0x02) until end marker/EOF.
/// Unlike [`read_binary_body`] this doesn't buffer: chunks are yielded as
/// they arrive, which is required for SSE and other unbounded responses.
pub fn read_binary_chunks_stream(
    mut tunnel: libp2p::Stream,
) -> impl futures_util::Stream<Item = Result<Vec<u8>, std::io::Error>> {
    async_stream::stream! {
        loop {
            match framing::read_tagged_frame(&mut tunnel).await {
                Ok(Some(framing::TaggedFrame::Binary(data))) => yield Ok(data),
                Ok(Some(framing::TaggedFrame::End)) | Ok(None) => break,
                Ok(Some(framing::TaggedFrame::Json(_))) => continue,
                Err(e) => { yield Err(e); break; }
            }
        }
    }
}

/// Async stream of JSON frames from a tunnel substream (for SSE).
pub fn read_json_frames_stream(
    mut tunnel: libp2p::Stream,
) -> impl futures_util::Stream<Item = Result<serde_json::Value, std::io::Error>> {
    async_stream::stream! {
        loop {
            match framing::read_tagged_frame(&mut tunnel).await {
                Ok(Some(framing::TaggedFrame::Json(val))) => yield Ok(val),
                Ok(Some(framing::TaggedFrame::End)) | Ok(None) => break,
                Ok(Some(framing::TaggedFrame::Binary(_))) => continue,
                Err(e) => { yield Err(e); break; }
            }
        }
    }
}
