use anyhow::Result;

use super::Connector;
use crate::sentry_ext;
use crate::services::opencode::{config_path, merge_and_write};

/// Configures OpenCode to use LM Studio as its LLM backend.
pub struct LmsOpencode {
    pub host: String,
    pub port: u16,
    pub default_model: String,
}

impl Connector for LmsOpencode {
    fn name(&self) -> &str {
        "lms→opencode"
    }

    fn depends_on(&self) -> &[&str] {
        &["lms", "opencode"]
    }

    fn connect(&self, _configs: &std::collections::HashMap<String, serde_json::Value>) -> Result<()> {
        let base_url = format!("http://{}:{}/v1", self.host, self.port);
        tracing::info!(
            "connecting lms to opencode (baseURL={base_url}, model={})",
            self.default_model
        );
        sentry_ext::breadcrumb(
            "connector",
            &format!("lms→opencode baseURL={base_url}"),
            &[("connector", "lms→opencode"), ("base_url", &base_url), ("model", &self.default_model)],
        );

        let path = config_path()?;
        if !path.exists() {
            tracing::warn!("opencode config not found, skipping lms connector");
            return Ok(());
        }

        let model_id = &self.default_model;
        let patch = serde_json::json!({
            "provider": {
                "lmstudio": {
                    "options": {
                        "baseURL": base_url,
                    }
                }
            },
            "model": format!("lmstudio/{model_id}"),
        });

        merge_and_write(&path, &patch)?;
        tracing::info!("lms→opencode connected");
        Ok(())
    }
}
