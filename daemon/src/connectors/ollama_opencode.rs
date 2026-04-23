use anyhow::Result;

use super::{Connector, ConnectorPhase};
use crate::sentry_ext;
use crate::services::opencode::{config_path, merge_and_write};

/// Registers Ollama as an LLM provider in OpenCode.
pub struct OllamaOpencode {
    pub default_model: String,
    pub set_default: bool,
}

impl Connector for OllamaOpencode {
    fn name(&self) -> &str {
        "ollama→opencode"
    }

    fn phase(&self) -> ConnectorPhase {
        ConnectorPhase::PreStart
    }

    fn depends_on(&self) -> &[&str] {
        &["ollama", "opencode"]
    }

    fn connect(
        &self,
        _configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let model = &self.default_model;
        tracing::info!("connecting ollama to opencode (model={model}, default={})", self.set_default);
        sentry_ext::breadcrumb(
            "connector",
            &format!("ollama→opencode model={model}"),
            &[("connector", "ollama→opencode"), ("model", model)],
        );

        let path = config_path()?;
        if !path.exists() {
            tracing::warn!("opencode config not found, skipping ollama connector");
            return Ok(());
        }

        let mut patch = serde_json::json!({
            "provider": {
                "ollama": {
                    "options": {
                        "baseURL": "http://127.0.0.1:11434",
                    }
                }
            }
        });
        if self.set_default {
            patch["model"] = serde_json::json!(format!("ollama/{model}"));
        }

        merge_and_write(&path, &patch)?;
        tracing::info!("ollama→opencode connected");
        Ok(())
    }
}
