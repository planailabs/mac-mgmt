use anyhow::Result;

use super::{Connector, ConnectorPhase};
use crate::sentry_ext;
use crate::services::hermes::{config_path, merge_and_validate};

/// Registers Unsloth as an LLM provider in Hermes.
pub struct UnslothHermes {
    pub host: String,
    pub port: u16,
    pub default_model: String,
    pub set_default: bool,
}

impl Connector for UnslothHermes {
    fn name(&self) -> &str {
        "unsloth→hermes"
    }

    fn phase(&self) -> ConnectorPhase {
        ConnectorPhase::PreStart
    }

    fn depends_on(&self) -> &[&str] {
        &["unsloth", "hermes"]
    }

    fn connect(
        &self,
        _configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let model = &self.default_model;
        let base_url = format!("http://{}:{}/v1", self.host, self.port);
        tracing::info!(
            "connecting unsloth to hermes (base_url={base_url}, model={model}, default={})",
            self.set_default
        );
        sentry_ext::breadcrumb(
            "connector",
            &format!("unsloth→hermes base_url={base_url} model={model}"),
            &[("connector", "unsloth→hermes"), ("model", model)],
        );

        let path = config_path()?;
        if !path.exists() {
            tracing::warn!("hermes config not found, skipping unsloth connector");
            return Ok(());
        }

        let mut patch = serde_json::json!({
            "model": {
                "provider": "custom",
                "base_url": base_url,
            }
        });

        if self.set_default && !model.is_empty() {
            patch["model"]["default"] = serde_json::json!(model);
        }

        merge_and_validate(&path, &patch)?;
        tracing::info!("unsloth→hermes connected");
        Ok(())
    }
}
