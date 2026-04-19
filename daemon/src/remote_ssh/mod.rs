pub mod fifo_watcher;
pub mod host_keys;
pub mod pty;
pub mod relay_client;
pub mod ssh_keys;
pub mod ssh_server;
use russh::keys::{PrivateKey, PublicKey};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;

use crate::file_tunnels::FileTunnelRegistry;
use crate::shell_tunnels::ShellTunnelRegistry;
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
    relay_proxy_url: Arc<RwLock<Option<String>>>,
    /// Shared channel to send messages on the relay WS (for tunnel re-advertisements).
    ws_outgoing_tx: Arc<RwLock<Option<tokio::sync::mpsc::Sender<String>>>>,
    /// Signal the main loop to send a heartbeat (e.g. after tunnel changes).
    heartbeat_tx: tokio::sync::mpsc::Sender<()>,
    /// File tunnel registry shared with the relay client.
    file_tunnel_registry: Arc<RwLock<FileTunnelRegistry>>,
    /// Shell tunnel registry shared with the relay client.
    shell_tunnel_registry: Arc<RwLock<ShellTunnelRegistry>>,
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
    ) -> (Self, tokio::sync::mpsc::Receiver<()>) {
        let ssh_allowed = Arc::new(AtomicBool::new(remote_ssh_enabled));
        let server_ssh_keys = Arc::new(RwLock::new(Vec::new()));
        let tunnel_defs = Arc::new(RwLock::new(HashMap::new()));
        let relay_proxy_hostname = Arc::new(RwLock::new(None));
        let relay_proxy_url: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(None));
        let ws_outgoing_tx: Arc<RwLock<Option<tokio::sync::mpsc::Sender<String>>>> =
            Arc::new(RwLock::new(None));
        let (heartbeat_tx, heartbeat_rx) = tokio::sync::mpsc::channel(4);
        let file_tunnel_registry = Arc::new(RwLock::new(FileTunnelRegistry::new()));
        let shell_tunnel_registry = Arc::new(RwLock::new(ShellTunnelRegistry::new()));

        let (ssh_cmd_tx, ssh_cmd_rx) = tokio::sync::mpsc::channel(4);
        #[cfg(not(feature = "sim"))]
        tokio::spawn(async move {
            if let Err(e) = fifo_watcher::watch(ssh_cmd_tx).await {
                tracing::error!("FIFO watcher failed: {e:#}");
            }
        });
        #[cfg(feature = "sim")]
        drop(ssh_cmd_tx);

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
            let rpu = Arc::clone(&relay_proxy_url);
            let wstx = Arc::clone(&ws_outgoing_tx);
            let ftreg = Arc::clone(&file_tunnel_registry);
            let streg = Arc::clone(&shell_tunnel_registry);
            tokio::spawn(async move {
                let hk = Arc::unwrap_or_clone(host_key);
                if let Err(e) = relay_client::run(
                    &url,
                    &token,
                    &iid,
                    None,
                    hk,
                    keys,
                    allowed,
                    metrics_port,
                    tdefs,
                    rph,
                    rpu,
                    wstx,
                    ftreg,
                    streg,
                )
                .await
                {
                    tracing::error!("relay client exited: {e:#}");
                }
            });
        } else {
            tracing::debug!(
                "relay not configured (url or token missing), relay client not spawned"
            );
        }

        (
            Self {
                ssh_allowed,
                ssh_cmd_rx,
                server_ssh_keys,
                server_url,
                server_token,
                tunnel_defs,
                relay_proxy_hostname,
                relay_proxy_url,
                ws_outgoing_tx,
                heartbeat_tx,
                file_tunnel_registry,
                shell_tunnel_registry,
            },
            heartbeat_rx,
        )
    }

    /// Sync SSH keys from the server in a background task (non-blocking).
    pub fn sync_ssh_keys(&self) {
        if let (Some(url), Some(token)) = (&self.server_url, &self.server_token) {
            let url = url.clone();
            let token = token.clone();
            let keys_store = Arc::clone(&self.server_ssh_keys);
            tokio::spawn(async move {
                match ssh_keys::sync(&url, &token).await {
                    Ok(keys) => {
                        tracing::debug!("synced {} SSH key(s) from server", keys.len());
                        *keys_store.write().await = keys;
                    }
                    Err(e) => tracing::warn!("SSH keys sync failed: {e}"),
                }
            });
        }
    }

    /// Receive the next command from the FIFO watcher (async).
    pub async fn recv_cmd(&mut self) -> Option<RemoteSshCommand> {
        self.ssh_cmd_rx.recv().await
    }

    /// Return the relay's proxy hostname (set after registration).
    /// Uses try_read to avoid blocking the main loop.
    pub fn relay_proxy_hostname(&self) -> Option<String> {
        self.relay_proxy_hostname.try_read().ok()?.clone()
    }

    /// Return the relay's full proxy URL (set after registration).
    pub fn relay_proxy_url(&self) -> Option<String> {
        self.relay_proxy_url.try_read().ok()?.clone()
    }

    /// Update the tunnel definitions and re-advertise to the relay.
    /// All operations are non-blocking to avoid stalling the main event loop.
    #[cfg(feature = "services")]
    pub fn update_tunnel_defs(&self, defs: Vec<crate::managed_service::TunnelDef>) {
        let tunnels_json: Vec<serde_json::Value> = defs
            .iter()
            .map(|t| serde_json::json!({ "name": t.name, "tcp_port": t.tcp_port }))
            .collect();

        let Ok(mut map) = self.tunnel_defs.try_write() else {
            tracing::warn!("tunnel_defs lock contention, skipping update");
            return;
        };
        map.clear();
        for d in defs {
            map.insert(
                d.name.clone(),
                TunnelTarget {
                    host: d.host,
                    port: d.tcp_port,
                },
            );
        }
        drop(map);

        // Re-advertise to the relay if connected (non-blocking to avoid stalling the main loop).
        let Ok(ws_tx_guard) = self.ws_outgoing_tx.try_read() else {
            return;
        };
        if let Some(tx) = ws_tx_guard.as_ref() {
            let advert = serde_json::json!({
                "type": "tunnel_advertisement",
                "tunnels": tunnels_json,
            });
            if tx.try_send(advert.to_string()).is_err() {
                tracing::warn!("relay WS outgoing channel full, tunnel advertisement dropped");
            }
        }

        // Signal the main loop to send a heartbeat with the updated tunnels.
        if self.heartbeat_tx.try_send(()).is_err() {
            tracing::debug!("heartbeat signal channel full, heartbeat will fire on next tick");
        }
    }

    /// Update the file tunnel registry. Non-blocking.
    #[cfg(feature = "services")]
    pub fn update_file_tunnel_defs(&self, defs: Vec<crate::managed_service::FileTunnel>) {
        let Ok(mut reg) = self.file_tunnel_registry.try_write() else {
            tracing::warn!("file_tunnel_registry lock contention, skipping update");
            return;
        };
        reg.update(defs);
    }

    /// Update the shell tunnel registry and register virtual handlers. Non-blocking.
    #[cfg(feature = "services")]
    pub fn update_shell_tunnel_defs(&self, defs: Vec<crate::managed_service::ShellTunnel>) {
        let Ok(mut reg) = self.shell_tunnel_registry.try_write() else {
            tracing::warn!("shell_tunnel_registry lock contention, skipping update");
            return;
        };
        reg.update(defs);

        // service-restart: restart a managed service via the supervisor.
        // Unregisters the service, then the daemon's health tick re-registers it.
        reg.register_virtual(
            "service-restart",
            std::sync::Arc::new(|user_arg: Option<&str>| {
                use crate::shell_tunnels::VirtualOutput;
                let Some(service_name) = user_arg.filter(|s| !s.is_empty()) else {
                    return VirtualOutput {
                        lines: vec![("stderr".into(), "Error: service name required".into())],
                        exit_code: 1,
                    };
                };
                let svc = service_name.to_string();
                let result = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async {
                        let socket_path = mac_mgmt_services::default_socket_path();
                        let timeout = std::time::Duration::from_secs(5);
                        let mut client =
                            mac_mgmt_services::client::Client::connect(&socket_path, timeout)
                                .await?;
                        client.unregister(&svc).await?;
                        anyhow::Ok(())
                    })
                });
                match result {
                    Ok(()) => VirtualOutput {
                        lines: vec![(
                            "stdout".into(),
                            format!(
                                "Service '{service_name}' stopped. \
                                 It will be re-registered on the next health tick."
                            ),
                        )],
                        exit_code: 0,
                    },
                    Err(e) => VirtualOutput {
                        lines: vec![("stderr".into(), format!("Restart failed: {e}"))],
                        exit_code: 1,
                    },
                }
            }),
        );

        // restart-daemon: gracefully stop the daemon (service manager restarts it).
        // Sends SIGTERM to self so the normal shutdown path runs (flushes
        // notifications, closes relay WS, drains supervisors).
        reg.register_virtual(
            "restart-daemon",
            std::sync::Arc::new(|_user_arg: Option<&str>| {
                use crate::shell_tunnels::VirtualOutput;
                tracing::info!("restart-daemon: sending SIGTERM to self for graceful restart");
                std::thread::spawn(|| {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    unsafe { libc::kill(libc::getpid(), libc::SIGTERM); }
                });
                VirtualOutput {
                    lines: vec![("stdout".into(), "Daemon shutting down gracefully for restart...".into())],
                    exit_code: 0,
                }
            }),
        );
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
