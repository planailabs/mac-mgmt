//! Control protocol — request-response for session, metrics, proxy,
//! file and shell requests between relay↔daemon and daemon↔daemon.

use async_trait::async_trait;
use futures_util::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use libp2p::request_response;
use libp2p::StreamProtocol;
use serde::{Deserialize, Serialize};

/// Protocol identifier for the control channel.
pub const PROTOCOL_NAME: StreamProtocol = StreamProtocol::new("/mac-mgmt/control/1.0.0");

// ── Request types ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlRequest {
    SessionRequest {
        session_id: String,
        session_secret: String,
    },
    MetricsRequest {
        request_id: String,
        path: String,
    },
    ProxyRequest {
        request_id: String,
        tunnel_name: String,
        method: String,
        path: String,
        headers: Vec<(String, String)>,
        body: Option<String>,
    },
    ProxyStreamRequest {
        request_id: String,
        tunnel_name: String,
        method: String,
        path: String,
        headers: serde_json::Value,
        body: Option<String>,
    },
    ProxySessionRequest {
        session_id: String,
        session_secret: String,
        tunnel_name: String,
        mode: String,
        path: String,
    },
    FileListRequest {
        request_id: String,
        tunnel_name: String,
        path: Option<String>,
    },
    FileSessionRequest {
        session_id: String,
        session_secret: String,
        tunnel_name: String,
        mode: String,
        path: Option<String>,
        expected_mtime: Option<i64>,
    },
    ShellSessionRequest {
        session_id: String,
        session_secret: String,
        command_name: String,
        user_arg: Option<String>,
    },
    /// Register with the relay node.
    Register {
        instance_id: String,
        cluster_id: Option<String>,
        hostname: Option<String>,
        agent_name: Option<String>,
    },
    /// Advertise tunnel definitions to the relay.
    TunnelAdvertisement {
        tunnels: serde_json::Value,
        file_tunnels: serde_json::Value,
        shell_tunnels: serde_json::Value,
    },
}

// ── Response types ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlResponse {
    Ok,
    Error {
        message: String,
    },
    MetricsResponse {
        request_id: String,
        body: String,
    },
    ProxyResponse {
        request_id: String,
        status: u16,
        headers: Vec<(String, String)>,
        body: String,
    },
    ProxyStreamHeaders {
        request_id: String,
        status: u16,
        headers: Vec<(String, String)>,
    },
    ProxyStreamChunk {
        request_id: String,
        data: String,
    },
    ProxyStreamEnd {
        request_id: String,
    },
    FileResponse {
        request_id: String,
        data: serde_json::Value,
    },
}

// ── Codec ────────────────────────────────────────────────────────────

/// Length-prefixed JSON codec for control messages.
#[derive(Debug, Clone, Default)]
pub struct ControlCodec;

/// Maximum control message size: 16 MiB (matches old WS limit).
const MAX_MSG_SIZE: u64 = 16 * 1024 * 1024;

#[async_trait]
impl request_response::Codec for ControlCodec {
    type Protocol = StreamProtocol;
    type Request = ControlRequest;
    type Response = ControlResponse;

    async fn read_request<T>(&mut self, _: &StreamProtocol, io: &mut T) -> std::io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        read_length_prefixed_json(io).await
    }

    async fn read_response<T>(&mut self, _: &StreamProtocol, io: &mut T) -> std::io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        read_length_prefixed_json(io).await
    }

    async fn write_request<T>(&mut self, _: &StreamProtocol, io: &mut T, req: Self::Request) -> std::io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_length_prefixed_json(io, &req).await
    }

    async fn write_response<T>(&mut self, _: &StreamProtocol, io: &mut T, resp: Self::Response) -> std::io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_length_prefixed_json(io, &resp).await
    }
}

// ── Wire helpers ─────────────────────────────────────────────────────

async fn read_length_prefixed_json<T, M>(io: &mut T) -> std::io::Result<M>
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
            format!("message too large: {len} bytes"),
        ));
    }
    let mut buf = vec![0u8; len as usize];
    io.read_exact(&mut buf).await?;
    serde_json::from_slice(&buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

async fn write_length_prefixed_json<T, M>(io: &mut T, msg: &M) -> std::io::Result<()>
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
