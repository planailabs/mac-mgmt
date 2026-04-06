use anyhow::{Context, Result};

use super::Connector;
use crate::sentry_ext;
use mac_mgmt_common::CloudConfig;

/// Configures OpenClaw to use a cloud LLM provider (Anthropic, OpenAI, etc.).
///
/// Writes the provider config into `models.providers.<name>` and sets
/// `agents.defaults.model.primary` in `~/.openclaw/openclaw.json`.
pub struct CloudOpenClaw {
    pub config: CloudConfig,
}

impl Connector for CloudOpenClaw {
    fn name(&self) -> &str {
        "cloud→openclaw"
    }

    fn depends_on(&self) -> &[&str] {
        &["openclaw"]
    }

    fn connect(&self) -> Result<()> {
        let provider = self.config.provider.as_str();
        let model = &self.config.default_model;
        tracing::info!("connecting cloud provider {provider} to openclaw (model={model})");
        sentry_ext::breadcrumb(
            "connector",
            &format!("cloud→openclaw provider={provider} model={model}"),
            &[
                ("connector", "cloud→openclaw"),
                ("provider", provider),
                ("model", model),
            ],
        );

        let config_path = dirs::home_dir()
            .context("HOME not set")?
            .join(".openclaw/openclaw.json");

        if !config_path.exists() {
            tracing::warn!(
                "openclaw config not found at {}, skipping cloud connector",
                config_path.display()
            );
            return Ok(());
        }

        let contents = std::fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read {}", config_path.display()))?;
        let mut existing: serde_json::Value =
            serde_json::from_str(&contents).context("failed to parse openclaw.json")?;

        // For built-in providers (anthropic, openai, google, etc.) that are in the
        // OpenClaw catalog, we only need to set the API key env var and the default model.
        // For custom/proxy providers, we write a full models.providers entry.
        let needs_custom_provider = self.config.base_url.is_some()
            || self.config.api.is_some()
            || self.config.auth.is_some();

        if needs_custom_provider {
            // Custom provider entry under models.providers
            let mut provider_cfg = serde_json::json!({});
            if let Some(ref url) = self.config.base_url {
                provider_cfg["baseUrl"] = serde_json::json!(url);
            }
            if let Some(ref api) = self.config.api {
                provider_cfg["api"] = serde_json::json!(api);
            }
            if let Some(ref auth) = self.config.auth {
                provider_cfg["auth"] = serde_json::json!(auth);
            }
            if let Some(ref key) = self.config.api_key {
                provider_cfg["apiKey"] = serde_json::json!(key);
            }
            // Add a model entry derived from the default_model
            let model_id = model.split('/').last().unwrap_or(model);
            provider_cfg["models"] = serde_json::json!([{
                "id": model_id,
                "name": model_id,
            }]);

            let patch = serde_json::json!({
                "models": {
                    "providers": {
                        provider: provider_cfg,
                    }
                }
            });
            super::merge_json(&mut existing, &patch);
        } else if let Some(ref key) = self.config.api_key {
            // Built-in provider: set the API key via env.vars
            let env_var = self.config.provider.env_var();
            let patch = serde_json::json!({
                "env": {
                    "vars": {
                        env_var: key,
                    }
                }
            });
            super::merge_json(&mut existing, &patch);
        }

        // Set default model
        let patch = serde_json::json!({
            "agents": {
                "defaults": {
                    "model": {
                        "primary": model,
                    }
                }
            }
        });
        super::merge_json(&mut existing, &patch);

        let merged =
            serde_json::to_string_pretty(&existing).context("failed to serialize config")?;
        std::fs::write(&config_path, &merged)
            .with_context(|| format!("failed to write {}", config_path.display()))?;

        tracing::info!("cloud→openclaw connected: {provider} configured as LLM provider");
        Ok(())
    }
}
