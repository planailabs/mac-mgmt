//! Targeted chaos scenarios with specific fault patterns.
//!
//! Unlike the random chaos tests, these use scenario generators tuned for
//! specific failure modes. Each still uses seeds for reproduction.

use sim_tests::scenarios;
use sim_tests::timeline::Timeline;
use std::time::Duration;

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "warn".to_string()),
        )
        .try_init();
}

fn get_seed() -> u64 {
    std::env::var("CHAOS_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(scenarios::random_seed)
}

/// Run a targeted scenario, checking invariants and liveness.
async fn run_targeted(
    schedule: &scenarios::FaultSchedule,
    timeline: &Timeline,
) -> Result<(), String> {
    let (addr, state) = sim_tests::start_mock_server().await;

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
            format!("iid={}", &iid[..12]),
        );
        shutdowns.push(tx);
        instance_ids.push(iid);
    }

    // Wait for initial heartbeats
    let init_ok = sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(100), || {
        instance_ids.iter().all(|iid| !state.heartbeats_from(iid).is_empty())
    }).await;

    if !init_ok {
        for tx in shutdowns { let _ = tx.send(()); }
        return Err(format!("initial heartbeat timeout\n{}", timeline.format_full()));
    }

    // Execute schedule
    scenarios::execute(schedule, &state, &timeline).await;

    // Settle
    tokio::time::sleep(schedule.settle_time).await;

    // Check invariants
    let violations = sim_tests::invariants::check_all(&state, &instance_ids, &timeline);

    // Post-settle liveness
    state.clear_heartbeats();
    let liveness_ok = sim_tests::wait_until(Duration::from_secs(10), Duration::from_millis(200), || {
        instance_ids.iter().all(|iid| !state.heartbeats_from(iid).is_empty())
    }).await;

    for tx in shutdowns { let _ = tx.send(()); }

    let total = violations + if liveness_ok { 0 } else { 1 };
    if total > 0 {
        Err(format!(
            "{violations} invariant violation(s), liveness={}\n{}\nSchedule:\n{schedule}",
            if liveness_ok { "OK" } else { "FAIL" },
            timeline.format_full(),
        ))
    } else {
        Ok(())
    }
}

/// Config churn: rapid SyncConfig pushes with intermittent /api/config failures.
#[tokio::test]
async fn targeted_config_churn() {
    init_tracing();
    let seed = get_seed();
    let timeline = Timeline::new();
    let schedule = scenarios::generate_config_churn(seed, 15);

    eprintln!("targeted_config_churn: seed={seed}, {} events", schedule.events.len());

    if let Err(msg) = run_targeted(&schedule, &timeline).await {
        panic!(
            "Config churn FAILED (CHAOS_SEED={seed})\n{msg}"
        );
    }
}

/// Endpoint cycling: rapidly toggle individual endpoint faults.
#[tokio::test]
async fn targeted_endpoint_cycling() {
    init_tracing();
    let seed = get_seed();
    let timeline = Timeline::new();
    let schedule = scenarios::generate_endpoint_cycling(seed, 10);

    eprintln!("targeted_endpoint_cycling: seed={seed}, {} events", schedule.events.len());

    if let Err(msg) = run_targeted(&schedule, &timeline).await {
        panic!(
            "Endpoint cycling FAILED (CHAOS_SEED={seed})\n{msg}"
        );
    }
}

/// Cascading failure: all endpoints down simultaneously, then gradual recovery.
#[tokio::test]
async fn targeted_cascading_failure() {
    init_tracing();
    let seed = get_seed();
    let timeline = Timeline::new();
    let schedule = scenarios::generate_cascading_failure(seed);

    eprintln!("targeted_cascading_failure: seed={seed}, {} events", schedule.events.len());

    if let Err(msg) = run_targeted(&schedule, &timeline).await {
        panic!(
            "Cascading failure FAILED (CHAOS_SEED={seed})\n{msg}"
        );
    }
}
