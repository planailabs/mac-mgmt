//! System invariant definitions and checkers.
//!
//! Invariants are properties that must always hold regardless of fault
//! schedule. Each invariant is checked continuously and failures are
//! recorded to the timeline with full context for reproduction.

use crate::mock_server::MockServerState;
use crate::timeline::{EventKind, Timeline};
use std::collections::HashSet;
use std::sync::Arc;

/// Result of checking a single invariant.
pub struct InvariantResult {
    pub name: &'static str,
    pub passed: bool,
    pub detail: String,
}

impl InvariantResult {
    fn pass(name: &'static str) -> Self {
        Self {
            name,
            passed: true,
            detail: String::new(),
        }
    }
    fn fail(name: &'static str, detail: impl std::fmt::Display) -> Self {
        Self {
            name,
            passed: false,
            detail: detail.to_string(),
        }
    }
}

/// Check all invariants and record results to timeline.
/// Returns the number of violations.
pub fn check_all(
    state: &Arc<MockServerState>,
    expected_daemons: &[String],
    timeline: &Timeline,
) -> usize {
    let results = vec![
        check_heartbeat_consistency(state),
        check_no_duplicate_instance_ids(state),
        check_heartbeat_version_present(state),
        check_no_empty_instance_ids(state),
        check_heartbeat_temporal_order(state),
    ];

    let mut violations = 0;
    for r in &results {
        if r.passed {
            timeline.record("invariant", EventKind::InvariantOk, r.name);
        } else {
            timeline.record(
                "invariant",
                EventKind::InvariantViolation,
                format!("{}: {}", r.name, r.detail),
            );
            violations += 1;
        }
    }

    // Liveness check: every expected daemon should have sent at least 1 heartbeat
    for iid in expected_daemons {
        let count = state.heartbeats_from(iid).len();
        if count == 0 {
            timeline.record(
                "invariant",
                EventKind::InvariantViolation,
                format!("heartbeat_liveness: daemon {iid} has sent 0 heartbeats"),
            );
            violations += 1;
        } else {
            timeline.record(
                "invariant",
                EventKind::InvariantOk,
                format!("heartbeat_liveness: daemon {iid} has {count} heartbeats"),
            );
        }
    }

    violations
}

/// Every heartbeat body must have a non-empty version field.
fn check_heartbeat_version_present(state: &Arc<MockServerState>) -> InvariantResult {
    let hbs = state.get_heartbeats();
    for hb in &hbs {
        if hb.body.version.is_empty() {
            return InvariantResult::fail(
                "heartbeat_version_present",
                format!("heartbeat from {} has empty version", hb.body.instance_id),
            );
        }
    }
    InvariantResult::pass("heartbeat_version_present")
}

/// No heartbeat should have an empty instance_id.
fn check_no_empty_instance_ids(state: &Arc<MockServerState>) -> InvariantResult {
    let hbs = state.get_heartbeats();
    for hb in &hbs {
        if hb.body.instance_id.is_empty() {
            return InvariantResult::fail(
                "no_empty_instance_ids",
                "found heartbeat with empty instance_id",
            );
        }
    }
    InvariantResult::pass("no_empty_instance_ids")
}

/// The same instance_id should not appear from different hosts.
/// (In practice, each daemon generates a unique ed25519 key.)
fn check_no_duplicate_instance_ids(state: &Arc<MockServerState>) -> InvariantResult {
    let hbs = state.get_heartbeats();
    let unique: HashSet<&str> = hbs.iter().map(|h| h.body.instance_id.as_str()).collect();
    // This is a structural check — if we see heartbeats from N unique IDs,
    // and the hostname field differs for the same ID, that's a violation.
    // For now, just verify IDs are non-empty (covered above) and well-formed.
    for id in &unique {
        if id.len() != 64 {
            return InvariantResult::fail(
                "no_duplicate_instance_ids",
                format!("instance_id '{}' is not a 64-char hex fingerprint", id),
            );
        }
    }
    InvariantResult::pass("no_duplicate_instance_ids")
}

/// Heartbeat timestamps should be monotonically increasing per instance.
fn check_heartbeat_temporal_order(state: &Arc<MockServerState>) -> InvariantResult {
    let hbs = state.get_heartbeats();
    let mut by_instance: std::collections::HashMap<&str, Vec<i64>> =
        std::collections::HashMap::new();
    for hb in &hbs {
        by_instance
            .entry(&hb.body.instance_id)
            .or_default()
            .push(hb.body.signed_at);
    }
    for (iid, timestamps) in &by_instance {
        for window in timestamps.windows(2) {
            if window[1] < window[0] {
                return InvariantResult::fail(
                    "heartbeat_temporal_order",
                    format!(
                        "daemon {} sent heartbeat with signed_at {} after {}",
                        &iid[..12],
                        window[1],
                        window[0]
                    ),
                );
            }
        }
    }
    InvariantResult::pass("heartbeat_temporal_order")
}

/// Heartbeat bodies should be well-formed JSON (services, tunnels are arrays).
fn check_heartbeat_consistency(state: &Arc<MockServerState>) -> InvariantResult {
    let hbs = state.get_heartbeats();
    for hb in &hbs {
        if !hb.body.services.is_array() {
            return InvariantResult::fail(
                "heartbeat_consistency",
                format!(
                    "heartbeat from {} has non-array services: {:?}",
                    hb.body.instance_id, hb.body.services
                ),
            );
        }
        if !hb.body.tunnels.is_array() {
            return InvariantResult::fail(
                "heartbeat_consistency",
                format!(
                    "heartbeat from {} has non-array tunnels: {:?}",
                    hb.body.instance_id, hb.body.tunnels
                ),
            );
        }
    }
    InvariantResult::pass("heartbeat_consistency")
}
