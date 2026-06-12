//! LM Studio functional probe: OpenAI-compatible `/v1/chat/completions` round-trip
//! against the canary model.
//!
//! Uses `lmstudio-community/Qwen3-0.6B-GGUF` as the canary, which is always
//! loaded during `post_start`. If the model is missing, the probe reports an
//! error (indicating `post_start` failed or the service restarted).

use anyhow::{Context, Result};
use async_trait::async_trait;
use mac_mgmt_common::LmsConfig;
use reqwest::Client;

use super::{
    ChatBody, ChatMessage, ChatResponse, Probe, ProbeCtx, ProbeKind, ProbeResult, digest_hex,
    response_has_content, snippet, timed,
};

pub struct LmsProbe {
    base_url: String,
    canary_model: String,
}

impl LmsProbe {
    pub fn from_config(cfg: &LmsConfig) -> Self {
        let host = if cfg.host.is_empty() {
            "127.0.0.1"
        } else {
            &cfg.host
        };
        let port = if cfg.port == 0 { 1234 } else { cfg.port };
        Self {
            base_url: format!("http://{host}:{port}"),
            canary_model: crate::canary::LMS_CANARY.to_string(),
        }
    }

    async fn run_impl(&self, ctx: &ProbeCtx) -> Result<ProbeResult> {
        let client = Client::builder().timeout(ctx.timeout).build()?;
        let resp: ChatResponse = client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .json(&ChatBody {
                model: self.canary_model.clone(),
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
            model: Some(self.canary_model.clone()),
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
