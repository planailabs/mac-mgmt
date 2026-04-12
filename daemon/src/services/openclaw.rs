use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::connectors::merge_json;
use crate::managed_service::{ManagedService, TunnelDef};
use crate::sentry_ext;
pub use mac_mgmt_common::OpenClawConfig;

/// Returns the path to ~/.openclaw/openclaw.json
pub fn config_path() -> Result<PathBuf> {
    Ok(dirs::home_dir()
        .context("HOME not set")?
        .join(".openclaw/openclaw.json"))
}

/// Atomically merge a JSON patch into openclaw.json with validation and rollback.
///
/// 1. Reads current config
/// 2. Deep-merges the patch
/// 3. Writes the result
/// 4. Runs `openclaw config validate`
/// 5. If invalid: restores the original and returns Err
pub fn merge_and_validate(config_path: &Path, patch: &serde_json::Value) -> Result<()> {
    if !config_path.exists() {
        anyhow::bail!("openclaw config not found at {}", config_path.display());
    }

    let backup = std::fs::read_to_string(config_path)
        .with_context(|| format!("failed to read {}", config_path.display()))?;
    let mut existing: serde_json::Value =
        serde_json::from_str(&backup).context("failed to parse openclaw.json")?;

    merge_json(&mut existing, patch);

    let merged = serde_json::to_string_pretty(&existing)
        .context("failed to serialize merged config")?;
    std::fs::write(config_path, &merged)
        .with_context(|| format!("failed to write {}", config_path.display()))?;

    // Validate
    let valid = match Command::new("openclaw").args(["config", "validate"]).output() {
        Ok(output) if output.status.success() => true,
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            tracing::warn!("openclaw config invalid: {} {}", stdout.trim(), stderr.trim());
            false
        }
        Err(e) => {
            tracing::warn!("failed to run openclaw config validate: {e}");
            true // Can't validate — don't block
        }
    };

    if !valid {
        tracing::warn!("rolling back openclaw.json");
        std::fs::write(config_path, &backup)
            .with_context(|| format!("failed to rollback {}", config_path.display()))?;
        anyhow::bail!("openclaw config validation failed, rolled back");
    }

    Ok(())
}

pub struct OpenClaw {
    config: OpenClawConfig,
    active_sessions: prometheus::IntGauge,
}

impl OpenClaw {
    pub fn new(config: OpenClawConfig) -> Self {
        let active_sessions = prometheus::IntGauge::new(
            "mac_mgmt_openclaw_active_sessions",
            "Number of active openclaw sessions",
        )
        .unwrap();
        Self { config, active_sessions }
    }

    /// Uninstall any preexisting openclaw daemon service so it doesn't race ours.
    fn uninstall_existing_daemon(phase: &str) {
        tracing::info!("uninstalling preexisting openclaw daemon service");
        sentry_ext::breadcrumb(
            phase,
            "running openclaw daemon uninstall",
            &[("service", "openclaw")],
        );
        match Command::new("openclaw").args(["daemon", "uninstall"]).output() {
            Ok(o) if o.status.success() => {
                tracing::info!("openclaw daemon uninstall completed");
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                tracing::debug!(
                    "openclaw daemon uninstall exited with status {}: {}",
                    o.status,
                    stderr.trim()
                );
            }
            Err(e) => {
                tracing::debug!("openclaw daemon uninstall not available: {e}");
            }
        }
    }

    /// Build and apply a config patch from the managed OpenClawConfig.
    fn apply_config_patch(&self) -> Result<()> {
        let config_path = dirs::home_dir()
            .context("HOME not set")?
            .join(".openclaw/openclaw.json");

        let mut patch = serde_json::json!({});

        if let Some(gw) = &self.config.gateway {
            let mut gw_cfg = serde_json::json!({ "port": gw.port });
            if gw.host != "127.0.0.1" && gw.host != "localhost" {
                gw_cfg["bind"] = serde_json::json!("custom");
                gw_cfg["customBindHost"] = serde_json::json!(gw.host);
            }
            patch["gateway"] = gw_cfg;
        }

        if let Some(skills) = &self.config.skills {
            patch["skills"] = serde_json::json!({ "load": { "watch": skills.auto_update } });
        }

        if let Some(tg) = &self.config.telegram {
            let mut tg_cfg = serde_json::json!({ "enabled": tg.enabled });
            if !tg.bot_token.is_empty() {
                tg_cfg["botToken"] = serde_json::json!(tg.bot_token);
            }
            if !tg.allowed_chat_ids.is_empty() {
                tg_cfg["allowFrom"] = serde_json::json!(tg.allowed_chat_ids);
            }
            patch["channels"] = serde_json::json!({ "telegram": tg_cfg });
        }

        if let Some(extra) = &self.config.extra_config {
            merge_json(&mut patch, extra);
        }

        // Add skills directory
        let skills_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/root"))
            .join(".plan-ai-skills");
        if skills_dir.exists() {
            let skills_dir_str = skills_dir.to_string_lossy().to_string();
            let current = std::fs::read_to_string(&config_path).unwrap_or_default();
            let current_json: serde_json::Value =
                serde_json::from_str(&current).unwrap_or_default();
            let already_has = current_json
                .pointer("/skills/load/extraDirs")
                .and_then(|v| v.as_array())
                .is_some_and(|arr| arr.iter().any(|v| v.as_str() == Some(&skills_dir_str)));
            if !already_has {
                patch["skills"] = serde_json::json!({ "load": { "extraDirs": [skills_dir_str] } });
            }
        }

        if patch.as_object().is_some_and(|o| !o.is_empty()) {
            match merge_and_validate(&config_path, &patch) {
                Ok(()) => tracing::info!("openclaw config updated and validated"),
                Err(e) => tracing::warn!("openclaw config merge failed: {e}"),
            }
        }

        Ok(())
    }

    fn active_session_count() -> usize {
        let output = Command::new("openclaw")
            .args(["sessions", "--active", "1", "--json"])
            .output();
        let Ok(output) = output else { return 0 };
        if !output.status.success() { return 0; }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value = match serde_json::from_str(&stdout) {
            Ok(v) => v,
            Err(_) => return 0,
        };
        match &json {
            serde_json::Value::Array(arr) => arr.len(),
            serde_json::Value::Object(obj) => obj.len(),
            _ => 0,
        }
    }

    /// Stop any existing openclaw gateway so we don't conflict on ports.
    fn stop_existing_gateway(phase: &str) {
        let output = Command::new("openclaw").args(["gateway", "stop"]).output();
        match output {
            Ok(o) if o.status.success() => {
                tracing::info!("stopped existing openclaw gateway");
                sentry_ext::breadcrumb(
                    phase,
                    "stopped existing openclaw gateway",
                    &[("service", "openclaw")],
                );
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                tracing::debug!("openclaw gateway stop: {}", stderr.trim());
            }
            Err(e) => {
                tracing::debug!("openclaw gateway stop not available: {e}");
            }
        }
    }
}

impl ManagedService for OpenClaw {
    fn name(&self) -> &str {
        "openclaw"
    }

    fn preflight(&self) -> Result<()> {
        Self::stop_existing_gateway("preflight");
        Self::uninstall_existing_daemon("preflight");
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

        self.apply_config_patch()?;
        Ok(())
    }

    fn configure(&self) -> Result<()> {
        tracing::info!("openclaw: re-applying config (hot reload)");
        self.apply_config_patch()?;
        Ok(())
    }

    fn supports_hot_reload(&self) -> bool {
        true
    }

    fn spawn_spec(&self) -> crate::service_ipc::protocol::SpawnSpec {
        crate::service_ipc::protocol::SpawnSpec {
            program: "openclaw".into(),
            args: vec!["gateway".into()],
            env: Default::default(),
        }
    }

    fn spawn(&self) -> Result<std::process::Child> {
        let spec = self.spawn_spec();
        let child = Command::new(&spec.program)
            .args(&spec.args)
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
        Self::stop_existing_gateway("repair");
        Self::uninstall_existing_daemon("repair");

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
        let upgradable = crate::nix::packages_with_upgrades(&["openclaw"])?;

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
        let count = Self::active_session_count();
        if count > 0 {
            tracing::info!("openclaw has {count} active session(s)");
        } else {
            tracing::debug!("openclaw is idle");
        }
        Ok(count > 0)
    }

    fn metric_collectors(&self) -> Vec<Box<dyn prometheus::core::Collector>> {
        vec![Box::new(self.active_sessions.clone())]
    }

    fn collect_metrics(&self) {
        self.active_sessions.set(Self::active_session_count() as i64);
    }

    fn expose_tunnels(&self) -> Vec<TunnelDef> {
        let gw = self.config.gateway.as_ref().cloned().unwrap_or_default();
        let host = if gw.host.is_empty() { "127.0.0.1".to_string() } else { gw.host };
        let port = if gw.port == 0 { 18789 } else { gw.port };
        vec![TunnelDef {
            name: "openclaw".into(),
            host,
            tcp_port: port,
        }]
    }
}
