//! Deep probes: end-to-end functional validation of managed services.
//!
//! Every probe implements [`Probe`] and is registered in [`registry`]. Each probe
//! is expensive by design — a full-prompt round-trip against an LLM backend can
//! take tens of seconds. Do **not** run on the heartbeat cadence.

pub mod apprise;
pub mod lms;
pub mod mcporter;
pub mod ollama;
pub mod openclaw;

use std::time::{Duration, Instant};

use async_trait::async_trait;
use mac_mgmt_common::DaemonConfig;

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

/// Context threaded into every probe run — timeouts, canary prompt, etc.
#[derive(Debug, Clone)]
pub struct ProbeCtx {
    pub timeout: Duration,
    /// Fixed canary prompt. Deterministic, short, no user data — safe for GDPR.
    pub canary_prompt: String,
    /// Expected substring in the response. Used as a light validity check.
    pub canary_expected: String,
}

impl Default for ProbeCtx {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(60),
            canary_prompt: "Reply with exactly: READY-42. No other text.".into(),
            canary_expected: "READY".into(),
        }
    }
}

#[async_trait]
pub trait Probe: Send + Sync {
    fn name(&self) -> &'static str;
    fn kind(&self) -> ProbeKind;
    async fn run(&self, ctx: &ProbeCtx) -> ProbeResult;
}

/// Run a fallible probe body and fill in `duration_ms` / error fields.
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

pub fn classify_error(e: &anyhow::Error) -> String {
    let s = format!("{e:#}").to_lowercase();
    if s.contains("timeout") || s.contains("timed out") {
        "timeout".into()
    } else if s.contains("connection") || s.contains("refused") {
        "connection".into()
    } else if s.contains("pull") {
        "pull_failed".into()
    } else if s.contains("unexpected") || s.contains("mismatch") {
        "bad_response".into()
    } else {
        "error".into()
    }
}

/// Hex SHA-256 of a byte slice — used for canary response regression digests.
pub fn digest_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(data))
}

/// Build the full probe registry from the current daemon config. Called each
/// probe tick so config reloads take effect on the next run.
pub fn registry(cfg: &DaemonConfig) -> Vec<Box<dyn Probe>> {
    use mac_mgmt_common::{AgentProvider, LlmProvider};

    let mut probes: Vec<Box<dyn Probe>> = Vec::new();

    match cfg.global.llm_provider {
        LlmProvider::Ollama => {
            probes.push(Box::new(ollama::OllamaProbe::from_config(&cfg.ollama)));
        }
        LlmProvider::Lms => {
            probes.push(Box::new(lms::LmsProbe::from_config(&cfg.lms)));
        }
        LlmProvider::Cloud | LlmProvider::None => {}
    }

    match cfg.global.agent_provider {
        AgentProvider::Openclaw => {
            probes.push(Box::new(openclaw::OpenClawProbe::from_config(&cfg.openclaw)));
        }
        AgentProvider::None => {}
    }

    // Always probe apprise + mcporter if configured (cheap liveness checks).
    probes.push(Box::new(apprise::AppriseProbe));
    probes.push(Box::new(mcporter::McPorterProbe));

    probes
}
