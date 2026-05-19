//! Deep probes: end-to-end functional validation of managed services.
//!
//! Every probe implements [`Probe`] and is registered in [`registry`]. Each probe
//! is expensive by design — a full-prompt round-trip against an LLM backend can
//! take tens of seconds. Do **not** run on the heartbeat cadence.

pub mod ai_proxy;
pub mod apprise;
pub mod custom;
pub mod hermes;
pub mod lms;
pub mod mcporter;
pub mod ollama;
pub mod openclaw;
pub mod opencode;

use std::time::{Duration, Instant};

use async_trait::async_trait;
use mac_mgmt_common::DaemonConfig;
use serde::{Deserialize, Serialize};

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

/// Did the LLM respond with real content?
///
/// The probe's job is to verify the pipeline (daemon → backend → model →
/// response) works end-to-end, not to enforce that a tiny canary model
/// followed the prompt verbatim. qwen3:0.6b and other sub-1B models
/// routinely paraphrase, omit casing, or pad responses in ways that break
/// a literal `.contains("READY")` check even when the pipeline is fine.
///
/// Accept any response with at least 4 non-whitespace characters. That
/// catches truly broken outputs (empty strings, whitespace-only,
/// stuck-on-one-token failure modes) without failing on model style.
/// The canary_digest is still emitted for regression detection where
/// operators care about output stability.
pub fn response_has_content(s: &str) -> bool {
    s.chars().filter(|c| !c.is_whitespace()).take(4).count() >= 4
}

/// Shorten a response for error reporting. Single-line, capped so an
/// accidental dump doesn't blow up a Sentry event.
pub fn snippet(s: &str) -> String {
    let one_line: String = s.chars().map(|c| if c == '\n' { ' ' } else { c }).collect();
    let trimmed = one_line.trim();
    if trimmed.chars().count() <= 120 {
        trimmed.to_string()
    } else {
        let mut out: String = trimmed.chars().take(120).collect();
        out.push('…');
        out
    }
}

// ── Shared OpenAI-compatible wire types for probe requests ──────────

#[derive(Serialize)]
pub struct ChatBody {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: f32,
    pub max_tokens: u32,
    pub stream: bool,
}

#[derive(Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct ChatResponse {
    #[serde(default)]
    pub model: String,
    pub choices: Vec<ChatChoice>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

#[derive(Deserialize)]
pub struct ChatChoice {
    pub message: ChatReplyMessage,
}

#[derive(Deserialize)]
pub struct ChatReplyMessage {
    pub content: String,
}

#[derive(Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

/// Build the full probe registry from the current daemon config. Called each
/// probe tick so config reloads take effect on the next run.
pub fn registry(cfg: &DaemonConfig) -> Vec<Box<dyn Probe>> {
    let mut probes: Vec<Box<dyn Probe>> = Vec::new();

    if cfg.ollama.enabled {
        probes.push(Box::new(ollama::OllamaProbe::from_config(&cfg.ollama)));
    }
    if cfg.lms.enabled {
        probes.push(Box::new(lms::LmsProbe::from_config(&cfg.lms)));
    }

    if cfg.openclaw.enabled {
        probes.push(Box::new(openclaw::OpenClawHealthProbe));
        probes.push(Box::new(openclaw::OpenClawProbe::new(
            &cfg.openclaw,
            crate::canary::openclaw_canary(cfg),
        )));
    }

    if cfg.opencode.enabled {
        probes.push(Box::new(opencode::OpencodeProbe::from_config(
            &cfg.opencode,
        )));
    }

    if cfg.hermes.enabled {
        probes.push(Box::new(hermes::HermesHealthProbe::new(&cfg.hermes)));
        probes.push(Box::new(hermes::HermesProbe::new(
            &cfg.hermes,
            crate::canary::hermes_canary(cfg),
        )));
    }

    // Always probe apprise + mcporter if configured (cheap liveness checks).
    probes.push(Box::new(apprise::AppriseProbe));
    probes.push(Box::new(mcporter::McPorterProbe));

    if cfg.ai_proxy.enabled {
        probes.push(Box::new(ai_proxy::AiProxyProbe::from_config(&cfg.ai_proxy)));
        if let Some(fp) = ai_proxy::AiProxyFunctionalProbe::from_config(&cfg.ai_proxy) {
            probes.push(Box::new(fp));
        }
    }

    // Custom service probes
    probes.extend(custom::CustomProbe::from_configs(&cfg.custom_services));

    probes
}
