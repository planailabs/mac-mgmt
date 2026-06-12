use anyhow::Result;

use super::{Connector, ConnectorPhase, enabled_cloud_configs, non_empty_secret};
use crate::sentry_ext;
use crate::services::hermes::{config_path, merge_and_validate};
use mac_mgmt_common::CloudProvider;

/// Registers cloud LLM providers in Hermes.
///
/// Maps cloud provider configs to Hermes provider names and injects API keys
/// via SpawnSpec env vars.
pub struct CloudHermes {
    pub set_default: bool,
}

impl CloudHermes {
    /// Map CloudProvider to hermes provider name.
    fn hermes_provider(provider: &CloudProvider) -> &'static str {
        match provider {
            CloudProvider::Anthropic => "anthropic",
            CloudProvider::Openai => "openai",
            CloudProvider::Google => "gemini",
            CloudProvider::Groq => "custom",
            CloudProvider::Xai => "custom",
            CloudProvider::Deepseek => "custom",
            CloudProvider::Openrouter => "openrouter",
            CloudProvider::Mistral => "custom",
            CloudProvider::Together => "custom",
            CloudProvider::Bedrock => "custom",
        }
    }
}

impl Connector for CloudHermes {
    fn name(&self) -> &str {
        "cloud→hermes"
    }

    fn phase(&self) -> ConnectorPhase {
        ConnectorPhase::PreStart
    }

    fn depends_on(&self) -> &[&str] {
        &["cloud", "hermes"]
    }

    fn connect(
        &self,
        configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let enabled = enabled_cloud_configs(configs);
        if enabled.is_empty() {
            return Ok(());
        }

        let primary = &enabled[0];
        let provider_name = Self::hermes_provider(&primary.provider);

        tracing::info!(
            "connecting cloud ({}) to hermes (provider={provider_name}, default={})",
            primary.provider.as_str(),
            self.set_default,
        );
        sentry_ext::breadcrumb(
            "connector",
            &format!(
                "cloud→hermes provider={} hermes_provider={provider_name}",
                primary.provider.as_str()
            ),
            &[("connector", "cloud→hermes")],
        );

        let path = config_path()?;
        if !path.exists() {
            tracing::warn!("hermes config not found, skipping cloud connector");
            return Ok(());
        }

        let mut patch = serde_json::json!({
            "model": {
                "provider": provider_name,
            }
        });

        if !primary.default_model.is_empty() {
            patch["model"]["default"] = serde_json::json!(&primary.default_model);
        }

        merge_and_validate(&path, &patch)?;
        tracing::info!("cloud→hermes connected");
        Ok(())
    }

    fn service_env(
        &self,
        service_name: &str,
        configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> std::collections::HashMap<String, String> {
        let mut env = std::collections::HashMap::new();
        if service_name != "hermes" {
            return env;
        }

        let enabled = enabled_cloud_configs(configs);
        for cfg in &enabled {
            if let Some(key) = non_empty_secret(&cfg.api_key) {
                let env_var = match cfg.provider {
                    CloudProvider::Anthropic => "ANTHROPIC_API_KEY",
                    CloudProvider::Openai => "OPENAI_API_KEY",
                    CloudProvider::Google => "GOOGLE_API_KEY",
                    CloudProvider::Groq => "GROQ_API_KEY",
                    CloudProvider::Xai => "XAI_API_KEY",
                    CloudProvider::Deepseek => "DEEPSEEK_API_KEY",
                    CloudProvider::Openrouter => "OPENROUTER_API_KEY",
                    CloudProvider::Mistral => "MISTRAL_API_KEY",
                    CloudProvider::Together => "TOGETHER_API_KEY",
                    CloudProvider::Bedrock => continue,
                };
                env.insert(env_var.into(), key.to_string());
            }
        }

        env
    }
}
