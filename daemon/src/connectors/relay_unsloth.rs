use anyhow::Result;

use super::Connector;

/// Placeholder relay connector for Unsloth.
///
/// Unlike Ollama, Unsloth does not need CORS origins patched —
/// it accepts connections from any origin by default. This connector
/// exists so the relay tunnel is wired when both relay and unsloth
/// are enabled, keeping the pattern consistent with other services.
pub struct RelayUnsloth;

impl Connector for RelayUnsloth {
    fn name(&self) -> &str {
        "relay\u{2192}unsloth"
    }

    fn depends_on(&self) -> &[&str] {
        &["relay", "unsloth"]
    }

    fn connect(
        &self,
        _configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        tracing::debug!("relay\u{2192}unsloth: no extra config needed");
        Ok(())
    }
}
