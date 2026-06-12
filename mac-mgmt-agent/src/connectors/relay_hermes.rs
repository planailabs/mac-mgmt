use anyhow::Result;

use super::Connector;

/// Relay connector for Hermes.
///
/// Unlike OpenClaw, Hermes doesn't have an allowedOrigins config.
/// This connector logs relay availability for observability.
pub struct RelayHermes;

impl Connector for RelayHermes {
    fn name(&self) -> &str {
        "relay→hermes"
    }

    fn depends_on(&self) -> &[&str] {
        &["relay", "hermes"]
    }

    fn connect(
        &self,
        configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let Some(relay_meta) = configs.get("relay") else {
            tracing::warn!("relay virtual service not found, skipping");
            return Ok(());
        };

        let proxy_hostname = relay_meta
            .get("proxy_hostname")
            .and_then(|v| v.as_str())
            .unwrap_or("<unknown>");
        let instance_prefix = relay_meta
            .get("instance_id_prefix")
            .and_then(|v| v.as_str())
            .unwrap_or("<unknown>");

        tracing::info!(
            "relay→hermes: tunnel available at {instance_prefix}-hermes.{proxy_hostname}"
        );

        Ok(())
    }
}
