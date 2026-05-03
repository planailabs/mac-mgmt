use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

use std::sync::LazyLock;

use crate::managed_service::{DataPath, FileTunnelDef, ManagedService, TunnelDef};
use crate::sentry_ext;
use crate::validator::{Validator, merge_json};
pub use mac_mgmt_common::OpenClawConfig;

/// Validator for openclaw JSON config files.
pub static VALIDATOR: LazyLock<Validator> = LazyLock::new(|| {
    Validator::json("openclaw.json").with_exec(vec![
        "openclaw".into(),
        "config".into(),
        "schema".into(),
    ])
});

/// Returns the path to ~/.openclaw/openclaw.json
pub fn config_path() -> Result<PathBuf> {
    Ok(dirs::home_dir()
        .context("HOME not set")?
        .join(".openclaw/openclaw.json"))
}

/// Atomically merge a JSON patch into openclaw.json with validation and rollback.
pub fn merge_and_validate(config_path: &Path, patch: &serde_json::Value) -> Result<()> {
    if !config_path.exists() {
        anyhow::bail!("openclaw config not found at {}", config_path.display());
    }
    VALIDATOR.merge_validate_and_write(config_path, patch)
}

/// The NODE_COMPILE_CACHE directory used for all openclaw invocations.
fn cache_dir() -> PathBuf {
    std::env::temp_dir().join("openclaw-cache")
}

/// Create a `Command` for openclaw with NODE_COMPILE_CACHE set.
fn openclaw_cmd() -> Command {
    let mut cmd = Command::new("openclaw");
    cmd.env("NODE_COMPILE_CACHE", cache_dir());
    cmd
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
        Self {
            config,
            active_sessions,
        }
    }

    /// Uninstall any preexisting openclaw daemon service so it doesn't race ours.
    fn uninstall_existing_daemon(phase: &str) {
        tracing::info!("uninstalling preexisting openclaw daemon service");
        sentry_ext::breadcrumb(
            phase,
            "running openclaw daemon uninstall",
            &[("service", "openclaw")],
        );
        match openclaw_cmd()
            .args(["daemon", "uninstall"])
            .output()
        {
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

        {
            let gw_cfg = if let Some(gw) = &self.config.gateway {
                let mut cfg = serde_json::json!({ "port": gw.port });
                if gw.host != "127.0.0.1" && gw.host != "localhost" {
                    cfg["bind"] = serde_json::json!("custom");
                    cfg["customBindHost"] = serde_json::json!(gw.host);
                }
                cfg
            } else {
                serde_json::json!({})
            };
            patch["gateway"] = gw_cfg;
            // Enable OpenAI-compatible HTTP chat completions endpoint
            // for probing and external integrations.
            patch["gateway"]["http"] = serde_json::json!({
                "endpoints": {
                    "chatCompletions": { "enabled": true }
                }
            });
        }

        if let Some(skills) = &self.config.skills {
            patch["skills"] = serde_json::json!({ "load": { "watch": skills.auto_update } });
        }

        if let Some(tg) = &self.config.telegram {
            let mut tg_cfg = serde_json::json!({
                "enabled": tg.enabled,
                "dmPolicy": "pairing",
                "groupPolicy": "allowlist",
            });
            if !tg.bot_token.is_empty() {
                tg_cfg["botToken"] = serde_json::json!(tg.bot_token.expose());
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
        let output = openclaw_cmd()
            .args(["sessions", "--active", "1", "--json"])
            .output();
        let Ok(output) = output else { return 0 };
        if !output.status.success() {
            return 0;
        }
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
        let output = openclaw_cmd().args(["gateway", "stop"]).output();
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
        sentry_ext::breadcrumb(
            "install",
            "installing openclaw via nix",
            &[("service", "openclaw")],
        );
        crate::nix::profile_install("openclaw", false)?;
        Ok(())
    }

    fn ensure_setup(&self) -> Result<()> {
        let config_path = dirs::home_dir()
            .context("HOME not set")?
            .join(".openclaw/openclaw.json");

        if !config_path.exists() {
            tracing::info!("openclaw config not found, running openclaw setup");
            sentry_ext::breadcrumb(
                "setup",
                "running openclaw setup",
                &[("service", "openclaw")],
            );
            let output = openclaw_cmd()
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

    fn spawn_spec(&self) -> crate::managed_service::SpawnSpec {
        let dir = cache_dir();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::warn!("failed to create NODE_COMPILE_CACHE dir: {e}");
        }

        let mut env = std::collections::HashMap::new();
        env.insert(
            "NODE_COMPILE_CACHE".into(),
            dir.to_string_lossy().into_owned(),
        );
        env.insert("OPENCLAW_NO_RESPAWN".into(), "1".into());

        crate::managed_service::SpawnSpec {
            program: "openclaw".into(),
            args: vec!["gateway".into()],
            env,
        }
    }

    fn check_health(&self) -> Result<bool> {
        let output = crate::cmd::output_with_timeout(
            openclaw_cmd().args(["health", "--json"]),
            crate::cmd::DEFAULT_TIMEOUT,
        )
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
        sentry_ext::breadcrumb(
            "repair",
            "running openclaw doctor --fix",
            &[("service", "openclaw")],
        );

        let output = crate::cmd::output_with_timeout(
            openclaw_cmd().args(["doctor", "--fix"]),
            std::time::Duration::from_secs(60),
        )
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
        sentry_ext::breadcrumb(
            "upgrade",
            "upgrading openclaw via nix",
            &[("service", "openclaw")],
        );
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
        self.active_sessions
            .set(Self::active_session_count() as i64);
    }

    fn data_paths(&self, home: &std::path::Path) -> Vec<DataPath> {
        vec![
            DataPath {
                name: "config",
                path: home.join(".openclaw/openclaw.json"),
                backup: true,
            },
            DataPath {
                name: "data",
                path: home.join(".openclaw"),
                backup: true,
            },
        ]
    }

    fn expose_tunnels(&self) -> Vec<TunnelDef> {
        let gw = self.config.gateway.as_ref().cloned().unwrap_or_default();
        let host = if gw.host.is_empty() {
            "127.0.0.1".to_string()
        } else {
            gw.host
        };
        let port = if gw.port == 0 { 18789 } else { gw.port };
        vec![TunnelDef {
            name: "openclaw".into(),
            host,
            tcp_port: port,
        }]
    }

    fn expose_shell_commands(&self) -> Vec<crate::managed_service::ShellCommandDef> {
        use crate::managed_service::ShellCommandDef;
        vec![
            ShellCommandDef {
                name: "openclaw-health".into(),
                command: "openclaw".into(),
                args: vec!["health".into(), "--json".into()],
                description: "Check gateway health".into(),
                arg_template: None,
                timeout_secs: None,
            },
            ShellCommandDef {
                name: "openclaw-config-validate".into(),
                command: "openclaw".into(),
                args: vec!["config".into(), "validate".into()],
                description: "Validate configuration".into(),
                arg_template: None,
                timeout_secs: None,
            },
            ShellCommandDef {
                name: "openclaw-doctor".into(),
                command: "openclaw".into(),
                args: vec!["doctor".into(), "--fix".into()],
                description: "Run diagnostics and auto-fix".into(),
                arg_template: None,
                timeout_secs: None,
            },
        ]
    }

    fn expose_files(&self) -> Vec<FileTunnelDef> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/root"));
        let mut files = vec![FileTunnelDef::Folder {
            name: "openclaw-config".into(),
            path: home.join(".openclaw").to_string_lossy().into(),
            writable: true,
            allow_write: Vec::new(),
            include: Some(vec!["openclaw.json".into()]),
            validators: vec![VALIDATOR.clone()],
            description: "OpenClaw gateway configuration".into(),
        }];
        let skills_dir = home.join(".plan-ai-skills");
        if skills_dir.exists() {
            files.push(FileTunnelDef::Folder {
                name: "openclaw-skills".into(),
                path: skills_dir.to_string_lossy().into(),
                writable: false,
                allow_write: Vec::new(),
                include: None,
                validators: Vec::new(),
                description: "Plan.ai skills directory".into(),
            });
        }
        files
    }

    fn service_inventory(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<mac_mgmt_common::InventoryEntry>> + Send + '_>> {
        use mac_mgmt_common::{InventoryEntry, InventoryValueType};
        Box::pin(async move {
            let mut entries = Vec::new();
            if let Ok(out) = crate::cmd::output_with_timeout(
                openclaw_cmd().arg("--version"),
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

    fn service_sample(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<mac_mgmt_common::InventoryEntry>> + Send + '_>> {
        use mac_mgmt_common::{InventoryEntry, InventoryValueType};
        Box::pin(async move {
            let count = Self::active_session_count();
            vec![InventoryEntry {
                id: "active_sessions".into(),
                name: "Active Sessions".into(),
                value: serde_json::json!(count),
                value_type: InventoryValueType::Number,
            }]
        })
    }

    fn service_security(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<mac_mgmt_common::SecurityFinding>> + Send + '_>> {
        use mac_mgmt_common::{FindingSeverity, SecurityFinding};
        Box::pin(async move {
            let mut findings = Vec::new();

            // Check auth config from ~/.openclaw/openclaw.json
            let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/root"));
            let config_path = home.join(".openclaw/openclaw.json");
            if let Ok(contents) = std::fs::read_to_string(&config_path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&contents) {
                    // Check for auth: none
                    let auth = json.pointer("/gateway/auth")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let auth_none = auth.eq_ignore_ascii_case("none");
                    findings.push(SecurityFinding {
                        id: "openclaw_auth_none".into(),
                        severity: FindingSeverity::High,
                        message: if auth_none {
                            "Gateway authentication disabled (auth: none)".into()
                        } else {
                            format!("Gateway authentication enabled (auth: {auth})")
                        },
                        pass: !auth_none,
                    });

                    // Check for gateway token presence
                    let has_token = json.pointer("/gateway/auth/token")
                        .and_then(|v| v.as_str())
                        .is_some_and(|t| !t.is_empty());
                    if !auth_none {
                        findings.push(SecurityFinding {
                            id: "openclaw_gateway_token".into(),
                            severity: FindingSeverity::Medium,
                            message: if has_token {
                                "Gateway auth token is configured".into()
                            } else {
                                "Gateway auth token is missing".into()
                            },
                            pass: has_token,
                        });
                    }
                }
            }

            findings
        })
    }
}
