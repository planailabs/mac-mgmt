use anyhow::Result;

use super::{Connector, ConnectorPhase, non_empty_secret};
use crate::sentry_ext;
use crate::services::opencode::{config_path, merge_and_validate};

/// Configures OpenCode to use LiteLLM as a single cloud provider.
///
/// When LiteLLM is enabled, this connector replaces [`CloudOpencode`] and
/// registers LiteLLM's OpenAI-compatible endpoint as the sole cloud
/// provider.
pub struct LitellmOpencode {
    pub host: String,
    pub port: u16,
    pub set_default: bool,
}

impl LitellmOpencode {
    fn base_url(&self) -> String {
        let host = if self.host.is_empty() {
            "127.0.0.1"
        } else {
            &self.host
        };
        let port = if self.port == 0 { 4100 } else { self.port };
        format!("http://{host}:{port}/v1")
    }
}

impl Connector for LitellmOpencode {
    fn name(&self) -> &str {
        "litellm→opencode"
    }

    fn phase(&self) -> ConnectorPhase {
        ConnectorPhase::PreStart
    }

    fn depends_on(&self) -> &[&str] {
        &["opencode", "litellm"]
    }

    fn connect(
        &self,
        configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let path = config_path()?;
        if !path.exists() {
            tracing::warn!("opencode config not found, skipping litellm connector");
            return Ok(());
        }

        let base_url = self.base_url();
        tracing::info!("connecting litellm to opencode (baseURL={base_url})");
        sentry_ext::breadcrumb(
            "connector",
            &format!("litellm→opencode baseURL={base_url}"),
            &[("connector", "litellm→opencode")],
        );

        // Register litellm as a provider with the OpenAI-compatible endpoint.
        let mut patch = serde_json::json!({
            "provider": {
                "litellm": {
                    "options": {
                        "baseURL": base_url,
                    }
                }
            }
        });

        // Set default model from cloud configs.
        if self.set_default {
            if let Some(model) = default_model_from_configs(configs) {
                patch["model"] = serde_json::json!(model);
            }
        }

        merge_and_validate(&path, &patch)?;
        tracing::info!("litellm→opencode connected");
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
        let mut env = std::collections::HashMap::new();
        if let Some(litellm_cfg) = extract_litellm_config(configs) {
            if let Some(key) = non_empty_secret(&litellm_cfg.master_key) {
                env.insert("LITELLM_API_KEY".to_string(), key.to_string());
            }
        }
        env
    }
}

/// Extract the first enabled cloud model as the default.
fn default_model_from_configs(
    configs: &std::collections::HashMap<String, serde_json::Value>,
) -> Option<String> {
    let v = configs.get("cloud")?;
    let all: Vec<mac_mgmt_common::CloudConfig> = serde_json::from_value(v.clone()).ok()?;
    let first = all.into_iter().find(|c| c.enabled)?;
    let model = &first.default_model;
    if model.is_empty() {
        Some(first.provider.default_model().to_string())
    } else {
        Some(model.clone())
    }
}

/// Extract LitellmConfig from the config store.
fn extract_litellm_config(
    configs: &std::collections::HashMap<String, serde_json::Value>,
) -> Option<mac_mgmt_common::LitellmConfig> {
    let v = configs.get("litellm")?;
    serde_json::from_value(v.clone()).ok()
}
