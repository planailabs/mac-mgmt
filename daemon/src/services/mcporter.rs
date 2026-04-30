use anyhow::Result;
use std::path::PathBuf;

use crate::managed_service::{FileTunnelDef, ManagedService, ServiceMode};
use crate::sentry_ext;
use crate::validator::Validator;

const PKG: &str = "mcporter";
pub const SCHEMA_URL: &str =
    "https://raw.githubusercontent.com/steipete/mcporter/main/mcporter.schema.json";

/// Validator for mcporter JSON config files.
pub fn validator() -> Validator {
    Validator::json("*.json").with_schema_url(SCHEMA_URL)
}

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
        sentry_ext::breadcrumb(
            "install",
            &format!("installing {PKG} via nix"),
            &[("service", "mcporter"), ("package", PKG)],
        );
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
        sentry_ext::breadcrumb(
            "upgrade",
            &format!("upgrading {PKG} via nix"),
            &[("service", "mcporter"), ("package", PKG)],
        );
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
        vec![FileTunnelDef::Folder {
            name: "mcporter-config".into(),
            path: config_dir.to_string_lossy().into(),
            writable: false,
            allow_write: Vec::new(),
            include: Some(vec!["*.json".into()]),
            validators: vec![validator()],
            description: "McPorter MCP server configuration (managed by daemon)".into(),
        }]
    }

    fn service_inventory(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<mac_mgmt_common::InventoryEntry>> + Send + '_>> {
        use mac_mgmt_common::{InventoryEntry, InventoryValueType};
        Box::pin(async {
            let mut entries = Vec::new();
            if let Ok(out) = crate::cmd::output_with_timeout(
                std::process::Command::new("mcporter").arg("--version"),
                crate::cmd::DEFAULT_TIMEOUT,
            ) {
                if out.status.success() {
                    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !v.is_empty() {
                        entries.push(InventoryEntry {
                            id: "version".into(),
                            name: "Version".into(),
                            value: serde_json::Value::String(v),
                            value_type: InventoryValueType::String,
                        });
                    }
                }
            }
            entries
        })
    }
}
