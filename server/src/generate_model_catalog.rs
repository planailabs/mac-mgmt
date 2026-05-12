//! CLI entry point for generating `server/ext/model-catalog.json`.
//!
//! Called via: `cargo run -p mac-mgmt-server -- generate-model-catalog [options]`
//!
//! Each provider is only fetched if its API key is provided (except Ollama and
//! OpenRouter which are public).

use crate::model_catalog_fetch::{
    self, ModelCatalog,
};
use mac_mgmt_common::model_source::ModelSource;

/// Known cloud providers with their default base URLs.
const CLOUD_PROVIDERS: &[(&str, &str, &str)] = &[
    ("anthropic", "ANTHROPIC_API_KEY", "https://api.anthropic.com/v1"),
    ("openai", "OPENAI_API_KEY", "https://api.openai.com/v1"),
    ("google", "GEMINI_API_KEY", "https://generativelanguage.googleapis.com/v1beta"),
    ("mistral", "MISTRAL_API_KEY", "https://api.mistral.ai/v1"),
    ("groq", "GROQ_API_KEY", "https://api.groq.com/openai/v1"),
    ("xai", "XAI_API_KEY", "https://api.x.ai/v1"),
    ("deepseek", "DEEPSEEK_API_KEY", "https://api.deepseek.com/v1"),
    ("together", "TOGETHER_API_KEY", "https://api.together.xyz/v1"),
];

pub async fn run(args: &crate::GenerateModelCatalogArgs) {
    let mut sources: Vec<ModelSource> = Vec::new();

    // ── Ollama (public) ─────────────────────────────────────────
    {
        let url = &args.ollama_url;
        tracing::info!("fetching ollama ({url})...");
        match model_catalog_fetch::fetch_ollama_models(url).await {
            Ok(src) => {
                let n: usize = src.groups.iter().map(|g| g.count_models()).sum();
                tracing::info!("ollama: {n} models");
                sources.push(src);
            }
            Err(e) => tracing::error!("ollama: {e}"),
        }
    }

    // ── OpenClaw (local gateway) ──────────────────────────────────
    if !args.no_openclaw {
        let token = args
            .openclaw_token
            .clone()
            .or_else(|| std::env::var("OPENCLAW_TOKEN").ok());
        tracing::info!("fetching openclaw ({}:{})...", args.openclaw_host, args.openclaw_port);
        match model_catalog_fetch::fetch_openclaw_models(
            &args.openclaw_host,
            args.openclaw_port,
            token.as_deref(),
        )
        .await
        {
            Ok(src) => {
                let n: usize = src.groups.iter().map(|g| g.count_models()).sum();
                tracing::info!("openclaw: {n} models");
                sources.push(src);
            }
            Err(e) => tracing::warn!("openclaw: {e} (skipping)"),
        }
    }

    // ── LM Studio (public) ───────────────────────────────────────
    if !args.no_lms {
        let url = &args.lms_url;
        tracing::info!("fetching lms ({url})...");
        match model_catalog_fetch::fetch_lms_models(url).await {
            Ok(src) => {
                let n: usize = src.groups.iter().map(|g| g.count_models()).sum();
                tracing::info!("lms: {n} models");
                sources.push(src);
            }
            Err(e) => tracing::error!("lms: {e}"),
        }
    }

    // ── OpenRouter (public) ─────────────────────────────────────
    if !args.no_openrouter {
        tracing::info!("fetching openrouter...");
        match model_catalog_fetch::fetch_openrouter_models().await {
            Ok(src) => {
                let n: usize = src.groups.iter().map(|g| g.count_models()).sum();
                tracing::info!("openrouter: {n} models");
                sources.push(src);
            }
            Err(e) => tracing::error!("openrouter: {e}"),
        }
    }

    // ── Cloud providers (keyed) ─────────────────────────────────
    for &(provider, env_var, base_url) in CLOUD_PROVIDERS {
        let key = resolve_key(args, provider, env_var);
        if let Some(api_key) = key {
            tracing::info!("fetching {provider}...");
            match model_catalog_fetch::fetch_cloud_provider_models(provider, base_url, &api_key)
                .await
            {
                Ok(src) => {
                    let n: usize = src.groups.iter().map(|g| g.count_models()).sum();
                    tracing::info!("{provider}: {n} models");
                    sources.push(src);
                }
                Err(e) => tracing::error!("{provider}: {e}"),
            }
        }
    }

    if sources.is_empty() {
        tracing::warn!("no sources fetched, catalog will be empty");
    }

    let catalog = ModelCatalog {
        generated_at: chrono::Utc::now().to_rfc3339(),
        sources,
    };

    let json = serde_json::to_string_pretty(&catalog).expect("failed to serialize catalog");

    // Ensure parent directory exists.
    if let Some(parent) = std::path::Path::new(&args.output).parent() {
        std::fs::create_dir_all(parent).expect("failed to create output directory");
    }
    std::fs::write(&args.output, &json).expect("failed to write catalog file");

    let total: usize = catalog
        .sources
        .iter()
        .flat_map(|s| &s.groups)
        .map(|g| g.count_models())
        .sum();
    tracing::info!(
        "wrote {} source(s), {} total models to {}",
        catalog.sources.len(),
        total,
        args.output
    );
}

/// Resolve an API key: CLI arg takes precedence over env var.
fn resolve_key(args: &crate::GenerateModelCatalogArgs, provider: &str, env_var: &str) -> Option<String> {
    let cli_key = match provider {
        "anthropic" => args.anthropic_key.as_deref(),
        "openai" => args.openai_key.as_deref(),
        "google" => args.google_key.as_deref(),
        "mistral" => args.mistral_key.as_deref(),
        "groq" => args.groq_key.as_deref(),
        "xai" => args.xai_key.as_deref(),
        "deepseek" => args.deepseek_key.as_deref(),
        "together" => args.together_key.as_deref(),
        _ => None,
    };
    cli_key
        .map(|s| s.to_string())
        .or_else(|| std::env::var(env_var).ok())
}
