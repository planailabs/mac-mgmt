//! Tunnel data stream protocol — raw bidirectional byte streams for
//! SSH sessions, file transfers, and shell commands.
//!
//! Each tunnel session opens a new libp2p substream. The first message
//! is a handshake containing the session ID and secret; after
//! validation the stream carries raw bytes in both directions.

use futures_util::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use libp2p::StreamProtocol;
use serde::{Deserialize, Serialize};

pub const PROTOCOL_NAME: StreamProtocol = StreamProtocol::new("/mac-mgmt/tunnel/1.0.0");

/// Handshake sent at the beginning of every tunnel substream.
#[derive(Debug, Serialize, Deserialize)]
pub struct TunnelHandshake {
    pub session_id: String,
    pub session_secret: String,
    /// What kind of tunnel this is (ssh, file, shell, proxy).
    pub kind: TunnelKind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TunnelKind {
    Ssh,
    File,
    Shell,
    Proxy,
}

/// Send the handshake on a new tunnel substream.
pub async fn write_handshake<T>(io: &mut T, hs: &TunnelHandshake) -> std::io::Result<()>
where
    T: AsyncWrite + Unpin + Send,
{
    let data = serde_json::to_vec(hs)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let len = data.len() as u32;
    io.write_all(&len.to_be_bytes()).await?;
    io.write_all(&data).await?;
    Ok(())
}

/// Read the handshake from a tunnel substream.
pub async fn read_handshake<T>(io: &mut T) -> std::io::Result<TunnelHandshake>
where
    T: AsyncRead + Unpin + Send,
{
    let mut len_buf = [0u8; 4];
    io.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf);
    if len > 4096 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "tunnel handshake too large",
        ));
    }
    let mut buf = vec![0u8; len as usize];
    io.read_exact(&mut buf).await?;
    serde_json::from_slice(&buf)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}
