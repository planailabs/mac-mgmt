use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

use crate::sentry_ext;

const UPDATE_BASE: &str = env!("UPDATE_BASE_URL");
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
#[allow(dead_code)]
const ENVIRONMENT: &str = env!("ENVIRONMENT");
#[allow(dead_code)]
const TARGET: &str = env!("TARGET");

/// Effective update base URL: runtime `MAC_MGMT_UPDATE_URL` overrides the
/// compile-time default. Used by the legacy CLI `apply` path.
fn update_base() -> String {
    std::env::var("MAC_MGMT_UPDATE_URL").unwrap_or_else(|_| UPDATE_BASE.to_string())
}

/// Target the server has assigned: a version, and the nix store path that
/// contains the binary for our system (resolved by the server from
/// `daemon_versions`). Set by the daemon after fetching `/api/update`.
#[derive(Clone, Default)]
struct Target {
    version: Option<String>,
    store_path: Option<String>,
}

static TARGET_STATE: std::sync::RwLock<Target> = std::sync::RwLock::new(Target {
    version: None,
    store_path: None,
});

pub fn set_target(version: String, store_path: Option<String>) {
    let mut t = TARGET_STATE.write().unwrap();
    t.version = Some(version);
    t.store_path = store_path;
}

/// Back-compat shim retained for callers that only know the version.
#[allow(dead_code)]
pub fn set_target_version(ver: String) {
    set_target(ver, None);
}

fn target() -> Target {
    TARGET_STATE.read().unwrap().clone()
}

#[allow(dead_code)]
pub fn current_version() -> &'static str {
    CURRENT_VERSION
}

/// File next to the current binary that records the last applied
/// store path. Used to detect "same version, different store path"
/// (e.g. someone re-uploaded the same version with a fix) so we
/// re-apply instead of skipping.
fn applied_marker_path() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let parent = exe.parent()?;
    Some(parent.join(".mac-mgmt.store_path"))
}

fn read_last_store_path() -> Option<String> {
    let p = applied_marker_path()?;
    std::fs::read_to_string(&p).ok().map(|s| s.trim().to_string())
}

fn write_last_store_path(store_path: &str) {
    if let Some(p) = applied_marker_path() {
        if let Err(e) = std::fs::write(&p, store_path) {
            tracing::warn!("failed to write applied marker {}: {e}", p.display());
        }
    }
}

/// Check if the server has assigned a target and apply it.
pub fn check_and_apply() {
    let t = target();
    let Some(version) = t.version else {
        tracing::debug!("no target version set, skipping self-update");
        return;
    };

    if version_cmp(&version) < 0 {
        tracing::warn!(
            "target version {version} is older than current {CURRENT_VERSION}, refusing downgrade"
        );
        return;
    }

    let Some(store_path) = t.store_path else {
        if version != CURRENT_VERSION {
            tracing::warn!(
                "target version {version} set but server did not provide a store path; \
                 update skipped (sync daemon versions on the server)"
            );
        } else {
            tracing::debug!("already at target version {CURRENT_VERSION}");
        }
        return;
    };

    // If we are already on the target version *and* the store path
    // matches what we last applied, nothing to do. If the store path
    // changed (re-upload of the same version), fall through and apply.
    if version == CURRENT_VERSION {
        match read_last_store_path() {
            Some(last) if last == store_path => {
                tracing::info!("already at target version {CURRENT_VERSION} ({store_path})");
                return;
            }
            Some(last) => {
                tracing::info!(
                    "version {version} unchanged but store path changed: {last} -> {store_path}, reapplying"
                );
            }
            None => {
                tracing::info!(
                    "version {version} unchanged, no applied marker found, applying {store_path}"
                );
            }
        }
    }

    tracing::info!("updating: {CURRENT_VERSION} -> {version} (store {store_path})");
    sentry_ext::breadcrumb("self-update", "updating", &[
        ("from", CURRENT_VERSION),
        ("to", &version),
        ("store_path", &store_path),
    ]);

    if let Err(e) = apply_store_path(&version, &store_path) {
        tracing::warn!("update to {version} failed: {e}");
        sentry_ext::capture_error(
            &format!("self-update to {version} failed: {e}"),
            &[("from", CURRENT_VERSION), ("to", &version)],
        );
    }
}

/// Compare a version string against CURRENT_VERSION.
/// Returns -1 if ver < current, 0 if equal, 1 if ver > current.
fn version_cmp(ver: &str) -> i32 {
    let parse = |s: &str| -> Vec<u64> {
        s.split('.').filter_map(|p| p.parse().ok()).collect()
    };
    let a = parse(ver);
    let b = parse(CURRENT_VERSION);
    a.cmp(&b) as i32
}

/// Realise `store_path` via `nix-store --realise` (which substitutes from
/// configured binary caches such as xzar.plan.ai) and self-replace from
/// `{store_path}/bin/mac-mgmt`.
fn apply_store_path(version: &str, store_path: &str) -> Result<()> {
    tracing::info!("realising {store_path}");
    let output = Command::new("nix-store")
        .args(["--realise", store_path])
        .output()
        .context("failed to run nix-store --realise")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("nix-store --realise failed: {stderr}");
    }

    let realised: &Path = Path::new(store_path);
    let new_bin = realised.join("bin").join("mac-mgmt");
    if !new_bin.exists() {
        anyhow::bail!("binary not found at {}", new_bin.display());
    }

    if let Err(e) = self_replace::self_replace(&new_bin) {
        tracing::warn!("self_replace failed ({e}), falling back to rename-over");
        let current_exe = std::env::current_exe().context("failed to get current exe path")?;
        let parent = current_exe.parent().context("current exe has no parent dir")?;
        let tmp_target = parent.join(".mac-mgmt.update");
        std::fs::copy(&new_bin, &tmp_target)
            .context("failed to copy new binary to temp location")?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp_target, std::fs::Permissions::from_mode(0o755))
                .context("failed to set permissions on new binary")?;
        }

        std::fs::rename(&tmp_target, &current_exe)
            .context("failed to rename new binary over current")?;
    }

    write_last_store_path(store_path);

    tracing::info!("binary updated to {version}");
    sentry_ext::breadcrumb("self-update", "binary updated", &[
        ("from", CURRENT_VERSION),
        ("to", version),
        ("store_path", store_path),
    ]);
    Ok(())
}

/// CLI entrypoint kept for backwards compatibility. Updates are now
/// driven by the server (`/api/update` returns the version + nix store
/// path), so this command just triggers an immediate apply against
/// whatever target the in-process state holds — useful when the daemon
/// is running and has already fetched a target.
pub fn apply(_force: bool) -> Result<()> {
    let _ = update_base; // silence dead-code warning for retired path
    let t = target();
    if t.version.is_none() {
        println!(
            "no target version known in this process; updates are driven by the server"
        );
        return Ok(());
    }
    check_and_apply();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_base_is_set() {
        assert!(!UPDATE_BASE.is_empty(), "UPDATE_BASE_URL must be set at build time");
    }

    #[test]
    fn default_update_base_is_plan_ai() {
        assert_eq!(UPDATE_BASE, "https://update.plan.ai");
    }

    #[test]
    fn current_version_is_valid_semver() {
        assert!(
            CURRENT_VERSION.split('.').count() >= 3,
            "CURRENT_VERSION should be semver: {CURRENT_VERSION}"
        );
    }

    #[test]
    fn environment_is_set() {
        assert!(!ENVIRONMENT.is_empty());
    }

    #[test]
    fn target_is_set() {
        assert!(!TARGET.is_empty());
    }

    #[test]
    fn version_cmp_works() {
        // These compare against CURRENT_VERSION (0.1.5)
        assert!(version_cmp("0.1.5") == 0);
        assert!(version_cmp("0.1.6") > 0);
        assert!(version_cmp("0.1.4") < 0);
        assert!(version_cmp("0.2.0") > 0);
        assert!(version_cmp("1.0.0") > 0);
    }
}
