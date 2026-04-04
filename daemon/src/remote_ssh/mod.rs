pub mod fifo_watcher;
pub mod host_keys;
pub mod pty;
pub mod relay_client;
pub mod ssh_keys;
pub mod ssh_server;
pub mod ws_stream;

use std::sync::Arc;
use russh::keys::PublicKey;
use tokio::sync::RwLock;

#[derive(Debug)]
pub enum RemoteSshCommand {
    Enable,
    Disable,
}

/// Manages relay connection state, FIFO watcher, and periodic SSH key sync.
pub struct Manager {
    relay_url: Option<String>,
    server_token: Option<String>,
    server_url: Option<String>,
    instance_id: String,
    ssh_cmd_rx: tokio::sync::mpsc::Receiver<RemoteSshCommand>,
    relay_task: Option<tokio::task::JoinHandle<()>>,
    server_ssh_keys: Arc<RwLock<Vec<PublicKey>>>,
}

impl Manager {
    /// Create a new Manager and spawn the FIFO watcher.
    pub fn new(
        relay_url: Option<String>,
        server_url: Option<String>,
        server_token: Option<String>,
        instance_id: String,
    ) -> Self {
        let (ssh_cmd_tx, ssh_cmd_rx) = tokio::sync::mpsc::channel::<RemoteSshCommand>(4);
        tokio::spawn(async move {
            if let Err(e) = fifo_watcher::watch(ssh_cmd_tx).await {
                tracing::error!("FIFO watcher failed: {e:#}");
            }
        });

        Self {
            relay_url,
            server_token,
            server_url,
            instance_id,
            ssh_cmd_rx,
            relay_task: None,
            server_ssh_keys: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Sync SSH keys from the server. Call periodically (e.g. every hour).
    pub async fn sync_ssh_keys(&self) {
        if let (Some(url), Some(token)) = (&self.server_url, &self.server_token) {
            match ssh_keys::sync(url, token).await {
                Ok(keys) => *self.server_ssh_keys.write().await = keys,
                Err(e) => tracing::warn!("SSH keys sync failed: {e}"),
            }
        }
    }

    /// Receive the next command from the FIFO watcher (async).
    pub async fn recv_cmd(&mut self) -> Option<RemoteSshCommand> {
        self.ssh_cmd_rx.recv().await
    }

    /// Handle a remote SSH command (enable/disable relay).
    pub fn handle_cmd(&mut self, cmd: RemoteSshCommand) {
        match cmd {
            RemoteSshCommand::Enable => {
                if self.relay_task.as_ref().is_some_and(|t| !t.is_finished()) {
                    tracing::info!("remote SSH already enabled");
                    return;
                }
                let Some(ref url) = self.relay_url else {
                    tracing::warn!("cannot enable remote SSH: no relay URL configured");
                    return;
                };
                let Some(ref token) = self.server_token else {
                    tracing::warn!("cannot enable remote SSH: no server token configured");
                    return;
                };
                tracing::info!("enabling remote SSH");
                let url = url.clone();
                let token = token.clone();
                let iid = self.instance_id.clone();
                let ssh_keys = Arc::clone(&self.server_ssh_keys);
                self.relay_task = Some(tokio::spawn(async move {
                    if let Err(e) = relay_client::run(&url, &token, &iid, None, ssh_keys).await {
                        tracing::error!("relay client exited: {e:#}");
                    }
                }));
            }
            RemoteSshCommand::Disable => {
                if let Some(task) = self.relay_task.take() {
                    tracing::info!("disabling remote SSH");
                    task.abort();
                }
            }
        }
    }
}
