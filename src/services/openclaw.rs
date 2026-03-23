use anyhow::{Context, Result};
use std::process::Command;

use crate::config::OpenClawConfig;
use crate::managed_service::ManagedService;

pub struct OpenClaw {
    config: OpenClawConfig,
}

impl OpenClaw {
    pub fn new(config: OpenClawConfig) -> Self {
        Self { config }
    }
}

impl ManagedService for OpenClaw {
    fn name(&self) -> &str {
        "openclaw"
    }

    fn ensure_installed(&self) -> Result<()> {
        if crate::nix::is_installed("openclaw")? {
            tracing::info!("openclaw is already installed");
            return Ok(());
        }

        tracing::info!("openclaw not found, installing via nix");
        crate::nix::profile_install("nixpkgs#openclaw", false)?;
        Ok(())
    }

    fn ensure_setup(&self) -> Result<()> {
        let home = std::env::var("HOME").context("HOME not set")?;
        let config_path = std::path::PathBuf::from(&home).join(".openclaw/openclaw.json");

        if !config_path.exists() {
            tracing::info!("openclaw config not found, running openclaw setup");
            let status = Command::new("openclaw")
                .arg("setup")
                .status()
                .context("failed to run openclaw setup")?;

            if !status.success() {
                anyhow::bail!("openclaw setup exited with status {status}");
            }
        } else {
            tracing::info!("openclaw config found at {}", config_path.display());
        }

        // Merge extra_config into openclaw.json if configured
        if let Some(extra) = &self.config.extra_config {
            tracing::info!("merging extra_config into {}", config_path.display());
            let contents = std::fs::read_to_string(&config_path)
                .with_context(|| format!("failed to read {}", config_path.display()))?;
            let mut existing: serde_json::Value =
                serde_json::from_str(&contents).context("failed to parse openclaw.json")?;

            merge_json(&mut existing, extra);

            let merged = serde_json::to_string_pretty(&existing)
                .context("failed to serialize merged config")?;
            std::fs::write(&config_path, merged)
                .with_context(|| format!("failed to write {}", config_path.display()))?;
            tracing::info!("extra_config merged into openclaw.json");
        }

        Ok(())
    }

    fn spawn(&self) -> Result<std::process::Child> {
        let child = Command::new("openclaw")
            .arg("gateway")
            .spawn()
            .context("failed to start openclaw gateway")?;
        tracing::info!("openclaw gateway started (pid: {})", child.id());
        Ok(child)
    }

    fn check_health(&self) -> Result<bool> {
        let output = Command::new("openclaw")
            .args(["health", "--json"])
            .output()
            .context("failed to run openclaw health")?;

        if !output.status.success() {
            tracing::warn!(
                "openclaw health exited with status {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            );
            return Ok(false);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        tracing::debug!("openclaw health output: {stdout}");
        Ok(true)
    }

    fn repair(&self) -> Result<()> {
        tracing::info!("running openclaw doctor --fix");

        let output = Command::new("openclaw")
            .args(["doctor", "--fix"])
            .output()
            .context("failed to run openclaw doctor --fix")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            tracing::info!("openclaw doctor --fix completed successfully");
        } else {
            tracing::warn!(
                "openclaw doctor --fix exited with status {}: {}",
                output.status,
                stderr.trim()
            );
        }

        if !stdout.is_empty() {
            tracing::info!("doctor output: {}", stdout.trim());
        }

        Ok(())
    }

    fn check_and_upgrade(&self) -> Result<bool> {
        let upgradable = crate::nix::packages_with_upgrades()?;

        if !upgradable.iter().any(|name| name == "openclaw") {
            return Ok(false);
        }

        tracing::info!("upgrading openclaw via nix");
        crate::nix::profile_install("nixpkgs#openclaw", true)?;
        tracing::info!("openclaw upgraded, restart pending until idle");
        Ok(true)
    }

    fn is_busy(&self) -> Result<bool> {
        let output = Command::new("openclaw")
            .args(["sessions", "--active", "1", "--json"])
            .output()
            .context("failed to run openclaw sessions")?;

        if !output.status.success() {
            tracing::warn!(
                "openclaw sessions exited with status {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            );
            return Ok(true);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value =
            serde_json::from_str(&stdout).context("failed to parse sessions json")?;

        let busy = match &json {
            serde_json::Value::Array(arr) => !arr.is_empty(),
            serde_json::Value::Object(obj) => !obj.is_empty(),
            _ => false,
        };

        if busy {
            tracing::info!("openclaw is currently busy");
        } else {
            tracing::debug!("openclaw is idle");
        }

        Ok(busy)
    }
}

/// Recursively merge `source` into `target`. For objects, keys from source
/// are merged into target. For all other types, source overwrites target.
fn merge_json(target: &mut serde_json::Value, source: &serde_json::Value) {
    match (target, source) {
        (serde_json::Value::Object(target), serde_json::Value::Object(source)) => {
            for (key, value) in source {
                merge_json(target.entry(key.clone()).or_insert(serde_json::Value::Null), value);
            }
        }
        (target, source) => {
            *target = source.clone();
        }
    }
}
