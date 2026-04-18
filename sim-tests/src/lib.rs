//! Simulation testing harness for mac-mgmt.
//!
//! Provides a mock management server and helper utilities for testing
//! daemon protocol behaviour under controlled conditions.

pub mod mock_server;

use std::net::SocketAddr;
use std::sync::Arc;

/// Configuration for a simulated daemon instance.
pub struct SimDaemonConfig {
    /// Mock server address (e.g. "127.0.0.1:12345").
    pub server_addr: SocketAddr,
    /// Bearer token the daemon uses.
    pub token: String,
}

/// Start a mock server and return its address + state handle.
pub async fn start_mock_server() -> (SocketAddr, Arc<mock_server::MockServerState>) {
    mock_server::start().await
}

/// Build a daemon config pointing at the given mock server.
pub fn daemon_config_for(addr: SocketAddr) -> mac_mgmt_common::DaemonConfig {
    let mut cfg = mac_mgmt_common::DaemonConfig::default();
    cfg.server.url = Some(format!("http://{addr}"));
    cfg.server.token = Some("test-token".to_string());
    // Use short intervals for faster tests
    cfg.daemon.health_interval = "1s".to_string();
    cfg.daemon.update_interval = "5s".to_string();
    cfg
}

/// Generate a random ed25519 host key for simulation.
pub fn generate_host_key() -> russh::keys::PrivateKey {
    let mut rng = rand::rngs::OsRng;
    russh::keys::PrivateKey::random(&mut rng, russh::keys::Algorithm::Ed25519).unwrap()
}
