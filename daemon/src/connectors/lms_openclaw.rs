use anyhow::Result;

use super::{Connector, ConnectorPhase};
use crate::sentry_ext;
use crate::services::openclaw::{config_path, merge_and_validate};

/// Configures OpenClaw to use LM Studio (lms) as its LLM backend.
///
/// LM Studio exposes an OpenAI-compatible API at `http://host:port/v1`.
pub struct LmsOpenClaw {
    pub host: String,
    pub port: u16,
    pub default_model: String,
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
            "connecting lms to openclaw (baseUrl={base_url}, model={})",
            self.default_model
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
        let patch = serde_json::json!({
            "models": {
                "providers": {
                    "lms": {
                        "baseUrl": base_url,
                        "api": "openai-completions",
                        "models": [{ "id": model_id, "name": model_id }],
                    }
                }
            },
            "agents": {
                "defaults": {
                    "model": {
                        "primary": format!("lms/{model_id}"),
                    }
                }
            }
        });

        merge_and_validate(&path, &patch)?;
        tracing::info!("lms→openclaw connected");
        Ok(())
    }
}
