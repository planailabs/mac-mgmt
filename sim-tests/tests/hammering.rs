//! Hammering tests — high-concurrency stress tests.
//!
//! These tests push the daemon hard: many daemons, rapid-fire pushes,
//! simultaneous fault injection, and short intervals. The goal is to
//! find race conditions and resource leaks.

use sim_tests::mock_server::EndpointFault;
use sim_tests::timeline::Timeline;
use std::time::Duration;

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "warn".to_string()))
        .try_init();
}

/// 10 daemons against one server with 100ms health interval.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn hammer_many_daemons() {
    init_tracing();
    let (addr, state) = sim_tests::start_mock_server().await;

    let mut shutdowns = Vec::new();
    let mut instance_ids = Vec::new();

    for _ in 0..10 {
        let cfg = sim_tests::daemon_config_with_intervals(addr, "500ms", "5s");
        let (tx, iid) = sim_tests::start_sim_daemon_with_config(cfg).await;
        shutdowns.push(tx);
        instance_ids.push(iid);
    }

    // Wait for all 10 to send at least 1 heartbeat
    let ok = sim_tests::wait_until(Duration::from_secs(20), Duration::from_millis(200), || {
        instance_ids
            .iter()
            .all(|iid| !state.heartbeats_from(iid).is_empty())
    })
    .await;
    assert!(ok, "all 10 daemons should send heartbeats");

    // Verify all unique
    let unique: std::collections::HashSet<&str> = instance_ids.iter().map(|s| s.as_str()).collect();
    assert_eq!(unique.len(), 10);

    // Now hammer with pushes while toggling faults
    for i in 0..30 {
        if i % 7 == 0 {
            state.set_fault(
                "/api/heartbeat",
                EndpointFault {
                    fail_status: Some(503),
                    ..Default::default()
                },
            );
        }
        if i % 7 == 4 {
            state.clear_faults();
        }
        state.push(mac_mgmt_common::PushEvent::SyncSkills);
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    state.clear_faults();

    // Wait for recovery
    state.clear_heartbeats();
    let recovered =
        sim_tests::wait_until(Duration::from_secs(15), Duration::from_millis(200), || {
            instance_ids
                .iter()
                .all(|iid| !state.heartbeats_from(iid).is_empty())
        })
        .await;
    assert!(recovered, "all 10 daemons should recover after hammering");

    // Check invariants
    let timeline = Timeline::new();
    let violations = sim_tests::invariants::check_all(&state, &instance_ids, &timeline);
    assert_eq!(
        violations,
        0,
        "invariant violations:\n{}",
        timeline.format_violations()
    );

    for tx in shutdowns {
        let _ = tx.send(());
    }
}

/// Rapid push storm: 100 pushes in 2 seconds across all event types.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hammer_push_storm() {
    init_tracing();
    let (addr, state) = sim_tests::start_mock_server().await;
    let (shutdown_tx, instance_id) = sim_tests::start_sim_daemon(addr).await;

    // Wait for SSE connection
    let ok = sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(100), || {
        state.request_count("/api/events") > 0
    })
    .await;
    assert!(ok, "daemon should connect to SSE");

    // Fire 100 pushes rapidly
    let push_types = [
        mac_mgmt_common::PushEvent::SyncSkills,
        mac_mgmt_common::PushEvent::SyncMcpServers,
        mac_mgmt_common::PushEvent::SyncSshKeys,
        mac_mgmt_common::PushEvent::SyncNixpkgs,
        mac_mgmt_common::PushEvent::SyncConfig,
    ];

    for i in 0..100 {
        state.push(push_types[i % push_types.len()].clone());
        if i % 10 == 0 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    // Give time to process
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Daemon should still be alive
    state.clear_heartbeats();
    let alive = sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(200), || {
        !state.heartbeats_from(&instance_id).is_empty()
    })
    .await;
    assert!(alive, "daemon should survive push storm");

    // Skills endpoint should have been hit at least once from the storm
    assert!(
        state.request_count("/api/skills") >= 1,
        "skills should be fetched at least once: got {}",
        state.request_count("/api/skills")
    );

    let _ = shutdown_tx.send(());
}

/// Endpoint fault cycling at high speed: toggle faults every 100ms.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hammer_rapid_fault_cycling() {
    init_tracing();
    let (addr, state) = sim_tests::start_mock_server().await;
    let (shutdown_tx, instance_id) = sim_tests::start_sim_daemon(addr).await;

    // Wait for initial heartbeat
    let ok = sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(100), || {
        !state.heartbeats_from(&instance_id).is_empty()
    })
    .await;
    assert!(ok, "should get initial heartbeat");

    let endpoints = [
        "/api/heartbeat",
        "/api/config",
        "/api/skills",
        "/api/mcp-servers",
        "/api/update",
        "/api/nixpkgs",
    ];

    // Toggle faults rapidly
    for i in 0..60 {
        let ep = endpoints[i % endpoints.len()];
        if i % 2 == 0 {
            state.set_fault(
                ep,
                EndpointFault {
                    fail_status: Some(500),
                    ..Default::default()
                },
            );
        } else {
            state.faults.lock().unwrap().remove(ep);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    state.clear_faults();

    // Daemon should recover
    state.clear_heartbeats();
    let recovered =
        sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(200), || {
            !state.heartbeats_from(&instance_id).is_empty()
        })
        .await;
    assert!(recovered, "daemon should recover from rapid fault cycling");

    let timeline = Timeline::new();
    let violations = sim_tests::invariants::check_all(&state, &[instance_id.clone()], &timeline);
    assert_eq!(
        violations,
        0,
        "invariant violations:\n{}",
        timeline.format_violations()
    );

    let _ = shutdown_tx.send(());
}

/// Simultaneous daemon starts: all 5 daemons start at the exact same time.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn hammer_simultaneous_start() {
    init_tracing();
    let (addr, state) = sim_tests::start_mock_server().await;

    // Start all 5 at once (no awaiting between starts)
    let mut shutdowns = Vec::new();
    let mut instance_ids = Vec::new();

    let mut handles = Vec::new();
    for _ in 0..5 {
        handles.push(tokio::spawn({
            let addr = addr;
            async move { sim_tests::start_sim_daemon(addr).await }
        }));
    }

    for h in handles {
        let (tx, iid) = h.await.unwrap();
        shutdowns.push(tx);
        instance_ids.push(iid);
    }

    // All should eventually send heartbeats
    let ok = sim_tests::wait_until(Duration::from_secs(15), Duration::from_millis(200), || {
        instance_ids
            .iter()
            .all(|iid| !state.heartbeats_from(iid).is_empty())
    })
    .await;
    assert!(
        ok,
        "all 5 simultaneously-started daemons should send heartbeats"
    );

    // All should be unique
    let unique: std::collections::HashSet<&str> = instance_ids.iter().map(|s| s.as_str()).collect();
    assert_eq!(unique.len(), 5, "all IDs should be unique");

    for tx in shutdowns {
        let _ = tx.send(());
    }
}
