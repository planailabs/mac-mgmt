//! AI proxy probes:
//! - **Liveness** (`AiProxyProbe`): HTTP GET `/health` returns 200.
//! - **Functional** (`AiProxyFunctionalProbe`): sends the canary model through
//!   `/v1/chat/completions` using the ollama canary (`qwen3:0.6b`).

use anyhow::{Context, Result};
use async_trait::async_trait;
use mac_mgmt_common::AiProxyConfig;

use super::{
    ChatBody, ChatMessage, ChatResponse, Probe, ProbeCtx, ProbeKind, ProbeResult, digest_hex,
    response_has_content, snippet, timed,
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
    canary_model: String,
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
            canary_model: crate::canary::OLLAMA_CANARY.to_string(),
        })
    }

    async fn run_impl(&self, ctx: &ProbeCtx) -> Result<ProbeResult> {
        let client = reqwest::Client::builder().timeout(ctx.timeout).build()?;
        let model = self.canary_model.clone();

        // Send a canary chat completion.
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

