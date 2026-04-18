use anyhow::Result;

use super::Connector;
use crate::services::opencode::{config_path, merge_and_write};

/// Adds relay CORS origins to OpenCode's server config.
pub struct RelayOpencode;

impl Connector for RelayOpencode {
    fn name(&self) -> &str {
        "relay→opencode"
    }

    fn depends_on(&self) -> &[&str] {
        &["relay", "opencode"]
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
            tracing::warn!("opencode config not found, skipping relay connector");
            return Ok(());
        }

        let origin = format!("{instance_prefix}-opencode.{proxy_hostname}");

        let origins_to_add = vec![
            format!("http://{origin}"),
            format!("https://{origin}"),
        ];

        // Read current config to check existing CORS origins
        let current = std::fs::read_to_string(&path).unwrap_or_default();
        let current_json: serde_json::Value =
            serde_json::from_str(&current).unwrap_or_default();

        let mut existing_cors: Vec<String> = current_json
            .pointer("/server/cors")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        let mut added = false;
        for o in &origins_to_add {
            if !existing_cors.contains(o) {
                existing_cors.push(o.clone());
                added = true;
            }
        }

        if !added {
            tracing::debug!("relay→opencode: CORS origins already present");
            return Ok(());
        }

        let patch = serde_json::json!({
            "server": {
                "cors": existing_cors,
            }
        });

        merge_and_write(&path, &patch)?;
        tracing::info!("relay→opencode: added {} to CORS origins", origins_to_add.join(", "));
        Ok(())
    }
}
