use anyhow::Result;

use super::Connector;
use crate::services::openclaw::{config_path, merge_and_validate};

/// Adds the relay tunnel origin to OpenClaw's gateway.controlUi.allowedOrigins.
///
/// Depends on the "relay" virtual service (set when the relay proxy hostname
/// is received) and "openclaw" (the gateway must be running).
pub struct RelayOpenClaw;

impl Connector for RelayOpenClaw {
    fn name(&self) -> &str {
        "relay→openclaw"
    }

    fn depends_on(&self) -> &[&str] {
        &["relay", "openclaw"]
    }

    fn connect(&self, virtual_services: &std::collections::HashMap<String, serde_json::Value>) -> Result<()> {
        let Some(relay_meta) = virtual_services.get("relay") else {
            tracing::warn!("relay virtual service not found, skipping");
            return Ok(());
        };

        let Some(proxy_hostname) = relay_meta.get("proxy_hostname").and_then(|v| v.as_str()) else {
            tracing::warn!("relay virtual service has no proxy_hostname, skipping");
            return Ok(());
        };

        let path = config_path()?;
        if !path.exists() {
            tracing::warn!("openclaw config not found, skipping relay connector");
            return Ok(());
        }

        // Read current config to check if origin is already present
        let current = std::fs::read_to_string(&path).unwrap_or_default();
        let current_json: serde_json::Value =
            serde_json::from_str(&current).unwrap_or_default();

        let wildcard_origin = format!("*.{proxy_hostname}");

        let already_has = current_json
            .pointer("/gateway/controlUi/allowedOrigins")
            .and_then(|v| v.as_array())
            .is_some_and(|arr| arr.iter().any(|v| v.as_str() == Some(&wildcard_origin)));

        if already_has {
            tracing::debug!("relay→openclaw: {wildcard_origin} already in allowedOrigins");
            return Ok(());
        }

        // Build the patch. We want to append to the array, not overwrite.
        // Read existing origins, add ours, then write the full list.
        let mut origins: Vec<String> = current_json
            .pointer("/gateway/controlUi/allowedOrigins")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        origins.push(wildcard_origin.clone());

        let patch = serde_json::json!({
            "gateway": {
                "controlUi": {
                    "allowedOrigins": origins,
                }
            }
        });

        merge_and_validate(&path, &patch)?;
        tracing::info!("relay→openclaw: added {wildcard_origin} to allowedOrigins");
        Ok(())
    }
}
