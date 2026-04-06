use anyhow::Result;

use super::Connector;
use crate::sentry_ext;
use crate::services::openclaw::{config_path, merge_and_validate};
use mac_mgmt_common::CloudConfig;

/// Configures OpenClaw to use a cloud LLM provider (Anthropic, OpenAI, etc.).
pub struct CloudOpenClaw {
    pub config: CloudConfig,
}

/// Return Some only if the string is non-empty.
fn non_empty(s: &Option<String>) -> Option<&str> {
    s.as_deref().filter(|s| !s.is_empty())
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
            &[("connector", "cloud→openclaw"), ("provider", provider), ("model", model)],
        );

        let path = config_path()?;
        if !path.exists() {
            tracing::warn!("openclaw config not found, skipping cloud connector");
            return Ok(());
        }

        let api_key = non_empty(&self.config.api_key);
        let base_url = non_empty(&self.config.base_url);

        let mut patch = serde_json::json!({});

        // Custom provider: needs a models.providers entry when base_url, api, or auth is set.
        // Built-in provider: just set the API key env var.
        let needs_custom = base_url.is_some()
            || self.config.api.is_some()
            || self.config.auth.is_some();

        if needs_custom {
            let mut provider_cfg = serde_json::json!({});
            if let Some(url) = base_url {
                provider_cfg["baseUrl"] = serde_json::json!(url);
            }
            if let Some(ref api) = self.config.api {
                provider_cfg["api"] = serde_json::json!(api);
            }
            if let Some(ref auth) = self.config.auth {
                provider_cfg["auth"] = serde_json::json!(auth);
            }
            if let Some(key) = api_key {
                provider_cfg["apiKey"] = serde_json::json!(key);
            }
            let model_id = model.split('/').last().unwrap_or(model);
            provider_cfg["models"] = serde_json::json!([{ "id": model_id, "name": model_id }]);

            patch["models"] = serde_json::json!({ "providers": { provider: provider_cfg } });
        } else if let Some(key) = api_key {
            let env_var = self.config.provider.env_var();
            patch["env"] = serde_json::json!({ "vars": { env_var: key } });
        }

        // Set default model
        if !model.is_empty() {
            patch["agents"] = serde_json::json!({ "defaults": { "model": { "primary": model } } });
        }

        if patch.as_object().is_some_and(|o| !o.is_empty()) {
            merge_and_validate(&path, &patch)?;
            tracing::info!("cloud→openclaw connected: {provider} configured");
        } else {
            tracing::info!("cloud→openclaw: nothing to configure");
        }

        Ok(())
    }
}
