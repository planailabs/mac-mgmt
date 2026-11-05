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
//! Frames are read by a dedicated task and handed over a channel. The swarm
//! loop polls `recv()` inside `tokio::select!`, which drops the future
//! whenever another arm wins; reading the socket directly there (two
//! `read_exact`s per frame) loses the bytes of a partly-read frame and
//! desynchronises the stream, after which `call()` waits forever.

use anyhow::Result;
use futures_util::io::{ReadHalf, WriteHalf};
use mac_mgmt_common::framing;
use tokio::sync::mpsc;

/// Protocol for the persistent RPC stream.
pub const RPC_PROTOCOL: libp2p::StreamProtocol = libp2p::StreamProtocol::new("/mac-mgmt/rpc/1.0.0");

/// An incoming RPC message (either a relay-initiated request or a response).
#[derive(Debug)]
pub enum RpcMessage {
    /// A response to a request we sent (matched by id).
    Response { id: u64, payload: serde_json::Value },
    /// An unsolicited request from the relay (id=0).
    Request { payload: serde_json::Value },
}

/// Persistent RPC connection over a libp2p stream.
pub struct RpcStream {
    frames: mpsc::Receiver<std::io::Result<serde_json::Value>>,
    reader: tokio::task::JoinHandle<()>,
    writer: WriteHalf<libp2p::Stream>,
    next_id: u64,
}

impl Drop for RpcStream {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

impl RpcStream {
    /// Wrap a raw libp2p stream into an RPC connection.
    pub fn new(stream: libp2p::Stream) -> Self {
        use futures_util::AsyncReadExt as _;
        let (reader, writer) = stream.split();
        let (tx, frames) = mpsc::channel(32);
        Self {
            frames,
            reader: tokio::spawn(read_frames(reader, tx)),
            writer,
            next_id: 1,
        }
    }

    /// Send a request and wait for the response. Not cancel-safe on the
    /// write side: if the caller times this out, drop the stream.
    pub async fn call(&mut self, mut request: serde_json::Value) -> Result<serde_json::Value> {
        let id = self.next_id;
        self.next_id += 1;
        request["id"] = serde_json::Value::Number(id.into());

        framing::write_lp_json(&mut self.writer, &request).await?;

        loop {
            let frame = self.read_frame().await?;
            if frame["id"].as_u64() == Some(id) {
                return Ok(frame);
            }
            tracing::warn!(frame_id = ?frame["id"], "dropping unrelated relay RPC frame during call()");
        }
    }

    /// Send a request without waiting for the response.
    pub async fn send(&mut self, mut request: serde_json::Value) -> Result<()> {
        let id = self.next_id;
        self.next_id += 1;
        request["id"] = serde_json::Value::Number(id.into());
        Ok(framing::write_lp_json(&mut self.writer, &request).await?)
    }

    /// Read the next incoming message. Cancel-safe.
    pub async fn recv(&mut self) -> Result<RpcMessage> {
        let frame = self.read_frame().await?;
        Ok(match frame["id"].as_u64().unwrap_or(0) {
            0 => RpcMessage::Request { payload: frame },
            id => RpcMessage::Response { id, payload: frame },
        })
    }

    async fn read_frame(&mut self) -> Result<serde_json::Value> {
        match self.frames.recv().await {
            Some(frame) => Ok(frame?),
            None => anyhow::bail!("RPC reader stopped"),
        }
    }
}

async fn read_frames(
    mut reader: ReadHalf<libp2p::Stream>,
    tx: mpsc::Sender<std::io::Result<serde_json::Value>>,
) {
    loop {
        let frame = framing::read_lp_json(&mut reader).await;
        let failed = frame.is_err();
        if tx.send(frame).await.is_err() || failed {
            return;
        }
    }
}
