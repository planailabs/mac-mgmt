use anyhow::{Context, Result};
use std::process::Command;

use super::manifest::InstallManifest;
use super::unit_generator;
use super::{ServiceStrategy, UnmanagedService};

/// Drive one service through the install state machine. Skips completed
/// steps, retries from the first incomplete one. Saves after every
/// transition so a crash can resume.
pub fn ensure_service(
    svc: &UnmanagedService,
    manifest: &mut InstallManifest,
    manifest_path: &std::path::Path,
) -> Result<()> {
    let name = svc.name().to_string();

    // Step 1: nix package
    if !manifest.get_or_create(&name).package_installed {
        tracing::info!("{name}: installing package");
        match svc.svc.ensure_installed() {
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
    }

    // Step 2: config / first-run setup
    if !manifest.get_or_create(&name).configured {
        tracing::info!("{name}: configuring");
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
    }

    // Step 3: create + start system service
    if !manifest.get_or_create(&name).service_active {
        tracing::info!("{name}: creating service ({:?})", svc.strategy);
        match create_service(svc) {
            Ok(()) => {
                let s = manifest.get_or_create(&name);
                s.service_active = true;
                s.last_error = None;
                manifest.save(manifest_path)?;
            }
            Err(e) => {
                manifest.get_or_create(&name).last_error =
                    Some(format!("create_service: {e}"));
                manifest.save(manifest_path)?;
                return Err(e).with_context(|| format!("{name}: create_service"));
            }
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
        manifest.get_or_create(&name).service_active = false;
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
/// touching anything on disk.
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

fn create_service(svc: &UnmanagedService) -> Result<()> {
    match &svc.strategy {
        ServiceStrategy::BuiltInDaemon { install_cmd, .. } => {
            tracing::info!("{}: running built-in installer: {:?}", svc.name(), install_cmd);
            let status = Command::new(&install_cmd[0])
                .args(&install_cmd[1..])
                .status()
                .with_context(|| format!("running {:?}", install_cmd))?;
            if !status.success() {
                anyhow::bail!("{:?} exited with {status}", install_cmd);
            }
            Ok(())
        }
        ServiceStrategy::GeneratedUnit => {
            let spec = svc.svc.spawn_spec();
            unit_generator::create_and_enable(svc.name(), &spec)
        }
        ServiceStrategy::InstallOnly => Ok(()),
    }
}

fn destroy_service(svc: &UnmanagedService) -> Result<()> {
    match &svc.strategy {
        ServiceStrategy::BuiltInDaemon { uninstall_cmd, .. } => {
            tracing::info!("{}: running built-in uninstaller: {:?}", svc.name(), uninstall_cmd);
            let _ = Command::new(&uninstall_cmd[0])
                .args(&uninstall_cmd[1..])
                .status();
            Ok(())
        }
        ServiceStrategy::GeneratedUnit => unit_generator::stop_and_remove(svc.name()),
        ServiceStrategy::InstallOnly => Ok(()),
    }
}
