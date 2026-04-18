//! Test: Heartbeat convergence and partition recovery.
//!
//! Validates that daemons send heartbeats reliably, and that after
//! a simulated fault (mock server returning errors), heartbeats
//! resume when the fault is cleared.

use sim_tests::mock_server::{EndpointFault, MockServerState};
use std::sync::Arc;
use std::time::Duration;

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("info")
        .try_init();
}

/// Start a daemon connected to the mock server.
/// Returns a shutdown sender that stops the daemon when dropped/sent.
async fn start_daemon(
    server_addr: std::net::SocketAddr,
) -> (tokio::sync::oneshot::Sender<()>, String) {
    let cfg = sim_tests::daemon_config_for(server_addr);
    let host_key = sim_tests::generate_host_key();
    let instance_id = mac_mgmt_daemon::host_keys::fingerprint_hex(&host_key);
    let host_key = std::sync::Arc::new(host_key);

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let iid = instance_id.clone();

    tokio::spawn(async move {
        if let Err(e) = mac_mgmt_daemon::daemon::run_sim(cfg, host_key, shutdown_rx).await {
            tracing::warn!("daemon {iid} exited with error: {e}");
        }
    });

    (shutdown_tx, instance_id)
}

/// Wait until the mock server has received at least `n` heartbeats from
/// the given instance, with a timeout.
async fn wait_for_heartbeats(
    state: &Arc<MockServerState>,
    instance_id: &str,
    count: usize,
    timeout: Duration,
) -> bool {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let n = state.heartbeats_from(instance_id).len();
        if n >= count {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn daemon_sends_heartbeats() {
    init_tracing();
    let (addr, state) = sim_tests::start_mock_server().await;
    let (shutdown_tx, instance_id) = start_daemon(addr).await;

    // Wait for at least 2 heartbeats (health interval is 1s)
    assert!(
        wait_for_heartbeats(&state, &instance_id, 2, Duration::from_secs(10)).await,
        "expected at least 2 heartbeats within 10s, got {}",
        state.heartbeats_from(&instance_id).len()
    );

    // Verify heartbeat content
    let hbs = state.heartbeats_from(&instance_id);
    for hb in &hbs {
        assert_eq!(hb.body.instance_id, instance_id);
        assert!(!hb.body.version.is_empty(), "version should not be empty");
    }

    let _ = shutdown_tx.send(());
}

#[tokio::test]
async fn heartbeat_resumes_after_server_fault() {
    init_tracing();
    let (addr, state) = sim_tests::start_mock_server().await;
    let (shutdown_tx, instance_id) = start_daemon(addr).await;

    // Wait for first heartbeat
    assert!(
        wait_for_heartbeats(&state, &instance_id, 1, Duration::from_secs(10)).await,
        "expected at least 1 heartbeat before fault injection"
    );

    // Inject fault: heartbeat endpoint returns 500
    state.set_fault(
        "/api/heartbeat",
        EndpointFault {
            fail_status: Some(500),
            ..Default::default()
        },
    );

    // Clear existing heartbeats so we can count new ones
    let count_before_fault = state.heartbeats_from(&instance_id).len();

    // Wait a few seconds — no new heartbeats should succeed
    tokio::time::sleep(Duration::from_secs(3)).await;
    let count_during_fault = state.heartbeats_from(&instance_id).len();
    assert_eq!(
        count_before_fault, count_during_fault,
        "no new heartbeats should be recorded while server returns 500"
    );

    // Clear the fault
    state.clear_faults();

    // Heartbeats should resume
    assert!(
        wait_for_heartbeats(
            &state,
            &instance_id,
            count_during_fault + 1,
            Duration::from_secs(10)
        )
        .await,
        "heartbeat should resume after fault is cleared"
    );

    let _ = shutdown_tx.send(());
}

#[tokio::test]
async fn multiple_daemons_send_heartbeats() {
    init_tracing();
    let (addr, state) = sim_tests::start_mock_server().await;

    let mut shutdowns = Vec::new();
    let mut instance_ids = Vec::new();

    // Start 3 daemons
    for _ in 0..3 {
        let (tx, iid) = start_daemon(addr).await;
        shutdowns.push(tx);
        instance_ids.push(iid);
    }

    // Wait for at least 1 heartbeat from each
    for iid in &instance_ids {
        assert!(
            wait_for_heartbeats(&state, iid, 1, Duration::from_secs(10)).await,
            "daemon {iid} should send at least 1 heartbeat"
        );
    }

    // Verify all instance IDs are unique
    let unique: std::collections::HashSet<&str> =
        instance_ids.iter().map(|s| s.as_str()).collect();
    assert_eq!(
        unique.len(),
        instance_ids.len(),
        "all instance IDs should be unique"
    );

    // Verify the mock server received heartbeats from all 3
    let total_heartbeats = state.get_heartbeats().len();
    assert!(
        total_heartbeats >= 3,
        "expected at least 3 total heartbeats, got {total_heartbeats}"
    );

    for tx in shutdowns {
        let _ = tx.send(());
    }
}
