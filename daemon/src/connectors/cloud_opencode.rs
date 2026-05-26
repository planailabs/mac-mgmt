use anyhow::Result;

use super::{Connector, ConnectorPhase, enabled_cloud_configs, non_empty_secret, resolve_model};
use crate::sentry_ext;
use crate::services::opencode::{config_path, merge_and_validate};

/// Registers all enabled cloud LLM providers in OpenCode.
pub struct CloudOpencode {
    pub set_default: bool,
}

impl Connector for CloudOpencode {
    fn name(&self) -> &str {
        "cloud→opencode"
    }

    fn phase(&self) -> ConnectorPhase {
        ConnectorPhase::PreStart
    }

    fn depends_on(&self) -> &[&str] {
        &["opencode", "cloud"]
    }

    fn connect(
        &self,
        configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let enabled = enabled_cloud_configs(configs);
        if enabled.is_empty() {
            anyhow::bail!("no enabled cloud provider found");
        }

        let path = config_path()?;
        if !path.exists() {
            tracing::warn!("opencode config not found, skipping cloud connector");
            return Ok(());
        }

        let primary_model = resolve_model(&enabled[0]);

        let provider_names: Vec<&str> = enabled.iter().map(|c| c.provider.as_str()).collect();
        tracing::info!(
            "connecting cloud providers [{}] to opencode (default={})",
            provider_names.join(", "),
            self.set_default,
        );
        sentry_ext::breadcrumb(
            "connector",
            &format!("cloud→opencode providers=[{}]", provider_names.join(", ")),
            &[("connector", "cloud→opencode")],
        );

        let mut providers = serde_json::Map::new();
        for config in &enabled {
            let provider = config.provider.as_str();
            let mut opts = serde_json::json!({});
            // API keys are passed via environment variables (service_env),
            // not embedded in the config file.
            if let Some(url) = &config.base_url {
                if !url.is_empty() {
                    opts["baseURL"] = serde_json::json!(url);
                }
            }
            providers.insert(provider.to_string(), serde_json::json!({ "options": opts }));
        }

        let mut patch = serde_json::json!({ "provider": providers });
        if self.set_default {
            patch["model"] = serde_json::json!(primary_model);
        }

        merge_and_validate(&path, &patch)?;
        tracing::info!(
            "cloud→opencode connected: {} provider(s) configured",
            enabled.len()
        );
        Ok(())
    }

    fn service_env(
        &self,
        service_name: &str,
        configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> std::collections::HashMap<String, String> {
        if service_name != "opencode" {
            return Default::default();
        }
        let enabled = enabled_cloud_configs(configs);
        let mut env = std::collections::HashMap::new();
        for config in &enabled {
            if let Some(key) = non_empty_secret(&config.api_key) {
                env.insert(config.provider.env_var().to_string(), key.to_string());
            }
        }
        env
    }
}
