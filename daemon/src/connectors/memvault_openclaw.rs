use anyhow::Result;

use super::{Connector, ConnectorPhase};
use crate::sentry_ext;
use crate::services::openclaw::{config_path, merge_and_validate};

/// Registers the memvault-memory plugin in OpenClaw.
///
/// Patches `~/.openclaw/openclaw.json` with a `plugins.entries.memvault-memory`
/// entry so OpenClaw can use memvault for persistent agent memory.
pub struct MemvaultOpenClaw {
    pub port: u16,
}

impl Connector for MemvaultOpenClaw {
    fn name(&self) -> &str {
        "memvault→openclaw"
    }

    fn phase(&self) -> ConnectorPhase {
        ConnectorPhase::PreStart
    }

    fn depends_on(&self) -> &[&str] {
        &["memvault", "openclaw"]
    }

    fn connect(
        &self,
        _configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let path = config_path()?;
        if !path.exists() {
            tracing::warn!("openclaw config not found, skipping memvault connector");
            return Ok(());
        }

        tracing::info!("connecting memvault plugin to openclaw (port={})", self.port);
        sentry_ext::breadcrumb(
            "connector",
            &format!("memvault→openclaw port={}", self.port),
            &[("connector", "memvault→openclaw")],
        );

        let patch = serde_json::json!({
            "plugins": {
                "entries": {
                    "memvault-memory": {
                        "enabled": true,
                        "config": {
                            "apiUrl": format!("http://127.0.0.1:{}", self.port),
                            "autoCapture": true,
                            "autoRecall": true,
                            "maxRecallResults": 5,
                            "defaultVisibility": "internal",
                            "defaultTags": ["agent:openclaw"],
                        },
                    }
                }
            }
        });

        merge_and_validate(&path, &patch)?;
        tracing::info!("memvault→openclaw connected");
        Ok(())
    }
}
