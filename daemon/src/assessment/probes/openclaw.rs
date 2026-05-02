//! OpenClaw probes.
//!
//! Two probes:
//! - **Liveness** (`openclaw_health`): runs `openclaw health --json` to check
//!   subsystem status. Cheap, catches daemon crashes and config errors.
//! - **Functional** (`openclaw`): sends a canary prompt through the gateway's
//!   `/v1/chat/completions` endpoint for a full agent round-trip.

use std::process::Stdio;
use std::time::Instant;

use anyhow::{Context, Result};
use async_trait::async_trait;
use mac_mgmt_common::OpenClawConfig;
use reqwest::Client;
use tokio::process::Command;

use super::{
    ChatBody, ChatMessage, ChatResponse, Probe, ProbeCtx, ProbeKind, ProbeResult, digest_hex,
    response_has_content, snippet, timed,
};

// ── Liveness probe: `openclaw health --json` ─────────────────────────

pub struct OpenClawHealthProbe;

#[async_trait]
impl Probe for OpenClawHealthProbe {
    fn name(&self) -> &'static str {
        "openclaw"
    }

    fn kind(&self) -> ProbeKind {
        ProbeKind::Liveness
    }

    async fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        timed(|| run_health(ctx)).await
    }
}

async fn run_health(ctx: &ProbeCtx) -> Result<ProbeResult> {
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

// ── Functional probe: gateway chat completions ───────────────────────

pub struct OpenClawProbe {
    gateway_url: String,
    auth_token: Option<String>,
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
            auth_token: read_gateway_token(),
        }
    }
}

/// Read the gateway auth token from ~/.openclaw/openclaw.json
/// (gateway.auth.token). Returns None if not configured or unreadable.
fn read_gateway_token() -> Option<String> {
    let path = dirs::home_dir()?.join(".openclaw/openclaw.json");
    let contents = std::fs::read_to_string(&path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&contents).ok()?;
    json.pointer("/gateway/auth/token")
        .and_then(|v| v.as_str())
        .map(String::from)
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
        timed(|| run_gateway(&self.gateway_url, self.auth_token.as_deref(), ctx)).await
    }
}

async fn run_gateway(base_url: &str, auth_token: Option<&str>, ctx: &ProbeCtx) -> Result<ProbeResult> {
    let client = Client::builder().timeout(ctx.timeout).build()?;
    let body = ChatBody {
        model: "openclaw/default".into(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: ctx.canary_prompt.clone(),
        }],
        temperature: 0.0,
        max_tokens: 32,
        stream: false,
    };

    let started = Instant::now();
    let mut req = client
        .post(format!("{base_url}/v1/chat/completions"))
        .json(&body);
    if let Some(token) = auth_token {
        req = req.bearer_auth(token);
    }
    let resp: ChatResponse = req
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

