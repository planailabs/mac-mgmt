//! Install-only service that ensures `nvidia-smi` is available for the GPU
//! assessment collector.
//!
//! We only attempt to install if **both** are true:
//!   1. `nvidia-smi` is not already on the daemon's `$PATH`.
//!   2. NVIDIA hardware is actually present (lspci reports vendor `10de`).
//!
//! The second gate matters because `nvidia-smi` in nixpkgs is bundled with
//! `cudatoolkit` — a multi-GB package we don't want on GPU-less hosts.
//! On macOS we skip entirely: there is no NVIDIA GPU support on Apple
//! Silicon, and `nvidia-smi` doesn't exist there.

use anyhow::Result;

use crate::managed_service::{ManagedService, ServiceMode};
use crate::sentry_ext;
use crate::services::gpu_tool_common::{is_in_path, lspci_has_vendor};

/// Nixpkgs attribute path for the package that ships `nvidia-smi`.
/// cudatoolkit is the sole standalone option — the kernel-module-coupled
/// `linuxPackages.nvidia_x11` isn't usable on hosts that don't already
/// have a matching kernel.
const PKG: &str = "cudatoolkit";
/// PCI vendor ID for NVIDIA. lspci uses lowercase.
const VENDOR_ID: &str = "10de";

pub struct NvidiaSmi;

impl ManagedService for NvidiaSmi {
    fn name(&self) -> &str {
        "nvidia-smi"
    }

    fn service_mode(&self) -> ServiceMode {
        ServiceMode::InstallOnly
    }

    fn ensure_installed(&self) -> Result<()> {
        if cfg!(target_os = "macos") {
            tracing::debug!("nvidia-smi: skipping on macOS");
            return Ok(());
        }

        if is_in_path("nvidia-smi") {
            tracing::debug!("nvidia-smi: already in PATH, no install needed");
            return Ok(());
        }

        match lspci_has_vendor(VENDOR_ID) {
            Ok(true) => {
                tracing::info!("nvidia-smi: NVIDIA hardware detected, installing {PKG}");
                sentry_ext::breadcrumb(
                    "install",
                    &format!("installing {PKG} (provides nvidia-smi)"),
                    &[("service", "nvidia-smi"), ("package", PKG)],
                );
            }
            Ok(false) => {
                tracing::info!(
                    "nvidia-smi: no NVIDIA hardware detected (via lspci), skipping install"
                );
                return Ok(());
            }
            Err(e) => {
                // lspci itself missing — we can't confirm hardware so bail
                // rather than pulling a multi-GB toolkit onto a random box.
                tracing::warn!(
                    "nvidia-smi: cannot detect hardware (lspci unavailable: {e}), skipping install"
                );
                return Ok(());
            }
        }

        if crate::nix::is_installed("cudatoolkit")? {
            tracing::info!("nvidia-smi: cudatoolkit already installed via nix");
            return Ok(());
        }

        crate::nix::profile_install(PKG, false)?;
        Ok(())
    }

    fn ensure_setup(&self) -> Result<()> {
        Ok(())
    }

    fn spawn_spec(&self) -> crate::managed_service::SpawnSpec {
        unreachable!("nvidia-smi is install-only")
    }

    fn check_health(&self) -> Result<bool> {
        Ok(true)
    }

    fn repair(&self) -> Result<()> {
        Ok(())
    }

    fn check_and_upgrade(&self) -> Result<bool> {
        // Skip upgrade checks on hosts where we never installed.
        if !crate::nix::is_installed("cudatoolkit")? {
            return Ok(false);
        }

        let upgradable = crate::nix::packages_with_upgrades(&[PKG])?;
        if !upgradable.iter().any(|name| name == PKG) {
            return Ok(false);
        }

        tracing::info!("upgrading {PKG} via nix");
        sentry_ext::breadcrumb(
            "upgrade",
            &format!("upgrading {PKG} via nix"),
            &[("service", "nvidia-smi"), ("package", PKG)],
        );
        crate::nix::profile_install(PKG, true)?;
        Ok(true)
    }
}
