//! Failure sampling and minimization for Antithesis-style bug discovery.
//!
//! Runs many chaos rounds with different seeds, collects failures, then
//! attempts to minimize failing schedules to the smallest reproducing case.
//! This is the core of autonomous bug finding — instead of hand-writing
//! scenarios, we search the space of possible fault schedules.

use crate::scenarios;
use crate::timeline::{EventKind, Timeline};
use std::time::Duration;

/// Result of a sampling run.
#[derive(Debug)]
pub struct SamplingReport {
    pub total_rounds: usize,
    pub failures: Vec<FailureCase>,
    pub seed_range: (u64, u64),
}

/// A single discovered failure with reproduction info.
#[derive(Debug)]
pub struct FailureCase {
    pub seed: u64,
    pub violation: String,
    pub timeline_snapshot: String,
    pub schedule_description: String,
    /// Minimized seed if shrinking was performed.
    pub minimized_seed: Option<u64>,
}

impl std::fmt::Display for FailureCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "┌─ Failure (seed={})", self.seed)?;
        if let Some(min) = self.minimized_seed {
            writeln!(f, "│  Minimized seed: {min}")?;
        }
        writeln!(f, "│  Violation: {}", self.violation)?;
        writeln!(f, "│  Reproduce: CHAOS_SEED={} cargo test -p sim-tests chaos_random_faults", self.seed)?;
        writeln!(f, "└─")?;
        Ok(())
    }
}

impl std::fmt::Display for SamplingReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "═══ SAMPLING REPORT ═══")?;
        writeln!(
            f,
            "Rounds: {}, Failures: {}, Seeds: {}..{}",
            self.total_rounds,
            self.failures.len(),
            self.seed_range.0,
            self.seed_range.1
        )?;
        if self.failures.is_empty() {
            writeln!(f, "No failures found.")?;
        } else {
            for (i, failure) in self.failures.iter().enumerate() {
                writeln!(f, "\nFailure #{}: {failure}", i + 1)?;
            }
        }
        writeln!(f, "═══ END REPORT ═══")?;
        Ok(())
    }
}

/// Run a single round and return the failure info if invariants are violated.
pub async fn run_round(
    seed: u64,
    num_events: usize,
    num_daemons: usize,
) -> Option<FailureCase> {
    let timeline = Timeline::new();
    let schedule = scenarios::generate(seed, num_events, num_daemons);

    timeline.record(
        "sampler",
        EventKind::Custom,
        format!("seed={seed} events={} daemons={num_daemons}", schedule.events.len()),
    );

    let (addr, state) = crate::mock_server::start().await;

    // Start daemons
    let mut shutdowns = Vec::new();
    let mut instance_ids = Vec::new();
    for i in 0..schedule.num_daemons {
        let cfg = crate::daemon_config_with_intervals(
            addr,
            &format!("{}ms", schedule.health_interval.as_millis()),
            "10s",
        );
        let (tx, iid) = crate::start_sim_daemon_with_config(cfg).await;
        timeline.record(
            &format!("daemon-{i}"),
            EventKind::DaemonStarted,
            format!("iid={}", &iid[..12]),
        );
        shutdowns.push(tx);
        instance_ids.push(iid);
    }

    // Wait for initial heartbeats
    let init_ok = crate::wait_until(Duration::from_secs(10), Duration::from_millis(100), || {
        instance_ids.iter().all(|iid| !state.heartbeats_from(iid).is_empty())
    }).await;

    if !init_ok {
        for tx in shutdowns { let _ = tx.send(()); }
        return Some(FailureCase {
            seed,
            violation: "initial heartbeat timeout".to_string(),
            timeline_snapshot: timeline.format_full(),
            schedule_description: format!("{schedule}"),
            minimized_seed: None,
        });
    }

    // Execute faults
    scenarios::execute(&schedule, &state, &timeline).await;

    // Settle
    tokio::time::sleep(schedule.settle_time).await;

    // Check invariants
    let violations = crate::invariants::check_all(&state, &instance_ids, &timeline);

    // Post-settle liveness
    state.clear_heartbeats();
    let liveness_ok = crate::wait_until(Duration::from_secs(10), Duration::from_millis(200), || {
        instance_ids.iter().all(|iid| !state.heartbeats_from(iid).is_empty())
    }).await;

    for tx in shutdowns { let _ = tx.send(()); }

    let total_violations = violations + if liveness_ok { 0 } else { 1 };

    if total_violations > 0 {
        let violation_text = if !liveness_ok {
            let stalled: Vec<_> = instance_ids.iter()
                .filter(|iid| state.heartbeats_from(iid).is_empty())
                .map(|iid| &iid[..12])
                .collect();
            format!("{violations} invariant violation(s) + liveness failure for {stalled:?}")
        } else {
            format!("{violations} invariant violation(s)")
        };

        Some(FailureCase {
            seed,
            violation: violation_text,
            timeline_snapshot: timeline.format_full(),
            schedule_description: format!("{schedule}"),
            minimized_seed: None,
        })
    } else {
        None
    }
}

/// Run a sampling campaign: many rounds with different seeds.
/// If a failure is found, attempt to shrink it.
pub async fn sample(
    base_seed: u64,
    rounds: usize,
    num_events: usize,
    num_daemons: usize,
) -> SamplingReport {
    let mut failures = Vec::new();

    for i in 0..rounds {
        let seed = base_seed.wrapping_add(i as u64);

        if let Some(mut failure) = run_round(seed, num_events, num_daemons).await {
            // Attempt to shrink: try with fewer events
            let minimized = shrink(seed, num_events, num_daemons).await;
            failure.minimized_seed = minimized;
            failures.push(failure);
        }
    }

    SamplingReport {
        total_rounds: rounds,
        failures,
        seed_range: (base_seed, base_seed.wrapping_add(rounds as u64 - 1)),
    }
}

/// Attempt to find a smaller schedule that still reproduces the failure.
/// Binary search on event count: if fewer events still fail, that's simpler.
async fn shrink(seed: u64, max_events: usize, num_daemons: usize) -> Option<u64> {
    // Try halving the event count
    let mut low = 1;
    let mut high = max_events;
    let mut smallest_failing = None;

    while low <= high {
        let mid = (low + high) / 2;
        if run_round(seed, mid, num_daemons).await.is_some() {
            smallest_failing = Some(seed);
            high = mid - 1;
        } else {
            low = mid + 1;
        }
    }

    smallest_failing
}
