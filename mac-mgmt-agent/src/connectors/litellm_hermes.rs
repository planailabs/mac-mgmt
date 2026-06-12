use anyhow::Result;

use super::{Connector, ConnectorPhase};
use crate::sentry_ext;
use crate::services::hermes::{config_path, merge_and_validate};

/// Registers LiteLLM as a unified LLM provider in Hermes.
pub struct LitellmHermes {
    pub host: String,
    pub port: u16,
    pub set_default: bool,
}

impl Connector for LitellmHermes {
    fn name(&self) -> &str {
        "litellm→hermes"
    }

    fn phase(&self) -> ConnectorPhase {
        ConnectorPhase::PreStart
    }

    fn depends_on(&self) -> &[&str] {
        &["litellm", "hermes"]
    }

    fn connect(
        &self,
        _configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let base_url = format!("http://{}:{}/v1", self.host, self.port);
        tracing::info!(
            "connecting litellm to hermes (base_url={base_url}, default={})",
            self.set_default
        );
        sentry_ext::breadcrumb(
            "connector",
            &format!("litellm→hermes base_url={base_url}"),
            &[("connector", "litellm→hermes")],
        );

        let path = config_path()?;
        if !path.exists() {
            tracing::warn!("hermes config not found, skipping litellm connector");
            return Ok(());
        }

        let patch = serde_json::json!({
            "model": {
                "provider": "custom",
                "base_url": base_url,
            }
        });

        merge_and_validate(&path, &patch)?;
        tracing::info!("litellm→hermes connected");
        Ok(())
    }
}
