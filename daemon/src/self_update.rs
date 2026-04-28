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

/// Set after a successful update. The main loop checks this to trigger
/// a clean shutdown followed by exec of the new binary.
static RESTART_EXEC: std::sync::RwLock<Option<std::path::PathBuf>> =
    std::sync::RwLock::new(None);

/// Check whether a self-update completed and a restart is pending.
/// Returns the path to the new binary if so.
pub fn take_restart_exec() -> Option<std::path::PathBuf> {
    RESTART_EXEC.write().unwrap().take()
}

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
    std::fs::read_to_string(&p)
        .ok()
        .map(|s| s.trim().to_string())
}

fn write_last_store_path(store_path: &str) {
    let Some(p) = applied_marker_path() else {
        return;
    };
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
/// `current_exe()` resolves symlinks, so even if invoked via
/// `/usr/local/bin/mac-mgmt`, this returns the `/nix/store/...` prefix.
/// Returns `None` when the binary doesn't live in `/nix/store/` (e.g.
/// dev builds from `cargo run`).
fn current_binary_store_path() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    crate::nix::store_path_prefix(exe.to_str()?)
}

/// Determine where the `mac-mgmt` symlink should be placed.
///
/// If the binary was invoked through a symlink outside `/nix/store/`
/// (the common case for installed daemons), we want to replace that
/// symlink. We read `/proc/self/exe` (the resolved path) and compare
/// it against `argv[0]`/`PATH` to find the pre-resolution path.
///
/// Fallback order:
/// 1. The original argv[0] path if it's outside /nix/store/ and exists
/// 2. `~/.local/bin/mac-mgmt`
pub fn install_bin_path() -> Result<std::path::PathBuf> {
    // Try argv[0]: on most systems this is the path the user typed or
    // the path the service manager used.
    if let Some(arg0) = std::env::args().next() {
        let p = Path::new(&arg0);
        // Resolve relative paths against cwd
        let p = if p.is_absolute() {
            p.to_path_buf()
        } else if let Ok(cwd) = std::env::current_dir() {
            cwd.join(p)
        } else {
            p.to_path_buf()
        };
        if !p.to_str().unwrap_or("").starts_with("/nix/store/") && p.exists() {
            return Ok(p);
        }
    }

    // Try finding mac-mgmt in PATH (outside /nix/store/)
    if let Ok(found) = which::which("mac-mgmt") {
        if !found.to_str().unwrap_or("").starts_with("/nix/store/") {
            return Ok(found);
        }
    }

    // Fallback: ~/.local/bin/mac-mgmt
    let home = dirs::home_dir().context("no home directory")?;
    Ok(home.join(".local/bin/mac-mgmt"))
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
        tracing::info!("store path mismatch: running={current} target={store_path} (v{version})");
    } else {
        // Binary isn't in the nix store (dev build, manual install, etc).
        // Fall back to the on-disk marker so we don't re-apply every tick.
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
                        "version {version} unchanged, no applied marker, applying {store_path}"
                    );
                }
            }
        }
    }

    tracing::info!("updating: {CURRENT_VERSION} -> {version} (store {store_path})");
    sentry_ext::breadcrumb(
        "self-update",
        "updating",
        &[
            ("from", CURRENT_VERSION),
            ("to", &version),
            ("store_path", &store_path),
        ],
    );

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
    let parse = |s: &str| -> Vec<u64> { s.split('.').filter_map(|p| p.parse().ok()).collect() };
    let a = parse(ver);
    let b = parse(CURRENT_VERSION);
    a.cmp(&b) as i32
}

/// Realise `store_path` and register a GC root so `nix-collect-garbage`
/// doesn't sweep the derivation out from under us. Then replace the
/// install symlink with one pointing to `{store_path}/bin/mac-mgmt`.
///
/// The install location is determined by walking up from `current_exe()`
/// to find the first path component outside `/nix/store/`. If the
/// binary was invoked via a symlink (e.g. `/usr/local/bin/mac-mgmt`
/// → `/nix/store/.../bin/mac-mgmt`), we replace the symlink, not the
/// store entry. Falls back to `~/.local/bin/mac-mgmt`.
fn apply_store_path(version: &str, store_path: &str) -> Result<()> {
    let install_path = install_bin_path().context("failed to determine install path")?;
    let install_dir = install_path
        .parent()
        .context("install path has no parent dir")?;

    let gcroot_dir = crate::config::config_dir();
    std::fs::create_dir_all(&gcroot_dir)
        .with_context(|| format!("failed to create {}", gcroot_dir.display()))?;
    let gcroot = gcroot_dir.join(".mac-mgmt.gcroot");

    // Remove a prior gcroot so --add-root can create a fresh symlink.
    let _ = std::fs::remove_file(&gcroot);

    tracing::info!("realising {store_path} (gcroot {})", gcroot.display());
    let cache_args = crate::nix::extra_substituter_args();
    let output = Command::new("nix-store")
        .args([
            "--realise",
            "--add-root",
            gcroot.to_str().unwrap_or(".mac-mgmt.gcroot"),
            store_path,
        ])
        .args(&cache_args)
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

    std::fs::create_dir_all(install_dir)
        .with_context(|| format!("failed to create {}", install_dir.display()))?;
    let tmp_link = install_dir.join(".mac-mgmt.update");
    let _ = std::fs::remove_file(&tmp_link);

    std::os::unix::fs::symlink(&new_bin, &tmp_link)
        .with_context(|| format!("symlink {} -> {}", tmp_link.display(), new_bin.display()))?;

    std::fs::rename(&tmp_link, &install_path)
        .with_context(|| format!("rename {} -> {}", tmp_link.display(), install_path.display()))?;

    write_last_store_path(store_path);

    tracing::info!("binary updated to {version} (symlink → {store_path}), restart pending");
    sentry_ext::breadcrumb(
        "self-update",
        "binary updated, restart pending",
        &[
            ("from", CURRENT_VERSION),
            ("to", version),
            ("store_path", store_path),
        ],
    );

    // Signal the main loop to perform a clean shutdown then exec the new binary.
    *RESTART_EXEC.write().unwrap() = Some(new_bin);
    Ok(())
}

/// Check if the install symlink now resolves to a different binary than
/// the one we're running. This catches external updates (e.g. `nix profile
/// upgrade`, manual symlink swap) that bypassed the self-update flow.
/// If a change is detected, sets `RESTART_EXEC` so the main loop restarts.
pub fn check_binary_changed() -> bool {
    let Ok(install) = install_bin_path() else {
        return false;
    };
    // Resolve the symlink to its final target.
    let Ok(target) = std::fs::canonicalize(&install) else {
        return false;
    };
    let Ok(running) = std::env::current_exe() else {
        return false;
    };
    if target != running {
        tracing::info!(
            "binary changed: running={} install symlink now points to {}",
            running.display(),
            target.display()
        );
        *RESTART_EXEC.write().unwrap() = Some(target);
        return true;
    }
    false
}

/// Ensure the running binary is a symlink into the nix store.
///
/// On first install the binary is often a plain file (copied manually or
/// built locally). Overwriting a running binary can fail ("text file
/// busy"), so we convert it to a symlink immediately:
///
/// 1. `nix-store --add <binary>` → `/nix/store/<hash>-mac-mgmt`
/// 2. Replace the file at `install_bin_path()` with a symlink to the
///    store copy.
/// 3. Register a GC root so nix doesn't sweep it.
///
/// Returns the stable symlink path (suitable for service unit files).
/// If the binary is already a symlink into `/nix/store/`, this is a
/// no-op and returns the existing symlink path.
pub fn ensure_symlink() -> Result<std::path::PathBuf> {
    let install_path = install_bin_path().context("failed to determine install path")?;

    // Already a symlink → nothing to do.
    if install_path.read_link().is_ok() {
        tracing::debug!(
            "binary at {} is already a symlink, skipping",
            install_path.display()
        );
        return Ok(install_path);
    }

    // Resolve the actual binary we're running.
    let exe = std::env::current_exe().context("cannot determine binary path")?;

    // If the binary is already in the nix store (e.g. invoked directly
    // from a nix profile), just create the symlink — no --add needed.
    let store_file = if exe.to_string_lossy().starts_with("/nix/store/") {
        exe.clone()
    } else {
        // Add the binary to the nix store so it has a stable, immutable
        // path we can symlink to.
        tracing::info!("adding {} to nix store", exe.display());
        let output = Command::new("nix-store")
            .args(["--add", exe.to_str().unwrap_or("mac-mgmt")])
            .output()
            .context("failed to run nix-store --add")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("nix-store --add failed: {stderr}");
        }
        let store_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if store_path.is_empty() {
            anyhow::bail!("nix-store --add returned an empty path");
        }
        std::path::PathBuf::from(store_path)
    };

    if !store_file.exists() {
        anyhow::bail!(
            "store path {} does not exist after nix-store --add",
            store_file.display()
        );
    }

    // Register a GC root so this store path survives garbage collection.
    let gcroot_dir = crate::config::config_dir();
    std::fs::create_dir_all(&gcroot_dir)
        .with_context(|| format!("failed to create {}", gcroot_dir.display()))?;
    let gcroot = gcroot_dir.join(".mac-mgmt.gcroot");
    let _ = std::fs::remove_file(&gcroot);
    let cache_args = crate::nix::extra_substituter_args();
    let realise_out = Command::new("nix-store")
        .args([
            "--realise",
            "--add-root",
            gcroot.to_str().unwrap_or(".mac-mgmt.gcroot"),
            store_file.to_str().unwrap_or(""),
        ])
        .args(&cache_args)
        .output();
    if let Ok(out) = &realise_out {
        if !out.status.success() {
            tracing::warn!(
                "nix-store --realise --add-root for initial symlink failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }

    // Atomic replace: tmp symlink → rename.
    let install_dir = install_path
        .parent()
        .context("install path has no parent dir")?;
    std::fs::create_dir_all(install_dir)
        .with_context(|| format!("failed to create {}", install_dir.display()))?;
    let tmp_link = install_dir.join(".mac-mgmt.update");
    let _ = std::fs::remove_file(&tmp_link);

    std::os::unix::fs::symlink(&store_file, &tmp_link).with_context(|| {
        format!(
            "symlink {} -> {}",
            tmp_link.display(),
            store_file.display()
        )
    })?;

    std::fs::rename(&tmp_link, &install_path).with_context(|| {
        format!(
            "rename {} -> {}",
            tmp_link.display(),
            install_path.display()
        )
    })?;

    tracing::info!(
        "binary at {} is now a symlink to {}",
        install_path.display(),
        store_file.display()
    );

    Ok(install_path)
}

/// CLI entrypoint kept for backwards compatibility. Updates are now
/// driven by the server (`/api/update` returns the version + nix store
/// path), so this command just triggers an immediate apply against
/// whatever target the in-process state holds — useful when the daemon
/// is running and has already fetched a target.
pub fn apply(force: bool) -> Result<()> {
    let t = target();
    let Some(version) = t.version else {
        println!("no target version known in this process; updates are driven by the server");
        return Ok(());
    };

    if force {
        let store_path = t
            .store_path
            .context("--force requires a store path; pass --store-path or rely on the server")?;
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
