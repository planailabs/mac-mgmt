use anyhow::Result;

use super::{Connector, ConnectorPhase};
use crate::sentry_ext;
use crate::services::hermes::{config_path, merge_and_validate};

/// Registers LM Studio as an LLM provider in Hermes.
pub struct LmsHermes {
    pub host: String,
    pub port: u16,
    pub default_model: String,
    pub set_default: bool,
}

impl Connector for LmsHermes {
    fn name(&self) -> &str {
        "lms→hermes"
    }

    fn phase(&self) -> ConnectorPhase {
        ConnectorPhase::PreStart
    }

    fn depends_on(&self) -> &[&str] {
        &["lms", "hermes"]
    }

    fn connect(
        &self,
        _configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let model = &self.default_model;
        let base_url = format!("http://{}:{}/v1", self.host, self.port);
        tracing::info!(
            "connecting lms to hermes (base_url={base_url}, model={model}, default={})",
            self.set_default
        );
        sentry_ext::breadcrumb(
            "connector",
            &format!("lms→hermes base_url={base_url} model={model}"),
            &[("connector", "lms→hermes"), ("model", model)],
        );

        let path = config_path()?;
        if !path.exists() {
            tracing::warn!("hermes config not found, skipping lms connector");
            return Ok(());
        }

        let mut patch = serde_json::json!({
            "model": {
                "provider": "lmstudio",
                "base_url": base_url,
            }
        });

        if self.set_default && !model.is_empty() {
            patch["model"]["default"] = serde_json::json!(model);
        }

        merge_and_validate(&path, &patch)?;
        tracing::info!("lms→hermes connected");
        Ok(())
    }
}
