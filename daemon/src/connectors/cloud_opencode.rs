use anyhow::Result;

use super::Connector;
use crate::sentry_ext;
use crate::services::opencode::{config_path, merge_and_write};
use mac_mgmt_common::CloudConfig;

/// Configures OpenCode to use a cloud LLM provider.
///
/// Reads live config from the "cloud" config store entry on each run.
pub struct CloudOpencode;

/// Extract the first enabled CloudConfig from the configs map.
fn cloud_config_from(
    configs: &std::collections::HashMap<String, serde_json::Value>,
) -> Option<CloudConfig> {
    let cloud_val = configs.get("cloud")?;
    let list: Vec<CloudConfig> = serde_json::from_value(cloud_val.clone()).ok()?;
    list.into_iter().find(|c| c.enabled)
}

impl Connector for CloudOpencode {
    fn name(&self) -> &str {
        "cloud→opencode"
    }

    fn depends_on(&self) -> &[&str] {
        &["opencode", "cloud"]
    }

    fn connect(
        &self,
        configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let config = cloud_config_from(configs)
            .ok_or_else(|| anyhow::anyhow!("no enabled cloud provider found"))?;
        let provider = config.provider.as_str();
        let model = if config.default_model.is_empty()
            || !config.default_model.starts_with(provider)
        {
            config.provider.default_model().to_string()
        } else {
            config.default_model.clone()
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
        if let Some(key) = &config.api_key {
            if !key.is_empty() {
                provider_opts["apiKey"] = serde_json::json!(key);
            }
        }
        if let Some(url) = &config.base_url {
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
