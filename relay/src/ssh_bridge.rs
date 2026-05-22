//! TCP-to-libp2p SSH bridge.
//!
//! For each SSH-capable daemon, allocates a TCP port from the configured range
//! and spawns a listener. Incoming TCP connections are bridged to the daemon
//! over a libp2p tunnel substream with `{"type": "ssh"}` handshake.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::task::JoinHandle;

use crate::daemon_registry::DaemonRegistry;
use crate::p2p::RelaySwarm;

const OPEN_STREAM_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

struct ListenerEntry {
    port: u16,
    handle: JoinHandle<()>,
}

pub struct SshBridge {
    registry: Arc<DaemonRegistry>,
    relay_swarm: Arc<RelaySwarm>,
    listeners: Mutex<HashMap<String, ListenerEntry>>,
}

impl SshBridge {
    pub fn new(relay_swarm: Arc<RelaySwarm>, registry: Arc<DaemonRegistry>) -> Self {
        Self {
            registry,
            relay_swarm,
            listeners: Mutex::new(HashMap::new()),
        }
    }

    /// Called when a daemon registers or advertises with `ssh_enabled: true`.
    /// Allocates a port (reusing a 30-day reservation if available) and spawns
    /// a TCP listener that bridges connections to the daemon over libp2p.
    pub fn on_ssh_enabled(&self, instance_id: &str) {
        let mut listeners = self.listeners.lock().unwrap();
        if listeners.contains_key(instance_id) {
            return; // already listening
        }

        let Some(port) = self.registry.allocate_port(instance_id) else {
            tracing::error!("no free SSH port for {instance_id}");
            return;
        };

        self.registry.set_ssh_port(instance_id, port);
        // Persist reservation immediately so the mapping survives relay crashes.
        self.registry.reserve_port(instance_id, port);

        let handle = spawn_listener(
            instance_id.to_string(),
            port,
            Arc::clone(&self.relay_swarm),
            Arc::clone(&self.registry),
        );

        listeners.insert(
            instance_id.to_string(),
            ListenerEntry { port, handle },
        );
        tracing::info!("SSH bridge started for {instance_id} on port {port}");
    }

    /// Called when a daemon advertises `ssh_enabled: false` (was previously true).
    pub fn on_ssh_disabled(&self, instance_id: &str) {
        self.remove_listener(instance_id);
    }

    /// Called when a daemon disconnects from the relay.
    pub fn on_daemon_disconnect(&self, instance_id: &str) {
        self.remove_listener(instance_id);
    }

    fn remove_listener(&self, instance_id: &str) {
        let mut listeners = self.listeners.lock().unwrap();
        if let Some(entry) = listeners.remove(instance_id) {
            entry.handle.abort();
            // Refresh the reservation so the daemon gets the same port on reconnect.
            self.registry.reserve_port(instance_id, entry.port);
            self.registry.clear_ssh_port(instance_id);
            tracing::info!(
                "SSH bridge stopped for {instance_id} (port {} reserved)",
                entry.port
            );
        }
    }
}

fn spawn_listener(
    instance_id: String,
    port: u16,
    relay_swarm: Arc<RelaySwarm>,
    registry: Arc<DaemonRegistry>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let listener = match tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("SSH listener bind failed on port {port} for {instance_id}: {e}");
                return;
            }
        };
        tracing::info!("SSH listener started on port {port} for {instance_id}");

        loop {
            let (tcp_stream, peer_addr) = match listener.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    tracing::warn!("SSH listener accept error on port {port}: {e}");
                    continue;
                }
            };

            tracing::info!("SSH client {peer_addr} -> {instance_id} (port {port})");

            let relay_swarm = Arc::clone(&relay_swarm);
            let registry = Arc::clone(&registry);
            let iid = instance_id.clone();

            tokio::spawn(async move {
                if let Err(e) = bridge_ssh(tcp_stream, &iid, &relay_swarm, &registry).await {
                    tracing::warn!("SSH bridge for {iid} from {peer_addr} failed: {e}");
                }
            });
        }
    })
}

async fn bridge_ssh(
    tcp_stream: tokio::net::TcpStream,
    instance_id: &str,
    relay_swarm: &RelaySwarm,
    registry: &DaemonRegistry,
) -> anyhow::Result<()> {
    use futures_util::AsyncWriteExt;
    use tokio_util::compat::FuturesAsyncReadCompatExt;

    let peer_id = registry
        .get_peer_id(instance_id)
        .ok_or_else(|| anyhow::anyhow!("daemon {instance_id} has no peer_id"))?;

    let mut tunnel = tokio::time::timeout(
        OPEN_STREAM_TIMEOUT,
        relay_swarm.open_tunnel_stream(peer_id),
    )
    .await
    .map_err(|_| anyhow::anyhow!("timeout opening tunnel to {instance_id}"))??;

    // Send SSH handshake.
    let handshake = serde_json::json!({ "type": "ssh" });
    let data = serde_json::to_vec(&handshake)?;
    tunnel
        .write_all(&(data.len() as u32).to_be_bytes())
        .await
        .map_err(|e| anyhow::anyhow!("handshake write failed: {e}"))?;
    tunnel
        .write_all(&data)
        .await
        .map_err(|e| anyhow::anyhow!("handshake write failed: {e}"))?;
    tunnel
        .flush()
        .await
        .map_err(|e| anyhow::anyhow!("handshake flush failed: {e}"))?;

    // Bridge bytes bidirectionally.
    let mut compat_tunnel = tunnel.compat();
    let (mut tcp_read, mut tcp_write) = tokio::io::split(tcp_stream);
    let (mut tunnel_read, mut tunnel_write) = tokio::io::split(&mut compat_tunnel);

    let client_to_daemon = tokio::io::copy(&mut tcp_read, &mut tunnel_write);
    let daemon_to_client = tokio::io::copy(&mut tunnel_read, &mut tcp_write);

    tokio::select! {
        r = client_to_daemon => {
            if let Err(e) = r {
                tracing::debug!("SSH client->daemon copy ended: {e}");
            }
        }
        r = daemon_to_client => {
            if let Err(e) = r {
                tracing::debug!("SSH daemon->client copy ended: {e}");
            }
        }
    }

    Ok(())
}
