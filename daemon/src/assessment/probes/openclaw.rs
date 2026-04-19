//! OpenClaw functional probe.
//!
//! When the gateway is configured, sends a canary prompt through the
//! OpenAI-compatible `/v1/chat/completions` endpoint exposed by the
//! openclaw gateway on loopback. Token counts and the model string come
//! back in the same `usage` + `model` fields a real client would read,
//! so a passing probe confirms gateway → LLM backend end-to-end.
//!
//! Without a gateway configured we fall back to `openclaw health --json`
//! to at least catch subsystem-level failures. The fallback can't report
//! tokens or a model string.

use std::process::Stdio;
use std::time::Instant;

use anyhow::{Context, Result};
use async_trait::async_trait;
use mac_mgmt_common::OpenClawConfig;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use super::{
    Probe, ProbeCtx, ProbeKind, ProbeResult, digest_hex, response_has_content, snippet, timed,
};

/// Loopback is unauthenticated in openclaw's gateway (see net.ts:53,
/// auth.ts:557), so no bearer-token handling is needed here.
pub struct OpenClawProbe {
    /// `http://<host>:<port>` base URL for the gateway. `None` when no
    /// gateway stanza is configured, in which case we use the CLI fallback.
    gateway_url: Option<String>,
}

impl OpenClawProbe {
    pub fn from_config(cfg: &OpenClawConfig) -> Self {
        let gateway_url = cfg.gateway.as_ref().map(|g| {
            let host = if g.host.is_empty() {
                "127.0.0.1"
            } else {
                &g.host
            };
            format!("http://{host}:{}", g.port)
        });
        Self { gateway_url }
    }

    async fn run_impl(&self, ctx: &ProbeCtx) -> Result<ProbeResult> {
        match &self.gateway_url {
            Some(url) => run_gateway(url, ctx).await,
            None => run_health_fallback(ctx).await,
        }
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

/// Fallback when no gateway is configured — walks the `openclaw health
/// --json` output and marks the service unhealthy if any subsystem reports
/// a non-ok status. Cannot capture tokens or a model string.
async fn run_health_fallback(ctx: &ProbeCtx) -> Result<ProbeResult> {
    let out = tokio::time::timeout(
        ctx.timeout,
        Command::new("openclaw")
            .args(["health", "--json"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .context("openclaw health timed out")?
    .context("failed to spawn openclaw health")?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    if !out.status.success() {
        anyhow::bail!(
            "openclaw health exited {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }

    let json: serde_json::Value =
        serde_json::from_str(&stdout).context("failed to parse openclaw health output")?;

    let ok = is_all_healthy(&json);

    Ok(ProbeResult {
        ok,
        canary_digest: Some(digest_hex(stdout.trim().as_bytes())),
        error_class: if ok {
            None
        } else {
            Some("bad_response".into())
        },
        error_detail: if ok {
            None
        } else {
            Some(first_unhealthy(&json).unwrap_or_else(|| "degraded".into()))
        },
        ..Default::default()
    })
}

/// Walk the health JSON and only consider the system healthy if every
/// object with a "status" field reports "ok" or "healthy".
fn is_all_healthy(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Object(map) => {
            if let Some(status) = map.get("status").and_then(|s| s.as_str()) {
                if !matches!(status.to_lowercase().as_str(), "ok" | "healthy" | "ready") {
                    return false;
                }
            }
            map.values().all(is_all_healthy)
        }
        serde_json::Value::Array(arr) => arr.iter().all(is_all_healthy),
        _ => true,
    }
}

fn first_unhealthy(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::Object(map) => {
            if let Some(status) = map.get("status").and_then(|s| s.as_str()) {
                if !matches!(status.to_lowercase().as_str(), "ok" | "healthy" | "ready") {
                    let name = map
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("component");
                    return Some(format!("{name}={status}"));
                }
            }
            map.values().find_map(first_unhealthy)
        }
        serde_json::Value::Array(arr) => arr.iter().find_map(first_unhealthy),
        _ => None,
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
