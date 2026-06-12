use anyhow::Result;

use super::{Connector, ConnectorPhase};
use crate::sentry_ext;
use crate::services::openclaw::{config_path, merge_and_validate};

/// Registers Ollama as an LLM provider in OpenClaw.
///
/// Patches `~/.openclaw/openclaw.json` with the ollama provider config.
/// When `set_default` is true, also sets it as the primary model.
pub struct OllamaOpenClaw {
    pub host: String,
    pub port: u16,
    pub default_model: String,
    pub set_default: bool,
}

impl Connector for OllamaOpenClaw {
    fn name(&self) -> &str {
        "ollama→openclaw"
    }

    fn phase(&self) -> ConnectorPhase {
        ConnectorPhase::PreStart
    }

    fn depends_on(&self) -> &[&str] {
        &["ollama", "openclaw"]
    }

    fn connect(
        &self,
        _configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let model = &self.default_model;
        let base_url = format!("http://{}:{}", self.host, self.port);
        tracing::info!(
            "connecting ollama to openclaw (baseUrl={base_url}, model={model}, default={})",
            self.set_default
        );
        sentry_ext::breadcrumb(
            "connector",
            &format!("ollama→openclaw baseUrl={base_url} model={model}"),
            &[("connector", "ollama→openclaw"), ("model", model)],
        );

        let path = config_path()?;
        if !path.exists() {
            tracing::warn!("openclaw config not found, skipping ollama connector");
            return Ok(());
        }

        let mut patch = serde_json::json!({
            "models": {
                "providers": {
                    "ollama": {
                        "baseUrl": base_url,
                        "apiKey": "ollama-local",
                        "api": "ollama",
                        "models": [{
                            "id": model,
                            "name": model,
                            "input": ["text"],
                            "cost": {
                                "input": 0,
                                "output": 0,
                                "cacheRead": 0,
                                "cacheWrite": 0,
                            },
                        }],
                    }
                }
            }
        });

        if self.set_default {
            patch["agents"] = serde_json::json!({ "defaults": { "model": { "primary": format!("ollama/{model}") } } });
        }

        merge_and_validate(&path, &patch)?;
        tracing::info!("ollama→openclaw connected");
        Ok(())
    }
}
