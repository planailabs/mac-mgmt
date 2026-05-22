//! Shared state for tunnel substream request handling.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tokio::sync::RwLock;

use super::proxy_helpers::{TunnelOverride, TunnelTarget};
use crate::file_tunnels::FileTunnelRegistry;
use crate::shell_tunnels::ShellTunnelRegistry;

/// Shared state for handling tunnel and proxy requests.
pub struct HandlerState {
    pub ssh_allowed: Arc<AtomicBool>,
    pub tunnel_defs: Arc<RwLock<HashMap<String, TunnelTarget>>>,
    pub tunnel_overrides: Arc<RwLock<HashMap<String, Vec<TunnelOverride>>>>,
    pub file_tunnel_registry: Arc<RwLock<FileTunnelRegistry>>,
    pub shell_tunnel_registry: Arc<RwLock<ShellTunnelRegistry>>,
    pub metrics_port: u16,
    pub fake_origin_local: bool,
    pub client: reqwest::Client,
    /// SSH keys synced from the server (cluster_ssh_keys table).
    pub server_ssh_keys: Arc<RwLock<Vec<russh::keys::PublicKey>>>,
    /// Ephemeral SSH public key provided by the relay during registration.
    pub relay_ssh_key: Arc<RwLock<Option<russh::keys::PublicKey>>>,
}
