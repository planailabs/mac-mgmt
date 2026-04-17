use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::process::Command;

use super::manifest::InstallManifest;
use super::unit_generator;
use super::{ServiceStrategy, UnmanagedService};

/// Acquire the daemon.lock so install/uninstall can't race with a
/// running daemon or another install process. Returns a guard that
/// releases the lock on drop.
pub fn acquire_lock() -> Result<std::fs::File> {
    let lock_path = crate::config::config_dir().join("daemon.lock");
    std::fs::create_dir_all(lock_path.parent().unwrap()).ok();
    let lock_file =
        std::fs::File::create(&lock_path).context("failed to create lockfile")?;
    match lock_file.try_lock() {
        Ok(()) => Ok(lock_file),
        Err(std::fs::TryLockError::WouldBlock) => {
            anyhow::bail!(
                "another daemon or install process is running (lockfile: {})",
                lock_path.display()
            );
        }
        Err(std::fs::TryLockError::Error(e)) => {
            Err(e).context("failed to lock lockfile")
        }
    }
}

/// Drive one service to its desired state. Every idempotent step runs on
/// every invocation so config changes propagate. Saves after every state
/// change for crash safety.
pub fn ensure_service(
    svc: &UnmanagedService,
    manifest: &mut InstallManifest,
    manifest_path: &std::path::Path,
) -> Result<()> {
    let name = svc.name().to_string();

    // 1. Package: always run. Verify the package is actually in the
    //    profile before trusting the manifest flag — a prior failed
    //    migration (remove succeeded, add failed) can leave the flag
    //    stale while the package is missing.
    let actually_installed = crate::nix::is_installed(svc.svc.name()).unwrap_or(false);
    if !actually_installed {
        manifest.get_or_create(&name).package_installed = false;
    }
    tracing::info!("{name}: ensuring package");
    match install_and_ensure_nixpkgs(svc) {
        Ok(()) => {
            let s = manifest.get_or_create(&name);
            s.package_installed = true;
            s.last_error = None;
            manifest.save(manifest_path)?;
        }
        Err(e) => {
            manifest.get_or_create(&name).last_error =
                Some(format!("ensure_installed: {e}"));
            manifest.save(manifest_path)?;
            return Err(e).with_context(|| format!("{name}: ensure_installed"));
        }
    }

    // 2. Config: always run (idempotent).
    tracing::info!("{name}: ensuring config");
    match svc.svc.ensure_setup() {
        Ok(()) => {
            let s = manifest.get_or_create(&name);
            s.configured = true;
            s.last_error = None;
            manifest.save(manifest_path)?;
        }
        Err(e) => {
            manifest.get_or_create(&name).last_error =
                Some(format!("ensure_setup: {e}"));
            manifest.save(manifest_path)?;
            return Err(e).with_context(|| format!("{name}: ensure_setup"));
        }
    }

    // 3. Service unit: create or update.
    match &svc.strategy {
        ServiceStrategy::GeneratedUnit => {
            let spec = svc.svc.spawn_spec();
            let desired = unit_generator::generate_unit_contents(svc.name(), &spec);
            let hash = hex::encode(Sha256::digest(desired.as_bytes()));
            let state = manifest.get_or_create(&name);
            let hash_changed = state.unit_hash.as_deref() != Some(&hash);
            let was_active = state.service_active;

            if hash_changed {
                // Unit contents changed (or first install) — write the file.
                tracing::info!("{name}: writing unit (hash changed)");
                unit_generator::write_unit(svc.name(), &spec)?;
                let s = manifest.get_or_create(&name);
                s.unit_hash = Some(hash);

                if was_active {
                    // Already running with old config → restart.
                    tracing::info!("{name}: restarting (config changed)");
                    unit_generator::restart(svc.name())?;
                } else {
                    // First install → enable + start.
                    unit_generator::enable_and_start(svc.name())?;
                }
                let s = manifest.get_or_create(&name);
                s.service_active = true;
                s.last_error = None;
                manifest.save(manifest_path)?;
            } else if !was_active {
                // Hash matches but service not active (e.g. interrupted
                // after write but before enable). Just enable + start.
                tracing::info!("{name}: enabling unit");
                unit_generator::enable_and_start(svc.name())?;
                let s = manifest.get_or_create(&name);
                s.service_active = true;
                manifest.save(manifest_path)?;
            }
            // Hash matches + already active → no-op.
        }
        ServiceStrategy::BuiltInDaemon { install_cmd, .. } => {
            // Built-in installers are idempotent — always re-run so
            // config changes (e.g. openclaw gateway port) propagate.
            tracing::info!("{name}: running built-in installer");
            let status = Command::new(&install_cmd[0])
                .args(&install_cmd[1..])
                .status()
                .with_context(|| format!("running {:?}", install_cmd))?;
            if !status.success() {
                anyhow::bail!("{:?} exited with {status}", install_cmd);
            }
            let s = manifest.get_or_create(&name);
            s.service_active = true;
            s.last_error = None;
            manifest.save(manifest_path)?;
        }
        ServiceStrategy::InstallOnly => {}
    }

    // 4. Post-start (model pulling, etc). Idempotent — already-pulled
    //    models are a fast no-op from the service's perspective.
    if manifest.get_or_create(&name).service_active
        && !manifest.get_or_create(&name).models_pulled
    {
        tracing::info!("{name}: running post_start");
        if let Err(e) = svc.post_start() {
            tracing::warn!("{name}: post_start failed: {e:#}");
        }
        manifest.get_or_create(&name).models_pulled = true;
        manifest.save(manifest_path)?;
    }

    tracing::info!("{name}: ready");
    Ok(())
}

/// Reverse the install: stop service → remove package.
pub fn remove_service(
    svc: &UnmanagedService,
    manifest: &mut InstallManifest,
    manifest_path: &std::path::Path,
) -> Result<()> {
    let name = svc.name().to_string();

    if manifest.get_or_create(&name).service_active {
        tracing::info!("{name}: stopping service");
        match destroy_service(svc) {
            Ok(()) => {
                let s = manifest.get_or_create(&name);
                s.service_active = false;
                s.unit_hash = None;
                s.models_pulled = false;
                manifest.save(manifest_path)?;
            }
            Err(e) => {
                // Leave service_active = true so next run retries.
                tracing::warn!("{name}: destroy_service failed (will retry): {e:#}");
                manifest.get_or_create(&name).last_error =
                    Some(format!("destroy_service: {e}"));
                manifest.save(manifest_path)?;
                return Err(e).with_context(|| format!("{name}: destroy_service"));
            }
        }
    }

    if manifest.get_or_create(&name).configured {
        manifest.get_or_create(&name).configured = false;
        manifest.save(manifest_path)?;
    }

    if manifest.get_or_create(&name).package_installed {
        tracing::info!("{name}: removing package");
        if let Err(e) = crate::nix::profile_remove(svc.svc.name()) {
            tracing::warn!("{name}: profile_remove: {e:#}");
        }
        manifest.get_or_create(&name).package_installed = false;
        manifest.save(manifest_path)?;
    }

    tracing::info!("{name}: removed");
    Ok(())
}

/// Detect an existing install and populate the manifest without
/// touching anything on disk (except migrating the flake ref).
pub fn import_service(
    svc: &UnmanagedService,
    manifest: &mut InstallManifest,
    manifest_path: &std::path::Path,
) -> Result<bool> {
    if which::which(svc.svc.name()).is_err() {
        return Ok(false);
    }

    let name = svc.name().to_string();

    if let Err(e) = crate::nix::migrate_to_nixpkgs(svc.svc.name()) {
        tracing::warn!("{name}: flake migration during import: {e:#}");
    }

    let state = manifest.get_or_create(&name);
    state.package_installed = true;

    if svc.paths.iter().any(|p| p.path.exists()) {
        state.configured = true;
    }

    match &svc.strategy {
        ServiceStrategy::BuiltInDaemon { .. } | ServiceStrategy::GeneratedUnit => {
            if unit_generator::is_active(svc.name()) {
                state.service_active = true;
            }
        }
        ServiceStrategy::InstallOnly => {}
    }

    state.last_error = None;
    manifest.save(manifest_path)?;
    Ok(true)
}

fn install_and_ensure_nixpkgs(svc: &UnmanagedService) -> Result<()> {
    svc.svc.ensure_installed()?;
    let _ = crate::nix::migrate_to_nixpkgs(svc.svc.name());
    Ok(())
}

fn destroy_service(svc: &UnmanagedService) -> Result<()> {
    match &svc.strategy {
        ServiceStrategy::BuiltInDaemon { uninstall_cmd, .. } => {
            let status = Command::new(&uninstall_cmd[0])
                .args(&uninstall_cmd[1..])
                .status()
                .with_context(|| format!("running {:?}", uninstall_cmd))?;
            if !status.success() {
                anyhow::bail!("{:?} exited with {status}", uninstall_cmd);
            }
            Ok(())
        }
        ServiceStrategy::GeneratedUnit => unit_generator::stop_and_remove(svc.name()),
        ServiceStrategy::InstallOnly => Ok(()),
    }
}
