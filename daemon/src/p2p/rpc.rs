//! Persistent bidirectional RPC over a single libp2p-stream substream.
//!
//! Wire format: length-prefixed JSON frames.
//! ```text
//! [4-byte big-endian length] [JSON payload]
//! ```
//!
//! Every request from the daemon carries an incrementing `id` field.
//! The relay echoes the same `id` in its response. Relay-initiated
//! (unsolicited) messages use `id: 0`.
//!
//! Both sides can send at any time (full duplex via tokio::select on
//! the split read/write halves).

use anyhow::{Result, bail};
use futures_util::{AsyncReadExt, AsyncWriteExt};
use std::collections::HashMap;
use tokio::sync::oneshot;

/// Maximum RPC frame payload: 16 MiB.
const MAX_FRAME_SIZE: u32 = 16 * 1024 * 1024;

/// Protocol for the persistent RPC stream.
pub const RPC_PROTOCOL: libp2p::StreamProtocol =
    libp2p::StreamProtocol::new("/mac-mgmt/rpc/1.0.0");

/// An incoming RPC message (either a relay-initiated request or a response).
#[derive(Debug)]
pub enum RpcMessage {
    /// A response to a request we sent (matched by id).
    Response {
        id: u64,
        payload: serde_json::Value,
    },
    /// An unsolicited request from the relay (id=0).
    Request {
        payload: serde_json::Value,
    },
}

/// Persistent RPC connection over a libp2p stream.
///
/// The stream is split into read/write halves so the swarm loop can
/// poll for incoming messages while the state machine sends outbound
/// requests.
pub struct RpcStream {
    reader: futures_util::io::ReadHalf<libp2p::Stream>,
    writer: futures_util::io::WriteHalf<libp2p::Stream>,
    next_id: u64,
    pending: HashMap<u64, oneshot::Sender<serde_json::Value>>,
}

impl RpcStream {
    /// Wrap a raw libp2p stream into an RPC connection.
    pub fn new(stream: libp2p::Stream) -> Self {
        use futures_util::AsyncReadExt as _;
        let (reader, writer) = stream.split();
        Self {
            reader,
            writer,
            next_id: 1,
            pending: HashMap::new(),
        }
    }

    /// Send a request and wait for the response.
    pub async fn call(
        &mut self,
        mut request: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let id = self.next_id;
        self.next_id += 1;
        request["id"] = serde_json::Value::Number(id.into());

        self.write_frame(&request).await?;

        // Poll incoming frames until we get our response.
        loop {
            let frame = self.read_frame().await?;
            let frame_id = frame["id"].as_u64().unwrap_or(0);
            if frame_id == id {
                // Our response — remove from pending.
                self.pending.remove(&id);
                return Ok(frame);
            } else if frame_id == 0 {
                // Unsolicited relay request — can't handle here in call().
                // Drop it (the swarm loop should be polling recv() concurrently).
                tracing::warn!("dropping unsolicited relay RPC during call()");
            } else if let Some(tx) = self.pending.remove(&frame_id) {
                // Response for a different pending call.
                let _ = tx.send(frame);
            }
        }
    }

    /// Send a request without waiting for the response.
    /// Logs errors but doesn't fail.
    pub async fn send(&mut self, mut request: serde_json::Value) -> Result<()> {
        let id = self.next_id;
        self.next_id += 1;
        request["id"] = serde_json::Value::Number(id.into());
        self.write_frame(&request).await
    }

    /// Read the next incoming message.
    ///
    /// Returns `RpcMessage::Response` for responses to our requests
    /// (dispatches to pending callers), or `RpcMessage::Request` for
    /// unsolicited relay-initiated messages.
    pub async fn recv(&mut self) -> Result<RpcMessage> {
        loop {
            let frame = self.read_frame().await?;
            let id = frame["id"].as_u64().unwrap_or(0);

            if id == 0 {
                return Ok(RpcMessage::Request { payload: frame });
            }

            // It's a response — dispatch to the pending caller if any.
            if let Some(tx) = self.pending.remove(&id) {
                let _ = tx.send(frame.clone());
                return Ok(RpcMessage::Response { id, payload: frame });
            }

            // No pending caller — return as response anyway.
            return Ok(RpcMessage::Response { id, payload: frame });
        }
    }

    /// Write a single length-prefixed JSON frame.
    async fn write_frame(&mut self, value: &serde_json::Value) -> Result<()> {
        let data = serde_json::to_vec(value)?;
        if data.len() > MAX_FRAME_SIZE as usize {
            bail!("RPC frame too large: {} bytes", data.len());
        }
        self.writer
            .write_all(&(data.len() as u32).to_be_bytes())
            .await?;
        self.writer.write_all(&data).await?;
        self.writer.flush().await?;
        Ok(())
    }

    /// Read a single length-prefixed JSON frame.
    async fn read_frame(&mut self) -> Result<serde_json::Value> {
        let mut len_buf = [0u8; 4];
        self.reader.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf);
        if len > MAX_FRAME_SIZE {
            bail!("RPC frame too large: {len} bytes");
        }
        let mut buf = vec![0u8; len as usize];
        self.reader.read_exact(&mut buf).await?;
        Ok(serde_json::from_slice(&buf)?)
    }
}
