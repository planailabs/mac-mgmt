pub mod fifo_watcher;
pub mod host_keys;
pub mod pty;
pub mod relay_client;
pub mod ssh_keys;
pub mod ssh_server;
pub mod ws_stream;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use russh::keys::PublicKey;
use tokio::sync::RwLock;

#[derive(Debug)]
pub enum RemoteSshCommand {
    Enable,
    Disable,
}

/// Manages relay connection state, FIFO watcher, and periodic SSH key sync.
///
/// The relay WebSocket is always active when a relay URL is configured.
/// The FIFO toggles an allow/deny flag that controls whether inbound SSH
/// session requests are accepted. Metrics requests always flow through.
pub struct Manager {
    ssh_allowed: Arc<AtomicBool>,
    ssh_cmd_rx: tokio::sync::mpsc::Receiver<RemoteSshCommand>,
    server_ssh_keys: Arc<RwLock<Vec<PublicKey>>>,
    server_url: Option<String>,
    server_token: Option<String>,
}

impl Manager {
    /// Create a new Manager. If relay URL + server token are configured,
    /// immediately spawns the relay client (always-on).
    pub fn new(
        relay_url: Option<String>,
        server_url: Option<String>,
        server_token: Option<String>,
        instance_id: String,
        metrics_port: u16,
        remote_ssh_enabled: bool,
    ) -> Self {
        let ssh_allowed = Arc::new(AtomicBool::new(remote_ssh_enabled));
        let server_ssh_keys = Arc::new(RwLock::new(Vec::new()));

        let (ssh_cmd_tx, ssh_cmd_rx) = tokio::sync::mpsc::channel::<RemoteSshCommand>(4);
        tokio::spawn(async move {
            if let Err(e) = fifo_watcher::watch(ssh_cmd_tx).await {
                tracing::error!("FIFO watcher failed: {e:#}");
            }
        });

        // Always spawn relay client if relay URL and token are configured
        if let (Some(url), Some(token)) = (&relay_url, &server_token) {
            let url = url.clone();
            let token = token.clone();
            let iid = instance_id.clone();
            let keys = Arc::clone(&server_ssh_keys);
            let allowed = Arc::clone(&ssh_allowed);
            tokio::spawn(async move {
                if let Err(e) = relay_client::run(
                    &url, &token, &iid, None, keys, allowed, metrics_port,
                ).await {
                    tracing::error!("relay client exited: {e:#}");
                }
            });
        }

        Self {
            ssh_allowed,
            ssh_cmd_rx,
            server_ssh_keys,
            server_url,
            server_token,
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

    /// Handle a remote SSH command (toggle allow/deny).
    pub fn handle_cmd(&mut self, cmd: RemoteSshCommand) {
        match cmd {
            RemoteSshCommand::Enable => {
                tracing::info!("remote SSH allowed");
                self.ssh_allowed.store(true, Ordering::Relaxed);
            }
            RemoteSshCommand::Disable => {
                tracing::info!("remote SSH denied");
                self.ssh_allowed.store(false, Ordering::Relaxed);
            }
        }
    }
}
