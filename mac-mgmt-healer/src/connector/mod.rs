use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

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
}

impl Default for ConnectorConfig {
    fn default() -> Self {
        Self {
            ollama_url: None,
            ollama_model: None,
            anthropic_api_key: None,
            anthropic_model: None,
            token_budget: 200_000,
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
/// 1. Try local Ollama (probe with 2s timeout)
/// 2. Fall back to Anthropic cloud if API key provided
/// 3. Error if neither is available
pub async fn resolve_llm(config: &ConnectorConfig) -> Result<LlmHandle> {
    let token_usage = Arc::new(AtomicU64::new(0));

    // 1. Try Ollama
    let ollama_url = config
        .ollama_url
        .clone()
        .unwrap_or_else(|| "http://localhost:11434".to_string());

    if probe_ollama(&ollama_url).await {
        let model = config
            .ollama_model
            .clone()
            .unwrap_or_else(|| "qwen3".to_string());

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

    // 2. Try Anthropic cloud
    if let Some(api_key) = &config.anthropic_api_key {
        let model = config
            .anthropic_model
            .clone()
            .unwrap_or_else(|| "claude-sonnet-4-6".to_string());

        tracing::info!(model = %model, "using Anthropic cloud for healer agent (Ollama not available)");

        // Set env var for async_anthropic::Client, which reads ANTHROPIC_API_KEY.
        // SAFETY: this is safe as we're in an async context and the env var is set before
        // the client is created. The env var is process-global but that's fine —
        // the server config is authoritative.
        // SAFETY: called during server startup, before parallel agent tasks.
        unsafe { std::env::set_var("ANTHROPIC_API_KEY", api_key) };

        let anthropic = swiftide::integrations::anthropic::Anthropic::builder()
            .default_prompt_model(&model)
            .build()
            .context("failed to build Anthropic integration")?;

        return Ok(LlmHandle {
            provider: LlmProvider::Anthropic(anthropic),
            token_usage,
            is_cloud: true,
        });
    }

    anyhow::bail!(
        "no LLM available: Ollama not reachable at {ollama_url} and no Anthropic API key configured"
    )
}

/// Probe whether Ollama is running by hitting /api/tags with a short timeout.
async fn probe_ollama(url: &str) -> bool {
    let probe_url = format!("{}/api/tags", url.trim_end_matches('/'));
    match reqwest::Client::new()
        .get(&probe_url)
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
    {
        Ok(resp) => {
            let ok = resp.status().is_success();
            if !ok {
                tracing::debug!(status = %resp.status(), "Ollama probe failed");
            }
            ok
        }
        Err(e) => {
            tracing::debug!(err = %e, "Ollama not reachable");
            false
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
