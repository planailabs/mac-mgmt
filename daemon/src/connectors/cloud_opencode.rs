use anyhow::Result;

use super::Connector;
use crate::sentry_ext;
use crate::services::opencode::{config_path, merge_and_write};
use mac_mgmt_common::CloudConfig;

/// Configures OpenCode to use a cloud LLM provider.
pub struct CloudOpencode {
    pub config: CloudConfig,
}

impl Connector for CloudOpencode {
    fn name(&self) -> &str {
        "cloud→opencode"
    }

    fn depends_on(&self) -> &[&str] {
        &["opencode"]
    }

    fn connect(
        &self,
        _configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let provider = self.config.provider.as_str();
        let model = if self.config.default_model.is_empty()
            || !self.config.default_model.starts_with(provider)
        {
            self.config.provider.default_model().to_string()
        } else {
            self.config.default_model.clone()
        };

        tracing::info!("connecting cloud provider {provider} to opencode (model={model})");
        sentry_ext::breadcrumb(
            "connector",
            &format!("cloud→opencode provider={provider} model={model}"),
            &[
                ("connector", "cloud→opencode"),
                ("provider", provider),
                ("model", &model),
            ],
        );

        let path = config_path()?;
        if !path.exists() {
            tracing::warn!("opencode config not found, skipping cloud connector");
            return Ok(());
        }

        let mut provider_opts = serde_json::json!({});
        if let Some(key) = &self.config.api_key {
            if !key.is_empty() {
                provider_opts["apiKey"] = serde_json::json!(key);
            }
        }
        if let Some(url) = &self.config.base_url {
            if !url.is_empty() {
                provider_opts["baseURL"] = serde_json::json!(url);
            }
        }

        let patch = serde_json::json!({
            "provider": {
                provider: {
                    "options": provider_opts,
                }
            },
            "model": model,
        });

        merge_and_write(&path, &patch)?;
        tracing::info!("cloud→opencode connected: {provider} configured");
        Ok(())
    }
}
