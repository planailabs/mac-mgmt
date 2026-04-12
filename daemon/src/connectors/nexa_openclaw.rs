use anyhow::Result;

use super::Connector;
use crate::sentry_ext;
use crate::services::openclaw::{config_path, merge_and_validate};

/// Configures OpenClaw to use Nexa as its LLM backend.
///
/// Nexa exposes an OpenAI-compatible API at `http://host:port/v1`.
pub struct NexaOpenClaw {
    pub host: String,
    pub port: u16,
    pub default_model: String,
}

impl Connector for NexaOpenClaw {
    fn name(&self) -> &str {
        "nexa→openclaw"
    }

    fn depends_on(&self) -> &[&str] {
        &["nexa", "openclaw"]
    }

    fn connect(&self, _virtual_services: &std::collections::HashMap<String, serde_json::Value>) -> Result<()> {
        let base_url = format!("http://{}:{}/v1", self.host, self.port);
        tracing::info!("connecting nexa to openclaw (baseUrl={base_url}, model={})", self.default_model);
        sentry_ext::breadcrumb(
            "connector",
            &format!("nexa→openclaw baseUrl={base_url}"),
            &[("connector", "nexa→openclaw"), ("base_url", &base_url), ("model", &self.default_model)],
        );

        let path = config_path()?;
        if !path.exists() {
            tracing::warn!("openclaw config not found, skipping nexa connector");
            return Ok(());
        }

        let model_id = &self.default_model;
        let patch = serde_json::json!({
            "models": {
                "providers": {
                    "nexa": {
                        "baseUrl": base_url,
                        "api": "openai-completions",
                        "models": [{ "id": model_id, "name": model_id }],
                    }
                }
            },
            "agents": {
                "defaults": {
                    "model": {
                        "primary": format!("nexa/{model_id}"),
                    }
                }
            }
        });

        merge_and_validate(&path, &patch)?;
        tracing::info!("nexa→openclaw connected");
        Ok(())
    }
}
