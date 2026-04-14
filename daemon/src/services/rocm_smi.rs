//! Install-only service that ensures `rocm-smi` is available for the GPU
//! assessment collector.
//!
//! Scoped behind the same hardware gate as `nvidia-smi`: only install when
//! AMD hardware is actually present (vendor `1002`). `rocmPackages.rocm-smi`
//! is comparatively lightweight (~150 MB) but there's no value on boxes
//! without AMD GPUs. macOS skipped entirely (no ROCm on macOS).

use anyhow::Result;

use crate::managed_service::{ManagedService, ServiceMode};
use crate::sentry_ext;
use crate::services::gpu_tool_common::{is_in_path, lspci_has_vendor};

/// Nixpkgs attribute path. `rocmPackages.rocm-smi` resolves via the daemon's
/// pinned nixpkgs commit just like any other scoped package.
const PKG: &str = "rocmPackages.rocm-smi";
/// Leaf name that lands in the nix store path — used for both `is_installed`
/// string-contains matching and the `packages_with_upgrades` leaf comparison.
const PKG_LEAF: &str = "rocm-smi";
/// PCI vendor ID for AMD / ATI. Matches discrete AMD GPUs and APUs alike.
const VENDOR_ID: &str = "1002";

pub struct RocmSmi;

impl ManagedService for RocmSmi {
    fn name(&self) -> &str {
        "rocm-smi"
    }

    fn service_mode(&self) -> ServiceMode {
        ServiceMode::InstallOnly
    }

    fn ensure_installed(&self) -> Result<()> {
        if cfg!(target_os = "macos") {
            tracing::debug!("rocm-smi: skipping on macOS (no ROCm support)");
            return Ok(());
        }

        if is_in_path("rocm-smi") {
            tracing::debug!("rocm-smi: already in PATH, no install needed");
            return Ok(());
        }

        match lspci_has_vendor(VENDOR_ID) {
            Ok(true) => {
                tracing::info!("rocm-smi: AMD hardware detected, installing {PKG}");
                sentry_ext::breadcrumb(
                    "install",
                    &format!("installing {PKG}"),
                    &[("service", "rocm-smi"), ("package", PKG)],
                );
            }
            Ok(false) => {
                tracing::info!(
                    "rocm-smi: no AMD hardware detected (via lspci), skipping install"
                );
                return Ok(());
            }
            Err(e) => {
                tracing::warn!(
                    "rocm-smi: cannot detect hardware (lspci unavailable: {e}), skipping install"
                );
                return Ok(());
            }
        }

        if crate::nix::is_installed(PKG_LEAF)? {
            tracing::info!("rocm-smi: already installed via nix");
            return Ok(());
        }

        crate::nix::profile_install(PKG, false)?;
        Ok(())
    }

    fn ensure_setup(&self) -> Result<()> {
        Ok(())
    }

    fn spawn_spec(&self) -> crate::managed_service::SpawnSpec {
        unreachable!("rocm-smi is install-only")
    }

    fn check_health(&self) -> Result<bool> {
        Ok(true)
    }

    fn repair(&self) -> Result<()> {
        Ok(())
    }

    fn check_and_upgrade(&self) -> Result<bool> {
        if !crate::nix::is_installed(PKG_LEAF)? {
            return Ok(false);
        }

        let upgradable = crate::nix::packages_with_upgrades(&[PKG])?;
        if !upgradable.iter().any(|name| name == PKG_LEAF) {
            return Ok(false);
        }

        tracing::info!("upgrading {PKG} via nix");
        sentry_ext::breadcrumb(
            "upgrade",
            &format!("upgrading {PKG}"),
            &[("service", "rocm-smi"), ("package", PKG)],
        );
        crate::nix::profile_install(PKG, true)?;
        Ok(true)
    }
}
