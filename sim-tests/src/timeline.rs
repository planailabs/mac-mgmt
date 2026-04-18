//! Event timeline recorder for Antithesis-style failure reproduction.
//!
//! Captures all observable events (requests, responses, faults, pushes)
//! with timestamps so that failures can be replayed and debugged.
//! On failure, the timeline is printed as a human-readable log.

use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// A single recorded event in the simulation timeline.
#[derive(Clone)]
pub struct TimelineEvent {
    /// Time since simulation start.
    pub elapsed: std::time::Duration,
    /// Which actor produced this event.
    pub actor: String,
    /// What happened.
    pub kind: EventKind,
    /// Free-form detail.
    pub detail: String,
}

#[derive(Clone, Debug)]
pub enum EventKind {
    /// Mock server received a request.
    Request,
    /// Mock server sent a response.
    Response,
    /// SSE push sent to daemons.
    Push,
    /// Fault injected.
    FaultInjected,
    /// Fault cleared.
    FaultCleared,
    /// Daemon started.
    DaemonStarted,
    /// Daemon stopped.
    DaemonStopped,
    /// Invariant checked (passed).
    InvariantOk,
    /// Invariant violated.
    InvariantViolation,
    /// Custom event.
    Custom,
}

impl fmt::Display for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventKind::Request => write!(f, "REQ"),
            EventKind::Response => write!(f, "RSP"),
            EventKind::Push => write!(f, "PUSH"),
            EventKind::FaultInjected => write!(f, "FAULT+"),
            EventKind::FaultCleared => write!(f, "FAULT-"),
            EventKind::DaemonStarted => write!(f, "START"),
            EventKind::DaemonStopped => write!(f, "STOP"),
            EventKind::InvariantOk => write!(f, "INV:OK"),
            EventKind::InvariantViolation => write!(f, "INV:FAIL"),
            EventKind::Custom => write!(f, "EVENT"),
        }
    }
}

impl fmt::Display for TimelineEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{:>8.3}s] {:>8} {:<12} {}",
            self.elapsed.as_secs_f64(),
            self.kind,
            self.actor,
            self.detail,
        )
    }
}

/// Thread-safe timeline recorder.
#[derive(Clone)]
pub struct Timeline {
    start: Instant,
    events: Arc<Mutex<Vec<TimelineEvent>>>,
}

impl Timeline {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Record an event.
    pub fn record(&self, actor: &str, kind: EventKind, detail: impl fmt::Display) {
        let event = TimelineEvent {
            elapsed: self.start.elapsed(),
            actor: actor.to_string(),
            kind,
            detail: detail.to_string(),
        };
        self.events.lock().unwrap().push(event);
    }

    /// Get all events.
    pub fn events(&self) -> Vec<TimelineEvent> {
        self.events.lock().unwrap().clone()
    }

    /// Format the full timeline as a string for printing on failure.
    pub fn format_full(&self) -> String {
        let events = self.events.lock().unwrap();
        let mut out = String::new();
        out.push_str("═══ SIMULATION TIMELINE ═══\n");
        for event in events.iter() {
            out.push_str(&format!("{event}\n"));
        }
        out.push_str("═══ END TIMELINE ═══\n");
        out
    }

    /// Format only invariant violations for quick scanning.
    pub fn format_violations(&self) -> String {
        let events = self.events.lock().unwrap();
        let violations: Vec<_> = events
            .iter()
            .filter(|e| matches!(e.kind, EventKind::InvariantViolation))
            .collect();
        if violations.is_empty() {
            return "No invariant violations.\n".to_string();
        }
        let mut out = String::new();
        out.push_str(&format!("═══ {} INVARIANT VIOLATION(S) ═══\n", violations.len()));
        for v in &violations {
            out.push_str(&format!("{v}\n"));
        }
        out.push_str("═══ END VIOLATIONS ═══\n");
        out
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}
