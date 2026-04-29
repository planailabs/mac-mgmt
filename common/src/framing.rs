//! Shared wire framing primitives for libp2p tunnel substreams and RPC.
//!
//! Two protocols use these primitives:
//!
//! ## Tagged framing (tunnel data)
//! Each message is: `[1-byte tag] [4-byte BE length] [payload]`
//! - `TAG_JSON` (0x01): JSON text payload
//! - `TAG_BINARY` (0x02): raw binary payload
//! - `TAG_END` (0x03): end-of-stream marker (no length/payload)
//!
//! ## Length-prefixed framing (RPC)
//! Each message is: `[4-byte BE length] [JSON payload]`
//! Used by the persistent RPC stream between daemon and relay.

use futures_util::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use std::io;

// ── Constants ────────────────────────────────────────────────────────

pub const TAG_JSON: u8 = 0x01;
pub const TAG_BINARY: u8 = 0x02;
pub const TAG_END: u8 = 0x03;

/// Maximum frame payload: 16 MiB.
pub const MAX_FRAME_SIZE: u32 = 16 * 1024 * 1024;

// ── Tagged framing (tunnel data streams) ─────────────────────────────

/// A tagged frame read from a stream.
#[derive(Debug)]
pub enum TaggedFrame {
    Json(serde_json::Value),
    Binary(Vec<u8>),
    End,
}

/// Read one tagged frame. Returns `None` on EOF.
pub async fn read_tagged_frame<T>(io: &mut T) -> io::Result<Option<TaggedFrame>>
where
    T: AsyncRead + Unpin + Send,
{
    let mut tag_buf = [0u8; 1];
    match io.read_exact(&mut tag_buf).await {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }

    match tag_buf[0] {
        TAG_END => Ok(Some(TaggedFrame::End)),
        TAG_JSON => {
            let payload = read_payload(io).await?;
            let val = serde_json::from_slice(&payload)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            Ok(Some(TaggedFrame::Json(val)))
        }
        TAG_BINARY => {
            let payload = read_payload(io).await?;
            Ok(Some(TaggedFrame::Binary(payload)))
        }
        other => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unknown frame tag: 0x{other:02x}"),
        )),
    }
}

/// Write a JSON tagged frame.
pub async fn write_json<T>(io: &mut T, val: &serde_json::Value) -> io::Result<()>
where
    T: AsyncWrite + Unpin + Send,
{
    let data = serde_json::to_vec(val)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    write_tagged(io, TAG_JSON, &data).await
}

/// Write a binary tagged frame.
pub async fn write_binary<T>(io: &mut T, data: &[u8]) -> io::Result<()>
where
    T: AsyncWrite + Unpin + Send,
{
    write_tagged(io, TAG_BINARY, data).await
}

/// Write the end-of-stream marker.
pub async fn write_end<T>(io: &mut T) -> io::Result<()>
where
    T: AsyncWrite + Unpin + Send,
{
    io.write_all(&[TAG_END]).await
}

// ── Length-prefixed framing (RPC) ────────────────────────────────────

/// Read a length-prefixed JSON value.
pub async fn read_lp_json<T>(io: &mut T) -> io::Result<serde_json::Value>
where
    T: AsyncRead + Unpin + Send,
{
    let payload = read_payload(io).await?;
    serde_json::from_slice(&payload)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Write a length-prefixed JSON value.
pub async fn write_lp_json<T>(io: &mut T, val: &serde_json::Value) -> io::Result<()>
where
    T: AsyncWrite + Unpin + Send,
{
    let data = serde_json::to_vec(val)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    if data.len() > MAX_FRAME_SIZE as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame too large: {} bytes", data.len()),
        ));
    }
    io.write_all(&(data.len() as u32).to_be_bytes()).await?;
    io.write_all(&data).await?;
    io.flush().await?;
    Ok(())
}

// ── Internal ─────────────────────────────────────────────────────────

async fn read_payload<T>(io: &mut T) -> io::Result<Vec<u8>>
where
    T: AsyncRead + Unpin + Send,
{
    let mut len_buf = [0u8; 4];
    io.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf);
    if len > MAX_FRAME_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame too large: {len} bytes"),
        ));
    }
    let mut buf = vec![0u8; len as usize];
    io.read_exact(&mut buf).await?;
    Ok(buf)
}

async fn write_tagged<T>(io: &mut T, tag: u8, data: &[u8]) -> io::Result<()>
where
    T: AsyncWrite + Unpin + Send,
{
    io.write_all(&[tag]).await?;
    io.write_all(&(data.len() as u32).to_be_bytes()).await?;
    io.write_all(data).await?;
    Ok(())
}
