use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

const RESERVATION_TTL_DAYS: i64 = 30;

/// Message sent from relay to daemon over the control WebSocket.
#[derive(Debug)]
pub enum ControlMsg {
    SessionRequest {
        session_id: String,
        session_secret: String,
    },
    MetricsRequest {
        request_id: String,
        path: String,
        response_tx: oneshot::Sender<MetricsResponse>,
    },
    ProxyRequest {
        request_id: String,
        tunnel_name: String,
        method: String,
        path: String,
        headers: Vec<(String, String)>,
        body: Option<String>,
        response_tx: oneshot::Sender<ProxyResponse>,
    },
    /// Request a proxy data session (WS-to-WS bridge).
    ProxySessionRequest {
        session_id: String,
        session_secret: String,
        tunnel_name: String,
        mode: String,
        path: String,
    },
    /// Streaming proxy request multiplexed over the control channel.
    ProxyStream {
        request_id: String,
        tunnel_name: String,
        method: String,
        path: String,
        headers: Vec<(String, String)>,
        body: Option<String>,
        response_tx: mpsc::Sender<ProxyStreamEvent>,
    },
    /// List files in a file tunnel (response on control channel).
    FileListRequest {
        request_id: String,
        tunnel_name: String,
        path: Option<String>,
        response_tx: oneshot::Sender<FileResponse>,
    },
    /// Start a data session for file read or write.
    FileSessionRequest {
        session_id: String,
        session_secret: String,
        tunnel_name: String,
        mode: String,
        path: Option<String>,
        expected_mtime: Option<i64>,
    },
}

/// Events streamed back from daemon for a proxy stream request.
#[derive(Debug)]
pub enum ProxyStreamEvent {
    /// Response headers (first event).
    Headers { status: u16, headers: Vec<(String, String)> },
    /// Body chunk (base64-decoded by the relay).
    BodyChunk(Vec<u8>),
    /// Response complete.
    End,
}

/// Response from daemon for a file tunnel operation.
#[derive(Debug)]
pub struct FileResponse {
    pub status: u16,
    pub body: serde_json::Value,
}

/// Response from daemon for a proxied metrics request.
#[derive(Debug)]
pub struct MetricsResponse {
    pub status: u16,
    pub content_type: String,
    pub body: String,
}

/// Response from daemon for a proxied TCP tunnel request (non-streaming).
#[derive(Debug)]
pub struct ProxyResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

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
    pub ssh_port: u16,
    pub connected_at: DateTime<Utc>,
    pub control_tx: mpsc::Sender<ControlMsg>,
    /// Handle to the TCP listener task so we can abort it on disconnect
    pub listener_handle: tokio::task::JoinHandle<()>,
    /// TCP tunnels advertised by the daemon's managed services.
    pub tunnels: Vec<ServiceTunnel>,
}

#[derive(Debug, Serialize)]
pub struct TunnelInfo {
    pub instance_id: String,
    pub cluster_id: Option<Uuid>,
    pub cluster_name: Option<String>,
    pub agent_name: Option<String>,
    pub hostname: Option<String>,
    pub ssh_port: u16,
    pub connected_at: DateTime<Utc>,
    pub tunnels: Vec<ServiceTunnel>,
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
    port_min: u16,
    port_max: u16,
    max_daemons: usize,
    used_ports: RwLock<std::collections::HashSet<u16>>,
    /// instance_id → reserved port (kept for up to 30 days after disconnect).
    reservations: RwLock<HashMap<String, PortReservation>>,
    /// Path to the reservations file on disk.
    reservations_path: std::path::PathBuf,
}

impl DaemonRegistry {
    pub fn new(port_min: u16, port_max: u16, max_daemons: usize, data_dir: &std::path::Path) -> Self {
        let reservations_path = data_dir.join("port_reservations.json");

        // Load existing reservations from disk.
        let (reservations, used_ports) = match std::fs::read_to_string(&reservations_path) {
            Ok(contents) => {
                let file: ReservationsFile =
                    serde_json::from_str(&contents).unwrap_or_default();
                let cutoff = Utc::now() - ChronoDuration::days(RESERVATION_TTL_DAYS);
                let valid: HashMap<String, PortReservation> = file
                    .reservations
                    .into_iter()
                    .filter(|(_, r)| r.reserved_at >= cutoff)
                    .collect();
                let ports: std::collections::HashSet<u16> =
                    valid.values().map(|r| r.port).collect();
                tracing::info!(
                    "loaded {} port reservation(s) from {}",
                    valid.len(),
                    reservations_path.display()
                );
                (valid, ports)
            }
            Err(_) => (HashMap::new(), std::collections::HashSet::new()),
        };

        Self {
            daemons: RwLock::new(HashMap::new()),
            port_min,
            port_max,
            max_daemons,
            used_ports: RwLock::new(used_ports),
            reservations: RwLock::new(reservations),
            reservations_path,
        }
    }

    /// Allocate a port for `instance_id`. Reuses a reserved port if one
    /// exists, otherwise finds the next free port in the range.
    pub fn allocate_port(&self, instance_id: &str) -> Option<u16> {
        // Check for an existing reservation first.
        if let Some(reserved) = self.claim_reservation(instance_id) {
            tracing::info!("reusing reserved port {reserved} for {instance_id}");
            return Some(reserved);
        }

        let port = {
            let used = self.used_ports.read().unwrap();
            (self.port_min..=self.port_max).find(|p| !used.contains(p))
        };
        match port {
            Some(p) => {
                self.used_ports.write().unwrap().insert(p);
                tracing::debug!("allocated port {p} for {instance_id}");
                Some(p)
            }
            None => {
                // Try reclaiming an expired reservation.
                self.expire_reservations();
                let port = {
                    let used = self.used_ports.read().unwrap();
                    (self.port_min..=self.port_max).find(|p| !used.contains(p))
                };
                match port {
                    Some(p) => {
                        self.used_ports.write().unwrap().insert(p);
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
        }
    }

    fn release_port(&self, port: u16) {
        self.used_ports.write().unwrap().remove(&port);
        tracing::debug!("released port {port}");
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

    /// Reserve a port for a disconnecting daemon so it gets the same port
    /// back when it reconnects. The port stays in `used_ports`.
    fn reserve_port(&self, instance_id: &str, port: u16) {
        self.reservations.write().unwrap().insert(
            instance_id.to_string(),
            PortReservation {
                port,
                reserved_at: Utc::now(),
            },
        );
        tracing::info!("reserved port {port} for {instance_id} (up to {RESERVATION_TTL_DAYS} days)");
        self.save_reservations();
    }

    /// Persist reservations to disk (best-effort).
    fn save_reservations(&self) {
        let path = &self.reservations_path;
        let reservations = self.reservations.read().unwrap();
        let file = ReservationsFile {
            reservations: reservations
                .iter()
                .map(|(k, v)| (k.clone(), PortReservation { port: v.port, reserved_at: v.reserved_at }))
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

    pub fn is_full(&self) -> bool {
        self.daemons.read().unwrap().len() >= self.max_daemons
    }

    pub fn register(&self, conn: DaemonConn) {
        let id = conn.instance_id.clone();
        let port = conn.ssh_port;
        let mut daemons = self.daemons.write().unwrap();
        if let Some(old) = daemons.remove(&id) {
            tracing::info!(
                "replacing existing registration for {id} (old port {})",
                old.ssh_port
            );
            old.listener_handle.abort();
            if old.ssh_port != port {
                self.release_port(old.ssh_port);
            }
        }
        // Save/refresh the reservation so the port survives future disconnects.
        self.reserve_port(&id, port);
        daemons.insert(id.clone(), conn);
        tracing::info!("registered daemon {id} on port {port} (total: {})", daemons.len());
    }

    pub fn unregister(&self, instance_id: &str) {
        let mut daemons = self.daemons.write().unwrap();
        if let Some(conn) = daemons.remove(instance_id) {
            conn.listener_handle.abort();
            // Reserve the port instead of releasing it.
            self.reserve_port(instance_id, conn.ssh_port);
            tracing::info!("unregistered daemon {instance_id} (remaining: {})", daemons.len());
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
                ssh_port: d.ssh_port,
                connected_at: d.connected_at,
                tunnels: d.tunnels.clone(),
            })
            .collect()
    }

    pub fn get_control_tx(&self, instance_id: &str) -> Option<mpsc::Sender<ControlMsg>> {
        let daemons = self.daemons.read().unwrap();
        daemons.get(instance_id).map(|d| d.control_tx.clone())
    }

    /// Resolve a prefix to a full instance ID, then return its control channel.
    pub fn resolve_control_tx(&self, prefix: &str) -> Option<mpsc::Sender<ControlMsg>> {
        let full_id = self.resolve_prefix(prefix)?;
        self.get_control_tx(&full_id)
    }

    /// Update the advertised tunnels for a connected daemon. Capped at 100 per daemon.
    pub fn update_tunnels(&self, instance_id: &str, tunnels: Vec<ServiceTunnel>) {
        let mut daemons = self.daemons.write().unwrap();
        if let Some(d) = daemons.get_mut(instance_id) {
            let count = tunnels.len().min(100);
            if tunnels.len() > 100 {
                tracing::warn!("daemon {instance_id} advertised {} tunnels, capping to 100", tunnels.len());
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

    /// Find a specific tunnel on a daemon. Returns (control_tx, tcp_port) if found.
    /// `instance_id` may be a short prefix.
    pub fn find_tunnel(
        &self,
        instance_id: &str,
        tunnel_name: &str,
    ) -> Option<(mpsc::Sender<ControlMsg>, u16)> {
        let full_id = self.resolve_prefix(instance_id)?;
        let daemons = self.daemons.read().unwrap();
        let d = daemons.get(&full_id)?;
        let tunnel = d.tunnels.iter().find(|t| t.name == tunnel_name)?;
        Some((d.control_tx.clone(), tunnel.tcp_port))
    }

    /// Return the cluster_id of a connected daemon.
    /// `instance_id` may be a short prefix.
    pub fn get_cluster_id(&self, instance_id: &str) -> Option<Uuid> {
        let full_id = self.resolve_prefix(instance_id)?;
        let daemons = self.daemons.read().unwrap();
        daemons.get(&full_id).and_then(|d| d.cluster_id)
    }
}
