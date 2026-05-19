//! Hermes agent probes.
//!
//! Two probes:
//! - **Liveness** (`hermes_health`): HTTP GET /health on the API server.
//! - **Functional** (`hermes`): sends a canary prompt through the gateway's
//!   `/v1/chat/completions` endpoint for a full agent round-trip.

use anyhow::{Context, Result};
use async_trait::async_trait;
use mac_mgmt_common::HermesConfig;
use reqwest::Client;

use super::{
    ChatBody, ChatMessage, ChatResponse, Probe, ProbeCtx, ProbeKind, ProbeResult, digest_hex,
    response_has_content, snippet, timed,
};

/// Read a value from ~/.hermes/.env by key.
fn read_hermes_env_var(key: &str) -> Option<String> {
    let path = dirs::home_dir()?.join(".hermes/.env");
    let contents = std::fs::read_to_string(&path).ok()?;
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix(key) {
            if let Some(val) = rest.strip_prefix('=') {
                return Some(val.to_string());
            }
        }
    }
    None
}

// ── Liveness probe: HTTP /health ──────────────────────────────────────

pub struct HermesHealthProbe {
    health_url: String,
    auth_token: Option<String>,
}

impl HermesHealthProbe {
    pub fn new(cfg: &HermesConfig) -> Self {
        let (host, port) = match &cfg.gateway {
            Some(g) => {
                let h = if g.host.is_empty() { "127.0.0.1" } else { &g.host };
                (h.to_string(), g.port)
            }
            None => ("127.0.0.1".to_string(), 8642),
        };
        Self {
            health_url: format!("http://{host}:{port}/health"),
            auth_token: read_hermes_env_var("API_SERVER_KEY"),
        }
    }
}

#[async_trait]
impl Probe for HermesHealthProbe {
    fn name(&self) -> &'static str {
        "hermes"
    }

    fn kind(&self) -> ProbeKind {
        ProbeKind::Liveness
    }

    async fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        timed(|| run_health(&self.health_url, self.auth_token.as_deref(), ctx)).await
    }
}

async fn run_health(url: &str, auth_token: Option<&str>, ctx: &ProbeCtx) -> Result<ProbeResult> {
    let client = Client::builder().timeout(ctx.timeout).build()?;
    let mut req = client.get(url);
    if let Some(token) = auth_token {
        req = req.bearer_auth(token);
    }

    let resp = req
        .send()
        .await
        .context("hermes health request failed")?;

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        anyhow::bail!("hermes health returned {status}: {body}");
    }

    let ok = if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
        json.get("status")
            .and_then(|v| v.as_str())
            .is_some_and(|s| matches!(s, "ok" | "healthy" | "ready"))
    } else {
        // 200 without JSON is acceptable
        true
    };

    Ok(ProbeResult {
        ok,
        canary_digest: Some(digest_hex(body.trim().as_bytes())),
        error_class: if ok { None } else { Some("bad_response".into()) },
        error_detail: if ok {
            None
        } else {
            Some(format!("unexpected health response: {}", snippet(&body)))
        },
        ..Default::default()
    })
}

// ── Functional probe: gateway chat completions ────────────────────────

pub struct HermesProbe {
    gateway_url: String,
    auth_token: Option<String>,
    canary_model: String,
}

impl HermesProbe {
    pub fn new(cfg: &HermesConfig, canary_model: String) -> Self {
        let (host, port) = match &cfg.gateway {
            Some(g) => {
                let h = if g.host.is_empty() { "127.0.0.1" } else { &g.host };
                (h.to_string(), g.port)
            }
            None => ("127.0.0.1".to_string(), 8642),
        };
        Self {
            gateway_url: format!("http://{host}:{port}"),
            auth_token: read_hermes_env_var("API_SERVER_KEY"),
            canary_model,
        }
    }
}

#[async_trait]
impl Probe for HermesProbe {
    fn name(&self) -> &'static str {
        "hermes"
    }

    fn kind(&self) -> ProbeKind {
        ProbeKind::Functional
    }

    async fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        timed(|| {
            run_gateway(
                &self.gateway_url,
                self.auth_token.as_deref(),
                &self.canary_model,
                ctx,
            )
        })
        .await
    }
}

async fn run_gateway(
    base_url: &str,
    auth_token: Option<&str>,
    canary_model: &str,
    ctx: &ProbeCtx,
) -> Result<ProbeResult> {
    let client = Client::builder().timeout(ctx.timeout).build()?;
    let body = ChatBody {
        model: canary_model.into(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: ctx.canary_prompt.clone(),
        }],
        temperature: 0.0,
        max_tokens: 32,
        stream: false,
    };

    let started = std::time::Instant::now();
    let mut req = client
        .post(format!("{base_url}/v1/chat/completions"))
        .json(&body);
    if let Some(token) = auth_token {
        req = req.bearer_auth(token);
    }
    let resp: ChatResponse = req
        .send()
        .await
        .context("hermes gateway chat completions failed")?
        .error_for_status()
        .context("hermes gateway non-2xx")?
        .json()
        .await
        .context("failed to parse hermes chat response")?;
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
