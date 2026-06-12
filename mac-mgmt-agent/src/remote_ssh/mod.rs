// The interactive relay-SSH shell is built on a real PTY and a control FIFO —
// both Unix-only mechanisms (openpty / mkfifo). They are gated to Unix; the
// Windows build keeps the rest of the relay state machine but cannot serve an
// interactive shell (the relay/SSH plane is `future`-gated and off by default).
#[cfg(unix)]
pub mod fifo_watcher;
pub mod host_keys;
#[cfg(unix)]
pub mod pty;
pub mod ssh_keys;
#[cfg(unix)]
pub mod ssh_server;

use russh::keys::PublicKey;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;

use crate::file_tunnels::FileTunnelRegistry;
use crate::shell_tunnels::ShellTunnelRegistry;

pub use crate::p2p::proxy_helpers::{TunnelOverride, TunnelTarget};

// Re-export for backwards compatibility with daemon.rs references.
pub type TunnelTargetCompat = TunnelTarget;

#[derive(Debug)]
pub enum RemoteSshCommand {
    Enable,
    Disable,
}

/// Shared state for remote SSH, tunnel management, and FIFO watcher.
///
/// This replaces the old `Manager` struct which mixed shared state with
/// the WS relay client. The libp2p P2pManager handles connectivity;
/// this struct only holds the shared state.
pub struct RemoteSshState {
    pub ssh_allowed: Arc<AtomicBool>,
    ssh_cmd_rx: tokio::sync::mpsc::Receiver<RemoteSshCommand>,
    pub server_ssh_keys: Arc<RwLock<Vec<PublicKey>>>,
    server_url: Option<String>,
    server_token: Option<String>,
    pub tunnel_defs: Arc<RwLock<HashMap<String, TunnelTarget>>>,
    pub tunnel_overrides: Arc<RwLock<HashMap<String, Vec<TunnelOverride>>>>,
    pub file_tunnel_registry: Arc<RwLock<FileTunnelRegistry>>,
    pub shell_tunnel_registry: Arc<RwLock<ShellTunnelRegistry>>,
    heartbeat_tx: tokio::sync::mpsc::Sender<()>,
}

impl RemoteSshState {
    pub fn new(
        server_url: Option<String>,
        server_token: Option<String>,
        remote_ssh_enabled: bool,
    ) -> (Self, tokio::sync::mpsc::Receiver<()>) {
        let ssh_allowed = Arc::new(AtomicBool::new(remote_ssh_enabled));
        let server_ssh_keys = Arc::new(RwLock::new(Vec::new()));
        let tunnel_defs = Arc::new(RwLock::new(HashMap::new()));
        let tunnel_overrides = Arc::new(RwLock::new(HashMap::new()));
        let (heartbeat_tx, heartbeat_rx) = tokio::sync::mpsc::channel(4);
        let file_tunnel_registry = Arc::new(RwLock::new(FileTunnelRegistry::new()));
        let shell_tunnel_registry = Arc::new(RwLock::new(ShellTunnelRegistry::new()));

        let (ssh_cmd_tx, ssh_cmd_rx) = tokio::sync::mpsc::channel(4);
        // The FIFO watcher is a Unix mechanism (named pipe); on non-Unix the
        // control FIFO does not exist, so the sender is simply dropped.
        #[cfg(all(unix, not(feature = "sim")))]
        tokio::spawn(async move {
            if let Err(e) = fifo_watcher::watch(ssh_cmd_tx).await {
                tracing::error!("FIFO watcher failed: {e:#}");
            }
        });
        #[cfg(not(all(unix, not(feature = "sim"))))]
        drop(ssh_cmd_tx);

        (
            Self {
                ssh_allowed,
                ssh_cmd_rx,
                server_ssh_keys,
                server_url,
                server_token,
                tunnel_defs,
                tunnel_overrides,
                file_tunnel_registry,
                shell_tunnel_registry,
                heartbeat_tx,
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

    /// Shared file tunnel registry.
    pub fn file_tunnel_registry(&self) -> Arc<RwLock<FileTunnelRegistry>> {
        self.file_tunnel_registry.clone()
    }

    /// Shared shell tunnel registry.
    pub fn shell_tunnel_registry(&self) -> Arc<RwLock<ShellTunnelRegistry>> {
        self.shell_tunnel_registry.clone()
    }

    /// Update the tunnel definitions. Non-blocking.
    #[cfg(feature = "services")]
    pub fn update_tunnel_defs(&self, defs: Vec<crate::managed_service::TunnelDef>) {
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

        if self.heartbeat_tx.try_send(()).is_err() {
            tracing::debug!("heartbeat signal channel full, heartbeat will fire on next tick");
        }
    }

    /// Update the tunnel override map. Non-blocking.
    #[cfg(feature = "services")]
    pub fn update_tunnel_overrides(
        &self,
        overrides: std::collections::HashMap<String, Vec<TunnelOverride>>,
    ) {
        let Ok(mut map) = self.tunnel_overrides.try_write() else {
            tracing::warn!("tunnel_overrides lock contention, skipping update");
            return;
        };
        *map = overrides;
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

        reg.register_virtual(
            "restart-daemon",
            std::sync::Arc::new(|_user_arg: Option<&str>| {
                use crate::shell_tunnels::VirtualOutput;
                tracing::info!("restart-daemon: sending SIGTERM to self for graceful restart");
                std::thread::spawn(|| {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    // Graceful self-restart: SIGTERM on Unix; on non-Unix exit
                    // and let the launcher/supervisor respawn the daemon.
                    #[cfg(unix)]
                    unsafe {
                        libc::kill(libc::getpid(), libc::SIGTERM);
                    }
                    #[cfg(not(unix))]
                    std::process::exit(0);
                });
                VirtualOutput {
                    lines: vec![(
                        "stdout".into(),
                        "Daemon shutting down gracefully for restart...".into(),
                    )],
                    exit_code: 0,
                }
            }),
        );
    }

    /// Clean up resources (FIFO) on shutdown.
    pub fn cleanup(&self) {
        #[cfg(unix)]
        fifo_watcher::cleanup();
    }

    /// Handle a remote SSH command (toggle allow/deny).
    /// Triggers a heartbeat so the relay is notified of the state change.
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
        // Signal a heartbeat so the relay learns the new SSH state via
        // tunnel_advertisement.
        if self.heartbeat_tx.try_send(()).is_err() {
            tracing::debug!("heartbeat signal channel full, heartbeat will fire on next tick");
        }
    }
}
