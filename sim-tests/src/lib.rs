//! Simulation testing harness for mac-mgmt.
//!
//! Provides a mock management server, invariant checkers, fault schedule
//! generation, and timeline recording for Antithesis-style deterministic
//! testing of daemon protocol behaviour.

pub mod invariants;
pub mod mock_server;
pub mod mock_service;
pub mod sampler;
pub mod scenarios;
pub mod timeline;

use std::net::SocketAddr;
use std::sync::Arc;

/// Install the rustls crypto provider required by reqwest.
///
/// The daemon creates reqwest clients which need a TLS provider even for
/// plain HTTP. In production this is done in `main()`; in sim-tests we
/// call this before spawning a daemon. Safe to call multiple times.
pub fn ensure_tls_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// Start a mock server and return its address + state handle.
pub async fn start_mock_server() -> (SocketAddr, Arc<mock_server::MockServerState>) {
    mock_server::start().await
}

/// Build a daemon config pointing at the given mock server.
pub fn daemon_config_for(addr: SocketAddr) -> mac_mgmt_common::DaemonConfig {
    daemon_config_with_intervals(addr, "1s", "5s")
}

/// Build a daemon config with custom intervals.
pub fn daemon_config_with_intervals(
    addr: SocketAddr,
    health_interval: &str,
    update_interval: &str,
) -> mac_mgmt_common::DaemonConfig {
    let mut cfg = mac_mgmt_common::DaemonConfig::default();
    cfg.server.url = Some(format!("http://{addr}"));
    cfg.server.token = Some(mac_mgmt_common::Secret::from("test-token"));
    cfg.daemon.health_interval = health_interval.to_string();
    cfg.daemon.update_interval = update_interval.to_string();
    cfg
}

/// Build a daemon config with relay support.
pub fn daemon_config_with_relay(
    server_addr: SocketAddr,
    relay_multiaddr: &str,
) -> mac_mgmt_common::DaemonConfig {
    let mut cfg = daemon_config_for(server_addr);
    cfg.relay.relay_multiaddr = Some(relay_multiaddr.to_string());
    cfg.relay.remote_ssh_enabled = true;
    cfg
}

/// Generate a random ed25519 host key for simulation.
pub fn generate_host_key() -> russh::keys::PrivateKey {
    let mut rng = rand::rngs::OsRng;
    russh::keys::PrivateKey::random(&mut rng, russh::keys::Algorithm::Ed25519).unwrap()
}

/// Start a daemon, returning (shutdown_sender, instance_id).
pub async fn start_sim_daemon(
    server_addr: SocketAddr,
) -> (tokio::sync::oneshot::Sender<()>, String) {
    start_sim_daemon_with_config(daemon_config_for(server_addr)).await
}

/// Start a daemon with a custom config.
pub async fn start_sim_daemon_with_config(
    cfg: mac_mgmt_common::DaemonConfig,
) -> (tokio::sync::oneshot::Sender<()>, String) {
    ensure_tls_provider();

    let host_key = generate_host_key();
    let instance_id = mac_mgmt_daemon::host_keys::fingerprint_hex(&host_key);
    let host_key = std::sync::Arc::new(host_key);
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let iid = instance_id.clone();

    tokio::spawn(async move {
        if let Err(e) = mac_mgmt_daemon::daemon::run_sim(cfg, host_key, shutdown_rx).await {
            tracing::warn!("daemon {iid} exited: {e}");
        }
    });

    (shutdown_tx, instance_id)
}

/// Wait until a condition is true or timeout expires.
pub async fn wait_until(
    timeout: std::time::Duration,
    interval: std::time::Duration,
    condition: impl Fn() -> bool,
) -> bool {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if condition() {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(interval).await;
    }
}
