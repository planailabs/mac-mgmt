//! Ollama functional probe: full-prompt round-trip against the canary model.
//!
//! Uses `qwen3:0.6b` as the canary so it's cheap to pull and quick to infer.
//! If the model isn't present the probe triggers a blocking pull the first
//! time it runs — subsequent probes reuse the cached weights.

use std::time::Instant;

use anyhow::{Context, Result};
use async_trait::async_trait;
use mac_mgmt_common::OllamaConfig;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{
    Probe, ProbeCtx, ProbeKind, ProbeResult, digest_hex, response_has_content, snippet, timed,
};

pub struct OllamaProbe {
    base_url: String,
    canary_model: String,
}

impl OllamaProbe {
    pub fn from_config(cfg: &OllamaConfig) -> Self {
        let host = if cfg.host.is_empty() {
            "127.0.0.1"
        } else {
            &cfg.host
        };
        let port = if cfg.port == 0 { 11434 } else { cfg.port };
        Self {
            base_url: format!("http://{host}:{port}"),
            canary_model: crate::canary::OLLAMA_CANARY.to_string(),
        }
    }

    async fn ensure_canary_model(&self, client: &Client) -> Result<()> {
        let tags: TagsResponse = client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await
            .context("failed to list ollama models")?
            .json()
            .await
            .context("failed to parse /api/tags")?;

        let present = tags.models.iter().any(|m| {
            m.name == self.canary_model || m.name.starts_with(&format!("{}:", self.canary_model))
        });
        if present {
            return Ok(());
        }

        tracing::info!("probe: pulling {} for ollama canary", self.canary_model);
        let resp = client
            .post(format!("{}/api/pull", self.base_url))
            .json(&PullBody {
                name: self.canary_model.clone(),
                stream: false,
            })
            .send()
            .await
            .context("failed to request ollama pull")?;
        if !resp.status().is_success() {
            anyhow::bail!("ollama pull returned {}", resp.status());
        }
        Ok(())
    }

    async fn run_impl(&self, ctx: &ProbeCtx) -> Result<ProbeResult> {
        let client = Client::builder().timeout(ctx.timeout).build()?;
        self.ensure_canary_model(&client).await?;

        let started = Instant::now();
        let resp = client
            .post(format!("{}/api/generate", self.base_url))
            .json(&GenerateBody {
                model: self.canary_model.clone(),
                prompt: ctx.canary_prompt.clone(),
                stream: false,
                options: GenerateOptions {
                    num_predict: 32,
                    temperature: 0.0,
                },
            })
            .send()
            .await
            .context("ollama /api/generate failed")?
            .error_for_status()
            .context("ollama /api/generate non-2xx")?
            .json::<GenerateResponse>()
            .await
            .context("failed to parse generate response")?;
        let first_token_ms = started.elapsed().as_millis() as u64;

        let ok = response_has_content(&resp.response);
        let _ = ctx.canary_expected; // kept for the Regression probe kind
        Ok(ProbeResult {
            ok,
            tokens_in: resp.prompt_eval_count,
            tokens_out: resp.eval_count,
            first_token_ms: Some(first_token_ms),
            model: Some(self.canary_model.clone()),
            canary_digest: Some(digest_hex(resp.response.trim().as_bytes())),
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
                    resp.response.len(),
                    snippet(&resp.response)
                ))
            },
            ..Default::default()
        })
    }
}

#[async_trait]
impl Probe for OllamaProbe {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn kind(&self) -> ProbeKind {
        ProbeKind::Functional
    }

    async fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        timed(|| self.run_impl(ctx)).await
    }
}

#[derive(Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<TagEntry>,
}

#[derive(Deserialize)]
struct TagEntry {
    name: String,
}

#[derive(Serialize)]
struct PullBody {
    name: String,
    stream: bool,
}

#[derive(Serialize)]
struct GenerateBody {
    model: String,
    prompt: String,
    stream: bool,
    options: GenerateOptions,
}

#[derive(Serialize)]
struct GenerateOptions {
    num_predict: u32,
    temperature: f32,
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
    #[serde(default)]
    eval_count: Option<u32>,
}
