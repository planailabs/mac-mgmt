use anyhow::{Context, Result};
use mac_mgmt_common::McpServerEntry;
use std::collections::HashMap;

pub async fn sync_mcp_servers(server_url: &str, token: &str) -> Result<()> {
    tracing::info!("syncing MCP server configs");

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{server_url}/api/mcp-servers"))
        .bearer_auth(token)
        .send()
        .await
        .context("failed to reach MCP servers API")?;

    let status = resp.status();
    if !status.is_success() {
        anyhow::bail!("MCP servers API returned {status}");
    }

    let servers: HashMap<String, McpServerEntry> = resp
        .json()
        .await
        .context("failed to parse MCP servers response")?;

    sync_mcporter_config(&servers)?;

    // Nix package sync is now handled by crate::packages::sync_packages().
    // Legacy MCP-only nix sync removed — the unified package manager
    // aggregates MCP, skill, and manual packages server-side.

    tracing::info!("MCP server sync complete ({} servers)", servers.len());
    Ok(())
}

fn sync_mcporter_config(servers: &HashMap<String, McpServerEntry>) -> Result<()> {
    let config_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/root"))
        .join(".mcporter");
    let config_path = config_dir.join("plan-ai.json");

    // Ensure directory exists
    std::fs::create_dir_all(&config_dir)
        .with_context(|| format!("failed to create {}", config_dir.display()))?;

    // Build config with plain slug keys — this file is fully managed by us
    let mut mcp_servers = serde_json::Map::new();
    for (slug, entry) in servers {
        tracing::info!("setting MCP server: {slug}");
        mcp_servers.insert(slug.clone(), entry.config.clone());
    }

    let config = serde_json::json!({ "mcpServers": mcp_servers });
    let output =
        serde_json::to_string_pretty(&config).context("failed to serialize mcporter config")?;
    std::fs::write(&config_path, &output)
        .with_context(|| format!("failed to write {}", config_path.display()))?;

    Ok(())
}
