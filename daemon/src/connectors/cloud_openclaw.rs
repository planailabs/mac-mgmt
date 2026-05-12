use anyhow::Result;

use super::{
    Connector, ConnectorPhase, enabled_cloud_configs, non_empty, non_empty_secret, resolve_model,
};
use crate::sentry_ext;
use crate::services::openclaw::{config_path, merge_and_validate};

/// Configures OpenClaw with all enabled cloud LLM providers.
///
/// The connector reads live config from the "cloud" config store entry on each
/// run, so hot-reloaded cloud settings take effect without a daemon restart.
/// All enabled providers are configured; the first one's model is set as the
/// default.
pub struct CloudOpenClaw {
    pub set_default: bool,
}

impl Connector for CloudOpenClaw {
    fn name(&self) -> &str {
        "cloud→openclaw"
    }

    fn phase(&self) -> ConnectorPhase {
        ConnectorPhase::PreStart
    }

    fn depends_on(&self) -> &[&str] {
        &["openclaw", "cloud"]
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
            tracing::warn!("openclaw config not found, skipping cloud connector");
            return Ok(());
        }

        let mut patch = serde_json::json!({});
        let mut custom_providers = serde_json::Map::new();

        // The first enabled provider's model becomes the default.
        let primary_model = resolve_model(&enabled[0]);

        let provider_names: Vec<&str> = enabled.iter().map(|c| c.provider.as_str()).collect();
        tracing::info!(
            "connecting cloud providers [{}] to openclaw (primary model={primary_model})",
            provider_names.join(", ")
        );
        sentry_ext::breadcrumb(
            "connector",
            &format!(
                "cloud→openclaw providers=[{}] primary_model={primary_model}",
                provider_names.join(", ")
            ),
            &[("connector", "cloud→openclaw")],
        );

        for config in &enabled {
            let provider = config.provider.as_str();
            let model = resolve_model(config);
            let base_url =
                non_empty(&config.base_url).unwrap_or_else(|| config.provider.base_url());

            let mut provider_cfg = serde_json::json!({ "baseUrl": base_url });
            if let Some(ref api) = config.api {
                provider_cfg["api"] = serde_json::json!(api);
            }
            if let Some(ref auth) = config.auth {
                provider_cfg["auth"] = serde_json::json!(auth);
            }
            // API keys are passed via environment variables (service_env),
            // not embedded in the config file.
            let model_id = model.splitn(2, '/').nth(1).unwrap_or(&model);
            provider_cfg["models"] = serde_json::json!([{ "id": model_id, "name": model_id }]);
            custom_providers.insert(provider.to_string(), provider_cfg);
        }

        if !custom_providers.is_empty() {
            patch["models"] = serde_json::json!({ "providers": custom_providers });
        }

        // Set default model from the first enabled provider.
        if self.set_default && !primary_model.is_empty() {
            patch["agents"] =
                serde_json::json!({ "defaults": { "model": { "primary": primary_model } } });
        }

        if patch.as_object().is_some_and(|o| !o.is_empty()) {
            merge_and_validate(&path, &patch)?;
            tracing::info!(
                "cloud→openclaw connected: {} provider(s) configured",
                enabled.len()
            );
        } else {
            tracing::info!("cloud→openclaw: nothing to configure");
        }

        Ok(())
    }

    fn service_env(
        &self,
        service_name: &str,
        configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> std::collections::HashMap<String, String> {
        if service_name != "openclaw" {
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
