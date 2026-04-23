use anyhow::Result;

use super::Connector;
use crate::sentry_ext;
use crate::services::openclaw::{config_path, merge_and_validate};
use mac_mgmt_common::CloudConfig;

/// Configures OpenClaw with all enabled cloud LLM providers.
///
/// The connector reads live config from the "cloud" config store entry on each
/// run, so hot-reloaded cloud settings take effect without a daemon restart.
/// All enabled providers are configured; the first one's model is set as the
/// default.
pub struct CloudOpenClaw;

/// Return Some only if the string is non-empty.
fn non_empty(s: &Option<String>) -> Option<&str> {
    s.as_deref().filter(|s| !s.is_empty())
}

/// Extract all enabled CloudConfigs from the configs map.
fn enabled_cloud_configs(
    configs: &std::collections::HashMap<String, serde_json::Value>,
) -> Vec<CloudConfig> {
    let Some(v) = configs.get("cloud") else {
        tracing::warn!("cloud config not found in config store");
        return Vec::new();
    };
    let all = match serde_json::from_value::<Vec<CloudConfig>>(v.clone()) {
        Ok(list) => list,
        Err(e) => {
            tracing::error!("failed to deserialize cloud configs: {e}");
            return Vec::new();
        }
    };
    let enabled: Vec<CloudConfig> = all.into_iter().filter(|c| c.enabled).collect();
    if enabled.is_empty() {
        tracing::debug!("no enabled cloud provider entries in config store");
    }
    enabled
}

/// Resolve the model for a provider, falling back to the provider's default
/// if the configured model doesn't match the provider prefix.
fn resolve_model(config: &CloudConfig) -> String {
    let provider = config.provider.as_str();
    if config.default_model.is_empty() || !config.default_model.starts_with(provider) {
        config.provider.default_model().to_string()
    } else {
        config.default_model.clone()
    }
}

impl Connector for CloudOpenClaw {
    fn name(&self) -> &str {
        "cloud→openclaw"
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
        let mut env_vars = serde_json::Map::new();
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
            let api_key = non_empty(&config.api_key);
            let base_url = non_empty(&config.base_url);

            let needs_custom =
                base_url.is_some() || config.api.is_some() || config.auth.is_some();

            if needs_custom {
                let mut provider_cfg = serde_json::json!({});
                if let Some(url) = base_url {
                    provider_cfg["baseUrl"] = serde_json::json!(url);
                }
                if let Some(ref api) = config.api {
                    provider_cfg["api"] = serde_json::json!(api);
                }
                if let Some(ref auth) = config.auth {
                    provider_cfg["auth"] = serde_json::json!(auth);
                }
                if let Some(key) = api_key {
                    provider_cfg["apiKey"] = serde_json::json!(key);
                }
                let model_id = model.split('/').last().unwrap_or(&model);
                provider_cfg["models"] =
                    serde_json::json!([{ "id": model_id, "name": model_id }]);
                custom_providers.insert(provider.to_string(), provider_cfg);
            } else if let Some(key) = api_key {
                env_vars.insert(
                    config.provider.env_var().to_string(),
                    serde_json::Value::String(key.to_string()),
                );
            }
        }

        if !env_vars.is_empty() {
            patch["env"] = serde_json::json!({ "vars": env_vars });
        }
        if !custom_providers.is_empty() {
            patch["models"] = serde_json::json!({ "providers": custom_providers });
        }

        // Set default model from the first enabled provider.
        if !primary_model.is_empty() {
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
}
