pub mod fifo_watcher;
pub mod host_keys;
pub mod pty;
pub mod relay_client;
pub mod ssh_keys;
pub mod ssh_server;
pub mod ws_stream;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use russh::keys::{PrivateKey, PublicKey};
use tokio::sync::RwLock;

pub use relay_client::TunnelTarget;

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
    tunnel_defs: Arc<RwLock<HashMap<String, TunnelTarget>>>,
    relay_proxy_hostname: Arc<RwLock<Option<String>>>,
    /// Shared channel to send messages on the relay WS (for tunnel re-advertisements).
    ws_outgoing_tx: Arc<RwLock<Option<tokio::sync::mpsc::Sender<String>>>>,
}

impl Manager {
    /// Create a new Manager. If relay URL + server token are configured,
    /// immediately spawns the relay client (always-on).
    pub fn new(
        relay_url: Option<String>,
        server_url: Option<String>,
        server_token: Option<String>,
        instance_id: String,
        host_key: Arc<PrivateKey>,
        metrics_port: u16,
        remote_ssh_enabled: bool,
    ) -> Self {
        let ssh_allowed = Arc::new(AtomicBool::new(remote_ssh_enabled));
        let server_ssh_keys = Arc::new(RwLock::new(Vec::new()));
        let tunnel_defs = Arc::new(RwLock::new(HashMap::new()));
        let relay_proxy_hostname = Arc::new(RwLock::new(None));
        let ws_outgoing_tx: Arc<RwLock<Option<tokio::sync::mpsc::Sender<String>>>> =
            Arc::new(RwLock::new(None));

        let (ssh_cmd_tx, ssh_cmd_rx) = tokio::sync::mpsc::channel(4);
        tokio::spawn(async move {
            if let Err(e) = fifo_watcher::watch(ssh_cmd_tx).await {
                tracing::error!("FIFO watcher failed: {e:#}");
            }
        });

        // Always spawn relay client if relay URL and token are configured
        if let (Some(url), Some(token)) = (&relay_url, &server_token) {
            tracing::info!(
                "relay configured: url={url} instance={instance_id} ssh_initially_allowed={remote_ssh_enabled}"
            );
            let url = url.clone();
            let token = token.clone();
            let iid = instance_id.clone();
            let keys = Arc::clone(&server_ssh_keys);
            let allowed = Arc::clone(&ssh_allowed);
            let tdefs = Arc::clone(&tunnel_defs);
            let rph = Arc::clone(&relay_proxy_hostname);
            let wstx = Arc::clone(&ws_outgoing_tx);
            tokio::spawn(async move {
                let hk = Arc::unwrap_or_clone(host_key);
                if let Err(e) = relay_client::run(
                    &url, &token, &iid, None, hk, keys, allowed, metrics_port, tdefs, rph, wstx,
                ).await {
                    tracing::error!("relay client exited: {e:#}");
                }
            });
        } else {
            tracing::debug!("relay not configured (url or token missing), relay client not spawned");
        }

        Self {
            ssh_allowed,
            ssh_cmd_rx,
            server_ssh_keys,
            server_url,
            server_token,
            tunnel_defs,
            relay_proxy_hostname,
            ws_outgoing_tx,
        }
    }

    /// Sync SSH keys from the server. Call periodically (e.g. every hour).
    pub async fn sync_ssh_keys(&self) {
        if let (Some(url), Some(token)) = (&self.server_url, &self.server_token) {
            match ssh_keys::sync(url, token).await {
                Ok(keys) => {
                    tracing::debug!("synced {} SSH key(s) from server", keys.len());
                    *self.server_ssh_keys.write().await = keys;
                }
                Err(e) => tracing::warn!("SSH keys sync failed: {e}"),
            }
        }
    }

    /// Receive the next command from the FIFO watcher (async).
    pub async fn recv_cmd(&mut self) -> Option<RemoteSshCommand> {
        self.ssh_cmd_rx.recv().await
    }

    /// Return the relay's proxy hostname (set after registration).
    pub async fn relay_proxy_hostname(&self) -> Option<String> {
        self.relay_proxy_hostname.read().await.clone()
    }

    /// Update the tunnel definitions and re-advertise to the relay.
    pub async fn update_tunnel_defs(&self, defs: Vec<crate::managed_service::TunnelDef>) {
        let tunnels_json: Vec<serde_json::Value> = defs
            .iter()
            .map(|t| serde_json::json!({ "name": t.name, "tcp_port": t.tcp_port }))
            .collect();

        let mut map = self.tunnel_defs.write().await;
        map.clear();
        for d in defs {
            map.insert(d.name.clone(), TunnelTarget { host: d.host, port: d.tcp_port });
        }
        drop(map);

        // Re-advertise to the relay if connected.
        if let Some(tx) = self.ws_outgoing_tx.read().await.as_ref() {
            let advert = serde_json::json!({
                "type": "tunnel_advertisement",
                "tunnels": tunnels_json,
            });
            let _ = tx.send(advert.to_string()).await;
        }
    }

    /// Clean up resources (FIFO) on shutdown.
    pub fn cleanup(&self) {
        fifo_watcher::cleanup();
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
