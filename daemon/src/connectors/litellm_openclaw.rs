use anyhow::Result;

use super::{non_empty_secret, Connector, ConnectorPhase};
use crate::sentry_ext;
use crate::services::openclaw::{config_path, merge_and_validate};

/// Configures OpenClaw to use LiteLLM as a single cloud provider.
///
/// When LiteLLM is enabled, this connector replaces [`CloudOpenClaw`] and
/// registers LiteLLM's OpenAI-compatible endpoint as the sole cloud
/// provider. All individual cloud provider routing is handled by LiteLLM.
pub struct LitellmOpenClaw {
    pub host: String,
    pub port: u16,
    pub set_default: bool,
}

impl LitellmOpenClaw {
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

impl Connector for LitellmOpenClaw {
    fn name(&self) -> &str {
        "litellm→openclaw"
    }

    fn phase(&self) -> ConnectorPhase {
        ConnectorPhase::PreStart
    }

    fn depends_on(&self) -> &[&str] {
        &["openclaw", "litellm"]
    }

    fn connect(
        &self,
        configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let path = config_path()?;
        if !path.exists() {
            tracing::warn!("openclaw config not found, skipping litellm connector");
            return Ok(());
        }

        let base_url = self.base_url();
        tracing::info!("connecting litellm to openclaw (baseUrl={base_url})");
        sentry_ext::breadcrumb(
            "connector",
            &format!("litellm→openclaw baseUrl={base_url}"),
            &[("connector", "litellm→openclaw")],
        );

        // Register litellm as an openai-compatible custom provider.
        let mut patch = serde_json::json!({
            "models": {
                "providers": {
                    "litellm": {
                        "baseUrl": base_url,
                        "api": "openai-completions",
                    }
                }
            }
        });

        // Determine the default model from the cloud configs if available.
        if self.set_default {
            if let Some(model) = default_model_from_configs(configs) {
                patch["agents"] =
                    serde_json::json!({ "defaults": { "model": { "primary": model } } });
            }
        }

        merge_and_validate(&path, &patch)?;
        tracing::info!("litellm→openclaw connected");

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
        // Pass through the LiteLLM master key if set, and all cloud API keys
        // so that the agent's env has them available (litellm reads them too).
        let mut env = std::collections::HashMap::new();
        if let Some(litellm_cfg) = extract_litellm_config(configs) {
            if let Some(key) = non_empty_secret(&litellm_cfg.master_key) {
                env.insert("LITELLM_API_KEY".to_string(), key.to_string());
            }
        }
        env
    }
}

/// Extract the first enabled cloud model as the default for the agent.
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
        // For litellm, models are routed as provider/model so pass through as-is.
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
