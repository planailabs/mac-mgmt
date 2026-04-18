//! Seed-based fault schedule generation for reproducible chaos testing.
//!
//! Every scenario is driven by a u64 seed. Given the same seed, the same
//! fault schedule is generated, enabling exact reproduction of failures.
//! When a test fails, print the seed — re-running with that seed reproduces
//! the exact same sequence of faults.

use crate::mock_server::{EndpointFault, MockServerState};
use crate::timeline::{EventKind, Timeline};
use mac_mgmt_common::PushEvent;
use std::sync::Arc;
use std::time::Duration;

/// A fault event that can be applied to the mock server.
#[derive(Debug, Clone)]
pub enum FaultEvent {
    /// Make an endpoint return an error status.
    FailEndpoint {
        endpoint: &'static str,
        status: u16,
    },
    /// Restore an endpoint to normal operation.
    RecoverEndpoint {
        endpoint: &'static str,
    },
    /// Push a command via SSE.
    Push(PushEvent),
    /// Clear all faults.
    ClearAllFaults,
    /// Do nothing (pad the schedule).
    Noop,
}

/// A timed sequence of fault events.
#[derive(Debug, Clone)]
pub struct FaultSchedule {
    pub seed: u64,
    pub events: Vec<(Duration, FaultEvent)>,
    pub num_daemons: usize,
    pub health_interval: Duration,
    pub settle_time: Duration,
}

impl std::fmt::Display for FaultSchedule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "FaultSchedule(seed={}, daemons={}, events={})", self.seed, self.num_daemons, self.events.len())?;
        for (t, e) in &self.events {
            writeln!(f, "  +{:.1}s  {:?}", t.as_secs_f64(), e)?;
        }
        Ok(())
    }
}

/// Simple PRNG for deterministic schedule generation (xorshift64).
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // Avoid seed=0 which is a fixed point of xorshift
        Self(seed | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn range(&mut self, min: u64, max: u64) -> u64 {
        min + self.next() % (max - min + 1)
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.next() as usize % items.len()]
    }

    fn bool(&mut self, probability_pct: u64) -> bool {
        self.range(0, 99) < probability_pct
    }
}

const ENDPOINTS: &[&str] = &[
    "/api/heartbeat",
    "/api/config",
    "/api/update",
    "/api/nixpkgs",
    "/api/skills",
    "/api/mcp-servers",
    "/api/ssh-keys",
];

const PUSH_EVENTS: &[PushEvent] = &[
    PushEvent::SyncConfig,
    PushEvent::SyncSkills,
    PushEvent::SyncMcpServers,
    PushEvent::SyncSshKeys,
    PushEvent::SyncNixpkgs,
];

const ERROR_STATUSES: &[u16] = &[500, 502, 503, 429];

/// Generate a fault schedule from a seed.
pub fn generate(seed: u64, num_events: usize, num_daemons: usize) -> FaultSchedule {
    let mut rng = Rng::new(seed);
    let health_interval = Duration::from_millis(rng.range(500, 2000));
    let mut events = Vec::with_capacity(num_events);
    let mut time_cursor = Duration::from_secs(0);

    for _ in 0..num_events {
        // Advance time by 100ms - 3s
        time_cursor += Duration::from_millis(rng.range(100, 3000));

        let event = match rng.range(0, 5) {
            0 => {
                // Fail an endpoint
                let endpoint = rng.pick(ENDPOINTS);
                let status = *rng.pick(ERROR_STATUSES);
                FaultEvent::FailEndpoint {
                    endpoint,
                    status,
                }
            }
            1 => {
                // Recover an endpoint
                let endpoint = rng.pick(ENDPOINTS);
                FaultEvent::RecoverEndpoint { endpoint }
            }
            2 => {
                // Push an SSE event
                let push = rng.pick(PUSH_EVENTS).clone();
                FaultEvent::Push(push)
            }
            3 => {
                // Clear all faults
                FaultEvent::ClearAllFaults
            }
            _ => FaultEvent::Noop,
        };

        events.push((time_cursor, event));
    }

    // Always end with clearing all faults
    time_cursor += Duration::from_millis(100);
    events.push((time_cursor, FaultEvent::ClearAllFaults));

    let settle_time = Duration::from_millis(health_interval.as_millis() as u64 * 3);

    FaultSchedule {
        seed,
        events,
        num_daemons,
        health_interval,
        settle_time,
    }
}

/// Execute a fault schedule against the mock server, recording to timeline.
pub async fn execute(
    schedule: &FaultSchedule,
    state: &Arc<MockServerState>,
    timeline: &Timeline,
) {
    let start = tokio::time::Instant::now();

    for (target_time, event) in &schedule.events {
        // Wait until the target time
        let now = start.elapsed();
        if *target_time > now {
            tokio::time::sleep(*target_time - now).await;
        }

        match event {
            FaultEvent::FailEndpoint { endpoint, status } => {
                state.set_fault(
                    endpoint,
                    EndpointFault {
                        fail_status: Some(*status),
                        ..Default::default()
                    },
                );
                timeline.record(
                    "scheduler",
                    EventKind::FaultInjected,
                    format!("{endpoint} → {status}"),
                );
            }
            FaultEvent::RecoverEndpoint { endpoint } => {
                state.faults.lock().unwrap().remove(*endpoint);
                timeline.record(
                    "scheduler",
                    EventKind::FaultCleared,
                    format!("{endpoint} recovered"),
                );
            }
            FaultEvent::Push(push) => {
                state.push(push.clone());
                timeline.record(
                    "scheduler",
                    EventKind::Push,
                    format!("{push:?}"),
                );
            }
            FaultEvent::ClearAllFaults => {
                state.clear_faults();
                timeline.record("scheduler", EventKind::FaultCleared, "all faults cleared");
            }
            FaultEvent::Noop => {}
        }
    }
}

/// Generate a seed from the current time (for non-deterministic runs).
pub fn random_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}
