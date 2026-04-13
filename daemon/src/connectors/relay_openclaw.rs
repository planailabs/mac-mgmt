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

    fn connect(&self, configs: &std::collections::HashMap<String, serde_json::Value>) -> Result<()> {
        let Some(relay_meta) = configs.get("relay") else {
            tracing::warn!("relay virtual service not found, skipping");
            return Ok(());
        };

        let Some(proxy_hostname) = relay_meta.get("proxy_hostname").and_then(|v| v.as_str()) else {
            tracing::warn!("relay virtual service has no proxy_hostname, skipping");
            return Ok(());
        };
        let Some(instance_prefix) = relay_meta.get("instance_id_prefix").and_then(|v| v.as_str()) else {
            tracing::warn!("relay virtual service has no instance_id_prefix, skipping");
            return Ok(());
        };

        let path = config_path()?;
        if !path.exists() {
            tracing::warn!("openclaw config not found, skipping relay connector");
            return Ok(());
        }

        // Build the exact origin: http(s)://{prefix}-openclaw.{proxy_hostname}
        // Use both http and https variants since we don't know the scheme.
        let origin = format!("{instance_prefix}-openclaw.{proxy_hostname}");

        let port = configs
            .get("openclaw")
            .and_then(|v| v.pointer("/gateway/port"))
            .and_then(|v| v.as_u64())
            .unwrap_or(18789);

        let origins_to_add = vec![
            format!("http://{origin}"),
            format!("https://{origin}"),
            format!("http://127.0.0.1:{port}"),
            format!("http://[::1]:{port}"),
            format!("http://localhost:{port}"),
        ];

        // Read current config
        let current = std::fs::read_to_string(&path).unwrap_or_default();
        let current_json: serde_json::Value =
            serde_json::from_str(&current).unwrap_or_default();

        let mut existing_origins: Vec<String> = current_json
            .pointer("/gateway/controlUi/allowedOrigins")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        let mut added = false;
        for o in &origins_to_add {
            if !existing_origins.contains(o) {
                existing_origins.push(o.clone());
                added = true;
            }
        }

        if !added {
            tracing::debug!("relay→openclaw: origins already present");
            return Ok(());
        }

        let patch = serde_json::json!({
            "gateway": {
                "controlUi": {
                    "allowedOrigins": existing_origins,
                }
            }
        });

        merge_and_validate(&path, &patch)?;
        tracing::info!("relay→openclaw: added {} to allowedOrigins", origins_to_add.join(", "));
        Ok(())
    }
}
