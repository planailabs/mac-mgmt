use anyhow::{Context, Result};
use std::process::Command;

use crate::managed_service::ManagedService;

pub struct OpenClaw;

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
        let config = std::path::PathBuf::from(home).join(".openclaw/openclaw.json");

        if config.exists() {
            tracing::info!("openclaw config found at {}", config.display());
            return Ok(());
        }

        tracing::info!("openclaw config not found, running openclaw setup");
        let status = Command::new("openclaw")
            .arg("setup")
            .status()
            .context("failed to run openclaw setup")?;

        if !status.success() {
            anyhow::bail!("openclaw setup exited with status {status}");
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
