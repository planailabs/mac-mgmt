use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;

/// A TCP tunnel exposed by a managed service on a daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceTunnel {
    pub name: String,
    pub tcp_port: u16,
}

/// A connected daemon.
pub struct DaemonConn {
    pub instance_id: String,
    pub cluster_id: Option<Uuid>,
    pub cluster_name: Option<String>,
    pub agent_name: Option<String>,
    pub hostname: Option<String>,
    pub connected_at: DateTime<Utc>,
    /// TCP tunnels advertised by the daemon's managed services.
    pub tunnels: Vec<ServiceTunnel>,
    /// File tunnels (config editing).
    pub file_tunnels: serde_json::Value,
    /// Shell command tunnels.
    pub shell_tunnels: serde_json::Value,
    /// libp2p PeerId if the daemon is connected via p2p.
    pub peer_id: Option<libp2p::PeerId>,
}

/// Full instance info returned by the batch endpoint.
#[derive(Debug, Serialize)]
pub struct InstanceInfo {
    pub instance_id: String,
    pub cluster_id: Option<Uuid>,
    pub cluster_name: Option<String>,
    pub hostname: Option<String>,
    pub connected_at: DateTime<Utc>,
    pub tunnels: Vec<ServiceTunnel>,
    pub file_tunnels: serde_json::Value,
    pub shell_tunnels: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct TunnelInfo {
    pub instance_id: String,
    pub cluster_id: Option<Uuid>,
    pub cluster_name: Option<String>,
    pub agent_name: Option<String>,
    pub hostname: Option<String>,
    pub connected_at: DateTime<Utc>,
    pub tunnels: Vec<ServiceTunnel>,
}

#[allow(dead_code)]
pub struct DaemonRegistry {
    daemons: RwLock<HashMap<String, DaemonConn>>,
    max_daemons: usize,
}

#[allow(dead_code)]
impl DaemonRegistry {
    pub fn new(max_daemons: usize) -> Self {
        Self {
            daemons: RwLock::new(HashMap::new()),
            max_daemons,
        }
    }

    pub fn is_full(&self) -> bool {
        self.daemons.read().unwrap().len() >= self.max_daemons
    }

    pub fn register(&self, conn: DaemonConn) {
        let id = conn.instance_id.clone();
        let mut daemons = self.daemons.write().unwrap();
        if daemons.contains_key(&id) {
            tracing::info!("replacing existing registration for {id}");
        }
        daemons.insert(id.clone(), conn);
        tracing::info!(
            "registered daemon {id} (total: {})",
            daemons.len()
        );
    }

    /// Unregister a daemon, but only if its `connected_at` matches.
    /// This prevents a stale cleanup from deleting a newer
    /// registration that replaced it during a rapid reconnect.
    pub fn unregister(&self, instance_id: &str, connected_at: DateTime<Utc>) {
        let mut daemons = self.daemons.write().unwrap();
        let dominated = daemons
            .get(instance_id)
            .is_some_and(|c| c.connected_at != connected_at);
        if dominated {
            tracing::info!(
                "skipping unregister for {instance_id}: newer connection exists"
            );
            return;
        }
        if let Some(_conn) = daemons.remove(instance_id) {
            tracing::info!(
                "unregistered daemon {instance_id} (remaining: {})",
                daemons.len()
            );
        } else {
            tracing::debug!("unregister called for unknown daemon {instance_id}");
        }
    }

    pub fn list_tunnels(&self) -> Vec<TunnelInfo> {
        let daemons = self.daemons.read().unwrap();
        daemons
            .values()
            .map(|d| TunnelInfo {
                instance_id: d.instance_id.clone(),
                cluster_id: d.cluster_id,
                cluster_name: d.cluster_name.clone(),
                agent_name: d.agent_name.clone(),
                hostname: d.hostname.clone(),
                connected_at: d.connected_at,
                tunnels: d.tunnels.clone(),
            })
            .collect()
    }

    /// List all connected daemons with full tunnel info (for batch endpoint).
    pub fn list_instances(&self) -> Vec<InstanceInfo> {
        let daemons = self.daemons.read().unwrap();
        daemons
            .values()
            .map(|d| InstanceInfo {
                instance_id: d.instance_id.clone(),
                cluster_id: d.cluster_id,
                cluster_name: d.cluster_name.clone(),
                hostname: d.hostname.clone(),
                connected_at: d.connected_at,
                tunnels: d.tunnels.clone(),
                file_tunnels: d.file_tunnels.clone(),
                shell_tunnels: d.shell_tunnels.clone(),
            })
            .collect()
    }

    /// Update all tunnel definitions for a daemon at once.
    pub fn update_all_tunnels(
        &self,
        instance_id: &str,
        tunnels: Vec<ServiceTunnel>,
        file_tunnels: serde_json::Value,
        shell_tunnels: serde_json::Value,
    ) {
        let mut daemons = self.daemons.write().unwrap();
        if let Some(d) = daemons.get_mut(instance_id) {
            let count = tunnels.len();
            tracing::info!("daemon {instance_id} advertised {count} TCP tunnel(s)");
            d.tunnels = tunnels.into_iter().take(100).collect();
            d.file_tunnels = file_tunnels;
            d.shell_tunnels = shell_tunnels;
        }
    }

    /// Update the advertised tunnels for a connected daemon. Capped at 100 per daemon.
    pub fn update_tunnels(&self, instance_id: &str, tunnels: Vec<ServiceTunnel>) {
        let mut daemons = self.daemons.write().unwrap();
        if let Some(d) = daemons.get_mut(instance_id) {
            let count = tunnels.len().min(100);
            if tunnels.len() > 100 {
                tracing::warn!(
                    "daemon {instance_id} advertised {} tunnels, capping to 100",
                    tunnels.len()
                );
            }
            tracing::info!("daemon {instance_id} advertised {count} tunnel(s)");
            d.tunnels = tunnels.into_iter().take(100).collect();
        }
    }

    /// Resolve a prefix (or full) instance_id to the full ID.
    /// Returns `Some(full_id)` if exactly one daemon matches.
    pub fn resolve_prefix(&self, prefix: &str) -> Option<String> {
        let daemons = self.daemons.read().unwrap();
        // Try exact match first.
        if daemons.contains_key(prefix) {
            return Some(prefix.to_string());
        }
        // Prefix match — must be unambiguous.
        let mut matches = daemons.keys().filter(|k| k.starts_with(prefix));
        let first = matches.next()?.clone();
        if matches.next().is_some() {
            return None; // ambiguous
        }
        Some(first)
    }

    /// Check if a specific tunnel exists on a daemon.
    /// `instance_id` may be a short prefix.
    /// Returns true if the tunnel is found.
    pub fn has_tunnel(&self, instance_id: &str, tunnel_name: &str) -> bool {
        let Some(full_id) = self.resolve_prefix(instance_id) else {
            return false;
        };
        let daemons = self.daemons.read().unwrap();
        let Some(d) = daemons.get(&full_id) else {
            return false;
        };
        d.tunnels.iter().any(|t| t.name == tunnel_name)
    }

    /// Return the cluster_id of a connected daemon.
    /// `instance_id` may be a short prefix.
    pub fn get_cluster_id(&self, instance_id: &str) -> Option<Uuid> {
        let full_id = self.resolve_prefix(instance_id)?;
        let daemons = self.daemons.read().unwrap();
        daemons.get(&full_id).and_then(|d| d.cluster_id)
    }

    /// Get the PeerId for a daemon by full instance_id.
    pub fn get_peer_id(&self, instance_id: &str) -> Option<libp2p::PeerId> {
        let daemons = self.daemons.read().unwrap();
        daemons.get(instance_id).and_then(|d| d.peer_id)
    }

    /// Get the PeerId for a daemon by prefix.
    pub fn resolve_peer_id(&self, prefix: &str) -> Option<libp2p::PeerId> {
        let full_id = self.resolve_prefix(prefix)?;
        self.get_peer_id(&full_id)
    }

    /// Set the PeerId for a daemon (called when daemon connects via libp2p).
    pub fn set_peer_id(&self, instance_id: &str, peer_id: libp2p::PeerId) {
        let mut daemons = self.daemons.write().unwrap();
        if let Some(conn) = daemons.get_mut(instance_id) {
            conn.peer_id = Some(peer_id);
            tracing::info!(%instance_id, %peer_id, "p2p peer ID set for daemon");
        }
    }

    /// Check if a daemon is connected (by prefix).
    pub fn is_connected(&self, prefix: &str) -> bool {
        self.resolve_prefix(prefix).is_some()
    }
}
