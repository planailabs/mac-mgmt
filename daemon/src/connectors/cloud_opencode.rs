use anyhow::Result;

use super::{enabled_cloud_configs, resolve_model, Connector, ConnectorPhase};
use crate::sentry_ext;
use crate::services::opencode::{config_path, merge_and_write};

/// Configures OpenCode with all enabled cloud LLM providers.
///
/// Reads live config from the "cloud" config store entry on each run.
/// All enabled providers are configured; the first one's model is set as the
/// default.
pub struct CloudOpencode;

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

        // The first enabled provider's model becomes the default.
        let primary_model = resolve_model(&enabled[0]);

        let provider_names: Vec<&str> = enabled.iter().map(|c| c.provider.as_str()).collect();
        tracing::info!(
            "connecting cloud providers [{}] to opencode (primary model={primary_model})",
            provider_names.join(", ")
        );
        sentry_ext::breadcrumb(
            "connector",
            &format!(
                "cloud→opencode providers=[{}] primary_model={primary_model}",
                provider_names.join(", ")
            ),
            &[("connector", "cloud→opencode")],
        );

        // Configure all enabled providers.
        let mut providers = serde_json::Map::new();
        for config in &enabled {
            let provider = config.provider.as_str();
            let mut opts = serde_json::json!({});
            if let Some(key) = &config.api_key {
                if !key.is_empty() {
                    opts["apiKey"] = serde_json::json!(key);
                }
            }
            if let Some(url) = &config.base_url {
                if !url.is_empty() {
                    opts["baseURL"] = serde_json::json!(url);
                }
            }
            providers.insert(
                provider.to_string(),
                serde_json::json!({ "options": opts }),
            );
        }

        let patch = serde_json::json!({
            "provider": providers,
            "model": primary_model,
        });

        merge_and_write(&path, &patch)?;
        tracing::info!(
            "cloud→opencode connected: {} provider(s) configured",
            enabled.len()
        );
        Ok(())
    }
}
