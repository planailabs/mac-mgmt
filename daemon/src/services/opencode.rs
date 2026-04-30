use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::connectors::merge_json;
use crate::managed_service::{DataPath, FileTunnelDef, FileValidator, ManagedService, TunnelDef};
use crate::sentry_ext;
pub use mac_mgmt_common::OpencodeConfig;

/// Returns the path to ~/.config/opencode/config.json
pub fn config_path() -> Result<PathBuf> {
    Ok(dirs::home_dir()
        .context("HOME not set")?
        .join(".config/opencode/config.json"))
}

/// Atomically merge a JSON patch into the opencode config with JSON validation.
pub fn merge_and_validate(config_path: &Path, patch: &serde_json::Value) -> Result<()> {
    super::merge_json_config(config_path, patch, None)
}

pub struct Opencode {
    config: OpencodeConfig,
}

impl Opencode {
    pub fn new(config: OpencodeConfig) -> Self {
        Self { config }
    }

    /// Build and apply a config patch from the managed OpencodeConfig.
    fn apply_config_patch(&self) -> Result<()> {
        let path = config_path()?;

        let mut patch = serde_json::json!({});

        // Server configuration
        patch["server"] = serde_json::json!({
            "port": self.config.port,
            "hostname": self.config.host,
        });

        if let Some(extra) = &self.config.extra_config {
            merge_json(&mut patch, extra);
        }

        if patch.as_object().is_some_and(|o| !o.is_empty()) {
            match merge_and_validate(&path, &patch) {
                Ok(()) => tracing::info!("opencode config updated"),
                Err(e) => tracing::warn!("opencode config merge failed: {e}"),
            }
        }

        Ok(())
    }
}

impl ManagedService for Opencode {
    fn name(&self) -> &str {
        "opencode"
    }

    fn ensure_installed(&self) -> Result<()> {
        if crate::nix::is_installed("opencode")? {
            tracing::info!("opencode is already installed");
            return Ok(());
        }

        tracing::info!("opencode not found, installing via nix");
        sentry_ext::breadcrumb(
            "install",
            "installing opencode via nix",
            &[("service", "opencode")],
        );
        crate::nix::profile_install("opencode", false)?;
        Ok(())
    }

    fn ensure_setup(&self) -> Result<()> {
        let path = config_path()?;

        if !path.exists() {
            tracing::info!("opencode config not found, creating default");
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, "{}")?;
        }

        self.apply_config_patch()?;
        Ok(())
    }

    fn configure(&self) -> Result<()> {
        tracing::info!("opencode: re-applying config (hot reload)");
        self.apply_config_patch()?;
        Ok(())
    }

    fn supports_hot_reload(&self) -> bool {
        true
    }

    fn spawn_spec(&self) -> crate::managed_service::SpawnSpec {
        crate::managed_service::SpawnSpec {
            program: "opencode".into(),
            args: vec![
                "serve".into(),
                "--port".into(),
                self.config.port.to_string(),
                "--hostname".into(),
                self.config.host.clone(),
            ],
            env: Default::default(),
        }
    }

    fn check_health(&self) -> Result<bool> {
        // Simple HTTP check against the opencode server
        let url = format!("http://{}:{}/", self.config.host, self.config.port);
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()?;
        match client.get(&url).send() {
            Ok(resp) => Ok(resp.status().is_success() || resp.status().as_u16() == 404),
            Err(_) => Ok(false),
        }
    }

    fn repair(&self) -> Result<()> {
        tracing::info!("opencode repair: re-applying config");
        sentry_ext::breadcrumb(
            "repair",
            "re-applying opencode config",
            &[("service", "opencode")],
        );
        self.apply_config_patch()?;
        Ok(())
    }

    fn check_and_upgrade(&self) -> Result<bool> {
        let upgradable = crate::nix::packages_with_upgrades(&["opencode"])?;

        if !upgradable.iter().any(|name| name == "opencode") {
            return Ok(false);
        }

        tracing::info!("upgrading opencode via nix");
        sentry_ext::breadcrumb(
            "upgrade",
            "upgrading opencode via nix",
            &[("service", "opencode")],
        );
        crate::nix::profile_install("opencode", true)?;
        tracing::info!("opencode upgraded, restart pending until idle");
        Ok(true)
    }

    fn data_paths(&self, home: &std::path::Path) -> Vec<DataPath> {
        vec![
            DataPath {
                name: "config",
                path: home.join(".config/opencode/config.json"),
                backup: true,
            },
            DataPath {
                name: "data",
                path: home.join(".local/share/opencode"),
                backup: true,
            },
        ]
    }

    fn expose_tunnels(&self) -> Vec<TunnelDef> {
        let host = if self.config.host.is_empty() {
            "127.0.0.1".to_string()
        } else {
            self.config.host.clone()
        };
        let port = if self.config.port == 0 {
            18790
        } else {
            self.config.port
        };
        vec![TunnelDef {
            name: "opencode".into(),
            host,
            tcp_port: port,
        }]
    }

    fn expose_files(&self) -> Vec<FileTunnelDef> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/root"));
        vec![FileTunnelDef::Folder {
            name: "opencode-config".into(),
            path: home.join(".config/opencode").to_string_lossy().into(),
            writable: true,
            allow_write: Vec::new(),
            include: Some(vec!["config.json".into()]),
            validators: vec![FileValidator {
                glob: "*.json".into(),
                command: Vec::new(),
                builtin: Some("json".into()),
            }],
            description: "OpenCode configuration".into(),
        }]
    }

    fn service_inventory(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<mac_mgmt_common::InventoryEntry>> + Send + '_>> {
        use mac_mgmt_common::{InventoryEntry, InventoryValueType};
        Box::pin(async {
            let mut entries = Vec::new();
            if let Ok(out) = crate::cmd::output_with_timeout(
                std::process::Command::new("opencode").arg("version"),
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

    fn service_security(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<mac_mgmt_common::SecurityFinding>> + Send + '_>> {
        use mac_mgmt_common::{FindingSeverity, SecurityFinding};
        Box::pin(async move {
            let mut findings = Vec::new();

            // Check if the opencode server has a password configured.
            if let Ok(path) = config_path() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&content) {
                        let has_password = cfg
                            .get("server")
                            .and_then(|s| s.get("password"))
                            .and_then(|p| p.as_str())
                            .is_some_and(|p| !p.is_empty());

                        findings.push(SecurityFinding {
                            id: "opencode_server_password".into(),
                            severity: FindingSeverity::High,
                            message: if has_password {
                                "OpenCode server password is set".into()
                            } else {
                                "OpenCode server has no password — anyone with network access can use it".into()
                            },
                            pass: has_password,
                        });
                    }
                }
            }

            findings
        })
    }
}
