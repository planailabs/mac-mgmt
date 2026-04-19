//! OpenClaw functional probe.
//!
//! Sends a canary prompt through the OpenAI-compatible `/v1/chat/completions`
//! endpoint exposed by the openclaw gateway on loopback. Token counts and the
//! model string come back in the same `usage` + `model` fields a real client
//! would read, so a passing probe confirms gateway → LLM backend end-to-end.
//!
//! Always uses the gateway endpoint (default port 18789) — no fallback to
//! `openclaw health` since that doesn't exercise the full agent pipeline.

use std::time::Instant;

use anyhow::{Context, Result};
use async_trait::async_trait;
use mac_mgmt_common::OpenClawConfig;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{
    Probe, ProbeCtx, ProbeKind, ProbeResult, digest_hex, response_has_content, snippet, timed,
};

/// Loopback is unauthenticated in openclaw's gateway (see net.ts:53,
/// auth.ts:557), so no bearer-token handling is needed here.
pub struct OpenClawProbe {
    gateway_url: String,
}

impl OpenClawProbe {
    pub fn from_config(cfg: &OpenClawConfig) -> Self {
        let (host, port) = match &cfg.gateway {
            Some(g) => {
                let h = if g.host.is_empty() { "127.0.0.1" } else { &g.host };
                (h.to_string(), g.port)
            }
            None => ("127.0.0.1".to_string(), 18789),
        };
        Self {
            gateway_url: format!("http://{host}:{port}"),
        }
    }

    async fn run_impl(&self, ctx: &ProbeCtx) -> Result<ProbeResult> {
        run_gateway(&self.gateway_url, ctx).await
    }
}

#[async_trait]
impl Probe for OpenClawProbe {
    fn name(&self) -> &'static str {
        "openclaw"
    }

    fn kind(&self) -> ProbeKind {
        ProbeKind::Functional
    }

    async fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        timed(|| self.run_impl(ctx)).await
    }
}

async fn run_gateway(base_url: &str, ctx: &ProbeCtx) -> Result<ProbeResult> {
    let client = Client::builder().timeout(ctx.timeout).build()?;
    let body = ChatBody {
        // "openclaw" is the conventional model name the gateway routes to
        // the configured backend. The real model actually used comes back
        // in the response `model` field.
        model: "openclaw".into(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: ctx.canary_prompt.clone(),
        }],
        temperature: 0.0,
        max_tokens: 32,
        stream: false,
    };

    let started = Instant::now();
    let resp: ChatResponse = client
        .post(format!("{base_url}/v1/chat/completions"))
        .json(&body)
        .send()
        .await
        .context("openclaw gateway chat completions failed")?
        .error_for_status()
        .context("openclaw gateway non-2xx")?
        .json()
        .await
        .context("failed to parse openclaw chat response")?;
    let first_token_ms = started.elapsed().as_millis() as u64;

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
        first_token_ms: Some(first_token_ms),
        model: Some(resp.model),
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
    model: String,
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
