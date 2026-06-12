use anyhow::Result;

use super::{Connector, ConnectorPhase};
use crate::sentry_ext;
use crate::services::openclaw::{config_path, merge_and_validate};

/// Registers LM Studio as an LLM provider in OpenClaw.
pub struct LmsOpenClaw {
    pub host: String,
    pub port: u16,
    pub default_model: String,
    pub set_default: bool,
}

impl Connector for LmsOpenClaw {
    fn name(&self) -> &str {
        "lms→openclaw"
    }

    fn phase(&self) -> ConnectorPhase {
        ConnectorPhase::PreStart
    }

    fn depends_on(&self) -> &[&str] {
        &["lms", "openclaw"]
    }

    fn connect(
        &self,
        _configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let base_url = format!("http://{}:{}/v1", self.host, self.port);
        tracing::info!(
            "connecting lms to openclaw (baseUrl={base_url}, model={}, default={})",
            self.default_model,
            self.set_default
        );
        sentry_ext::breadcrumb(
            "connector",
            &format!("lms→openclaw baseUrl={base_url}"),
            &[
                ("connector", "lms→openclaw"),
                ("base_url", &base_url),
                ("model", &self.default_model),
            ],
        );

        let path = config_path()?;
        if !path.exists() {
            tracing::warn!("openclaw config not found, skipping lms connector");
            return Ok(());
        }

        let model_id = &self.default_model;
        let mut patch = serde_json::json!({
            "models": {
                "providers": {
                    "lms": {
                        "baseUrl": base_url,
                        "api": "openai-completions",
                        "models": [{ "id": model_id, "name": model_id }],
                    }
                }
            }
        });

        if self.set_default {
            patch["agents"] = serde_json::json!({ "defaults": { "model": { "primary": format!("lms/{model_id}") } } });
        }

        merge_and_validate(&path, &patch)?;
        tracing::info!("lms→openclaw connected");
        Ok(())
    }
}
