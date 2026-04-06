use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::RwLock;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

/// Message sent from relay to daemon over the control WebSocket.
#[derive(Debug)]
pub enum ControlMsg {
    SessionRequest { session_id: String },
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
    pub ssh_port: u16,
    pub connected_at: DateTime<Utc>,
}

pub struct DaemonRegistry {
    daemons: RwLock<HashMap<String, DaemonConn>>,
    port_min: u16,
    port_max: u16,
    used_ports: RwLock<std::collections::HashSet<u16>>,
}

impl DaemonRegistry {
    pub fn new(port_min: u16, port_max: u16) -> Self {
        Self {
            daemons: RwLock::new(HashMap::new()),
            port_min,
            port_max,
            used_ports: RwLock::new(std::collections::HashSet::new()),
        }
    }

    pub fn allocate_port(&self) -> Option<u16> {
        let used = self.used_ports.read().unwrap();
        for port in self.port_min..=self.port_max {
            if !used.contains(&port) {
                drop(used);
                self.used_ports.write().unwrap().insert(port);
                return Some(port);
            }
        }
        None
    }

    pub fn release_port(&self, port: u16) {
        self.used_ports.write().unwrap().remove(&port);
    }

    pub fn register(&self, conn: DaemonConn) {
        let id = conn.instance_id.clone();
        let mut daemons = self.daemons.write().unwrap();
        if let Some(old) = daemons.remove(&id) {
            old.listener_handle.abort();
            self.release_port(old.ssh_port);
        }
        daemons.insert(id, conn);
    }

    pub fn unregister(&self, instance_id: &str) {
        let mut daemons = self.daemons.write().unwrap();
        if let Some(conn) = daemons.remove(instance_id) {
            conn.listener_handle.abort();
            self.release_port(conn.ssh_port);
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
