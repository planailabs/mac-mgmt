//! AI proxy distribution protocol — forwards OpenAI-compatible requests
//! to cluster peers that have available backends.

use async_trait::async_trait;
use futures_util::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use libp2p::request_response;
use libp2p::StreamProtocol;
use serde::{Deserialize, Serialize};

pub const PROTOCOL_NAME: StreamProtocol = StreamProtocol::new("/mac-mgmt/ai-proxy/1.0.0");

// ── Request / Response ───────────────────────────────────────────────

/// An AI proxy request forwarded to a peer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProxyRequest {
    /// The raw JSON body of the OpenAI chat completion request.
    pub body: serde_json::Value,
    /// Whether the caller wants streaming SSE.
    pub stream: bool,
}

/// A non-streaming AI proxy response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProxyResponse {
    pub status: u16,
    pub body: serde_json::Value,
}

// ── Codec ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct AiProxyCodec;

/// Max AI proxy message: 32 MiB (large context windows).
const MAX_MSG_SIZE: u64 = 32 * 1024 * 1024;

#[async_trait]
impl request_response::Codec for AiProxyCodec {
    type Protocol = StreamProtocol;
    type Request = AiProxyRequest;
    type Response = AiProxyResponse;

    async fn read_request<T>(&mut self, _: &StreamProtocol, io: &mut T) -> std::io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        read_lp_json(io).await
    }

    async fn read_response<T>(&mut self, _: &StreamProtocol, io: &mut T) -> std::io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        read_lp_json(io).await
    }

    async fn write_request<T>(&mut self, _: &StreamProtocol, io: &mut T, req: Self::Request) -> std::io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_lp_json(io, &req).await
    }

    async fn write_response<T>(&mut self, _: &StreamProtocol, io: &mut T, resp: Self::Response) -> std::io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_lp_json(io, &resp).await
    }
}

async fn read_lp_json<T, M>(io: &mut T) -> std::io::Result<M>
where
    T: AsyncRead + Unpin + Send,
    M: serde::de::DeserializeOwned,
{
    let mut len_buf = [0u8; 4];
    io.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as u64;
    if len > MAX_MSG_SIZE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("ai-proxy message too large: {len} bytes"),
        ));
    }
    let mut buf = vec![0u8; len as usize];
    io.read_exact(&mut buf).await?;
    serde_json::from_slice(&buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

async fn write_lp_json<T, M>(io: &mut T, msg: &M) -> std::io::Result<()>
where
    T: AsyncWrite + Unpin + Send,
    M: serde::Serialize,
{
    let data = serde_json::to_vec(msg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let len = data.len() as u32;
    io.write_all(&len.to_be_bytes()).await?;
    io.write_all(&data).await?;
    io.close().await?;
    Ok(())
}
