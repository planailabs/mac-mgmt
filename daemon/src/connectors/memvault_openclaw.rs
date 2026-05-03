use anyhow::{Context, Result};

use super::{Connector, ConnectorPhase};
use crate::sentry_ext;
use crate::services::openclaw::{config_path, merge_and_validate};

#[derive(rust_embed::Embed)]
#[folder = "../extensions/memvault-memory/"]
struct MemvaultExtension;

/// Registers the memvault-memory plugin in OpenClaw.
///
/// Materializes the embedded extension files to
/// `~/.openclaw/extensions/memvault-memory/`, then patches
/// `~/.openclaw/openclaw.json` with a `plugins.entries.memvault-memory`
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
            if let Some(arr) = existing.pointer("/plugins/load/paths").and_then(|v| v.as_array()) {
                load_paths = arr.iter().filter_map(|v| v.as_str().map(String::from)).collect();
            }
        }
        if !load_paths.iter().any(|p| p == &ext_dir_str) {
            load_paths.push(ext_dir_str);
        }

        // Register the memvault MCP server so all tools are available.
        let bin = std::env::current_exe()
            .context("failed to resolve current binary path")?;
        let bin_str = bin.to_string_lossy().to_string();

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
        tracing::info!("memvault→openclaw connected");
        Ok(())
    }
}