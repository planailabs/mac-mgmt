use anyhow::{Context, Result};
use std::process::Command;

use crate::managed_service::ManagedService;
use crate::sentry_ext;
pub use mac_mgmt_common::OpenClawConfig;

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

    fn preflight(&self) -> Result<()> {
        // Stop any existing openclaw gateway so we don't conflict on ports.
        let output = Command::new("openclaw")
            .args(["gateway", "stop"])
            .output();
        match output {
            Ok(o) if o.status.success() => {
                tracing::info!("stopped existing openclaw gateway");
                sentry_ext::breadcrumb("preflight", "stopped existing openclaw gateway", &[("service", "openclaw")]);
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                tracing::debug!("openclaw gateway stop: {}", stderr.trim());
            }
            Err(e) => {
                tracing::debug!("openclaw gateway stop not available: {e}");
            }
        }
        Ok(())
    }

    fn ensure_installed(&self) -> Result<()> {
        if crate::nix::is_installed("openclaw")? {
            tracing::info!("openclaw is already installed");
            return Ok(());
        }

        tracing::info!("openclaw not found, installing via nix");
        sentry_ext::breadcrumb("install", "installing openclaw via nix", &[("service", "openclaw")]);
        crate::nix::profile_install("openclaw", false)?;
        Ok(())
    }

    fn ensure_setup(&self) -> Result<()> {
        let config_path = dirs::home_dir()
            .context("HOME not set")?
            .join(".openclaw/openclaw.json");

        if !config_path.exists() {
            tracing::info!("openclaw config not found, running openclaw setup");
            sentry_ext::breadcrumb("setup", "running openclaw setup", &[("service", "openclaw")]);
            let output = Command::new("openclaw")
                .arg("setup")
                .output()
                .context("failed to run openclaw setup")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                sentry_ext::capture_cmd_failure(
                    "openclaw setup",
                    output.status.code(),
                    stderr.trim(),
                );
                anyhow::bail!("openclaw setup exited with status {}", output.status);
            }
        } else {
            tracing::info!("openclaw config found at {}", config_path.display());
        }

        // Merge managed config into openclaw.json
        let contents = std::fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read {}", config_path.display()))?;
        let mut existing: serde_json::Value =
            serde_json::from_str(&contents).context("failed to parse openclaw.json")?;
        let mut changed = false;

        // Apply typed gateway fields
        if let Some(gw) = &self.config.gateway {
            let patch = serde_json::json!({
                "gateway": {
                    "port": gw.port,
                    "host": gw.host,
                }
            });
            merge_json(&mut existing, &patch);
            changed = true;
        }

        // Apply typed skills fields
        if let Some(skills) = &self.config.skills {
            let patch = serde_json::json!({
                "skills": {
                    "autoUpdate": skills.auto_update,
                }
            });
            merge_json(&mut existing, &patch);
            changed = true;
        }

        // Apply typed telegram fields under integrations.telegram
        if let Some(tg) = &self.config.telegram {
            let mut tg_cfg = serde_json::json!({
                "botToken": tg.bot_token,
                "enabled": tg.enabled,
            });
            if !tg.allowed_chat_ids.is_empty() {
                tg_cfg["allowedChatIds"] = serde_json::json!(tg.allowed_chat_ids);
            }
            let integrations = serde_json::json!({
                "integrations": {
                    "telegram": tg_cfg,
                }
            });
            merge_json(&mut existing, &integrations);
            changed = true;
        }

        // Merge extra_config if configured (applied AFTER typed fields)
        if let Some(extra) = &self.config.extra_config {
            tracing::info!("merging extra_config into openclaw.json");
            merge_json(&mut existing, extra);
            changed = true;
        }

        // Add skills directory to skills.load.extraDirs
        let skills_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("/root"))
            .join(".plan-ai-skills");
        if skills_dir.exists() {
            let skills_dir_str = skills_dir.to_string_lossy().to_string();
            let extra_dirs = existing
                .pointer_mut("/skills/load/extraDirs")
                .and_then(|v| v.as_array_mut());

            match extra_dirs {
                Some(arr) => {
                    if !arr.iter().any(|v| v.as_str() == Some(&skills_dir_str)) {
                        arr.push(serde_json::Value::String(skills_dir_str));
                        changed = true;
                    }
                }
                None => {
                    let patch: serde_json::Value = serde_json::json!({
                        "skills": {
                            "load": {
                                "extraDirs": [skills_dir_str]
                            }
                        }
                    });
                    merge_json(&mut existing, &patch);
                    changed = true;
                }
            }
        }

        if changed {
            let merged = serde_json::to_string_pretty(&existing)
                .context("failed to serialize merged config")?;
            std::fs::write(&config_path, &merged)
                .with_context(|| format!("failed to write {}", config_path.display()))?;
            tracing::info!("config merged into openclaw.json");

            // Validate the merged config; if openclaw rejects it, run doctor --fix
            // to remove unrecognized keys so we don't cause a crash loop.
            let doctor = Command::new("openclaw")
                .arg("doctor")
                .output()
                .context("failed to run openclaw doctor")?;
            if !doctor.status.success() {
                let stderr = String::from_utf8_lossy(&doctor.stderr);
                tracing::warn!("openclaw config invalid after merge, running doctor --fix: {}", stderr.trim());
                let fix = Command::new("openclaw")
                    .args(["doctor", "--fix"])
                    .output()
                    .context("failed to run openclaw doctor --fix")?;
                if fix.status.success() {
                    tracing::info!("openclaw doctor --fix corrected the config");
                } else {
                    let fix_stderr = String::from_utf8_lossy(&fix.stderr);
                    tracing::error!("openclaw doctor --fix failed: {}", fix_stderr.trim());
                }
            } else {
                tracing::info!("config merged and validated successfully");
            }
        }

        Ok(())
    }

    fn spawn(&self) -> Result<std::process::Child> {
        let child = Command::new("openclaw")
            .arg("gateway")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("failed to start openclaw gateway")?;
        tracing::info!("openclaw gateway started (pid: {})", child.id());
        sentry_ext::breadcrumb("spawn", "openclaw gateway started", &[
            ("service", "openclaw"),
            ("pid", &child.id().to_string()),
        ]);
        Ok(child)
    }

    fn check_health(&self) -> Result<bool> {
        let output = Command::new("openclaw")
            .args(["health", "--json"])
            .output()
            .context("failed to run openclaw health")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!(
                "openclaw health exited with status {}: {}",
                output.status,
                stderr.trim()
            );
            sentry_ext::capture_cmd_failure(
                "openclaw health --json",
                output.status.code(),
                stderr.trim(),
            );
            return Ok(false);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        tracing::debug!("openclaw health output: {stdout}");
        Ok(true)
    }

    fn repair(&self) -> Result<()> {
        tracing::info!("running openclaw doctor --fix");
        sentry_ext::breadcrumb("repair", "running openclaw doctor --fix", &[("service", "openclaw")]);

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
            sentry_ext::capture_cmd_failure(
                "openclaw doctor --fix",
                output.status.code(),
                stderr.trim(),
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
        sentry_ext::breadcrumb("upgrade", "upgrading openclaw via nix", &[("service", "openclaw")]);
        crate::nix::profile_install("openclaw", true)?;
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

use crate::connectors::merge_json;
