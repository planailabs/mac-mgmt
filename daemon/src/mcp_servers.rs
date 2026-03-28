use anyhow::{Context, Result};
use std::collections::HashMap;

const NAMESPACE: &str = "plan-ai/";

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

    let servers: HashMap<String, serde_json::Value> = resp
        .json()
        .await
        .context("failed to parse MCP servers response")?;

    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let config_dir = std::path::PathBuf::from(&home).join(".mcporter");
    let config_path = config_dir.join("mcporter.json");

    // Ensure directory exists
    std::fs::create_dir_all(&config_dir)
        .with_context(|| format!("failed to create {}", config_dir.display()))?;

    // Read existing config or create default
    let mut config: serde_json::Value = if config_path.exists() {
        let contents = std::fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read {}", config_path.display()))?;
        serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse {}", config_path.display()))?
    } else {
        serde_json::json!({ "mcpServers": {} })
    };

    // Ensure mcpServers object exists
    let mcp_servers = config
        .as_object_mut()
        .context("mcporter config is not an object")?
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));

    let mcp_obj = mcp_servers
        .as_object_mut()
        .context("mcpServers is not an object")?;

    // Remove plan-ai/ keys that aren't in the new set
    let old_keys: Vec<String> = mcp_obj
        .keys()
        .filter(|k| k.starts_with(NAMESPACE))
        .cloned()
        .collect();

    for key in &old_keys {
        let slug = &key[NAMESPACE.len()..];
        if !servers.contains_key(slug) {
            tracing::info!("removing MCP server: {key}");
            mcp_obj.remove(key);
        }
    }

    // Upsert plan-ai/{slug} entries
    for (slug, config_json) in &servers {
        let key = format!("{NAMESPACE}{slug}");
        tracing::info!("setting MCP server: {key}");
        mcp_obj.insert(key, config_json.clone());
    }

    // Write back pretty-printed
    let output = serde_json::to_string_pretty(&config)
        .context("failed to serialize mcporter config")?;
    std::fs::write(&config_path, &output)
        .with_context(|| format!("failed to write {}", config_path.display()))?;

    tracing::info!("MCP server config synced ({} servers)", servers.len());
    Ok(())
}
