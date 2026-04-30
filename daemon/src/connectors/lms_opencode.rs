use anyhow::Result;

use super::{Connector, ConnectorPhase};
use crate::sentry_ext;
use crate::services::opencode::{config_path, merge_and_validate};

/// Registers LM Studio as an LLM provider in OpenCode.
pub struct LmsOpencode {
    pub host: String,
    pub port: u16,
    pub default_model: String,
    pub set_default: bool,
}

impl Connector for LmsOpencode {
    fn name(&self) -> &str {
        "lms→opencode"
    }

    fn phase(&self) -> ConnectorPhase {
        ConnectorPhase::PreStart
    }

    fn depends_on(&self) -> &[&str] {
        &["lms", "opencode"]
    }

    fn connect(
        &self,
        _configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let base_url = format!("http://{}:{}/v1", self.host, self.port);
        tracing::info!("connecting lms to opencode (baseUrl={base_url}, model={}, default={})", self.default_model, self.set_default);
        sentry_ext::breadcrumb(
            "connector",
            &format!("lms→opencode baseUrl={base_url}"),
            &[("connector", "lms→opencode"), ("base_url", &base_url), ("model", &self.default_model)],
        );

        let path = config_path()?;
        if !path.exists() {
            tracing::warn!("opencode config not found, skipping lms connector");
            return Ok(());
        }

        let model_id = &self.default_model;
        let mut patch = serde_json::json!({
            "provider": {
                "lmstudio": {
                    "options": {
                        "baseURL": base_url,
                    }
                }
            }
        });
        if self.set_default {
            patch["model"] = serde_json::json!(format!("lmstudio/{model_id}"));
        }

        merge_and_validate(&path, &patch)?;
        tracing::info!("lms→opencode connected");
        Ok(())
    }
}
