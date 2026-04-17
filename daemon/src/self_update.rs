use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

use crate::sentry_ext;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

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

/// `${config_dir}/mac-mgmt/.mac-mgmt.store-path` records the last
/// applied store path. Used to detect "same version, different store
/// path" (e.g. someone re-uploaded the same version with a fix) so we
/// re-apply instead of skipping.
fn applied_marker_path() -> Option<std::path::PathBuf> {
    let dir = dirs::config_dir()?.join("mac-mgmt");
    Some(dir.join(".mac-mgmt.store-path"))
}

fn read_last_store_path() -> Option<String> {
    let p = applied_marker_path()?;
    std::fs::read_to_string(&p).ok().map(|s| s.trim().to_string())
}

fn write_last_store_path(store_path: &str) {
    let Some(p) = applied_marker_path() else { return };
    if let Some(parent) = p.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            tracing::warn!("failed to create config dir {}: {e}", parent.display());
            return;
        }
    }
    if let Err(e) = std::fs::write(&p, store_path) {
        tracing::warn!("failed to write applied marker {}: {e}", p.display());
    }
}

/// Resolve the nix store path prefix of the currently running binary.
/// Returns `None` when the binary doesn't live in `/nix/store/` (e.g.
/// dev builds from `cargo run`).
fn current_binary_store_path() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    crate::nix::store_path_prefix(exe.to_str()?)
}

/// Check if the server has assigned a target and apply it.
///
/// Compares both the semver version AND the nix store path of the
/// running binary against the server's target. A store-path mismatch
/// triggers an update even when the version string is identical — this
/// catches re-uploads of the same version with a different build (e.g.
/// a patched derivation, a different nixpkgs pin, or a dirty-tree
/// rebuild).
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

    // Primary check: compare the running binary's actual store path
    // against the server's target. This is more reliable than the
    // version string alone because two builds of the same version can
    // live at different store paths.
    if let Some(current) = current_binary_store_path() {
        if current == store_path {
            tracing::debug!(
                "binary already at target store path {store_path} (v{CURRENT_VERSION})"
            );
            return;
        }
        tracing::info!(
            "store path mismatch: running={current} target={store_path} (v{version})"
        );
    } else {
        // Binary isn't in the nix store (dev build, manual install, etc).
        // Fall back to the on-disk marker so we don't re-apply every tick.
        if version == CURRENT_VERSION {
            match read_last_store_path() {
                Some(last) if last == store_path => {
                    tracing::info!(
                        "already at target version {CURRENT_VERSION} ({store_path})"
                    );
                    return;
                }
                Some(last) => {
                    tracing::info!(
                        "version {version} unchanged but store path changed: {last} -> {store_path}, reapplying"
                    );
                }
                None => {
                    tracing::info!(
                        "version {version} unchanged, no applied marker, applying {store_path}"
                    );
                }
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

/// Realise `store_path` and register a GC root so `nix-collect-garbage`
/// doesn't sweep the derivation out from under us. Then replace the
/// current executable with a symlink to `{store_path}/bin/mac-mgmt`.
///
/// The GC root lives at `{exe_dir}/.mac-mgmt.gcroot` and is created
/// via `nix-store --realise --add-root`, which both downloads the path
/// and registers the root atomically. The binary symlink lets
/// `current_exe() → canonicalize()` resolve to the store path so
/// `check_and_apply` can compare store paths directly.
fn apply_store_path(version: &str, store_path: &str) -> Result<()> {
    let current_exe = std::env::current_exe().context("failed to get current exe path")?;
    let parent = current_exe.parent().context("current exe has no parent dir")?;
    let gcroot = parent.join(".mac-mgmt.gcroot");

    // Remove a prior gcroot so --add-root can create a fresh symlink.
    let _ = std::fs::remove_file(&gcroot);

    tracing::info!("realising {store_path} (gcroot {})", gcroot.display());
    let output = Command::new("nix-store")
        .args([
            "--realise",
            "--add-root",
            gcroot.to_str().unwrap_or(".mac-mgmt.gcroot"),
            store_path,
        ])
        .output()
        .context("failed to run nix-store --realise --add-root")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("nix-store --realise --add-root failed: {stderr}");
    }

    let new_bin = Path::new(store_path).join("bin").join("mac-mgmt");
    if !new_bin.exists() {
        anyhow::bail!("binary not found at {}", new_bin.display());
    }

    let tmp_link = parent.join(".mac-mgmt.update");
    let _ = std::fs::remove_file(&tmp_link);

    std::os::unix::fs::symlink(&new_bin, &tmp_link).with_context(|| {
        format!("symlink {} -> {}", tmp_link.display(), new_bin.display())
    })?;

    std::fs::rename(&tmp_link, &current_exe)
        .context("failed to rename new binary link over current")?;

    write_last_store_path(store_path);

    tracing::info!("binary updated to {version} (symlink → {store_path})");
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
pub fn apply(force: bool) -> Result<()> {
    let t = target();
    let Some(version) = t.version else {
        println!(
            "no target version known in this process; updates are driven by the server"
        );
        return Ok(());
    };

    if force {
        let store_path = t.store_path.context(
            "--force requires a store path; pass --store-path or rely on the server",
        )?;
        apply_store_path(&version, &store_path)?;
        return Ok(());
    }

    check_and_apply();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_version_is_valid_semver() {
        assert!(
            CURRENT_VERSION.split('.').count() >= 3,
            "CURRENT_VERSION should be semver: {CURRENT_VERSION}"
        );
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
