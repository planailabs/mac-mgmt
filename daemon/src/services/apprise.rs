use anyhow::Result;

use crate::managed_service::{ManagedService, ServiceMode};
use crate::sentry_ext;

const PKG: &str = "apprise";

pub struct Apprise;

impl ManagedService for Apprise {
    fn name(&self) -> &str {
        "apprise"
    }

    fn service_mode(&self) -> ServiceMode {
        ServiceMode::InstallOnly
    }

    fn ensure_installed(&self) -> Result<()> {
        if crate::nix::is_installed(PKG)? {
            tracing::info!("{PKG} is already installed");
            return Ok(());
        }

        tracing::info!("{PKG} not found, installing via nix");
        sentry_ext::breadcrumb("install", &format!("installing {PKG} via nix"), &[
            ("service", "apprise"),
            ("package", PKG),
        ]);
        crate::nix::profile_install(PKG, false)?;
        Ok(())
    }

    fn ensure_setup(&self) -> Result<()> {
        Ok(())
    }

    fn spawn_spec(&self) -> crate::managed_service::SpawnSpec {
        unreachable!("apprise is install-only")
    }

    fn check_health(&self) -> Result<bool> {
        Ok(true)
    }

    fn repair(&self) -> Result<()> {
        Ok(())
    }

    fn check_and_upgrade(&self) -> Result<bool> {
        let upgradable = crate::nix::packages_with_upgrades(&[PKG])?;

        if !upgradable.iter().any(|name| name == PKG) {
            return Ok(false);
        }

        tracing::info!("upgrading {PKG} via nix");
        sentry_ext::breadcrumb("upgrade", &format!("upgrading {PKG} via nix"), &[
            ("service", "apprise"),
            ("package", PKG),
        ]);
        crate::nix::profile_install(PKG, true)?;
        tracing::info!("{PKG} upgraded");
        Ok(true)
    }
}
