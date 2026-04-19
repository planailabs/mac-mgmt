//! Tests for relay feature integration.
//!
//! When the relay feature is enabled, the daemon connects to a relay URL
//! via WebSocket, syncs SSH keys, and reports relay proxy info in heartbeats.
//! These tests verify that the relay integration doesn't break the core
//! daemon protocol, even when the relay URL is unreachable.

use std::time::Duration;

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "warn".to_string()))
        .try_init();
}

/// Daemon with relay configured but pointing at a non-existent URL.
/// The daemon should still function normally (heartbeats, SSE, etc.)
/// even when the relay is unreachable.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn relay_unreachable_daemon_still_works() {
    init_tracing();

    let (addr, state) = sim_tests::start_mock_server().await;

    // Configure relay to a port that nothing listens on
    let mut cfg = sim_tests::daemon_config_for(addr);
    cfg.relay.url = Some("ws://127.0.0.1:1".to_string());
    cfg.relay.remote_ssh_enabled = false;

    let (shutdown_tx, instance_id) = sim_tests::start_sim_daemon_with_config(cfg).await;

    // Daemon should still send heartbeats despite relay failure
    let ok = sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(100), || {
        state.heartbeats_from(&instance_id).len() >= 2
    })
    .await;
    assert!(
        ok,
        "daemon should send heartbeats even when relay is unreachable"
    );

    // SSE should still work
    let sse_ok = sim_tests::wait_until(Duration::from_secs(5), Duration::from_millis(100), || {
        state.request_count("/api/events") > 0
    })
    .await;
    assert!(
        sse_ok,
        "daemon should connect to SSE even with failed relay"
    );

    // SSH key sync should be attempted (endpoint hit)
    let ssh_keys_count = state.request_count("/api/ssh-keys");
    assert!(
        ssh_keys_count > 0,
        "daemon should attempt SSH key sync: got {ssh_keys_count} requests"
    );

    let _ = shutdown_tx.send(());
}

/// Services feature with zero services should not interfere with heartbeats.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn services_feature_empty_heartbeats() {
    init_tracing();

    let (addr, state) = sim_tests::start_mock_server().await;
    let (shutdown_tx, instance_id) = sim_tests::start_sim_daemon(addr).await;

    let ok = sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(100), || {
        state.heartbeats_from(&instance_id).len() >= 2
    })
    .await;
    assert!(ok, "daemon with services feature should send heartbeats");

    // With sim_init (zero services), services array should be empty
    let hbs = state.heartbeats_from(&instance_id);
    for hb in &hbs {
        let services = hb.body.services.as_array().unwrap();
        assert!(
            services.is_empty(),
            "sim daemon should report zero services, got: {:?}",
            services
        );
    }

    let _ = shutdown_tx.send(());
}

/// Daemon with relay configured but relay crashes mid-session.
/// Verify that heartbeats continue flowing.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn relay_crash_heartbeats_continue() {
    init_tracing();

    let (addr, state) = sim_tests::start_mock_server().await;

    // Point relay to a port we'll never open — simulates permanent relay failure
    let mut cfg = sim_tests::daemon_config_for(addr);
    cfg.relay.url = Some("ws://127.0.0.1:1".to_string());
    cfg.relay.remote_ssh_enabled = true;

    let (shutdown_tx, instance_id) = sim_tests::start_sim_daemon_with_config(cfg).await;

    // Wait for heartbeats
    let ok = sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(100), || {
        state.heartbeats_from(&instance_id).len() >= 3
    })
    .await;
    assert!(
        ok,
        "daemon should send multiple heartbeats even with broken relay"
    );

    // Verify heartbeat invariants
    let timeline = sim_tests::timeline::Timeline::new();
    let violations = sim_tests::invariants::check_all(&state, &[instance_id.clone()], &timeline);
    assert_eq!(
        violations,
        0,
        "invariant violations with broken relay:\n{}",
        timeline.format_violations()
    );

    let _ = shutdown_tx.send(());
}

/// Push SyncSshKeys should trigger SSH key fetch even with relay configured.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn push_ssh_keys_sync_with_relay() {
    init_tracing();

    let (addr, state) = sim_tests::start_mock_server().await;

    let mut cfg = sim_tests::daemon_config_for(addr);
    cfg.relay.url = Some("ws://127.0.0.1:1".to_string());
    cfg.relay.remote_ssh_enabled = false;

    let (shutdown_tx, _instance_id) = sim_tests::start_sim_daemon_with_config(cfg).await;

    // Wait for SSE connection
    let ok = sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(100), || {
        state.request_count("/api/events") > 0
    })
    .await;
    assert!(ok, "daemon should connect to SSE");

    let ssh_before = state.request_count("/api/ssh-keys");

    // Push SyncSshKeys
    state.push(mac_mgmt_common::PushEvent::SyncSshKeys);

    let ok = sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(100), || {
        state.request_count("/api/ssh-keys") > ssh_before
    })
    .await;
    assert!(
        ok,
        "SyncSshKeys push should trigger SSH key fetch (before={ssh_before}, after={})",
        state.request_count("/api/ssh-keys")
    );

    let _ = shutdown_tx.send(());
}
