use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::RwLock;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

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
}

/// Response from daemon for a proxied metrics request.
#[derive(Debug)]
pub struct MetricsResponse {
    pub status: u16,
    pub content_type: String,
    pub body: String,
}

/// A connected daemon.
pub struct DaemonConn {
    pub instance_id: String,
    pub customer_id: Option<Uuid>,
    pub customer_name: Option<String>,
    pub agent_name: Option<String>,
    pub hostname: Option<String>,
    pub ssh_port: u16,
    pub connected_at: DateTime<Utc>,
    pub control_tx: mpsc::Sender<ControlMsg>,
    /// Handle to the TCP listener task so we can abort it on disconnect
    pub listener_handle: tokio::task::JoinHandle<()>,
}

#[derive(Debug, Serialize)]
pub struct TunnelInfo {
    pub instance_id: String,
    pub customer_id: Option<Uuid>,
    pub customer_name: Option<String>,
    pub agent_name: Option<String>,
    pub hostname: Option<String>,
    pub ssh_port: u16,
    pub connected_at: DateTime<Utc>,
}

pub struct DaemonRegistry {
    daemons: RwLock<HashMap<String, DaemonConn>>,
    port_min: u16,
    port_max: u16,
    max_daemons: usize,
    used_ports: RwLock<std::collections::HashSet<u16>>,
}

impl DaemonRegistry {
    pub fn new(port_min: u16, port_max: u16, max_daemons: usize) -> Self {
        Self {
            daemons: RwLock::new(HashMap::new()),
            port_min,
            port_max,
            max_daemons,
            used_ports: RwLock::new(std::collections::HashSet::new()),
        }
    }

    pub fn allocate_port(&self) -> Option<u16> {
        let port = {
            let used = self.used_ports.read().unwrap();
            (self.port_min..=self.port_max).find(|p| !used.contains(p))
        };
        match port {
            Some(p) => {
                self.used_ports.write().unwrap().insert(p);
                tracing::debug!("allocated port {p}");
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

    pub fn release_port(&self, port: u16) {
        self.used_ports.write().unwrap().remove(&port);
        tracing::debug!("released port {port}");
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
            self.release_port(old.ssh_port);
        }
        daemons.insert(id.clone(), conn);
        tracing::info!("registered daemon {id} on port {port} (total: {})", daemons.len());
    }

    pub fn unregister(&self, instance_id: &str) {
        let mut daemons = self.daemons.write().unwrap();
        if let Some(conn) = daemons.remove(instance_id) {
            conn.listener_handle.abort();
            self.release_port(conn.ssh_port);
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
                customer_id: d.customer_id,
                customer_name: d.customer_name.clone(),
                agent_name: d.agent_name.clone(),
                hostname: d.hostname.clone(),
                ssh_port: d.ssh_port,
                connected_at: d.connected_at,
            })
            .collect()
    }

    pub fn get_control_tx(&self, instance_id: &str) -> Option<mpsc::Sender<ControlMsg>> {
        let daemons = self.daemons.read().unwrap();
        daemons.get(instance_id).map(|d| d.control_tx.clone())
    }
}
