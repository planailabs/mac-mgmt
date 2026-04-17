use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::process::Command;

use super::manifest::InstallManifest;
use super::unit_generator;
use super::{ServiceStrategy, UnmanagedService};

/// Drive one service to its desired state. Every idempotent step runs on
/// every invocation so config changes propagate; only the nix-install
/// step is skipped when already done (the package is in the profile).
/// Saves after every state change for crash safety.
pub fn ensure_service(
    svc: &UnmanagedService,
    manifest: &mut InstallManifest,
    manifest_path: &std::path::Path,
) -> Result<()> {
    let name = svc.name().to_string();

    // 1. Package: always run — handles flavour swap, upgrades, etc.
    //    ensure_installed() is already idempotent.
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

    // 2. Config: always run — ensure_setup() is idempotent, re-applies
    //    config patches so changes to config.toml propagate.
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

    // 3. Service unit: create or update. For GeneratedUnit, compare
    //    the hash of the desired unit to what we last wrote — rewrite
    //    + restart only when spawn_spec() output changed (env vars,
    //    command, etc). BuiltInDaemon installers are re-run on every
    //    invocation (they're idempotent).
    match &svc.strategy {
        ServiceStrategy::GeneratedUnit => {
            let spec = svc.svc.spawn_spec();
            let desired = unit_generator::generate_unit_contents(svc.name(), &spec);
            let hash = hex::encode(Sha256::digest(desired.as_bytes()));
            let state = manifest.get_or_create(&name);
            if state.unit_hash.as_deref() != Some(&hash) {
                tracing::info!("{name}: writing unit (hash changed)");
                unit_generator::create_and_enable(svc.name(), &spec)?;
                let s = manifest.get_or_create(&name);
                s.unit_hash = Some(hash);
                s.service_active = true;
                s.last_error = None;
                manifest.save(manifest_path)?;
            } else if !state.service_active {
                tracing::info!("{name}: enabling unit");
                unit_generator::create_and_enable(svc.name(), &spec)?;
                let s = manifest.get_or_create(&name);
                s.service_active = true;
                manifest.save(manifest_path)?;
            }
        }
        ServiceStrategy::BuiltInDaemon { install_cmd, .. } => {
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

    // 4. Models: pull after service is running. Idempotent — already-
    //    pulled models are a fast no-op from the service's perspective.
    if manifest.get_or_create(&name).service_active {
        if !manifest.get_or_create(&name).models_pulled {
            tracing::info!("{name}: pulling models");
            if let Err(e) = svc.pull_models() {
                tracing::warn!("{name}: model pull failed: {e:#}");
            }
            manifest.get_or_create(&name).models_pulled = true;
            manifest.save(manifest_path)?;
        }
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
        if let Err(e) = destroy_service(svc) {
            tracing::warn!("{name}: destroy_service: {e:#}");
        }
        let s = manifest.get_or_create(&name);
        s.service_active = false;
        s.unit_hash = None;
        s.models_pulled = false;
        manifest.save(manifest_path)?;
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
    let binary_found = Command::new("which")
        .arg(svc.svc.name())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !binary_found {
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
            let _ = Command::new(&uninstall_cmd[0])
                .args(&uninstall_cmd[1..])
                .status();
            Ok(())
        }
        ServiceStrategy::GeneratedUnit => unit_generator::stop_and_remove(svc.name()),
        ServiceStrategy::InstallOnly => Ok(()),
    }
}
