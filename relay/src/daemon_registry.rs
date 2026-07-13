use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;
use uuid::Uuid;

const RESERVATION_TTL_DAYS: i64 = 30;

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
    /// Whether the daemon accepts SSH sessions via libp2p.
    pub ssh_enabled: bool,
    /// Allocated SSH port on the relay (set by SshBridge).
    pub ssh_port: Option<u16>,
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
    pub ssh_enabled: bool,
    pub ssh_port: Option<u16>,
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

/// An SSH-capable daemon returned by the `/api/ssh` endpoint.
#[derive(Debug, Serialize)]
pub struct SshTarget {
    pub instance_id: String,
    pub cluster_id: Option<Uuid>,
    pub cluster_name: Option<String>,
    pub agent_name: Option<String>,
    pub hostname: Option<String>,
    pub ssh_port: Option<u16>,
}

/// A port reserved for a disconnected machine so it gets the same port back.
#[derive(Serialize, Deserialize)]
struct PortReservation {
    port: u16,
    reserved_at: DateTime<Utc>,
}

/// On-disk format for port reservations.
#[derive(Serialize, Deserialize, Default)]
struct ReservationsFile {
    reservations: HashMap<String, PortReservation>,
}

pub struct DaemonRegistry {
    daemons: RwLock<HashMap<String, DaemonConn>>,
    max_daemons: usize,
    port_min: u16,
    port_max: u16,
    used_ports: RwLock<HashSet<u16>>,
    /// instance_id → reserved port (kept for up to 30 days after disconnect).
    reservations: RwLock<HashMap<String, PortReservation>>,
    /// Path to the reservations file on disk.
    reservations_path: std::path::PathBuf,
}

impl DaemonRegistry {
    pub fn new(
        max_daemons: usize,
        port_min: u16,
        port_max: u16,
        data_dir: &std::path::Path,
    ) -> Self {
        let reservations_path = data_dir.join("port_reservations.json");

        // Load existing reservations from disk.
        let (reservations, used_ports) = match std::fs::read_to_string(&reservations_path) {
            Ok(contents) => {
                let file: ReservationsFile = serde_json::from_str(&contents).unwrap_or_default();
                let cutoff = Utc::now() - ChronoDuration::days(RESERVATION_TTL_DAYS);
                let mut entries: Vec<(String, PortReservation)> = file
                    .reservations
                    .into_iter()
                    .filter(|(_, r)| r.reserved_at >= cutoff)
                    .collect();
                // Two instances must never reserve the same port (files written
                // by older relays could contain duplicates); keep the newest.
                entries.sort_by(|a, b| b.1.reserved_at.cmp(&a.1.reserved_at));
                let mut seen_ports = HashSet::new();
                let valid: HashMap<String, PortReservation> = entries
                    .into_iter()
                    .filter(|(id, r)| {
                        let fresh = seen_ports.insert(r.port);
                        if !fresh {
                            tracing::warn!(
                                "dropping duplicate reservation for {id} port {}",
                                r.port
                            );
                        }
                        fresh
                    })
                    .collect();
                let ports: HashSet<u16> = valid.values().map(|r| r.port).collect();
                tracing::info!(
                    "loaded {} port reservation(s) from {}",
                    valid.len(),
                    reservations_path.display()
                );
                (valid, ports)
            }
            Err(_) => (HashMap::new(), HashSet::new()),
        };

        Self {
            daemons: RwLock::new(HashMap::new()),
            max_daemons,
            port_min,
            port_max,
            used_ports: RwLock::new(used_ports),
            reservations: RwLock::new(reservations),
            reservations_path,
        }
    }

    // ── Port allocation ─────────────────────────────────────────────────

    /// Allocate a port for `instance_id`. Reuses a reserved port if one
    /// exists, otherwise finds the next free port in the range.
    pub fn allocate_port(&self, instance_id: &str) -> Option<u16> {
        // Check for an existing reservation first.
        if let Some(reserved) = self.claim_reservation(instance_id) {
            tracing::info!("reusing reserved port {reserved} for {instance_id}");
            return Some(reserved);
        }

        if let Some(p) = self.take_free_port() {
            tracing::debug!("allocated port {p} for {instance_id}");
            return Some(p);
        }
        // Try reclaiming an expired reservation.
        self.expire_reservations();
        match self.take_free_port() {
            Some(p) => {
                tracing::info!("allocated port {p} after expiring reservations");
                Some(p)
            }
            None => {
                tracing::error!(
                    "no free ports in range {}-{}",
                    self.port_min,
                    self.port_max
                );
                None
            }
        }
    }

    /// Find and claim the next free port in one step. The write lock is held
    /// across find+insert so concurrent callers can never get the same port.
    fn take_free_port(&self) -> Option<u16> {
        let mut used = self.used_ports.write().unwrap();
        let p = (self.port_min..=self.port_max).find(|p| !used.contains(p))?;
        used.insert(p);
        Some(p)
    }

    /// Claim a reserved port: refresh its timestamp and return it.
    /// The port stays in `used_ports` and the reservation is kept (with
    /// updated timestamp) so it survives the next disconnect too.
    fn claim_reservation(&self, instance_id: &str) -> Option<u16> {
        let mut reservations = self.reservations.write().unwrap();
        let res = reservations.get_mut(instance_id)?;
        let cutoff = Utc::now() - ChronoDuration::days(RESERVATION_TTL_DAYS);
        if res.reserved_at < cutoff {
            let port = res.port;
            reservations.remove(instance_id);
            self.used_ports.write().unwrap().remove(&port);
            tracing::debug!("reservation for {instance_id} port {port} expired");
            None
        } else {
            res.reserved_at = Utc::now();
            let port = res.port;
            tracing::debug!("claimed reservation for {instance_id} port {port}");
            Some(port)
        }
    }

    /// Reserve a port for a daemon so it gets the same port back when it
    /// reconnects. The port stays in `used_ports`.
    pub fn reserve_port(&self, instance_id: &str, port: u16) {
        self.used_ports.write().unwrap().insert(port);
        self.reservations.write().unwrap().insert(
            instance_id.to_string(),
            PortReservation {
                port,
                reserved_at: Utc::now(),
            },
        );
        tracing::info!(
            "reserved port {port} for {instance_id} (up to {RESERVATION_TTL_DAYS} days)"
        );
        self.save_reservations();
    }

    /// Persist reservations to disk (best-effort).
    fn save_reservations(&self) {
        let path = &self.reservations_path;
        let reservations = self.reservations.read().unwrap();
        let file = ReservationsFile {
            reservations: reservations
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        PortReservation {
                            port: v.port,
                            reserved_at: v.reserved_at,
                        },
                    )
                })
                .collect(),
        };
        match serde_json::to_string_pretty(&file) {
            Ok(json) => {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Err(e) = std::fs::write(path, json) {
                    tracing::warn!("failed to save reservations to {}: {e}", path.display());
                }
            }
            Err(e) => tracing::warn!("failed to serialize reservations: {e}"),
        }
    }

    /// Remove expired reservations and free their ports.
    pub fn expire_reservations(&self) {
        let cutoff = Utc::now() - ChronoDuration::days(RESERVATION_TTL_DAYS);
        let mut reservations = self.reservations.write().unwrap();
        let mut used = self.used_ports.write().unwrap();
        let before = reservations.len();
        reservations.retain(|id, res| {
            if res.reserved_at < cutoff {
                used.remove(&res.port);
                tracing::info!("reservation expired for {id} port {}", res.port);
                false
            } else {
                true
            }
        });
        drop(used);
        drop(reservations);
        if before != self.reservations.read().unwrap().len() {
            self.save_reservations();
        }
    }

    // ── SSH state ───────────────────────────────────────────────────────

    /// Set the allocated SSH port for a daemon.
    pub fn set_ssh_port(&self, instance_id: &str, port: u16) {
        let mut daemons = self.daemons.write().unwrap();
        if let Some(d) = daemons.get_mut(instance_id) {
            d.ssh_port = Some(port);
        }
    }

    /// Check if a daemon has an SSH port allocated.
    pub fn has_ssh_port(&self, instance_id: &str) -> bool {
        let daemons = self.daemons.read().unwrap();
        daemons
            .get(instance_id)
            .is_some_and(|d| d.ssh_port.is_some())
    }

    /// Clear the SSH port for a daemon.
    pub fn clear_ssh_port(&self, instance_id: &str) {
        let mut daemons = self.daemons.write().unwrap();
        if let Some(d) = daemons.get_mut(instance_id) {
            d.ssh_port = None;
        }
    }

    /// Update ssh_enabled for a daemon. Returns the previous value.
    pub fn update_ssh_enabled(&self, instance_id: &str, enabled: bool) -> bool {
        let mut daemons = self.daemons.write().unwrap();
        if let Some(d) = daemons.get_mut(instance_id) {
            let prev = d.ssh_enabled;
            d.ssh_enabled = enabled;
            prev
        } else {
            false
        }
    }

    /// List all SSH-capable daemons.
    pub fn list_ssh_targets(&self) -> Vec<SshTarget> {
        let daemons = self.daemons.read().unwrap();
        daemons
            .values()
            .filter(|d| d.ssh_enabled)
            .map(|d| SshTarget {
                instance_id: d.instance_id.clone(),
                cluster_id: d.cluster_id,
                cluster_name: d.cluster_name.clone(),
                agent_name: d.agent_name.clone(),
                hostname: d.hostname.clone(),
                ssh_port: d.ssh_port,
            })
            .collect()
    }

    // ── Daemon registration ─────────────────────────────────────────────

    /// Register a daemon. Returns `false` (and does nothing) when the relay
    /// is at `max_daemons` capacity, unless the daemon is re-registering.
    pub fn register(&self, conn: DaemonConn) -> bool {
        let id = conn.instance_id.clone();
        let mut daemons = self.daemons.write().unwrap();
        if daemons.contains_key(&id) {
            tracing::info!("replacing existing registration for {id}");
            // Do NOT release the old SSH port here: the listener for it may
            // still be running (rapid reconnect) and a reservation still maps
            // this instance to it. Freeing it would let another daemon get
            // the same port. SshBridge::on_ssh_enabled re-syncs ssh_port
            // into the new conn; reservation expiry frees it eventually.
        } else if daemons.len() >= self.max_daemons {
            tracing::warn!(
                "daemon {id} registration rejected: relay full ({} daemons)",
                daemons.len()
            );
            return false;
        }
        daemons.insert(id.clone(), conn);
        tracing::info!("registered daemon {id} (total: {})", daemons.len());
        true
    }

    /// Unregister a daemon, but only if its `connected_at` matches.
    /// This prevents a stale cleanup from deleting a newer
    /// registration that replaced it during a rapid reconnect.
    /// Returns the SSH port the daemon had, if any (for bridge cleanup).
    pub fn unregister(&self, instance_id: &str, connected_at: DateTime<Utc>) -> Option<u16> {
        let mut daemons = self.daemons.write().unwrap();
        let dominated = daemons
            .get(instance_id)
            .is_some_and(|c| c.connected_at != connected_at);
        if dominated {
            tracing::info!("skipping unregister for {instance_id}: newer connection exists");
            return None;
        }
        if let Some(conn) = daemons.remove(instance_id) {
            let ssh_port = conn.ssh_port;
            // Reserve the SSH port so the daemon gets it back on reconnect.
            if let Some(port) = ssh_port {
                self.reserve_port(instance_id, port);
            }
            tracing::info!(
                "unregistered daemon {instance_id} (remaining: {})",
                daemons.len()
            );
            ssh_port
        } else {
            tracing::debug!("unregister called for unknown daemon {instance_id}");
            None
        }
    }

    // ── Tunnel queries ──────────────────────────────────────────────────

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
                ssh_enabled: d.ssh_enabled,
                ssh_port: d.ssh_port,
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
            let tcp_count = tunnels.len();
            let file_count = file_tunnels.as_array().map(|a| a.len()).unwrap_or(0);
            let shell_count = shell_tunnels.as_array().map(|a| a.len()).unwrap_or(0);
            tracing::info!(
                "daemon {instance_id} advertised {tcp_count} TCP, {file_count} file, {shell_count} shell tunnel(s)"
            );
            d.tunnels = tunnels.into_iter().take(100).collect();
            d.file_tunnels = file_tunnels;
            d.shell_tunnels = shell_tunnels;
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
    /// Check if a daemon is connected (by prefix).
    pub fn is_connected(&self, prefix: &str) -> bool {
        self.resolve_prefix(prefix).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn(id: &str, ssh_port: Option<u16>) -> DaemonConn {
        DaemonConn {
            instance_id: id.to_string(),
            cluster_id: None,
            cluster_name: None,
            agent_name: None,
            hostname: None,
            connected_at: Utc::now(),
            tunnels: Vec::new(),
            file_tunnels: serde_json::Value::Array(vec![]),
            shell_tunnels: serde_json::Value::Array(vec![]),
            peer_id: None,
            ssh_enabled: true,
            ssh_port,
        }
    }

    #[test]
    fn concurrent_allocations_never_collide() {
        let dir = std::env::temp_dir().join(format!("relay-test-{}", std::process::id()));
        let registry = std::sync::Arc::new(DaemonRegistry::new(100, 10000, 10063, &dir));
        let handles: Vec<_> = (0..64)
            .map(|i| {
                let r = std::sync::Arc::clone(&registry);
                std::thread::spawn(move || r.allocate_port(&format!("daemon-{i}")).unwrap())
            })
            .collect();
        let ports: HashSet<u16> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(ports.len(), 64, "duplicate ports were allocated");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reregistration_keeps_port_allocated() {
        let dir = std::env::temp_dir().join(format!("relay-test-rereg-{}", std::process::id()));
        let registry = DaemonRegistry::new(100, 10000, 10010, &dir);

        registry.register(conn("a", None));
        let port = registry.allocate_port("a").unwrap();
        registry.set_ssh_port("a", port);
        registry.reserve_port("a", port);

        // Rapid reconnect: daemon re-registers with ssh_port: None while the
        // old listener still holds the port.
        registry.register(conn("a", None));

        // Another daemon must not get the same port.
        let other = registry.allocate_port("b").unwrap();
        assert_ne!(other, port, "re-registration freed a port still in use");
        // Daemon "a" still gets its reserved port back.
        assert_eq!(registry.allocate_port("a").unwrap(), port);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn duplicate_reservations_deduped_on_load() {
        let dir = std::env::temp_dir().join(format!("relay-test-dedup-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let old = Utc::now() - ChronoDuration::days(1);
        let json = serde_json::json!({
            "reservations": {
                "newer": { "port": 10000, "reserved_at": Utc::now() },
                "older": { "port": 10000, "reserved_at": old },
            }
        });
        std::fs::write(
            dir.join("port_reservations.json"),
            serde_json::to_string(&json).unwrap(),
        )
        .unwrap();

        let registry = DaemonRegistry::new(100, 10000, 10010, &dir);
        // The newer reservation wins; the older instance gets a fresh port.
        assert_eq!(registry.allocate_port("newer").unwrap(), 10000);
        assert_ne!(registry.allocate_port("older").unwrap(), 10000);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
