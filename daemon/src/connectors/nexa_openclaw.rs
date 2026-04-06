use anyhow::{Context, Result};

use super::Connector;
use crate::sentry_ext;

/// Configures OpenClaw to use Nexa as its LLM backend.
///
/// Nexa exposes an OpenAI-compatible API at `http://host:port/v1`.
/// This connector writes the LLM backend configuration into
/// `~/.openclaw/openclaw.json` so OpenClaw discovers Nexa.
pub struct NexaOpenClaw {
    pub host: String,
    pub port: u16,
    pub default_model: String,
}

impl Connector for NexaOpenClaw {
    fn name(&self) -> &str {
        "nexa→openclaw"
    }

    fn connect(&self) -> Result<()> {
        let base_url = format!("http://{}:{}/v1", self.host, self.port);
        tracing::info!(
            "connecting nexa to openclaw (baseUrl={base_url}, model={})",
            self.default_model
        );
        sentry_ext::breadcrumb(
            "connector",
            &format!("nexa→openclaw baseUrl={base_url}"),
            &[
                ("connector", "nexa→openclaw"),
                ("base_url", &base_url),
                ("model", &self.default_model),
            ],
        );

        let config_path = dirs::home_dir()
            .context("HOME not set")?
            .join(".openclaw/openclaw.json");

        if !config_path.exists() {
            tracing::warn!(
                "openclaw config not found at {}, skipping nexa connector",
                config_path.display()
            );
            return Ok(());
        }

        let contents = std::fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read {}", config_path.display()))?;
        let mut existing: serde_json::Value =
            serde_json::from_str(&contents).context("failed to parse openclaw.json")?;

        let patch = serde_json::json!({
            "llm": {
                "provider": "openai",
                "baseUrl": base_url,
                "model": self.default_model,
            }
        });
        super::merge_json(&mut existing, &patch);

        let merged =
            serde_json::to_string_pretty(&existing).context("failed to serialize config")?;
        std::fs::write(&config_path, &merged)
            .with_context(|| format!("failed to write {}", config_path.display()))?;

        tracing::info!("nexa→openclaw connected: LLM config written to openclaw.json");
        Ok(())
    }
}
