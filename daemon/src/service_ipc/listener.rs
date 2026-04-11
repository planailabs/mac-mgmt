use anyhow::{Context, Result};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;

use super::protocol::{IpcMessage, IpcNotification, IpcRequest, IpcResponse};

/// Wrapper-side IPC listener.
///
/// Binds a Unix socket and accepts connections from the main daemon.
/// Incoming requests are forwarded via `request_tx`; the wrapper sends
/// responses and notifications back through the returned handle.
pub struct IpcListener {
    listener: UnixListener,
}

/// A single accepted daemon connection.
pub struct IpcConnection {
    reader: BufReader<tokio::io::ReadHalf<UnixStream>>,
    writer: tokio::io::WriteHalf<UnixStream>,
    line_buf: String,
}

impl IpcListener {
    /// Bind at `socket_path`, removing any stale socket file first.
    pub fn bind(socket_path: &Path) -> Result<Self> {
        if socket_path.exists() {
            // Remove stale socket left by a crashed wrapper.
            std::fs::remove_file(socket_path).ok();
        }
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let listener = UnixListener::bind(socket_path)
            .with_context(|| format!("failed to bind {}", socket_path.display()))?;
        Ok(Self { listener })
    }

    /// Accept the next connection. Returns `None` if the listener is closed.
    pub async fn accept(&self) -> Result<IpcConnection> {
        let (stream, _addr) = self.listener.accept().await.context("accept failed")?;
        let (reader, writer) = tokio::io::split(stream);
        Ok(IpcConnection {
            reader: BufReader::new(reader),
            writer,
            line_buf: String::new(),
        })
    }
}

impl IpcConnection {
    /// Read the next request from the daemon. Returns `None` on EOF.
    pub async fn recv_request(&mut self) -> Result<Option<IpcRequest>> {
        self.line_buf.clear();
        let n = self
            .reader
            .read_line(&mut self.line_buf)
            .await
            .context("read from daemon")?;
        if n == 0 {
            return Ok(None);
        }
        let msg: IpcMessage =
            serde_json::from_str(self.line_buf.trim()).context("invalid IPC message")?;
        match msg {
            IpcMessage::Request(req) => Ok(Some(req)),
            other => {
                tracing::warn!("unexpected IPC message from daemon: {other:?}");
                Ok(None)
            }
        }
    }

    /// Send any IPC message to the daemon.
    async fn send(&mut self, msg: IpcMessage) -> Result<()> {
        let mut line = serde_json::to_string(&msg).context("serialize IPC message")?;
        line.push('\n');
        self.writer.write_all(line.as_bytes()).await.context("write IPC")?;
        self.writer.flush().await.context("flush IPC")?;
        Ok(())
    }

    /// Send a response to the daemon.
    pub async fn send_response(&mut self, resp: IpcResponse) -> Result<()> {
        self.send(IpcMessage::Response(resp)).await
    }

    /// Send an unsolicited notification to the daemon.
    pub async fn send_notification(&mut self, notif: IpcNotification) -> Result<()> {
        self.send(IpcMessage::Notification(notif)).await
    }
}

/// Spawn a task that accepts daemon connections and forwards requests through
/// an mpsc channel. Returns the channel receiver and a handle to send
/// notifications back to the most recent connection.
pub fn spawn_listener(
    listener: IpcListener,
) -> (
    mpsc::Receiver<(IpcRequest, mpsc::Sender<IpcResponse>)>,
    NotificationSender,
) {
    let (req_tx, req_rx) = mpsc::channel::<(IpcRequest, mpsc::Sender<IpcResponse>)>(16);
    let notif_sender = NotificationSender::new();
    let notif_sender_clone = notif_sender.clone();

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok(conn) => {
                    tracing::info!("IPC: daemon connected");
                    let req_tx = req_tx.clone();
                    let notif_sender = notif_sender_clone.clone();
                    tokio::spawn(handle_connection(conn, req_tx, notif_sender));
                }
                Err(e) => {
                    tracing::error!("IPC accept error: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
    });

    (req_rx, notif_sender)
}

async fn handle_connection(
    mut conn: IpcConnection,
    req_tx: mpsc::Sender<(IpcRequest, mpsc::Sender<IpcResponse>)>,
    notif_sender: NotificationSender,
) {
    // Register this connection for notifications.
    let mut notif_rx = notif_sender.subscribe();

    loop {
        tokio::select! {
            result = conn.recv_request() => {
                match result {
                    Ok(Some(req)) => {
                        let (resp_tx, mut resp_rx) = mpsc::channel(1);
                        if req_tx.send((req, resp_tx)).await.is_err() {
                            break; // wrapper shutting down
                        }
                        if let Some(resp) = resp_rx.recv().await {
                            if conn.send_response(resp).await.is_err() {
                                break; // connection lost
                            }
                        }
                    }
                    Ok(None) => {
                        tracing::info!("IPC: daemon disconnected (EOF)");
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("IPC: recv error: {e}");
                        break;
                    }
                }
            }
            Ok(notif) = notif_rx.recv() => {
                if conn.send_notification(notif).await.is_err() {
                    tracing::debug!("IPC: notification send failed, daemon disconnected");
                    break;
                }
            }
        }
    }
}

/// Broadcast handle for sending notifications to all connected daemons.
#[derive(Clone)]
pub struct NotificationSender {
    tx: tokio::sync::broadcast::Sender<IpcNotification>,
}

impl NotificationSender {
    fn new() -> Self {
        let (tx, _) = tokio::sync::broadcast::channel(32);
        Self { tx }
    }

    fn subscribe(&self) -> tokio::sync::broadcast::Receiver<IpcNotification> {
        self.tx.subscribe()
    }

    /// Send a notification to all connected daemons.
    pub fn send(&self, notif: IpcNotification) {
        // Ignore error (no receivers connected).
        let _ = self.tx.send(notif);
    }
}
