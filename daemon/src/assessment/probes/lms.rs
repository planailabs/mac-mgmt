//! LM Studio functional probe: OpenAI-compatible `/v1/chat/completions` round-trip
//! against the configured default model.
//!
//! LMS doesn't support auto-pull of small canary models from a CLI the way
//! ollama does, so we use whatever model the cluster has configured as
//! `lms.default_model` — if that's absent, the probe reports a configuration
//! error rather than trying to load a model.

use anyhow::{Context, Result};
use async_trait::async_trait;
use mac_mgmt_common::LmsConfig;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{digest_hex, response_has_content, snippet, timed, Probe, ProbeCtx, ProbeKind, ProbeResult};

pub struct LmsProbe {
    base_url: String,
    default_model: String,
}

impl LmsProbe {
    pub fn from_config(cfg: &LmsConfig) -> Self {
        let host = if cfg.host.is_empty() { "127.0.0.1" } else { &cfg.host };
        let port = if cfg.port == 0 { 1234 } else { cfg.port };
        Self {
            base_url: format!("http://{host}:{port}"),
            default_model: cfg.default_model.clone(),
        }
    }

    async fn run_impl(&self, ctx: &ProbeCtx) -> Result<ProbeResult> {
        if self.default_model.is_empty() {
            anyhow::bail!("no default model configured for lms");
        }
        let client = Client::builder().timeout(ctx.timeout).build()?;
        let resp: ChatResponse = client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .json(&ChatBody {
                model: self.default_model.clone(),
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
            .context("lms chat completions failed")?
            .error_for_status()
            .context("lms chat completions non-2xx")?
            .json()
            .await
            .context("failed to parse lms response")?;

        let content = resp
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .unwrap_or_default();
        let ok = response_has_content(&content);
        let _ = ctx.canary_expected; // kept for the Regression probe kind

        Ok(ProbeResult {
            ok,
            tokens_in: resp.usage.as_ref().map(|u| u.prompt_tokens),
            tokens_out: resp.usage.as_ref().map(|u| u.completion_tokens),
            model: Some(self.default_model.clone()),
            canary_digest: Some(digest_hex(content.trim().as_bytes())),
            error_class: if ok { None } else { Some("empty_response".into()) },
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
impl Probe for LmsProbe {
    fn name(&self) -> &'static str {
        "lms"
    }

    fn kind(&self) -> ProbeKind {
        ProbeKind::Functional
    }

    async fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        timed(|| self.run_impl(ctx)).await
    }
}

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
