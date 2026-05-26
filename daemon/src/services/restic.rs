use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command;

use crate::managed_service::{DataPath, ManagedService, ServiceMode, ShellCommandDef};
use crate::sentry_ext;
use mac_mgmt_common::BackupConfig;

const PKG: &str = "restic";

/// Default timeout for restic commands (5 minutes).
const RESTIC_CMD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

pub struct Restic {
    config: BackupConfig,
}

impl Restic {
    pub fn new(config: BackupConfig) -> Self {
        Self { config }
    }

    /// Resolve the password file path — configured or auto-generated.
    pub fn password_file_path(config: &BackupConfig) -> PathBuf {
        config
            .password_file
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| crate::config::config_dir().join("restic-key"))
    }

    /// Check if the restic repository has been initialized.
    fn repo_initialized(&self) -> bool {
        let password_file = Self::password_file_path(&self.config);
        if !password_file.exists() {
            return false;
        }
        let result = crate::cmd::output_with_timeout(
            Command::new("restic")
                .args(["cat", "config"])
                .args(["--repo", &self.config.repository])
                .args(["--password-file", &password_file.to_string_lossy()])
                .envs(&self.config.env),
            RESTIC_CMD_TIMEOUT,
        );
        matches!(result, Ok(out) if out.status.success())
    }
}

impl ManagedService for Restic {
    fn name(&self) -> &str {
        "restic"
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
            &[("service", "restic"), ("package", PKG)],
        );
        crate::nix::profile_install(PKG, false)?;
        Ok(())
    }

    fn ensure_setup(&self) -> Result<()> {
        let password_file = Self::password_file_path(&self.config);

        // Generate password file if it doesn't exist.
        if !password_file.exists() {
            tracing::info!(
                "generating restic password file at {}",
                password_file.display()
            );
            if let Some(parent) = password_file.parent() {
                std::fs::create_dir_all(parent)?;
            }
            // Generate 32 bytes of random hex as the key.
            let key: [u8; 32] = rand::random();
            let hex_key = hex::encode(key);
            std::fs::write(&password_file, &hex_key)
                .with_context(|| format!("failed to write {}", password_file.display()))?;
            // Restrict permissions.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&password_file, std::fs::Permissions::from_mode(0o600))?;
            }
            tracing::info!("restic password file created");
        }

        // Initialize repository if not already done.
        if !self.config.repository.is_empty() && !self.repo_initialized() {
            tracing::info!("initializing restic repository: {}", self.config.repository);
            sentry_ext::breadcrumb(
                "setup",
                "initializing restic repository",
                &[
                    ("service", "restic"),
                    ("repository", &self.config.repository),
                ],
            );
            let output = crate::cmd::output_with_timeout(
                Command::new("restic")
                    .args(["init"])
                    .args(["--repo", &self.config.repository])
                    .args(["--password-file", &password_file.to_string_lossy()])
                    .envs(&self.config.env),
                RESTIC_CMD_TIMEOUT,
            )
            .context("failed to run restic init")?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("restic init failed: {}", stderr.trim());
            }
            tracing::info!("restic repository initialized");
        }

        Ok(())
    }

    fn spawn_spec(&self) -> crate::managed_service::SpawnSpec {
        unreachable!("restic is install-only")
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
            &[("service", "restic"), ("package", PKG)],
        );
        crate::nix::profile_install(PKG, true)?;
        tracing::info!("{PKG} upgraded");
        Ok(true)
    }

    fn data_paths(&self, _home: &std::path::Path) -> Vec<DataPath> {
        vec![DataPath {
            name: "password-file",
            path: Self::password_file_path(&self.config),
            backup: false, // don't back up the backup key itself
        }]
    }

    fn expose_shell_commands(&self) -> Vec<ShellCommandDef> {
        let repo = self.config.repository.clone();
        let pw = Self::password_file_path(&self.config)
            .to_string_lossy()
            .into_owned();
        vec![
            ShellCommandDef {
                name: "restic-snapshots".into(),
                command: "restic".into(),
                args: vec![
                    "snapshots".into(),
                    "--repo".into(),
                    repo.clone(),
                    "--password-file".into(),
                    pw.clone(),
                    "--json".into(),
                ],
                description: "List backup snapshots".into(),
                arg_template: None,
                timeout_secs: Some(60),
            },
            ShellCommandDef {
                name: "restic-stats".into(),
                command: "restic".into(),
                args: vec![
                    "stats".into(),
                    "--repo".into(),
                    repo.clone(),
                    "--password-file".into(),
                    pw.clone(),
                    "--json".into(),
                ],
                description: "Show backup repository statistics".into(),
                arg_template: None,
                timeout_secs: Some(120),
            },
            ShellCommandDef {
                name: "restic-backup-now".into(),
                command: "restic".into(),
                args: vec![
                    "backup".into(),
                    "--files-from".into(),
                    crate::config::config_dir()
                        .join("restic-includes.txt")
                        .to_string_lossy()
                        .into_owned(),
                    "--repo".into(),
                    repo,
                    "--password-file".into(),
                    pw,
                    "--json".into(),
                ],
                description: "Run a backup now".into(),
                arg_template: None,
                timeout_secs: Some(3600),
            },
        ]
    }

    fn service_inventory(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Vec<mac_mgmt_common::InventoryEntry>> + Send + '_>,
    > {
        use mac_mgmt_common::{InventoryEntry, InventoryValueType};
        Box::pin(async {
            let mut entries = Vec::new();

            // Version
            if let Ok(out) = crate::cmd::output_with_timeout(
                std::process::Command::new("restic").arg("version"),
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

            // Repository
            if !self.config.repository.is_empty() {
                entries.push(InventoryEntry {
                    id: "repository".into(),
                    name: "Repository".into(),
                    value: serde_json::Value::String(self.config.repository.clone()),
                    value_type: InventoryValueType::String,
                });
            }

            entries
        })
    }
}
