use anyhow::Result;
use std::path::PathBuf;

use crate::managed_service::{FileTunnelDef, FileTunnelKind, ManagedService, ServiceMode};
use crate::sentry_ext;

const PKG: &str = "mcporter";

pub struct McPorter;

impl ManagedService for McPorter {
    fn name(&self) -> &str {
        "mcporter"
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
            ("service", "mcporter"),
            ("package", PKG),
        ]);
        crate::nix::profile_install(PKG, false)?;
        Ok(())
    }

    fn ensure_setup(&self) -> Result<()> {
        Ok(())
    }

    fn spawn_spec(&self) -> crate::managed_service::SpawnSpec {
        unreachable!("mcporter is install-only")
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
            ("service", "mcporter"),
            ("package", PKG),
        ]);
        crate::nix::profile_install(PKG, true)?;
        tracing::info!("{PKG} upgraded");
        Ok(true)
    }

    fn expose_files(&self) -> Vec<FileTunnelDef> {
        let config_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/root"))
            .join(".mcporter");
        if !config_dir.exists() {
            return Vec::new();
        }
        vec![FileTunnelDef {
            name: "mcporter-config".into(),
            service: String::new(),
            path: config_dir.to_string_lossy().into(),
            kind: FileTunnelKind::Directory,
            writable: false,
            include: Some(vec!["*.json".into()]),
            validators: Vec::new(),
            description: "McPorter MCP server configuration (managed by daemon)".into(),
        }]
    }
}
