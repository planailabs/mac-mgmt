use anyhow::{Context, Result};
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use crate::transport::{self, IpcStream};

use crate::protocol::{Message, Notification, Request, Response, ServiceStatus, SpawnSpec};

/// Daemon-side RPC client. Spawns a background reader that forwards responses
/// and notifications through bounded channels.
pub struct Client {
    writer: tokio::io::WriteHalf<IpcStream>,
    notif_rx: mpsc::Receiver<Notification>,
    resp_rx: mpsc::Receiver<Response>,
}

impl Client {
    /// Connect to the supervisor, retrying with backoff until `timeout` elapses.
    pub async fn connect(path: &Path, timeout: Duration) -> Result<Self> {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut delay = Duration::from_millis(200);

        let stream = loop {
            match transport::connect(path).await {
                Ok(s) => break s,
                Err(e) => {
                    if tokio::time::Instant::now() + delay > deadline {
                        return Err(e).with_context(|| format!("connect to {}", path.display()));
                    }
                    tokio::time::sleep(delay).await;
                    delay = (delay * 2).min(Duration::from_secs(5));
                }
            }
        };

        let (reader, writer) = tokio::io::split(stream);
        let (notif_tx, notif_rx) = mpsc::channel(256);
        let (resp_tx, resp_rx) = mpsc::channel(8);

        tokio::spawn(async move {
            let mut reader = BufReader::new(reader);
            let mut buf = String::new();
            loop {
                buf.clear();
                match reader.read_line(&mut buf).await {
                    Ok(0) => break,
                    Ok(_) => {
                        let msg: Message = match serde_json::from_str(buf.trim()) {
                            Ok(m) => m,
                            Err(e) => {
                                tracing::warn!("services RPC: invalid message: {e}");
                                continue;
                            }
                        };
                        match msg {
                            Message::Notification(n) => {
                                if notif_tx.send(n).await.is_err() {
                                    break;
                                }
                            }
                            Message::Response(r) => {
                                if resp_tx.send(r).await.is_err() {
                                    break;
                                }
                            }
                            Message::Request(_) => {
                                tracing::warn!("services RPC: unexpected request from supervisor");
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("services RPC: read error: {e}");
                        break;
                    }
                }
            }
        });

        Ok(Self {
            writer,
            notif_rx,
            resp_rx,
        })
    }

    /// Timeout for individual RPC calls. Prevents a single slow supervisor
    /// response from blocking the entire health tick (which would stop phase
    /// transitions and health checks from running).
    const RPC_TIMEOUT: Duration = Duration::from_secs(10);

    async fn send(&mut self, req: Request) -> Result<Response> {
        let mut line = serde_json::to_string(&Message::Request(req)).context("serialize")?;
        line.push('\n');
        self.writer
            .write_all(line.as_bytes())
            .await
            .context("write")?;
        self.writer.flush().await.context("flush")?;
        tokio::time::timeout(Self::RPC_TIMEOUT, self.resp_rx.recv())
            .await
            .map_err(|_| {
                anyhow::anyhow!(
                    "supervisor RPC timed out ({}s)",
                    Self::RPC_TIMEOUT.as_secs()
                )
            })?
            .context("supervisor closed connection")
    }

    pub async fn register(&mut self, name: &str, spec: SpawnSpec) -> Result<()> {
        match self
            .send(Request::Register {
                name: name.to_string(),
                spec,
            })
            .await?
        {
            Response::Ok => Ok(()),
            Response::Error { message } => anyhow::bail!("register {name}: {message}"),
            other => anyhow::bail!("register {name}: unexpected response {other:?}"),
        }
    }

    pub async fn unregister(&mut self, name: &str) -> Result<()> {
        match self
            .send(Request::Unregister {
                name: name.to_string(),
            })
            .await?
        {
            Response::Ok => Ok(()),
            Response::Error { message } => anyhow::bail!("unregister {name}: {message}"),
            other => anyhow::bail!("unregister {name}: unexpected response {other:?}"),
        }
    }

    /// Ask the supervisor for the list of registered services.
    ///
    /// Prefers the `statuses` field (pid/exe included); falls back to the
    /// legacy `names` list when talking to an older supervisor.
    // compat: added 2026-04-15, removable after 2026-07-15 (drop the names
    // fallback and return `statuses` directly).
    pub async fn list(&mut self) -> Result<Vec<ServiceStatus>> {
        match self.send(Request::List).await? {
            Response::Services { statuses, names } => {
                if !statuses.is_empty() || names.is_empty() {
                    Ok(statuses)
                } else {
                    Ok(names
                        .into_iter()
                        .map(|name| ServiceStatus {
                            name,
                            pid: None,
                            exe: None,
                            resolved_program: None,
                            spec: None,
                            stopped: false,
                        })
                        .collect())
                }
            }
            Response::Error { message } => anyhow::bail!("list: {message}"),
            other => anyhow::bail!("list: unexpected response {other:?}"),
        }
    }

    /// Ask the supervisor to re-exec itself. Returns `true` when the
    /// supervisor will re-exec, `false` when its binary is already current
    /// and it skipped the reexec.
    pub async fn update_self(&mut self) -> Result<bool> {
        match self.send(Request::UpdateSelf).await? {
            Response::Ok => Ok(true),
            Response::NoChange => Ok(false),
            Response::Error { message } => anyhow::bail!("update_self: {message}"),
            other => anyhow::bail!("update_self: unexpected response {other:?}"),
        }
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        match self.send(Request::Shutdown).await? {
            Response::Ok => Ok(()),
            Response::Error { message } => anyhow::bail!("shutdown: {message}"),
            other => anyhow::bail!("shutdown: unexpected response {other:?}"),
        }
    }

    pub async fn stop_service(&mut self, name: &str) -> Result<()> {
        match self
            .send(Request::Stop {
                name: name.to_string(),
            })
            .await?
        {
            Response::Ok => Ok(()),
            Response::Error { message } => anyhow::bail!("stop {name}: {message}"),
            other => anyhow::bail!("stop {name}: unexpected response {other:?}"),
        }
    }

    pub async fn start_service(&mut self, name: &str) -> Result<()> {
        match self
            .send(Request::Start {
                name: name.to_string(),
            })
            .await?
        {
            Response::Ok => Ok(()),
            Response::Error { message } => anyhow::bail!("start {name}: {message}"),
            other => anyhow::bail!("start {name}: unexpected response {other:?}"),
        }
    }

    pub async fn restart_service(&mut self, name: &str) -> Result<()> {
        match self
            .send(Request::Restart {
                name: name.to_string(),
            })
            .await?
        {
            Response::Ok => Ok(()),
            Response::Error { message } => anyhow::bail!("restart {name}: {message}"),
            other => anyhow::bail!("restart {name}: unexpected response {other:?}"),
        }
    }

    pub async fn kill_service(&mut self, name: &str, signal: i32) -> Result<()> {
        match self
            .send(Request::Kill {
                name: name.to_string(),
                signal,
            })
            .await?
        {
            Response::Ok => Ok(()),
            Response::Error { message } => anyhow::bail!("kill {name}: {message}"),
            other => anyhow::bail!("kill {name}: unexpected response {other:?}"),
        }
    }

    /// Non-blocking pop of a pending notification.
    pub fn try_recv_notification(&mut self) -> Option<Notification> {
        self.notif_rx.try_recv().ok()
    }

    /// Returns true once the background reader has closed (supervisor died
    /// or the socket was torn down).
    pub fn is_disconnected(&self) -> bool {
        self.resp_rx.is_closed() || self.notif_rx.is_closed()
    }
}
