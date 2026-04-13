use anyhow::{Context, Result};
use std::process::Command;

use super::Connector;
use crate::sentry_ext;

/// Registers Ollama as the LLM backend for OpenClaw.
///
/// Runs `ollama launch --yes --config --model <model> openclaw` which
/// configures the Ollama↔OpenClaw integration on the Ollama side.
pub struct OllamaOpenClaw {
    pub default_model: String,
}

impl Connector for OllamaOpenClaw {
    fn name(&self) -> &str {
        "ollama→openclaw"
    }

    fn depends_on(&self) -> &[&str] {
        &["ollama", "openclaw"]
    }

    fn connect(&self, _configs: &std::collections::HashMap<String, serde_json::Value>) -> Result<()> {
        let model = &self.default_model;
        tracing::info!("connecting ollama to openclaw with model {model}");
        sentry_ext::breadcrumb(
            "connector",
            &format!("ollama launch --model {model} openclaw"),
            &[("connector", "ollama→openclaw"), ("model", model)],
        );

        let output = crate::cmd::output_with_timeout(
            Command::new("ollama").args(["launch", "--yes", "--config", "--model", model, "openclaw"]),
            crate::cmd::DEFAULT_TIMEOUT,
        ).with_context(|| format!("failed to run ollama launch --model {model}"))?;

        if output.status.success() {
            tracing::info!("ollama→openclaw connected successfully");
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("ollama launch exited with {}: {}", output.status, stderr.trim());
            sentry_ext::capture_cmd_failure(
                &format!("ollama launch --model {model} openclaw"),
                output.status.code(),
                stderr.trim(),
            );
        }

        Ok(())
    }
}
