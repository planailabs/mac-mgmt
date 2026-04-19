//! Antithesis-style chaos testing with seed-based reproduction.
//!
//! Each test run generates a random fault schedule from a seed. On failure,
//! the seed and full event timeline are printed so the exact scenario can
//! be reproduced by setting the CHAOS_SEED environment variable.
//!
//! Run a specific seed:  CHAOS_SEED=12345 cargo test -p sim-tests chaos
//! Run with more rounds: CHAOS_ROUNDS=50 cargo test -p sim-tests chaos

use sim_tests::scenarios;
use sim_tests::timeline::Timeline;
use std::time::Duration;

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "warn".to_string()))
        .try_init();
}

fn get_chaos_seed() -> Option<u64> {
    std::env::var("CHAOS_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
}

fn get_chaos_rounds() -> usize {
    std::env::var("CHAOS_ROUNDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5)
}

/// Run a single chaos round with the given seed.
/// Returns None on success, Some(failure_message) on failure.
async fn run_chaos_round(seed: u64) -> Option<String> {
    let timeline = Timeline::new();

    // Generate fault schedule
    let schedule = scenarios::generate(seed, 15, 2);
    timeline.record(
        "test",
        sim_tests::timeline::EventKind::Custom,
        format!("schedule: seed={seed}, events={}", schedule.events.len()),
    );

    // Start mock server
    let (addr, state) = sim_tests::start_mock_server().await;
    timeline.record(
        "test",
        sim_tests::timeline::EventKind::Custom,
        format!("mock server on {addr}"),
    );

    // Start daemons
    let mut shutdowns = Vec::new();
    let mut instance_ids = Vec::new();
    for i in 0..schedule.num_daemons {
        let cfg = sim_tests::daemon_config_with_intervals(
            addr,
            &format!("{}ms", schedule.health_interval.as_millis()),
            "10s",
        );
        let (tx, iid) = sim_tests::start_sim_daemon_with_config(cfg).await;
        timeline.record(
            &format!("daemon-{i}"),
            sim_tests::timeline::EventKind::DaemonStarted,
            format!("instance_id={iid}"),
        );
        shutdowns.push(tx);
        instance_ids.push(iid);
    }

    // Wait for initial heartbeats from all daemons
    let init_ok =
        sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(100), || {
            instance_ids
                .iter()
                .all(|iid| !state.heartbeats_from(iid).is_empty())
        })
        .await;

    if !init_ok {
        let missing: Vec<_> = instance_ids
            .iter()
            .filter(|iid| state.heartbeats_from(iid).is_empty())
            .collect();
        timeline.record(
            "test",
            sim_tests::timeline::EventKind::InvariantViolation,
            format!("daemons failed to send initial heartbeat: {missing:?}"),
        );

        for tx in shutdowns {
            let _ = tx.send(());
        }
        return Some(format!(
            "SEED={seed} — Initial heartbeat timeout\n{}",
            timeline.format_full()
        ));
    }

    timeline.record(
        "test",
        sim_tests::timeline::EventKind::Custom,
        "all daemons sent initial heartbeat",
    );

    // Execute fault schedule
    scenarios::execute(&schedule, &state, &timeline).await;

    // Settle period: wait for daemons to recover
    timeline.record(
        "test",
        sim_tests::timeline::EventKind::Custom,
        format!("settling for {:?}", schedule.settle_time),
    );
    tokio::time::sleep(schedule.settle_time).await;

    // Record heartbeat counts after settling
    for iid in &instance_ids {
        let count = state.heartbeats_from(iid).len();
        timeline.record(
            "test",
            sim_tests::timeline::EventKind::Custom,
            format!("daemon {}: {} total heartbeats", &iid[..12], count),
        );
    }

    // Check invariants
    let violations = sim_tests::invariants::check_all(&state, &instance_ids, &timeline);

    // Check post-settle liveness: heartbeats should be flowing
    state.clear_heartbeats();
    let liveness_ok =
        sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(200), || {
            instance_ids
                .iter()
                .all(|iid| !state.heartbeats_from(iid).is_empty())
        })
        .await;

    if !liveness_ok {
        let stalled: Vec<_> = instance_ids
            .iter()
            .filter(|iid| state.heartbeats_from(iid).is_empty())
            .map(|iid| &iid[..12])
            .collect();
        timeline.record(
            "test",
            sim_tests::timeline::EventKind::InvariantViolation,
            format!("post-settle liveness failure: daemons {stalled:?} not sending heartbeats"),
        );
    }

    // Shutdown
    for tx in shutdowns {
        let _ = tx.send(());
    }

    let total_violations = violations + if liveness_ok { 0 } else { 1 };

    if total_violations > 0 {
        Some(format!(
            "\n╔══════════════════════════════════════════╗\n\
             ║  CHAOS TEST FAILURE — SEED={seed:<14} ║\n\
             ║  Reproduce: CHAOS_SEED={seed} cargo test  ║\n\
             ╚══════════════════════════════════════════╝\n\n\
             {violations} invariant violation(s), liveness={}\n\n\
             {}\n\
             Schedule:\n{schedule}",
            if liveness_ok { "OK" } else { "FAIL" },
            timeline.format_full(),
        ))
    } else {
        None
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chaos_random_faults() {
    init_tracing();

    let rounds = get_chaos_rounds();
    let mut failures = Vec::new();

    if let Some(seed) = get_chaos_seed() {
        // Single deterministic seed
        eprintln!("chaos: running with fixed seed={seed}");
        if let Some(msg) = run_chaos_round(seed).await {
            failures.push(msg);
        }
    } else {
        // Multiple random seeds
        eprintln!("chaos: running {rounds} round(s) with random seeds");
        for round in 0..rounds {
            let seed = scenarios::random_seed().wrapping_add(round as u64);
            eprint!("  round {}/{rounds} seed={seed} ... ", round + 1);
            match run_chaos_round(seed).await {
                None => eprintln!("OK"),
                Some(msg) => {
                    eprintln!("FAIL");
                    failures.push(msg);
                }
            }
        }
    }

    if !failures.is_empty() {
        let mut report = format!(
            "\n{} of {} chaos round(s) failed:\n",
            failures.len(),
            rounds
        );
        for f in &failures {
            report.push_str(f);
            report.push('\n');
        }
        panic!("{report}");
    }
}

/// Targeted SSE stress: rapid push events while faults toggle.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chaos_sse_stress() {
    init_tracing();

    let timeline = Timeline::new();
    let (addr, state) = sim_tests::start_mock_server().await;
    let (shutdown_tx, instance_id) = sim_tests::start_sim_daemon(addr).await;

    // Wait for SSE connection
    let connected =
        sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(100), || {
            state.request_count("/api/events") > 0
        })
        .await;
    assert!(connected, "daemon should connect to SSE");

    // Rapid-fire pushes interleaved with faults
    for i in 0..20 {
        if i % 5 == 0 {
            state.set_fault(
                "/api/skills",
                sim_tests::mock_server::EndpointFault {
                    fail_status: Some(503),
                    ..Default::default()
                },
            );
            timeline.record(
                "test",
                sim_tests::timeline::EventKind::FaultInjected,
                "/api/skills → 503",
            );
        }
        if i % 5 == 3 {
            state.clear_faults();
            timeline.record(
                "test",
                sim_tests::timeline::EventKind::FaultCleared,
                "all faults cleared",
            );
        }

        state.push(mac_mgmt_common::PushEvent::SyncSkills);
        timeline.record("test", sim_tests::timeline::EventKind::Push, "SyncSkills");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    state.clear_faults();

    // After the storm, daemon should still be alive
    state.clear_heartbeats();
    let alive = sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(200), || {
        !state.heartbeats_from(&instance_id).is_empty()
    })
    .await;

    if !alive {
        panic!(
            "daemon stopped sending heartbeats after SSE stress\n{}",
            timeline.format_full()
        );
    }

    // Skills endpoint should have been hit
    assert!(
        state.request_count("/api/skills") > 0,
        "daemon should have fetched /api/skills at least once during stress test"
    );

    let _ = shutdown_tx.send(());
}

/// Heartbeat under sustained fault: server returns 500 for extended period.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chaos_sustained_heartbeat_fault() {
    init_tracing();

    let timeline = Timeline::new();
    let (addr, state) = sim_tests::start_mock_server().await;
    let (shutdown_tx, instance_id) = sim_tests::start_sim_daemon(addr).await;

    // Wait for initial heartbeat
    let got_initial =
        sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(100), || {
            !state.heartbeats_from(&instance_id).is_empty()
        })
        .await;
    assert!(got_initial, "should get initial heartbeat");

    // Sustained fault on heartbeat for 5 seconds
    state.set_fault(
        "/api/heartbeat",
        sim_tests::mock_server::EndpointFault {
            fail_status: Some(500),
            ..Default::default()
        },
    );
    timeline.record(
        "test",
        sim_tests::timeline::EventKind::FaultInjected,
        "/api/heartbeat → 500 for 5s",
    );

    let count_before = state.heartbeats_from(&instance_id).len();
    tokio::time::sleep(Duration::from_secs(5)).await;
    let count_during = state.heartbeats_from(&instance_id).len();

    // No new successful heartbeats during fault
    assert_eq!(
        count_before, count_during,
        "no heartbeats should succeed during 500 fault"
    );

    // Clear fault
    state.clear_faults();
    timeline.record(
        "test",
        sim_tests::timeline::EventKind::FaultCleared,
        "heartbeat fault cleared",
    );

    // Heartbeats should resume
    state.clear_heartbeats();
    let recovered =
        sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(200), || {
            !state.heartbeats_from(&instance_id).is_empty()
        })
        .await;

    if !recovered {
        panic!(
            "daemon did not resume heartbeats after 5s sustained fault\n{}",
            timeline.format_full()
        );
    }

    // Check invariants on recovered heartbeats
    let violations = sim_tests::invariants::check_all(&state, &[instance_id.clone()], &timeline);
    assert_eq!(
        violations,
        0,
        "invariant violations after recovery:\n{}",
        timeline.format_violations()
    );

    let _ = shutdown_tx.send(());
}
