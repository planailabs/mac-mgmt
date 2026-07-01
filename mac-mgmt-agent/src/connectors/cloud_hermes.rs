use anyhow::Result;

use super::{
    Connector, ConnectorPhase, custom_key_env, enabled_cloud_configs, non_empty, non_empty_secret,
};
use crate::sentry_ext;
use crate::services::hermes::{config_path, merge_and_validate};
use mac_mgmt_common::{CloudConfig, CloudProvider};

/// Registers cloud LLM providers in Hermes.
///
/// Built-in hermes providers (anthropic, gemini, openrouter, xai, deepseek,
/// bedrock) are referenced bare. Everything else — the OpenAI-compatible
/// providers without a built-in id (openai/mistral/groq/together) and any
/// `Custom(name)` — is registered as a `custom_providers` entry and referenced
/// as `custom:<name>`, with its API key supplied via a generated env var.
pub struct CloudHermes {
    pub set_default: bool,
}

impl CloudHermes {
    /// Hermes built-in provider id, or `None` if the provider must be
    /// registered as a `custom_providers` entry instead.
    fn builtin_id(provider: &CloudProvider) -> Option<&'static str> {
        match provider {
            CloudProvider::Anthropic => Some("anthropic"),
            CloudProvider::Google => Some("gemini"),
            CloudProvider::Xai => Some("xai"),
            CloudProvider::Deepseek => Some("deepseek"),
            CloudProvider::Openrouter => Some("openrouter"),
            CloudProvider::Bedrock => Some("bedrock"),
            // No built-in id → custom_providers + `custom:<name>` reference.
            CloudProvider::Openai
            | CloudProvider::Mistral
            | CloudProvider::Groq
            | CloudProvider::Together
            | CloudProvider::Custom(_) => None,
        }
    }

    /// The `model.provider` reference for this config: a bare built-in id, or
    /// `custom:<name>` for a custom_providers-backed provider.
    fn provider_ref(cfg: &CloudConfig) -> String {
        match Self::builtin_id(&cfg.provider) {
            Some(id) => id.to_string(),
            None => format!("custom:{}", cfg.provider.as_str()),
        }
    }

    /// Env-var name hermes reads this provider's API key from.
    fn key_env(cfg: &CloudConfig) -> &'static str {
        // ponytail: built-in ids read fixed env vars; custom providers use a
        // generated name (returned by `custom_key_env`, handled at call sites).
        match cfg.provider {
            CloudProvider::Anthropic => "ANTHROPIC_API_KEY",
            CloudProvider::Google => "GOOGLE_API_KEY",
            CloudProvider::Xai => "XAI_API_KEY",
            CloudProvider::Deepseek => "DEEPSEEK_API_KEY",
            CloudProvider::Openrouter => "OPENROUTER_API_KEY",
            _ => "",
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
        let provider_ref = Self::provider_ref(primary);

        tracing::info!(
            "connecting cloud ({}) to hermes (provider={provider_ref}, default={})",
            primary.provider.as_str(),
            self.set_default,
        );
        sentry_ext::breadcrumb(
            "connector",
            &format!(
                "cloud→hermes provider={} hermes_provider={provider_ref}",
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
                "provider": provider_ref,
            }
        });

        if !primary.default_model.is_empty() {
            patch["model"]["default"] = serde_json::json!(&primary.default_model);
        }

        // Register every non-built-in provider (openai/mistral/groq/together +
        // any Custom) as a custom_providers entry referenced via `custom:<name>`.
        // ponytail: merge_json replaces arrays, so this connector is the
        // authoritative owner of hermes custom_providers for cloud config —
        // manual entries in the file are not preserved across a sync.
        let custom_providers: Vec<serde_json::Value> = enabled
            .iter()
            .filter(|cfg| Self::builtin_id(&cfg.provider).is_none())
            .map(|cfg| {
                let name = cfg.provider.as_str();
                let base_url =
                    non_empty(&cfg.base_url).unwrap_or_else(|| cfg.provider.base_url());
                serde_json::json!({
                    "name": name,
                    "base_url": base_url,
                    "key_env": custom_key_env(name),
                })
            })
            .collect();
        if !custom_providers.is_empty() {
            patch["custom_providers"] = serde_json::json!(custom_providers);
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
            let Some(key) = non_empty_secret(&cfg.api_key) else {
                continue;
            };
            // Bedrock authenticates with AWS credentials, not a single key env.
            if matches!(cfg.provider, CloudProvider::Bedrock) {
                continue;
            }
            // Built-ins read fixed env vars; custom providers use the same
            // generated name written into their custom_providers `key_env`.
            let env_var = match Self::builtin_id(&cfg.provider) {
                Some(_) => Self::key_env(cfg).to_string(),
                None => custom_key_env(cfg.provider.as_str()),
            };
            env.insert(env_var, key.to_string());
        }

        env
    }
}
