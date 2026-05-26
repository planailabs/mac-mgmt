use anyhow::{Context, Result};

use super::{Connector, ConnectorPhase};
use crate::sentry_ext;
use crate::services::hermes::{config_path, merge_and_validate};

// ── Setup connector (memvault enabled) ─────────────────────────────────

/// Registers the memvault MCP server in Hermes config.yaml.
///
/// Patches `~/.hermes/config.yaml` with an `mcp_servers.plan-ai-memvault`
/// entry pointing at the local memvault API.
pub struct MemvaultHermes {
    pub port: u16,
}

impl Connector for MemvaultHermes {
    fn name(&self) -> &str {
        "memvault→hermes"
    }

    fn phase(&self) -> ConnectorPhase {
        ConnectorPhase::PreStart
    }

    fn depends_on(&self) -> &[&str] {
        &["memvault", "hermes"]
    }

    fn connect(
        &self,
        _configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let path = config_path()?;
        if !path.exists() {
            tracing::warn!("hermes config not found, skipping memvault connector");
            return Ok(());
        }

        tracing::info!(
            "connecting memvault MCP server to hermes (port={})",
            self.port
        );
        sentry_ext::breadcrumb(
            "connector",
            &format!("memvault→hermes port={}", self.port),
            &[("connector", "memvault→hermes")],
        );

        let bin = std::env::current_exe().context("failed to resolve current binary path")?;
        let bin_str = bin.to_string_lossy().to_string();

        let identity_dir = dirs::data_local_dir()
            .context("HOME not set")?
            .join("memvault")
            .join("agents")
            .join("hermes");
        let identity_dir_str = identity_dir.to_string_lossy().to_string();

        let patch = serde_json::json!({
            "mcp_servers": {
                "plan-ai-memvault": {
                    "command": bin_str,
                    "args": ["mcp-memvault"],
                    "env": {
                        "MEMVAULT_URL": format!("http://127.0.0.1:{}", self.port),
                        "MEMVAULT_DEFAULT_TAGS": "agent:hermes",
                        "MEMVAULT_DEFAULT_VISIBILITY": "internal",
                        "MEMVAULT_AGENT_ID": "hermes",
                        "MEMVAULT_IDENTITY_DIR": identity_dir_str,
                    },
                }
            }
        });

        merge_and_validate(&path, &patch)?;
        tracing::info!("memvault→hermes connected");
        Ok(())
    }
}

// ── Teardown connector (memvault disabled) ─────────────────────────────

/// Removes the memvault MCP server from Hermes config when memvault is disabled.
pub struct MemvaultHermesCleanup;

impl Connector for MemvaultHermesCleanup {
    fn name(&self) -> &str {
        "memvault→hermes (cleanup)"
    }

    fn phase(&self) -> ConnectorPhase {
        ConnectorPhase::PreStart
    }

    fn depends_on(&self) -> &[&str] {
        &["hermes"]
    }

    fn connect(
        &self,
        _configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let path = config_path()?;
        if !path.exists() {
            return Ok(());
        }

        let validator = &crate::services::hermes::VALIDATOR;
        let config_raw = std::fs::read_to_string(&path)?;
        let mut config: serde_json::Value = validator
            .parse(&config_raw)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let changed = config
            .pointer_mut("/mcp_servers")
            .and_then(|v| v.as_object_mut())
            .and_then(|servers| servers.remove("plan-ai-memvault"))
            .is_some();

        if changed {
            tracing::info!("cleaning up memvault MCP server from hermes config");
            sentry_ext::breadcrumb(
                "connector",
                "memvault→hermes cleanup",
                &[("connector", "memvault→hermes (cleanup)")],
            );
            let yaml = validator
                .serialize(&config)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            std::fs::write(&path, yaml)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }

        Ok(())
    }
}
