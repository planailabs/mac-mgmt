use anyhow::{Context, Result};

use super::{Connector, ConnectorPhase};
use crate::sentry_ext;
use crate::services::openclaw::{config_path, merge_and_validate};

#[derive(rust_embed::Embed)]
#[folder = "../extensions/memvault-memory/"]
struct MemvaultExtension;

// ── Setup connector (memvault enabled) ─────────────────────────────────

/// Registers the memvault-memory plugin and MCP server in OpenClaw.
///
/// Materializes the embedded extension files to
/// `~/.openclaw/extensions/memvault-memory/`, then patches
/// `~/.openclaw/openclaw.json` with plugin config, load path,
/// memory slot, and MCP server entry.
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

        tracing::info!(
            "connecting memvault plugin to openclaw (port={})",
            self.port
        );
        sentry_ext::breadcrumb(
            "connector",
            &format!("memvault→openclaw port={}", self.port),
            &[("connector", "memvault→openclaw")],
        );

        // Materialize the extension files.
        let ext_dir = dirs::home_dir()
            .context("HOME not set")?
            .join(".openclaw/extensions/memvault-memory");
        let written = crate::embed_write::materialize_all::<MemvaultExtension>(&ext_dir)?;
        if written > 0 {
            tracing::info!("installed memvault-memory openclaw extension ({written} files)");
        }

        // Build the load paths array: preserve any existing paths and append
        // the extension directory if not already present.
        let ext_dir_str = ext_dir.to_string_lossy().to_string();
        let mut load_paths: Vec<String> = Vec::new();
        let config_raw = std::fs::read_to_string(&path)?;
        if let Ok(existing) = serde_json::from_str::<serde_json::Value>(&config_raw) {
            if let Some(arr) = existing
                .pointer("/plugins/load/paths")
                .and_then(|v| v.as_array())
            {
                load_paths = arr
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
            }
        }
        if !load_paths.iter().any(|p| p == &ext_dir_str) {
            load_paths.push(ext_dir_str);
        }

        // Register the memvault MCP server so all tools are available.
        let bin = std::env::current_exe().context("failed to resolve current binary path")?;
        let bin_str = bin.to_string_lossy().to_string();

        // Compute agent identity directory for enrollment
        let identity_dir = dirs::data_local_dir()
            .context("HOME not set")?
            .join("memvault")
            .join("agents")
            .join("openclaw");
        let identity_dir_str = identity_dir.to_string_lossy().to_string();

        // Enable the plugin, register the load path, and add the MCP server.
        let patch = serde_json::json!({
            "mcp": {
                "servers": {
                    "plan-ai-memvault": {
                        "command": bin_str,
                        "args": ["mcp-memvault"],
                        "env": {
                            "MEMVAULT_URL": format!("http://127.0.0.1:{}", self.port),
                            "MEMVAULT_DEFAULT_TAGS": "agent:openclaw",
                            "MEMVAULT_DEFAULT_VISIBILITY": "internal",
                            "MEMVAULT_AGENT_ID": "openclaw",
                            "MEMVAULT_IDENTITY_DIR": identity_dir_str,
                        },
                    }
                }
            },
            "plugins": {
                "slots": {
                    "memory": "memvault-memory",
                },
                "load": {
                    "paths": load_paths,
                },
                "entries": {
                    "memvault-memory": {
                        "enabled": true,
                        "config": {
                            "apiUrl": format!("http://127.0.0.1:{}", self.port),
                            "autoRecall": true,
                            "maxRecallResults": 5,
                        },
                    }
                }
            }
        });

        merge_and_validate(&path, &patch)?;

        // Bootstrap the openclaw agent identity at MEMVAULT_IDENTITY_DIR.
        // Nothing else in the daemon does this — without it the MCP
        // server fails on its first connect with "failed to load agent
        // identity from …" because ClientArgs::connect() calls
        // AgentIdentity::load and there's no file on disk yet. Idempotent
        // via AgentIdentity::exists().
        if let Err(e) = super::ensure_agent_identity(&identity_dir, "openclaw") {
            tracing::warn!(
                error = %e,
                identity_dir = %identity_dir.display(),
                "could not bootstrap openclaw agent identity; \
                 the MCP server will fail until this is fixed manually"
            );
        }

        tracing::info!("memvault→openclaw connected");
        Ok(())
    }
}

// ── Teardown connector (memvault disabled) ─────────────────────────────

/// Removes memvault-memory plugin, MCP server, and load path from OpenClaw
/// config when memvault is disabled.
pub struct MemvaultOpenClawCleanup;

impl Connector for MemvaultOpenClawCleanup {
    fn name(&self) -> &str {
        "memvault→openclaw (cleanup)"
    }

    fn phase(&self) -> ConnectorPhase {
        ConnectorPhase::PreStart
    }

    fn depends_on(&self) -> &[&str] {
        &["openclaw"]
    }

    fn connect(
        &self,
        _configs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let path = config_path()?;
        if !path.exists() {
            return Ok(());
        }

        let config_raw = std::fs::read_to_string(&path)?;
        let mut config: serde_json::Value = serde_json::from_str(&config_raw)?;

        let mut changed = false;

        // Remove MCP server entry.
        if let Some(servers) = config
            .pointer_mut("/mcp/servers")
            .and_then(|v| v.as_object_mut())
        {
            if servers.remove("plan-ai-memvault").is_some() {
                changed = true;
            }
        }

        // Remove plugin entry.
        if let Some(entries) = config
            .pointer_mut("/plugins/entries")
            .and_then(|v| v.as_object_mut())
        {
            if entries.remove("memvault-memory").is_some() {
                changed = true;
            }
        }

        // Reset memory slot if it points to memvault-memory.
        if let Some(slot) = config
            .pointer("/plugins/slots/memory")
            .and_then(|v| v.as_str())
        {
            if slot == "memvault-memory" {
                if let Some(slots) = config
                    .pointer_mut("/plugins/slots")
                    .and_then(|v| v.as_object_mut())
                {
                    slots.remove("memory");
                    changed = true;
                }
            }
        }

        // Remove extension dir from load paths.
        let ext_dir = dirs::home_dir()
            .context("HOME not set")?
            .join(".openclaw/extensions/memvault-memory");
        let ext_dir_str = ext_dir.to_string_lossy().to_string();
        if let Some(paths) = config
            .pointer_mut("/plugins/load/paths")
            .and_then(|v| v.as_array_mut())
        {
            let before = paths.len();
            paths.retain(|p| p.as_str() != Some(&ext_dir_str));
            if paths.len() != before {
                changed = true;
            }
        }

        if changed {
            tracing::info!("cleaning up memvault entries from openclaw config");
            sentry_ext::breadcrumb(
                "connector",
                "memvault→openclaw cleanup",
                &[("connector", "memvault→openclaw (cleanup)")],
            );
            let json = serde_json::to_string_pretty(&config)?;
            std::fs::write(&path, json)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }

        Ok(())
    }
}
