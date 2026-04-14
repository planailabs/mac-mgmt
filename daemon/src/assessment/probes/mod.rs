//! Deep probes: end-to-end functional validation of managed services.
//!
//! Every probe implements [`Probe`] and is registered in [`registry`]. Each probe
//! is expensive by design — e.g. a full prompt round-trip against the LLM backend.
//! Do not run on the heartbeat cadence.
//!
//! The concrete per-service implementations land in step 7.

use std::time::{Duration, Instant};

use async_trait::async_trait;

/// Classification used both for scheduling and for rollout gate configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeKind {
    /// Pid/port check — cheap.
    Liveness,
    /// Full end-to-end round-trip through the service's primary API
    /// (e.g. prompt→completion for an LLM). The main signal for rollout gates.
    Functional,
    /// Compares current output digest against a stored baseline to catch silent
    /// regressions on the same canary prompt.
    Regression,
}

impl ProbeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Liveness => "liveness",
            Self::Functional => "functional",
            Self::Regression => "regression",
        }
    }
}

/// Per-probe outcome. Maps 1:1 to `mac_mgmt_common::ProbeReport`.
#[derive(Debug, Default)]
pub struct ProbeResult {
    pub ok: bool,
    pub duration_ms: u64,
    pub tokens_in: Option<u32>,
    pub tokens_out: Option<u32>,
    pub first_token_ms: Option<u64>,
    pub model: Option<String>,
    pub canary_digest: Option<String>,
    pub error_class: Option<String>,
    pub error_detail: Option<String>,
}

/// Context threaded into every probe run — timeouts, config snapshots, etc.
#[derive(Debug, Clone)]
pub struct ProbeCtx {
    pub timeout: Duration,
}

impl Default for ProbeCtx {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
        }
    }
}

#[async_trait]
pub trait Probe: Send + Sync {
    fn name(&self) -> &'static str;
    fn kind(&self) -> ProbeKind;
    async fn run(&self, ctx: &ProbeCtx) -> ProbeResult;
}

/// Time a fallible probe body and fill in `duration_ms` / error fields.
#[allow(dead_code)]
pub async fn timed<F, Fut>(body: F) -> ProbeResult
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<ProbeResult, anyhow::Error>>,
{
    let started = Instant::now();
    let mut result = body().await.unwrap_or_else(|e| ProbeResult {
        ok: false,
        error_class: Some(classify_error(&e)),
        error_detail: Some(format!("{e:#}")),
        ..Default::default()
    });
    result.duration_ms = started.elapsed().as_millis() as u64;
    result
}

#[allow(dead_code)]
fn classify_error(e: &anyhow::Error) -> String {
    let s = format!("{e:#}").to_lowercase();
    if s.contains("timeout") || s.contains("timed out") {
        "timeout".into()
    } else if s.contains("connection") || s.contains("refused") {
        "connection".into()
    } else if s.contains("pull") {
        "pull_failed".into()
    } else {
        "error".into()
    }
}

/// Return the registered probes. For now, empty — real probes land in step 7.
pub fn registry() -> Vec<Box<dyn Probe>> {
    Vec::new()
}
