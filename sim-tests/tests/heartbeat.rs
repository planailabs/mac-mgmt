//! Test: Heartbeat convergence and partition recovery.
//!
//! Validates that daemons send heartbeats reliably, and that after
//! a simulated fault (mock server returning errors), heartbeats
//! resume when the fault is cleared.

use sim_tests::mock_server::EndpointFault;
use std::time::Duration;

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("warn")
        .try_init();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn daemon_sends_heartbeats() {
    init_tracing();
    let (addr, state) = sim_tests::start_mock_server().await;
    let (shutdown_tx, instance_id) = sim_tests::start_sim_daemon(addr).await;

    // Wait for at least 2 heartbeats (health interval is 1s)
    let ok = sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(100), || {
        state.heartbeats_from(&instance_id).len() >= 2
    })
    .await;
    assert!(ok, "expected at least 2 heartbeats within 10s, got {}", state.heartbeats_from(&instance_id).len());

    // Verify heartbeat content
    for hb in &state.heartbeats_from(&instance_id) {
        assert_eq!(hb.body.instance_id, instance_id);
        assert!(!hb.body.version.is_empty());
    }

    let _ = shutdown_tx.send(());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn heartbeat_resumes_after_server_fault() {
    init_tracing();
    let (addr, state) = sim_tests::start_mock_server().await;
    let (shutdown_tx, instance_id) = sim_tests::start_sim_daemon(addr).await;

    // Wait for first heartbeat
    let ok = sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(100), || {
        !state.heartbeats_from(&instance_id).is_empty()
    })
    .await;
    assert!(ok, "expected at least 1 heartbeat before fault injection");

    // Inject fault: heartbeat endpoint returns 500
    state.set_fault(
        "/api/heartbeat",
        EndpointFault {
            fail_status: Some(500),
            ..Default::default()
        },
    );

    let count_before_fault = state.heartbeats_from(&instance_id).len();
    tokio::time::sleep(Duration::from_secs(3)).await;
    let count_during_fault = state.heartbeats_from(&instance_id).len();
    assert_eq!(count_before_fault, count_during_fault, "no new heartbeats during 500 fault");

    // Clear the fault
    state.clear_faults();

    // Heartbeats should resume
    let resumed = sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(100), || {
        state.heartbeats_from(&instance_id).len() > count_during_fault
    })
    .await;
    assert!(resumed, "heartbeat should resume after fault is cleared");

    let _ = shutdown_tx.send(());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multiple_daemons_send_heartbeats() {
    init_tracing();
    let (addr, state) = sim_tests::start_mock_server().await;

    let mut shutdowns = Vec::new();
    let mut instance_ids = Vec::new();

    for _ in 0..3 {
        let (tx, iid) = sim_tests::start_sim_daemon(addr).await;
        shutdowns.push(tx);
        instance_ids.push(iid);
    }

    // Wait for at least 1 heartbeat from each
    for iid in &instance_ids {
        let iid = iid.clone();
        let ok = sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(100), || {
            !state.heartbeats_from(&iid).is_empty()
        })
        .await;
        assert!(ok, "daemon {iid} should send at least 1 heartbeat");
    }

    // Verify all instance IDs are unique
    let unique: std::collections::HashSet<&str> = instance_ids.iter().map(|s| s.as_str()).collect();
    assert_eq!(unique.len(), instance_ids.len(), "all instance IDs should be unique");

    for tx in shutdowns {
        let _ = tx.send(());
    }
}
