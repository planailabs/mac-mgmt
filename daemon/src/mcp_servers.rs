use anyhow::{Context, Result};
use mac_mgmt_common::McpServerEntry;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use tokio::sync::Mutex;

use crate::sentry_ext;

/// Serializes the read-modify-write cycle of the MCP nix state file so
/// concurrent `sync_mcp_servers` calls cannot lose updates.
fn nix_state_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Path to the state file that tracks which nix packages were installed by MCP sync.
fn mcp_nix_state_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/root"))
        .join(".config/mac-mgmt/mcp-nix-packages.json")
}

fn read_nix_state() -> HashSet<String> {
    let path = mcp_nix_state_path();
    if let Ok(contents) = std::fs::read_to_string(&path) {
        if let Ok(pkgs) = serde_json::from_str::<Vec<String>>(&contents) {
            return pkgs.into_iter().collect();
        }
    }
    HashSet::new()
}

fn write_nix_state(pkgs: &HashSet<String>) -> Result<()> {
    let path = mcp_nix_state_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let sorted: Vec<&String> = {
        let mut v: Vec<_> = pkgs.iter().collect();
        v.sort();
        v
    };
    let json =
        serde_json::to_string_pretty(&sorted).context("failed to serialize MCP nix state")?;
    std::fs::write(&path, json).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

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
    {
        // Hold the lock for the entire read-modify-write of the nix state
        // file so concurrent syncs can't lose updates.
        let _guard = nix_state_lock().lock().await;
        sync_nix_packages(&servers);
    }

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

fn sync_nix_packages(servers: &HashMap<String, McpServerEntry>) {
    // Collect desired set of nix packages across all servers
    let desired: HashSet<String> = servers
        .values()
        .flat_map(|entry| entry.nix_packages.iter().cloned())
        .collect();

    let current = read_nix_state();

    // Verify desired packages that the state file claims are installed
    // actually exist in the nix profile — reinstall any that are missing.
    let installed: HashSet<String> = match crate::nix::installed_elements() {
        Ok(elems) => elems.into_iter().collect(),
        Err(e) => {
            tracing::warn!(
                "failed to list installed nix elements, skipping profile verification: {e}"
            );
            HashSet::new()
        }
    };

    if !installed.is_empty() {
        let missing: Vec<&String> = desired
            .intersection(&current)
            .filter(|pkg| !installed.contains(*pkg))
            .collect();
        for pkg in &missing {
            tracing::warn!("MCP nix package {pkg} missing from profile, reinstalling");
            sentry_ext::breadcrumb(
                "mcp-nix",
                &format!("reinstalling missing {pkg}"),
                &[("package", pkg)],
            );
            if let Err(e) = crate::nix::profile_install(pkg, false) {
                tracing::warn!("failed to reinstall missing MCP nix dependency {pkg}: {e}");
                sentry_ext::capture_error(
                    &format!("MCP nix reinstall failed: {pkg}: {e}"),
                    &[("package", pkg)],
                );
            }
        }
    }

    // Install new packages
    let to_install: Vec<&String> = desired.difference(&current).collect();
    for pkg in &to_install {
        tracing::info!("installing MCP nix dependency: {pkg}");
        sentry_ext::breadcrumb("mcp-nix", &format!("installing {pkg}"), &[("package", pkg)]);
        if let Err(e) = crate::nix::profile_install(pkg, false) {
            tracing::warn!("failed to install MCP nix dependency {pkg}: {e}");
            sentry_ext::capture_error(
                &format!("MCP nix install failed: {pkg}: {e}"),
                &[("package", pkg)],
            );
        }
    }

    // Remove packages no longer needed
    let to_remove: Vec<&String> = current.difference(&desired).collect();
    for pkg in &to_remove {
        tracing::info!("removing MCP nix dependency: {pkg}");
        sentry_ext::breadcrumb("mcp-nix", &format!("removing {pkg}"), &[("package", pkg)]);
        if let Err(e) = crate::nix::profile_remove(pkg) {
            tracing::warn!("failed to remove MCP nix dependency {pkg}: {e}");
            sentry_ext::capture_error(
                &format!("MCP nix remove failed: {pkg}: {e}"),
                &[("package", pkg)],
            );
        }
    }

    // Check for updates on existing packages
    let existing: Vec<&String> = desired.intersection(&current).collect();
    if !existing.is_empty() {
        let existing_strs: Vec<&str> = existing.iter().map(|s| s.as_str()).collect();
        let upgradable: HashSet<String> = match crate::nix::packages_with_upgrades(&existing_strs) {
            Ok(pkgs) => pkgs.into_iter().collect(),
            Err(e) => {
                tracing::warn!("failed to check MCP nix upgrades: {e}");
                HashSet::new()
            }
        };
        let mut upgraded = 0usize;
        for pkg in &existing {
            if upgradable.contains(*pkg) {
                tracing::info!("upgrading MCP nix dependency: {pkg}");
                sentry_ext::breadcrumb("mcp-nix", &format!("upgrading {pkg}"), &[("package", pkg)]);
                if let Err(e) = crate::nix::profile_install(pkg, true) {
                    tracing::warn!("failed to upgrade MCP nix dependency {pkg}: {e}");
                    sentry_ext::capture_error(
                        &format!("MCP nix upgrade failed: {pkg}: {e}"),
                        &[("package", pkg)],
                    );
                } else {
                    upgraded += 1;
                }
            }
        }
        if upgraded > 0 {
            tracing::info!("upgraded {upgraded} MCP nix packages");
        }
    }

    // Persist the new desired set
    if let Err(e) = write_nix_state(&desired) {
        tracing::warn!("failed to write MCP nix state: {e}");
    }

    if !to_install.is_empty() || !to_remove.is_empty() {
        tracing::info!(
            "MCP nix packages synced: +{} -{} (total {})",
            to_install.len(),
            to_remove.len(),
            desired.len(),
        );
    }
}
