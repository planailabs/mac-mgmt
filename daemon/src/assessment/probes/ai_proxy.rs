//! AI proxy probes:
//! - **Liveness** (`AiProxyProbe`): HTTP GET `/health` returns 200.
//! - **Functional** (`AiProxyFunctionalProbe`): discovers a model via
//!   `/v1/models`, then sends a canary prompt through `/v1/chat/completions`.

use anyhow::{Context, Result};
use async_trait::async_trait;
use mac_mgmt_common::AiProxyConfig;
use serde::{Deserialize, Serialize};

use super::{
    Probe, ProbeCtx, ProbeKind, ProbeResult, digest_hex, response_has_content, snippet, timed,
};

// ── Liveness probe: GET /health ─────────────────────────────────────

pub struct AiProxyProbe {
    url: String,
}

impl AiProxyProbe {
    pub fn from_config(cfg: &AiProxyConfig) -> Self {
        let host = if cfg.host.is_empty() {
            "127.0.0.1"
        } else {
            &cfg.host
        };
        Self {
            url: format!("http://{host}:{}/health", cfg.port),
        }
    }
}

#[async_trait]
impl Probe for AiProxyProbe {
    fn name(&self) -> &'static str {
        "ai-proxy"
    }

    fn kind(&self) -> ProbeKind {
        ProbeKind::Liveness
    }

    async fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        timed(|| liveness_impl(&self.url, ctx)).await
    }
}

async fn liveness_impl(url: &str, ctx: &ProbeCtx) -> Result<ProbeResult> {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(2))
        .timeout(ctx.timeout)
        .build()?;

    let resp = client
        .get(url)
        .send()
        .await
        .context("ai-proxy /health request failed")?;

    let ok = resp.status().is_success();
    Ok(ProbeResult {
        ok,
        error_class: if ok { None } else { Some("unhealthy".into()) },
        error_detail: if ok {
            None
        } else {
            Some(format!("health endpoint returned {}", resp.status()))
        },
        ..Default::default()
    })
}

// ── Functional probe: /v1/chat/completions ──────────────────────────

pub struct AiProxyFunctionalProbe {
    base_url: String,
    bearer_token: String,
}

impl AiProxyFunctionalProbe {
    /// Returns `None` when no probe token was generated (proxy not running).
    pub fn from_config(cfg: &AiProxyConfig) -> Option<Self> {
        let token = cfg.probe_token.as_ref()?;
        let host = if cfg.host.is_empty() {
            "127.0.0.1"
        } else {
            &cfg.host
        };
        Some(Self {
            base_url: format!("http://{host}:{}", cfg.port),
            bearer_token: token.clone(),
        })
    }

    async fn run_impl(&self, ctx: &ProbeCtx) -> Result<ProbeResult> {
        let client = reqwest::Client::builder().timeout(ctx.timeout).build()?;

        // 1. Discover available models.
        let models_resp: ModelsResponse = client
            .get(format!("{}/v1/models", self.base_url))
            .bearer_auth(&self.bearer_token)
            .send()
            .await
            .context("ai-proxy /v1/models request failed")?
            .error_for_status()
            .context("ai-proxy /v1/models non-2xx")?
            .json()
            .await
            .context("failed to parse models response")?;

        let model = match models_resp.data.first() {
            Some(m) => m.id.clone(),
            None => {
                return Ok(ProbeResult {
                    ok: false,
                    error_class: Some("no_models".into()),
                    error_detail: Some("no models available from any backend".into()),
                    ..Default::default()
                });
            }
        };

        // 2. Send a canary chat completion.
        let resp: ChatResponse = client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .bearer_auth(&self.bearer_token)
            .json(&ChatBody {
                model: model.clone(),
                messages: vec![ChatMessage {
                    role: "user".into(),
                    content: ctx.canary_prompt.clone(),
                }],
                temperature: 0.0,
                max_tokens: 32,
                stream: false,
            })
            .send()
            .await
            .context("ai-proxy chat completions failed")?
            .error_for_status()
            .context("ai-proxy chat completions non-2xx")?
            .json()
            .await
            .context("failed to parse chat response")?;

        let content = resp
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .unwrap_or_default();
        let ok = response_has_content(&content);

        Ok(ProbeResult {
            ok,
            tokens_in: resp.usage.as_ref().map(|u| u.prompt_tokens),
            tokens_out: resp.usage.as_ref().map(|u| u.completion_tokens),
            model: Some(model),
            canary_digest: Some(digest_hex(content.trim().as_bytes())),
            error_class: if ok {
                None
            } else {
                Some("empty_response".into())
            },
            error_detail: if ok {
                None
            } else {
                Some(format!(
                    "response had no non-whitespace content ({} bytes): {}",
                    content.len(),
                    snippet(&content)
                ))
            },
            ..Default::default()
        })
    }
}

#[async_trait]
impl Probe for AiProxyFunctionalProbe {
    fn name(&self) -> &'static str {
        "ai-proxy"
    }

    fn kind(&self) -> ProbeKind {
        ProbeKind::Functional
    }

    async fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        timed(|| self.run_impl(ctx)).await
    }
}

// ── Local types (not imported from ai_proxy::types) ─────────────────

#[derive(Serialize)]
struct ChatBody {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
    stream: bool,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatReplyMessage,
}

#[derive(Deserialize)]
struct ChatReplyMessage {
    content: String,
}

#[derive(Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<Model>,
}

#[derive(Deserialize)]
struct Model {
    id: String,
}
