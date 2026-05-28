use std::path::Path;

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
        if let Err(e) = ensure_agent_identity(&identity_dir, "openclaw") {
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

/// Generate the per-agent `AgentIdentity` (private key + node-signed
/// `AgentAttestation` + metadata) the openclaw MCP server expects to find
/// at `MEMVAULT_IDENTITY_DIR`. Skips if the directory already holds a
/// valid identity, so re-running the connector is safe.
///
/// Uses the daemon's libp2p host key (the node signing key) to sign the
/// attestation — same key the rest of the daemon uses for sigchain writes.
/// `cluster_id` is best-effort: if the store has it (post-genesis or
/// post-join) we use it, otherwise we fall back to the `<data_dir>/cluster_id`
/// sidecar, and finally the all-zero "pre-genesis" sentinel. The cluster
/// field is recorded in `agent.json` for audit; trust still flows through
/// the node attestation, so a zero cluster_id here doesn't break anything.
fn ensure_agent_identity(identity_dir: &Path, agent_id: &str) -> Result<()> {
    use memvault_api::agent_identity::AgentIdentity;
    use memvault_auth::Role;
    use memvault_core::ClusterId;

    if AgentIdentity::exists(identity_dir) {
        return Ok(());
    }

    let data_dir = dirs::data_local_dir()
        .context("data_local_dir is not set on this platform")?
        .join("memvault");

    // Same key the daemon uses for everything else — derived from
    // ~/.config/.../host_ed25519_key.
    let host_key = crate::host_keys::load_or_generate()
        .context("load daemon host key")?;
    let node_sk = crate::p2p::identity::ed25519_dalek_signing_key_from_russh(&host_key)
        .context("convert host key to ed25519_dalek signing key")?;

    let cluster_bytes = resolve_cluster_id_for_identity(&data_dir);
    let mut cluster_arr = [0u8; 32];
    if cluster_bytes.len() == 32 {
        cluster_arr.copy_from_slice(&cluster_bytes);
    }
    let cluster_id = ClusterId(cluster_arr);

    // 1-year attestation TTL — same default the web UI's `_ui` agent
    // uses (see init_ui_agent). Renewal is out of scope for this
    // connector; if it expires the operator re-runs `memctl
    // agent-enroll` or deletes the dir to regenerate.
    let ttl_ns: u64 = 365 * 24 * 60 * 60 * 1_000_000_000;
    AgentIdentity::generate_local(
        identity_dir,
        agent_id,
        &cluster_id,
        &node_sk,
        Role::AgentHost,
        ttl_ns,
    )
    .map_err(|e| anyhow::anyhow!("generate openclaw agent identity: {e}"))?;
    tracing::info!(
        agent_id,
        identity_dir = %identity_dir.display(),
        "bootstrapped openclaw agent identity"
    );
    Ok(())
}

/// Best-effort cluster_id resolution that doesn't require the daemon's
/// `MemvaultStore` handle. Tries the store first (single source of truth)
/// and falls back to the legacy `<data_dir>/cluster_id` sidecar.
fn resolve_cluster_id_for_identity(data_dir: &Path) -> Vec<u8> {
    let db_path = data_dir.join("blocks.redb");
    if db_path.exists() {
        if let Ok(store) = memvault_store::MemvaultStore::open(&db_path) {
            if let Ok(Some(cid)) = store.get_local_cluster_id() {
                return cid;
            }
        }
    }
    let id_path = data_dir.join("cluster_id");
    if let Ok(hex_str) = std::fs::read_to_string(&id_path) {
        if let Ok(bytes) = hex::decode(hex_str.trim()) {
            return bytes;
        }
    }
    vec![0u8; 32]
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
