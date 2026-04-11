use anyhow::{Context, Result};
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc;

use super::protocol::{IpcMessage, IpcNotification, IpcRequest, IpcResponse};

/// Managed connection to a single service wrapper. Spawns a background reader
/// task that forwards notifications through an mpsc channel.
pub struct ManagedClient {
    writer: tokio::io::WriteHalf<UnixStream>,
    notification_rx: mpsc::Receiver<IpcNotification>,
    response_rx: mpsc::Receiver<IpcResponse>,
    line_buf: String,
}

impl ManagedClient {
    /// Connect to a wrapper and spawn a background reader task.
    pub async fn connect(path: &Path, timeout: Duration) -> Result<Self> {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut delay = Duration::from_millis(200);

        let stream = loop {
            match UnixStream::connect(path).await {
                Ok(s) => break s,
                Err(e) => {
                    if tokio::time::Instant::now() + delay > deadline {
                        return Err(e)
                            .with_context(|| format!("connect to {}", path.display()));
                    }
                    tokio::time::sleep(delay).await;
                    delay = (delay * 2).min(Duration::from_secs(5));
                }
            }
        };

        let (reader, writer) = tokio::io::split(stream);
        let (notif_tx, notification_rx) = mpsc::channel(32);
        let (resp_tx, response_rx) = mpsc::channel(8);

        // Background reader task.
        tokio::spawn(async move {
            let mut reader = BufReader::new(reader);
            let mut buf = String::new();
            loop {
                buf.clear();
                match reader.read_line(&mut buf).await {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let msg: IpcMessage = match serde_json::from_str(buf.trim()) {
                            Ok(m) => m,
                            Err(e) => {
                                tracing::warn!("invalid IPC message from wrapper: {e}");
                                continue;
                            }
                        };
                        match msg {
                            IpcMessage::Notification(n) => {
                                if notif_tx.send(n).await.is_err() {
                                    break;
                                }
                            }
                            IpcMessage::Response(r) => {
                                if resp_tx.send(r).await.is_err() {
                                    break;
                                }
                            }
                            IpcMessage::Request(_) => {
                                tracing::warn!("unexpected request from wrapper");
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("IPC read error: {e}");
                        break;
                    }
                }
            }
        });

        Ok(Self {
            writer,
            notification_rx,
            response_rx,
            line_buf: String::new(),
        })
    }

    /// Send a request and wait for the response.
    pub async fn request(&mut self, req: &IpcRequest) -> Result<IpcResponse> {
        let msg = IpcMessage::Request(req.clone());
        self.line_buf.clear();
        self.line_buf = serde_json::to_string(&msg).context("serialize request")?;
        self.line_buf.push('\n');
        self.writer
            .write_all(self.line_buf.as_bytes())
            .await
            .context("write request")?;
        self.writer.flush().await.context("flush request")?;

        self.response_rx
            .recv()
            .await
            .context("wrapper closed connection")
    }

    /// Check if a notification is pending without blocking.
    pub fn try_recv_notification(&mut self) -> Option<IpcNotification> {
        self.notification_rx.try_recv().ok()
    }
}
