use anyhow::Result;

use super::{
    Connector, ConnectorPhase, custom_key_env, enabled_cloud_configs, model_id, non_empty,
    non_empty_secret, resolve_model,
};
use crate::sentry_ext;
use crate::services::openclaw::{config_path, merge_and_validate};
use mac_mgmt_common::{CloudConfig, CloudProvider};

/// Configures OpenClaw with all enabled cloud LLM providers.
///
/// The connector reads live config from the "cloud" config store entry on each
/// run, so hot-reloaded cloud settings take effect without a daemon restart.
/// All enabled providers are configured; the first one's model is set as the
/// default.
pub struct CloudOpenClaw {
    pub set_default: bool,
}

impl CloudOpenClaw {
    /// Env-var name openclaw reads this provider's API key from: the canonical
    /// name for built-ins, or a generated `<NAME>_API_KEY` for custom providers.
    /// openclaw resolves the matching `${NAME_API_KEY}` reference written into
    /// the provider's `apiKey` field.
    fn key_env(config: &CloudConfig) -> String {
        match config.provider {
            CloudProvider::Custom(_) => custom_key_env(config.provider.as_str()),
            _ => config.provider.env_var().to_string(),
        }
    }

    /// Build the openclaw config patch for the enabled cloud providers. Pure
    /// (no filesystem), so it can be unit-tested independently of the openclaw
    /// binary used by `merge_and_validate`.
    fn build_patch(enabled: &[CloudConfig], set_default: bool) -> serde_json::Value {
        let mut patch = serde_json::json!({});
        let mut providers = serde_json::Map::new();

        // The first enabled provider's model becomes the default.
        let primary_model = enabled.first().map(resolve_model).unwrap_or_default();

        for config in enabled {
            let provider = config.provider.as_str();
            let base_url =
                non_empty(&config.base_url).unwrap_or_else(|| config.provider.base_url());

            // Build the model list, honouring the openclaw provider schema
            // (each entry needs id + name). openclaw requires the list to be
            // non-empty and to contain the primary/default model, so fall back
            // to — and always include — the resolved default model. For a
            // custom provider with no model catalog, the configured
            // default_model is the only model info available.
            let mut model_ids: Vec<String> =
                config.models.iter().map(|m| model_id(m).to_string()).collect();
            let default_id = model_id(&resolve_model(config)).to_string();
            if !default_id.is_empty() && !model_ids.iter().any(|id| *id == default_id) {
                model_ids.push(default_id);
            }
            if model_ids.is_empty() {
                tracing::warn!(
                    "cloud→openclaw: provider {provider} has no models and no default_model, \
                     skipping"
                );
                continue;
            }

            let mut provider_cfg = serde_json::json!({ "baseUrl": base_url });
            if let Some(ref api) = config.api {
                provider_cfg["api"] = serde_json::json!(api);
            }
            if let Some(ref auth) = config.auth {
                provider_cfg["auth"] = serde_json::json!(auth);
            }
            // Reference the API key via env substitution — openclaw resolves
            // `${NAME_API_KEY}`; the value itself is injected via service_env.
            if non_empty_secret(&config.api_key).is_some() {
                provider_cfg["apiKey"] =
                    serde_json::json!(format!("${{{}}}", Self::key_env(config)));
            }
            provider_cfg["models"] = serde_json::Value::Array(
                model_ids
                    .iter()
                    .map(|id| serde_json::json!({ "id": id, "name": id }))
                    .collect(),
            );
            providers.insert(provider.to_string(), provider_cfg);
        }

        if !providers.is_empty() {
            patch["models"] = serde_json::json!({ "providers": providers });
        }
        if set_default && !primary_model.is_empty() {
            patch["agents"] =
                serde_json::json!({ "defaults": { "model": { "primary": primary_model } } });
        }
        patch
    }

    /// The per-provider `ENV_VAR → API-key` pairs to deliver to openclaw
    /// (written to ~/.openclaw/.env). Built-ins use their canonical env var;
    /// custom providers use the generated `<NAME>_API_KEY` matching the
    /// `apiKey: "${NAME_API_KEY}"` reference in the provider config.
    fn key_env_vars(enabled: &[CloudConfig]) -> Vec<(String, String)> {
        enabled
            .iter()
            .filter_map(|c| non_empty_secret(&c.api_key).map(|k| (Self::key_env(c), k.to_string())))
            .collect()
    }
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

        let provider_names: Vec<&str> = enabled.iter().map(|c| c.provider.as_str()).collect();
        tracing::info!(
            "connecting cloud providers [{}] to openclaw",
            provider_names.join(", ")
        );
        sentry_ext::breadcrumb(
            "connector",
            &format!("cloud→openclaw providers=[{}]", provider_names.join(", ")),
            &[("connector", "cloud→openclaw")],
        );

        let patch = Self::build_patch(&enabled, self.set_default);

        // Deliver API keys through openclaw's trusted ~/.openclaw/.env so the
        // `apiKey: "${NAME_API_KEY}"` references in the config resolve. openclaw
        // loads this file into its process env at startup; relying on the
        // supervisor's spawn-env injection alone is not a reliable delivery
        // path for openclaw.
        let env_vars = Self::key_env_vars(&enabled);
        if let Err(e) = crate::services::openclaw::write_env_vars(&env_vars) {
            tracing::warn!("cloud→openclaw: failed to write API keys to .env: {e}");
        }

        if patch.as_object().is_some_and(|o| !o.is_empty()) {
            merge_and_validate(&path, &patch)?;
            tracing::info!(
                "cloud→openclaw connected: {} provider(s) configured, {} key(s) in .env",
                enabled.len(),
                env_vars.len(),
            );
        } else {
            tracing::info!("cloud→openclaw: nothing to configure");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mac_mgmt_common::{CloudApiType, Secret};

    fn custom(name: &str) -> CloudConfig {
        CloudConfig {
            provider: CloudProvider::from_name(name),
            ..Default::default()
        }
    }

    #[test]
    fn custom_provider_empty_models_falls_back_to_default() {
        let c = CloudConfig {
            provider: CloudProvider::from_name("moonshot"),
            base_url: Some("https://api.moonshot.ai/v1".into()),
            api: Some(CloudApiType::OpenaiCompletions),
            api_key: Some(Secret::new("sk-test")),
            default_model: "moonshot/kimi-k2.6".into(),
            models: vec![],
            ..Default::default()
        };
        let patch = CloudOpenClaw::build_patch(&[c], true);
        let prov = &patch["models"]["providers"]["moonshot"];
        assert_eq!(prov["baseUrl"], "https://api.moonshot.ai/v1");
        assert_eq!(prov["api"], "openai-completions");
        assert_eq!(prov["apiKey"], "${MOONSHOT_API_KEY}");
        assert_eq!(
            prov["models"],
            serde_json::json!([{ "id": "kimi-k2.6", "name": "kimi-k2.6" }])
        );
        assert_eq!(
            patch["agents"]["defaults"]["model"]["primary"],
            "moonshot/kimi-k2.6"
        );
    }

    #[test]
    fn builtin_provider_maps_models_and_appends_missing_default() {
        let c = CloudConfig {
            provider: CloudProvider::from_name("anthropic"),
            api_key: Some(Secret::new("sk-ant")),
            default_model: "anthropic/claude-sonnet-4-6".into(),
            models: vec!["anthropic/claude-opus-4-8".into()],
            ..Default::default()
        };
        let patch = CloudOpenClaw::build_patch(&[c], false);
        let prov = &patch["models"]["providers"]["anthropic"];
        // Built-in base URL fallback + canonical key env.
        assert_eq!(prov["baseUrl"], "https://api.anthropic.com/v1");
        assert_eq!(prov["apiKey"], "${ANTHROPIC_API_KEY}");
        // Configured model kept, default appended (not duplicated).
        assert_eq!(
            prov["models"],
            serde_json::json!([
                { "id": "claude-opus-4-8", "name": "claude-opus-4-8" },
                { "id": "claude-sonnet-4-6", "name": "claude-sonnet-4-6" },
            ])
        );
        // set_default=false → no agents override.
        assert!(patch.get("agents").is_none());
    }

    #[test]
    fn default_model_in_list_is_not_duplicated() {
        let c = CloudConfig {
            provider: CloudProvider::from_name("moonshot"),
            base_url: Some("https://x".into()),
            default_model: "moonshot/kimi-k2.6".into(),
            models: vec!["moonshot/kimi-k2.6".into()],
            ..Default::default()
        };
        let patch = CloudOpenClaw::build_patch(&[c], true);
        assert_eq!(
            patch["models"]["providers"]["moonshot"]["models"],
            serde_json::json!([{ "id": "kimi-k2.6", "name": "kimi-k2.6" }])
        );
    }

    #[test]
    fn provider_without_any_model_is_skipped() {
        // Custom provider, no default_model and no models → nothing to write.
        let patch = CloudOpenClaw::build_patch(&[custom("moonshot")], true);
        assert!(patch.get("models").is_none());
        assert!(patch.get("agents").is_none());
    }

    #[test]
    fn no_api_key_means_no_apikey_field() {
        let c = CloudConfig {
            provider: CloudProvider::from_name("xai"),
            default_model: "xai/grok-3-mini".into(),
            ..Default::default()
        };
        let patch = CloudOpenClaw::build_patch(&[c], true);
        assert!(patch["models"]["providers"]["xai"].get("apiKey").is_none());
    }

    #[test]
    fn key_env_vars_use_per_provider_names() {
        let cfgs = vec![
            CloudConfig {
                provider: CloudProvider::from_name("anthropic"),
                api_key: Some(Secret::new("sk-ant")),
                ..Default::default()
            },
            CloudConfig {
                provider: CloudProvider::from_name("moonshot"),
                api_key: Some(Secret::new("sk-moon")),
                ..Default::default()
            },
            // A provider without a key contributes nothing.
            custom("groq"),
        ];
        let map: std::collections::HashMap<String, String> =
            CloudOpenClaw::key_env_vars(&cfgs).into_iter().collect();
        // Built-in uses its canonical env var; custom uses the generated name
        // matching the `${MOONSHOT_API_KEY}` reference written into the config.
        assert_eq!(map.get("ANTHROPIC_API_KEY").map(String::as_str), Some("sk-ant"));
        assert_eq!(map.get("MOONSHOT_API_KEY").map(String::as_str), Some("sk-moon"));
        assert_eq!(map.len(), 2, "providers without a key are skipped");
    }
}
