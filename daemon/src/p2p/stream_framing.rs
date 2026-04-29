//! Length-prefixed message framing for tunnel data sessions.
//!
//! Wire format per message:
//! ```text
//! [1 byte tag] [4 bytes big-endian length] [length bytes payload]
//! ```
//!
//! Tags:
//! - `0x01` — JSON text message
//! - `0x02` — binary data chunk
//! - `0x03` — end-of-stream (no payload)

use futures_util::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use std::io;

const TAG_JSON: u8 = 0x01;
const TAG_BINARY: u8 = 0x02;
const TAG_END: u8 = 0x03;

/// Maximum frame payload: 16 MiB.
const MAX_PAYLOAD: u32 = 16 * 1024 * 1024;

/// A framed message read from a stream.
#[derive(Debug)]
pub enum FrameMsg {
    /// A JSON text message.
    Json(serde_json::Value),
    /// A binary data chunk.
    Binary(Vec<u8>),
    /// End-of-stream marker.
    End,
}

/// Read one framed message from a stream. Returns `None` on EOF.
pub async fn read_frame<T>(io: &mut T) -> io::Result<Option<FrameMsg>>
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
        TAG_END => Ok(Some(FrameMsg::End)),
        TAG_JSON => {
            let payload = read_payload(io).await?;
            let val = serde_json::from_slice(&payload)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            Ok(Some(FrameMsg::Json(val)))
        }
        TAG_BINARY => {
            let payload = read_payload(io).await?;
            Ok(Some(FrameMsg::Binary(payload)))
        }
        other => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unknown frame tag: 0x{other:02x}"),
        )),
    }
}

/// Write a JSON text message.
pub async fn write_json<T>(io: &mut T, val: &serde_json::Value) -> io::Result<()>
where
    T: AsyncWrite + Unpin + Send,
{
    let data = serde_json::to_vec(val)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    write_frame(io, TAG_JSON, &data).await
}

/// Write a binary data chunk.
pub async fn write_binary<T>(io: &mut T, data: &[u8]) -> io::Result<()>
where
    T: AsyncWrite + Unpin + Send,
{
    write_frame(io, TAG_BINARY, data).await
}

/// Write the end-of-stream marker.
pub async fn write_end<T>(io: &mut T) -> io::Result<()>
where
    T: AsyncWrite + Unpin + Send,
{
    io.write_all(&[TAG_END]).await
}

async fn read_payload<T>(io: &mut T) -> io::Result<Vec<u8>>
where
    T: AsyncRead + Unpin + Send,
{
    let mut len_buf = [0u8; 4];
    io.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf);
    if len > MAX_PAYLOAD {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame too large: {len} bytes"),
        ));
    }
    let mut buf = vec![0u8; len as usize];
    io.read_exact(&mut buf).await?;
    Ok(buf)
}

async fn write_frame<T>(io: &mut T, tag: u8, data: &[u8]) -> io::Result<()>
where
    T: AsyncWrite + Unpin + Send,
{
    io.write_all(&[tag]).await?;
    io.write_all(&(data.len() as u32).to_be_bytes()).await?;
    io.write_all(data).await?;
    Ok(())
}
