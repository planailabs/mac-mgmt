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

    /// Build the hermes config patch for the enabled cloud providers. Pure (no
    /// filesystem) so it can be unit-tested independently of the hermes binary
    /// used by `merge_and_validate`. The first enabled provider sets
    /// `model.provider` (+ `model.default`); every non-built-in provider is
    /// registered under `custom_providers`.
    fn build_patch(enabled: &[CloudConfig]) -> serde_json::Value {
        let Some(primary) = enabled.first() else {
            return serde_json::json!({});
        };
        let mut patch = serde_json::json!({
            "model": { "provider": Self::provider_ref(primary) }
        });
        if !primary.default_model.is_empty() {
            patch["model"]["default"] = serde_json::json!(&primary.default_model);
        }

        // Register every non-built-in provider (openai/mistral/groq/together +
        // any Custom) as a custom_providers entry referenced via `custom:<name>`.
        // ponytail: merge_json replaces arrays, so this connector is the
        // authoritative owner of hermes custom_providers for cloud config.
        let custom_providers: Vec<serde_json::Value> = enabled
            .iter()
            .filter(|cfg| Self::builtin_id(&cfg.provider).is_none())
            .map(|cfg| {
                let name = cfg.provider.as_str();
                let base_url = non_empty(&cfg.base_url).unwrap_or_else(|| cfg.provider.base_url());
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
        patch
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

        let patch = Self::build_patch(&enabled);
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

#[cfg(test)]
mod tests {
    use super::*;
    use mac_mgmt_common::Secret;

    fn cfg(provider: &str) -> CloudConfig {
        CloudConfig {
            provider: CloudProvider::from_name(provider),
            ..Default::default()
        }
    }

    #[test]
    fn builtin_provider_ref_and_no_custom_entry() {
        let c = CloudConfig {
            default_model: "anthropic/claude-sonnet-4-6".into(),
            ..cfg("anthropic")
        };
        let patch = CloudHermes::build_patch(&[c]);
        assert_eq!(patch["model"]["provider"], "anthropic");
        assert_eq!(patch["model"]["default"], "anthropic/claude-sonnet-4-6");
        assert!(patch.get("custom_providers").is_none());
    }

    #[test]
    fn custom_provider_registered_and_referenced() {
        let c = CloudConfig {
            provider: CloudProvider::from_name("moonshot"),
            base_url: Some("https://api.moonshot.ai/v1".into()),
            default_model: "moonshot/kimi-k2.6".into(),
            ..Default::default()
        };
        let patch = CloudHermes::build_patch(&[c]);
        assert_eq!(patch["model"]["provider"], "custom:moonshot");
        assert_eq!(
            patch["custom_providers"],
            serde_json::json!([{
                "name": "moonshot",
                "base_url": "https://api.moonshot.ai/v1",
                "key_env": "MOONSHOT_API_KEY",
            }])
        );
    }

    #[test]
    fn openai_is_treated_as_custom_in_hermes() {
        assert_eq!(
            CloudHermes::builtin_id(&CloudProvider::from_name("openai")),
            None
        );
        assert_eq!(CloudHermes::provider_ref(&cfg("openai")), "custom:openai");
        assert_eq!(CloudHermes::provider_ref(&cfg("anthropic")), "anthropic");
    }

    #[test]
    fn service_env_maps_keys_and_skips_bedrock() {
        let cfgs = vec![
            CloudConfig {
                api_key: Some(Secret::new("sk-ant")),
                enabled: true,
                ..cfg("anthropic")
            },
            CloudConfig {
                api_key: Some(Secret::new("sk-moon")),
                enabled: true,
                ..cfg("moonshot")
            },
            CloudConfig {
                api_key: Some(Secret::new("aws")),
                enabled: true,
                ..cfg("bedrock")
            },
        ];
        let mut configs = std::collections::HashMap::new();
        configs.insert("cloud".into(), serde_json::to_value(&cfgs).unwrap());

        let env = CloudHermes { set_default: true }.service_env("hermes", &configs);
        assert_eq!(
            env.get("ANTHROPIC_API_KEY").map(String::as_str),
            Some("sk-ant")
        );
        assert_eq!(
            env.get("MOONSHOT_API_KEY").map(String::as_str),
            Some("sk-moon")
        );
        assert!(
            !env.contains_key("AWS_ACCESS_KEY_ID"),
            "bedrock authenticates via AWS creds, not a key env"
        );
        // Env is only injected into hermes.
        assert!(
            CloudHermes { set_default: true }
                .service_env("openclaw", &configs)
                .is_empty()
        );
    }
}
