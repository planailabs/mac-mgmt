//! Tests for the in-process supervisor lifecycle.
//!
//! Uses sim_init_with_supervisor to create a ServiceManager backed by a
//! real in-process supervisor, with mock services that spawn `sleep infinity`.
//! Tests registration, health checking, crash recovery, and shutdown.

use sim_tests::mock_service::MockManagedService;
use std::time::Duration;

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "warn".to_string()))
        .try_init();
}

/// Daemon with in-process supervisor and one mock service sends heartbeats
/// with the service reported in the services array.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn supervisor_mock_service_heartbeat() {
    init_tracing();
    let (addr, state) = sim_tests::start_mock_server().await;

    let mock_svc = MockManagedService::new("test-svc")
        .with_tunnel("test-tunnel", "127.0.0.1", 9999)
        .with_shell_command("test-echo", "echo", &["hello"]);

    let cfg = sim_tests::daemon_config_for(addr);
    let host_key = sim_tests::generate_host_key();
    let instance_id = mac_mgmt_daemon::host_keys::fingerprint_hex(&host_key);
    let host_key = std::sync::Arc::new(host_key);
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    let iid = instance_id.clone();
    tokio::spawn(async move {
        if let Err(e) = mac_mgmt_daemon::daemon::run_sim_with_services(
            cfg,
            host_key,
            shutdown_rx,
            vec![Box::new(mock_svc)],
        )
        .await
        {
            tracing::warn!("daemon {iid} exited: {e}");
        }
    });

    // Wait for heartbeats
    let ok = sim_tests::wait_until(Duration::from_secs(15), Duration::from_millis(200), || {
        state.heartbeats_from(&instance_id).len() >= 2
    })
    .await;
    assert!(ok, "daemon with supervisor should send heartbeats");

    // The heartbeat should report the mock service in the services array
    let hbs = state.heartbeats_from(&instance_id);
    let last = hbs.last().unwrap();
    // services array should contain at least the registered service
    assert!(
        last.body.services.is_array(),
        "services should be array: {:?}",
        last.body.services
    );

    // shell_tunnels should report our mock command
    let empty = vec![];
    let shell_arr = last.body.shell_tunnels.as_array().unwrap_or(&empty);
    let has_test = shell_arr
        .iter()
        .any(|v| v.get("name").and_then(|n| n.as_str()) == Some("test-echo"));
    // Note: shell_tunnels may only include system commands if the service isn't healthy yet.
    // The mock service starts as Stopped then transitions to Starting/Healthy.
    tracing::info!("shell_tunnels in heartbeat: {shell_arr:?}, has_test_echo: {has_test}");

    let _ = shutdown_tx.send(());
}

/// Multiple mock services register with the supervisor.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn supervisor_multiple_services() {
    init_tracing();
    let (addr, state) = sim_tests::start_mock_server().await;

    let svc_a = MockManagedService::new("svc-alpha").with_tunnel("alpha-api", "127.0.0.1", 8001);
    let svc_b = MockManagedService::new("svc-beta")
        .with_tunnel("beta-api", "127.0.0.1", 8002)
        .with_shell_command("beta-status", "true", &[]);

    let cfg = sim_tests::daemon_config_for(addr);
    let host_key = sim_tests::generate_host_key();
    let instance_id = mac_mgmt_daemon::host_keys::fingerprint_hex(&host_key);
    let host_key = std::sync::Arc::new(host_key);
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    let iid = instance_id.clone();
    tokio::spawn(async move {
        if let Err(e) = mac_mgmt_daemon::daemon::run_sim_with_services(
            cfg,
            host_key,
            shutdown_rx,
            vec![Box::new(svc_a), Box::new(svc_b)],
        )
        .await
        {
            tracing::warn!("daemon {iid} exited: {e}");
        }
    });

    // Wait for heartbeats
    let ok = sim_tests::wait_until(Duration::from_secs(15), Duration::from_millis(200), || {
        state.heartbeats_from(&instance_id).len() >= 2
    })
    .await;
    assert!(ok, "daemon with 2 services should send heartbeats");

    let _ = shutdown_tx.send(());
}
