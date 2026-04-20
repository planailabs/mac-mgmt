use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};

/// Configuration for the LLM connector.
#[derive(Debug, Clone)]
pub struct ConnectorConfig {
    /// Ollama URL. Defaults to `http://localhost:11434`.
    pub ollama_url: Option<String>,
    /// Ollama model to use. Defaults to a tool-use-capable model.
    pub ollama_model: Option<String>,
    /// Anthropic API key for cloud fallback.
    pub anthropic_api_key: Option<String>,
    /// Anthropic model. Defaults to `claude-sonnet-4-6`.
    pub anthropic_model: Option<String>,
    /// Max input+output tokens per session when using a cloud provider.
    /// Session auto-pauses when exceeded. 0 = unlimited.
    pub token_budget: u64,
    /// Context7 API key for documentation lookup MCP server.
    pub context7_api_key: Option<String>,
}

impl Default for ConnectorConfig {
    fn default() -> Self {
        Self {
            ollama_url: None,
            ollama_model: None,
            anthropic_api_key: None,
            anthropic_model: None,
            token_budget: 200_000,
            context7_api_key: None,
        }
    }
}

/// Resolved LLM handle ready for use with swiftide agents.
pub struct LlmHandle {
    pub provider: LlmProvider,
    /// Token usage counter. Only incremented when using cloud providers.
    pub token_usage: Arc<AtomicU64>,
    /// Whether this handle uses a cloud provider (subject to token budgets).
    pub is_cloud: bool,
}

pub enum LlmProvider {
    Ollama(swiftide::integrations::ollama::Ollama),
    Anthropic(swiftide::integrations::anthropic::Anthropic),
}

/// Resolve the best available LLM.
///
/// 1. Try local Ollama — check reachable AND model available (pull if missing)
/// 2. Fall back to Anthropic cloud if API key provided
/// 3. Error if neither is available
pub async fn resolve_llm(config: &ConnectorConfig) -> Result<LlmHandle> {
    let token_usage = Arc::new(AtomicU64::new(0));

    let ollama_url = config
        .ollama_url
        .clone()
        .unwrap_or_else(|| "http://localhost:11434".to_string());
    let ollama_url = ollama_url.trim_end_matches('/').to_string();

    let model = config
        .ollama_model
        .clone()
        .unwrap_or_else(|| "gemma4".to_string());

    match check_ollama(&ollama_url, &model).await {
        OllamaStatus::Ready => {
            tracing::info!(url = %ollama_url, model = %model, "using local Ollama for healer agent");

            let ollama = swiftide::integrations::ollama::Ollama::builder()
                .default_prompt_model(&model)
                .build()
                .context("failed to build Ollama integration")?;

            return Ok(LlmHandle {
                provider: LlmProvider::Ollama(ollama),
                token_usage,
                is_cloud: false,
            });
        }
        OllamaStatus::Unreachable(reason) => {
            tracing::info!(reason = %reason, "Ollama not available, trying cloud fallback");
        }
        OllamaStatus::ModelMissing => {
            tracing::info!(
                url = %ollama_url, model = %model,
                "Ollama online but model not found and pull failed, trying cloud fallback"
            );
        }
    }

    // 2. Try Anthropic cloud
    if let Some(api_key) = &config.anthropic_api_key {
        let model = config
            .anthropic_model
            .clone()
            .unwrap_or_else(|| "claude-sonnet-4-6".to_string());

        tracing::info!(model = %model, "using Anthropic cloud for healer agent");

        // SAFETY: called during server startup, before parallel agent tasks.
        unsafe { std::env::set_var("ANTHROPIC_API_KEY", api_key) };

        // Wire usage tracking so the token budget actually works.
        // input_tokens includes the full context (system prompt, history, tools)
        // on every call — this is what Anthropic bills for.
        let usage_counter = token_usage.clone();
        let anthropic = swiftide::integrations::anthropic::Anthropic::builder()
            .default_prompt_model(&model)
            .on_usage(move |usage| {
                let input = usage.prompt_tokens as u64;
                let output = usage.completion_tokens as u64;
                usage_counter.fetch_add(input + output, Ordering::Relaxed);
                Ok(())
            })
            .build()
            .context("failed to build Anthropic integration")?;

        return Ok(LlmHandle {
            provider: LlmProvider::Anthropic(anthropic),
            token_usage,
            is_cloud: true,
        });
    }

    anyhow::bail!(
        "no LLM available: Ollama not usable at {ollama_url} and no Anthropic API key configured"
    )
}

enum OllamaStatus {
    Ready,
    Unreachable(String),
    ModelMissing,
}

/// Check Ollama: is it reachable, and does it have the required model?
/// If the model is missing, attempt a pull.
async fn check_ollama(url: &str, model: &str) -> OllamaStatus {
    let client = reqwest::Client::new();
    let timeout = std::time::Duration::from_secs(3);

    // 1. Check reachable via /api/tags
    let tags_url = format!("{url}/api/tags");
    let resp = match client.get(&tags_url).timeout(timeout).send().await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => return OllamaStatus::Unreachable(format!("status {}", r.status())),
        Err(e) => return OllamaStatus::Unreachable(e.to_string()),
    };

    // 2. Parse model list and check if our model is present
    let has_model = match resp.json::<serde_json::Value>().await {
        Ok(body) => {
            if let Some(models) = body.get("models").and_then(|m| m.as_array()) {
                // Model names can be "qwen3:latest" or just "qwen3" — match the base name
                let base = model.split(':').next().unwrap_or(model);
                models.iter().any(|m| {
                    m.get("name").and_then(|n| n.as_str()).is_some_and(|n| {
                        let n_base = n.split(':').next().unwrap_or(n);
                        n_base == base || n == model
                    })
                })
            } else {
                false
            }
        }
        Err(_) => false,
    };

    if has_model {
        return OllamaStatus::Ready;
    }

    // 3. Model not found — try to pull it
    tracing::info!(model = %model, "model not found in Ollama, pulling...");
    let pull_url = format!("{url}/api/pull");
    let pull_body = serde_json::json!({ "name": model, "stream": false });
    match client
        .post(&pull_url)
        .json(&pull_body)
        .timeout(std::time::Duration::from_secs(600))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => {
            tracing::info!(model = %model, "successfully pulled model");
            OllamaStatus::Ready
        }
        Ok(r) => {
            let status = r.status();
            let body = r.text().await.unwrap_or_default();
            tracing::warn!(model = %model, status = %status, body = %body, "failed to pull model");
            OllamaStatus::ModelMissing
        }
        Err(e) => {
            tracing::warn!(model = %model, err = %e, "failed to pull model");
            OllamaStatus::ModelMissing
        }
    }
}

impl LlmHandle {
    /// Record token usage from an LLM response.
    pub fn record_usage(&self, input_tokens: u64, output_tokens: u64) {
        if self.is_cloud {
            self.token_usage
                .fetch_add(input_tokens + output_tokens, Ordering::Relaxed);
        }
    }

    /// Get current total token usage.
    pub fn total_usage(&self) -> u64 {
        self.token_usage.load(Ordering::Relaxed)
    }

    /// Reset the token usage counter (used on session resume).
    pub fn reset_usage(&self) {
        self.token_usage.store(0, Ordering::Relaxed);
    }
}
