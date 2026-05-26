//! Unified nix package sync: aggregates packages from MCP servers, skills,
//! and manual cluster additions via the `GET /api/packages` endpoint.
//! Replaces the MCP-specific nix package sync that was previously in
//! `mcp_servers.rs`.

use anyhow::{Context, Result};
use mac_mgmt_common::{PackageSource, PackageSyncResponse};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use tokio::sync::Mutex;

use crate::sentry_ext;

/// Serializes the read-modify-write cycle of the nix state file so
/// concurrent `sync_packages` calls cannot lose updates.
fn nix_state_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn state_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/root"))
        .join(".config/mac-mgmt/nix-packages.json")
}

/// Legacy state file from the MCP-only sync era.
fn legacy_state_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/root"))
        .join(".config/mac-mgmt/mcp-nix-packages.json")
}

/// Persisted state: which packages are installed and their sources.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct PackageState {
    packages: HashMap<String, Vec<PackageSource>>,
}

fn read_state() -> PackageState {
    let path = state_path();
    if let Ok(contents) = std::fs::read_to_string(&path) {
        if let Ok(state) = serde_json::from_str::<PackageState>(&contents) {
            return state;
        }
    }

    // Migration: read legacy MCP nix state if new state doesn't exist
    let legacy = legacy_state_path();
    if let Ok(contents) = std::fs::read_to_string(&legacy) {
        if let Ok(pkgs) = serde_json::from_str::<Vec<String>>(&contents) {
            tracing::info!(
                "migrating {} packages from legacy mcp-nix-packages.json",
                pkgs.len()
            );
            let packages = pkgs
                .into_iter()
                .map(|p| {
                    (
                        p,
                        vec![PackageSource::McpServer {
                            slug: "legacy".into(),
                        }],
                    )
                })
                .collect();
            return PackageState { packages };
        }
    }

    PackageState::default()
}

fn write_state(state: &PackageState) -> Result<()> {
    let path = state_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(state).context("failed to serialize package state")?;
    std::fs::write(&path, json).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

/// Sync nix packages from the unified `/api/packages` endpoint.
/// Returns `Ok(true)` if the unified endpoint was used, `Ok(false)` if
/// the server returned 404 (old server without unified packages).
pub async fn sync_packages(server_url: &str, token: &str) -> Result<bool> {
    tracing::info!("syncing unified nix packages");

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{server_url}/api/packages"))
        .bearer_auth(token)
        .send()
        .await
        .context("failed to reach packages API")?;

    let status = resp.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        tracing::debug!("server does not support /api/packages (404), skipping");
        return Ok(false);
    }
    if !status.is_success() {
        anyhow::bail!("packages API returned {status}");
    }

    let sync_resp: PackageSyncResponse = resp
        .json()
        .await
        .context("failed to parse packages response")?;

    let _guard = nix_state_lock().lock().await;

    let desired: HashSet<String> = sync_resp.packages.keys().cloned().collect();
    let current_state = read_state();
    let current: HashSet<String> = current_state.packages.keys().cloned().collect();

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
            tracing::warn!("package {pkg} missing from profile, reinstalling");
            sentry_ext::breadcrumb(
                "pkg-sync",
                &format!("reinstalling missing {pkg}"),
                &[("package", pkg)],
            );
            if let Err(e) = crate::nix::profile_install(pkg, false) {
                tracing::warn!("failed to reinstall package {pkg}: {e}");
                sentry_ext::capture_error(
                    &format!("package reinstall failed: {pkg}: {e}"),
                    &[("package", pkg)],
                );
            }
        }
    }

    // Install new packages
    let to_install: Vec<&String> = desired.difference(&current).collect();
    for pkg in &to_install {
        tracing::info!("installing package: {pkg}");
        sentry_ext::breadcrumb(
            "pkg-sync",
            &format!("installing {pkg}"),
            &[("package", pkg)],
        );
        if let Err(e) = crate::nix::profile_install(pkg, false) {
            tracing::warn!("failed to install package {pkg}: {e}");
            sentry_ext::capture_error(
                &format!("package install failed: {pkg}: {e}"),
                &[("package", pkg)],
            );
        }
    }

    // Remove packages no longer needed by any source
    let to_remove: Vec<&String> = current.difference(&desired).collect();
    for pkg in &to_remove {
        tracing::info!("removing package: {pkg}");
        sentry_ext::breadcrumb("pkg-sync", &format!("removing {pkg}"), &[("package", pkg)]);
        if let Err(e) = crate::nix::profile_remove(pkg) {
            tracing::warn!("failed to remove package {pkg}: {e}");
            sentry_ext::capture_error(
                &format!("package remove failed: {pkg}: {e}"),
                &[("package", pkg)],
            );
        }
    }

    // Check for upgrades on existing packages
    let existing: Vec<&String> = desired.intersection(&current).collect();
    if !existing.is_empty() {
        let existing_strs: Vec<&str> = existing.iter().map(|s| s.as_str()).collect();
        let upgradable: HashSet<String> = match crate::nix::packages_with_upgrades(&existing_strs) {
            Ok(pkgs) => pkgs.into_iter().collect(),
            Err(e) => {
                tracing::warn!("failed to check package upgrades: {e}");
                HashSet::new()
            }
        };
        let mut upgraded = 0usize;
        for pkg in &existing {
            if upgradable.contains(*pkg) {
                tracing::info!("upgrading package: {pkg}");
                sentry_ext::breadcrumb(
                    "pkg-sync",
                    &format!("upgrading {pkg}"),
                    &[("package", pkg)],
                );
                if let Err(e) = crate::nix::profile_install(pkg, true) {
                    tracing::warn!("failed to upgrade package {pkg}: {e}");
                    sentry_ext::capture_error(
                        &format!("package upgrade failed: {pkg}: {e}"),
                        &[("package", pkg)],
                    );
                } else {
                    upgraded += 1;
                }
            }
        }
        if upgraded > 0 {
            tracing::info!("upgraded {upgraded} packages");
        }
    }

    // Persist new state with full source info
    let new_state = PackageState {
        packages: sync_resp.packages,
    };
    if let Err(e) = write_state(&new_state) {
        tracing::warn!("failed to write package state: {e}");
    }

    if !to_install.is_empty() || !to_remove.is_empty() {
        tracing::info!(
            "packages synced: +{} -{} (total {})",
            to_install.len(),
            to_remove.len(),
            desired.len(),
        );
    }

    Ok(true)
}
