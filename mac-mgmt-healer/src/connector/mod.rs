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
    /// OpenRouter API key.
    pub openrouter_api_key: Option<String>,
    /// OpenRouter model. Defaults to `anthropic/claude-sonnet-4`.
    pub openrouter_model: Option<String>,
    /// Generic OpenAI-compatible API key.
    pub openai_compat_api_key: Option<String>,
    /// Generic OpenAI-compatible base URL (e.g. "http://my-vllm:8000/v1").
    pub openai_compat_url: Option<String>,
    /// Default model for the OpenAI-compatible provider.
    pub openai_compat_model: Option<String>,
    /// Max input+output tokens per session when using a cloud provider.
    /// Session auto-pauses when exceeded. 0 = unlimited.
    pub token_budget: u64,
    /// Context7 API key for documentation lookup MCP server.
    pub context7_api_key: Option<String>,
    /// Provider for the validator LLM ("ollama", "openrouter", "openai_compat").
    /// If None, only static validation (Layer 0) runs — no LLM pre-flight.
    pub validator_provider: Option<String>,
    /// Model for the validator LLM. Required when `validator_provider` is set.
    pub validator_model: Option<String>,
}

impl Default for ConnectorConfig {
    fn default() -> Self {
        Self {
            ollama_url: None,
            ollama_model: None,
            anthropic_api_key: None,
            anthropic_model: None,
            openrouter_api_key: None,
            openrouter_model: None,
            openai_compat_api_key: None,
            openai_compat_url: None,
            openai_compat_model: None,
            token_budget: 200_000,
            context7_api_key: None,
            validator_provider: None,
            validator_model: None,
        }
    }
}

/// Resolved LLM handle ready for use with swiftide agents.
pub struct LlmHandle {
    pub provider: LlmProvider,
    /// Whether this handle uses a cloud provider (subject to token budgets).
    pub is_cloud: bool,
    /// Which provider was actually selected.
    pub resolved_provider: ResolvedProvider,
    /// Which model name was actually used.
    pub resolved_model: String,
}

/// Context passed to `resolve_llm` for persisting token usage events.
pub struct TokenEventContext {
    pub store: crate::store::DynStore,
    pub session_id: uuid::Uuid,
    /// Notified when token budget is exceeded.
    pub budget_notify: Arc<tokio::sync::Notify>,
}

pub enum LlmProvider {
    Ollama(swiftide::integrations::ollama::Ollama),
    Anthropic(swiftide::integrations::anthropic::Anthropic),
    OpenRouter(swiftide::integrations::openai::OpenAI),
    OpenAICompat(swiftide::integrations::openai::OpenAI),
}

/// Which provider was actually resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedProvider {
    Ollama,
    Anthropic,
    OpenRouter,
    OpenAICompat,
}

impl ResolvedProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::Anthropic => "anthropic",
            Self::OpenRouter => "openrouter",
            Self::OpenAICompat => "openai_compat",
        }
    }
}

/// Resolve the best available LLM.
///
/// If `forced_provider` is set, only that provider is tried.
/// If `forced_model` is set, it overrides the configured default for the chosen provider.
///
/// Without forcing:
/// 1. Try local Ollama — check reachable AND model available (pull if missing)
/// 2. Fall back to Anthropic cloud if API key provided
/// 3. Error if neither is available
pub async fn resolve_llm(
    config: &ConnectorConfig,
    forced_provider: Option<&str>,
    forced_model: Option<&str>,
    token_ctx: Option<TokenEventContext>,
) -> Result<LlmHandle> {

    let try_ollama = forced_provider.is_none() || forced_provider == Some("ollama");
    let try_anthropic = forced_provider.is_none() || forced_provider == Some("anthropic");
    let try_openrouter = forced_provider.is_none() || forced_provider == Some("openrouter");
    let try_openai_compat = forced_provider == Some("openai_compat");

    let ollama_url = config
        .ollama_url
        .clone()
        .unwrap_or_else(|| "http://localhost:11434".to_string());
    let ollama_url = ollama_url.trim_end_matches('/').to_string();

    if try_ollama {
        let model = forced_model
            .map(String::from)
            .or_else(|| config.ollama_model.clone())
            .unwrap_or_else(|| "gemma4".to_string());

        match check_ollama(&ollama_url, &model).await {
            OllamaStatus::Ready => {
                tracing::info!(url = %ollama_url, model = %model, "using local Ollama for healer agent");

                let mut ollama_config =
                    swiftide::integrations::ollama::config::OllamaConfig::default();
                ollama_config.with_api_base(&format!("{ollama_url}/v1"));
                let ollama_client = async_openai::Client::with_config(ollama_config);

                let ollama = swiftide::integrations::ollama::Ollama::builder()
                    .client(ollama_client)
                    .default_prompt_model(&model)
                    .default_options(
                        swiftide::integrations::openai::Options::builder()
                            .temperature(0.0)
                    )
                    .build()
                    .context("failed to build Ollama integration")?;

                return Ok(LlmHandle {
                    provider: LlmProvider::Ollama(ollama),
                    is_cloud: false,
                    resolved_provider: ResolvedProvider::Ollama,
                    resolved_model: model,
                });
            }
            OllamaStatus::Unreachable(reason) => {
                if forced_provider == Some("ollama") {
                    anyhow::bail!("Ollama requested but not available: {reason}");
                }
                tracing::info!(reason = %reason, "Ollama not available, trying cloud fallback");
            }
            OllamaStatus::ModelMissing => {
                if forced_provider == Some("ollama") {
                    anyhow::bail!("Ollama requested but model '{model}' not available");
                }
                tracing::info!(
                    url = %ollama_url, model = %model,
                    "Ollama online but model not found and pull failed, trying cloud fallback"
                );
            }
        }
    }

    // 2. Try Anthropic cloud
    if try_anthropic {
        if let Some(api_key) = &config.anthropic_api_key {
            let model = forced_model
                .map(String::from)
                .or_else(|| config.anthropic_model.clone())
                .unwrap_or_else(|| "claude-sonnet-4-6".to_string());

            tracing::info!(model = %model, "using Anthropic cloud for healer agent");

            // SAFETY: called during server startup, before parallel agent tasks.
            unsafe { std::env::set_var("ANTHROPIC_API_KEY", api_key) };

            let mut builder = swiftide::integrations::anthropic::Anthropic::builder();
            builder.default_prompt_model(&model);

            if let Some(ref ctx) = token_ctx {
                let store = ctx.store.clone();
                let sid = ctx.session_id;
                let notify = ctx.budget_notify.clone();
                let provider_name = "anthropic".to_string();
                let model_name = model.clone();
                builder.on_usage_async(move |usage| {
                    let store = store.clone();
                    let provider = provider_name.clone();
                    let model = model_name.clone();
                    let notify = notify.clone();
                    let input = usage.prompt_tokens;
                    let output = usage.completion_tokens;
                    Box::pin(async move {
                        let new_total = store.append_token_event(
                            sid, &provider, &model, input, output,
                        ).await.unwrap_or(0);
                        let budget = store.get_token_budget(sid).await.unwrap_or(0);
                        if budget > 0 && new_total >= budget {
                            notify.notify_one();
                        }
                        Ok(())
                    })
                });
            }

            let anthropic = builder
                .build()
                .context("failed to build Anthropic integration")?;

            return Ok(LlmHandle {
                provider: LlmProvider::Anthropic(anthropic),
                is_cloud: true,
                resolved_provider: ResolvedProvider::Anthropic,
                resolved_model: model,
            });
        } else if forced_provider == Some("anthropic") {
            anyhow::bail!("Anthropic requested but no API key configured");
        }
    }

    // 3. Try OpenRouter (OpenAI-compatible)
    if try_openrouter {
        if let Some(api_key) = &config.openrouter_api_key {
            let model = forced_model
                .map(String::from)
                .or_else(|| config.openrouter_model.clone())
                .unwrap_or_else(|| "anthropic/claude-sonnet-4".to_string());

            tracing::info!(model = %model, "using OpenRouter for healer agent");

            let openai_config = async_openai::config::OpenAIConfig::default()
                .with_api_key(api_key)
                .with_api_base("https://openrouter.ai/api/v1");

            let client = async_openai::Client::with_config(openai_config);

            let mut builder = swiftide::integrations::openai::OpenAI::builder();
            builder.client(client).default_prompt_model(&model);

            if let Some(ref ctx) = token_ctx {
                let store = ctx.store.clone();
                let sid = ctx.session_id;
                let notify = ctx.budget_notify.clone();
                let provider_name = "openrouter".to_string();
                let model_name = model.clone();
                builder.on_usage_async(move |usage| {
                    let store = store.clone();
                    let provider = provider_name.clone();
                    let model = model_name.clone();
                    let notify = notify.clone();
                    let input = usage.prompt_tokens;
                    let output = usage.completion_tokens;
                    Box::pin(async move {
                        let new_total = store.append_token_event(
                            sid, &provider, &model, input, output,
                        ).await.unwrap_or(0);
                        let budget = store.get_token_budget(sid).await.unwrap_or(0);
                        if budget > 0 && new_total >= budget {
                            notify.notify_one();
                        }
                        Ok(())
                    })
                });
            }

            let openrouter = builder
                .build()
                .context("failed to build OpenRouter integration")?;

            return Ok(LlmHandle {
                provider: LlmProvider::OpenRouter(openrouter),
                is_cloud: true,
                resolved_provider: ResolvedProvider::OpenRouter,
                resolved_model: model,
            });
        } else if forced_provider == Some("openrouter") {
            anyhow::bail!("OpenRouter requested but no API key configured");
        }
    }

    // 4. Try generic OpenAI-compatible provider (only when explicitly requested)
    if try_openai_compat {
        let base_url = config
            .openai_compat_url
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("openai_compat requested but no base URL configured"))?;
        let api_key = config
            .openai_compat_api_key
            .as_deref()
            .unwrap_or("no-key");

        let model = forced_model
            .map(String::from)
            .or_else(|| config.openai_compat_model.clone())
            .ok_or_else(|| anyhow::anyhow!("openai_compat requested but no model specified"))?;

        tracing::info!(model = %model, url = %base_url, "using OpenAI-compatible provider for healer agent");

        let openai_config = async_openai::config::OpenAIConfig::default()
            .with_api_key(api_key)
            .with_api_base(base_url);

        let client = async_openai::Client::with_config(openai_config);

        // No on_usage_async callback — token tracking is intentionally skipped.
        let oai = swiftide::integrations::openai::OpenAI::builder()
            .client(client)
            .default_prompt_model(&model)
            .build()
            .context("failed to build OpenAI-compatible integration")?;

        return Ok(LlmHandle {
            provider: LlmProvider::OpenAICompat(oai),
            is_cloud: false,
            resolved_provider: ResolvedProvider::OpenAICompat,
            resolved_model: model,
        });
    }

    anyhow::bail!(
        "no LLM available: Ollama not usable at {ollama_url} and no Anthropic/OpenRouter API key configured"
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
    /// Human-readable provider name.
    pub fn provider_name(&self) -> &str {
        self.resolved_provider.as_str()
    }
}
