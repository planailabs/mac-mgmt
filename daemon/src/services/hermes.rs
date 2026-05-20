use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use std::sync::LazyLock;

use crate::managed_service::{DataPath, FileTunnelDef, ManagedService, TunnelDef};
use crate::sentry_ext;
use crate::validator::Validator;
pub use mac_mgmt_common::HermesConfig;

/// Validator for hermes YAML config files (syntax only, no schema command).
pub static VALIDATOR: LazyLock<Validator> = LazyLock::new(|| Validator::yaml("config.yaml"));

/// Returns ~/.hermes
pub fn hermes_home() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/root"))
        .join(".hermes")
}

/// Returns the path to ~/.hermes/config.yaml
pub fn config_path() -> Result<PathBuf> {
    Ok(dirs::home_dir()
        .context("HOME not set")?
        .join(".hermes/config.yaml"))
}

/// Returns the gateway API port from .env (API_SERVER_PORT) or the default 8642.
pub fn gateway_api_port() -> u16 {
    read_env_var("API_SERVER_PORT")
        .and_then(|p| p.parse().ok())
        .unwrap_or(8642)
}

/// Returns the path to ~/.hermes/.env
pub fn env_path() -> Result<PathBuf> {
    Ok(dirs::home_dir()
        .context("HOME not set")?
        .join(".hermes/.env"))
}

/// Atomically merge a JSON-compatible patch into config.yaml with validation and rollback.
pub fn merge_and_validate(config_path: &Path, patch: &serde_json::Value) -> Result<()> {
    if !config_path.exists() {
        anyhow::bail!("hermes config not found at {}", config_path.display());
    }
    VALIDATOR.merge_validate_and_write(config_path, patch)
}

/// Read a value from ~/.hermes/.env by key.
pub fn read_env_var(key: &str) -> Option<String> {
    let path = env_path().ok()?;
    let contents = std::fs::read_to_string(&path).ok()?;
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix(key) {
            if let Some(val) = rest.strip_prefix('=') {
                return Some(val.to_string());
            }
        }
    }
    None
}

/// Write (or upsert) a key=value pair in ~/.hermes/.env.
fn write_env_var(key: &str, val: &str) -> Result<()> {
    let path = env_path()?;
    let contents = std::fs::read_to_string(&path).unwrap_or_default();
    let needle = format!("{key}=");
    let new_line = format!("{key}={val}");

    let mut found = false;
    let mut lines: Vec<String> = contents
        .lines()
        .map(|line| {
            if line.starts_with(&needle) {
                found = true;
                new_line.clone()
            } else {
                line.to_string()
            }
        })
        .collect();
    if !found {
        lines.push(new_line);
    }

    let output = lines.join("\n") + "\n";
    std::fs::write(&path, output).context("failed to write ~/.hermes/.env")?;
    Ok(())
}

pub struct Hermes {
    config: HermesConfig,
}

impl Hermes {
    pub fn new(config: HermesConfig) -> Self {
        Self { config }
    }

    /// Stop any existing hermes gateway so we don't conflict on ports.
    fn stop_existing_gateway(phase: &str) {
        let output = std::process::Command::new("hermes")
            .args(["gateway", "stop"])
            .output();
        match output {
            Ok(o) if o.status.success() => {
                tracing::info!("stopped existing hermes gateway");
                sentry_ext::breadcrumb(
                    phase,
                    "stopped existing hermes gateway",
                    &[("service", "hermes")],
                );
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                tracing::debug!("hermes gateway stop: {}", stderr.trim());
            }
            Err(e) => {
                tracing::debug!("hermes gateway stop not available: {e}");
            }
        }
    }

    /// Build and apply a config patch from the managed HermesConfig.
    fn apply_config_patch(&self) -> Result<()> {
        let cfg_path = config_path()?;
        let env = env_path()?;

        // Ensure .env exists
        if !env.exists() {
            if let Some(parent) = env.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&env, "")?;
        }

        // Write gateway port to .env (hermes uses API_SERVER_PORT env var)
        if let Some(gw) = &self.config.gateway {
            if gw.port != 8642 {
                write_env_var("API_SERVER_PORT", &gw.port.to_string())?;
            }
            if gw.host != "127.0.0.1" && !gw.host.is_empty() {
                write_env_var("API_SERVER_HOST", &gw.host)?;
            }
        }

        // Write telegram config to .env
        if let Some(tg) = &self.config.telegram {
            if tg.enabled && !tg.bot_token.expose().is_empty() {
                write_env_var("TELEGRAM_BOT_TOKEN", tg.bot_token.expose())?;
            }
        }

        // Write extra_env vars
        if let Some(extra_env) = &self.config.extra_env {
            for (k, v) in extra_env {
                write_env_var(k, v.expose())?;
            }
        }

        // Merge extra_config into config.yaml
        if let Some(extra) = &self.config.extra_config {
            if extra.as_object().is_some_and(|o| !o.is_empty()) {
                match merge_and_validate(&cfg_path, extra) {
                    Ok(()) => tracing::info!("hermes config.yaml updated and validated"),
                    Err(e) => tracing::warn!("hermes config.yaml merge failed: {e}"),
                }
            }
        }

        Ok(())
    }

    /// Ensure API_SERVER_KEY is set in .env, generating if missing.
    fn ensure_api_server_key(&self) -> Result<String> {
        if let Some(key) = read_env_var("API_SERVER_KEY") {
            if !key.is_empty() {
                return Ok(key);
            }
        }
        let key = uuid::Uuid::new_v4().to_string();
        write_env_var("API_SERVER_KEY", &key)?;
        tracing::info!("generated API_SERVER_KEY for hermes");
        Ok(key)
    }

    fn gateway_host(&self) -> String {
        self.config
            .gateway
            .as_ref()
            .map(|g| {
                if g.host.is_empty() {
                    "127.0.0.1".to_string()
                } else {
                    g.host.clone()
                }
            })
            .unwrap_or_else(|| "127.0.0.1".to_string())
    }

    fn gateway_port(&self) -> u16 {
        self.config
            .gateway
            .as_ref()
            .map(|g| if g.port == 0 { 8642 } else { g.port })
            .unwrap_or(8642)
    }
}

impl ManagedService for Hermes {
    fn name(&self) -> &str {
        "hermes"
    }

    fn binary_name(&self) -> &str {
        "hermes"
    }

    fn preflight(&self) -> Result<()> {
        Self::stop_existing_gateway("preflight");
        Ok(())
    }

    fn ensure_installed(&self) -> Result<()> {
        if crate::nix::is_installed("hermes-agent")? {
            tracing::info!("hermes-agent is already installed");
            return Ok(());
        }

        tracing::info!("hermes-agent not found, installing via nix");
        sentry_ext::breadcrumb(
            "install",
            "installing hermes-agent via nix",
            &[("service", "hermes")],
        );
        crate::nix::profile_install("hermes-agent", false)?;
        Ok(())
    }

    fn ensure_setup(&self) -> Result<()> {
        let hh = hermes_home();
        let cfg_path = hh.join("config.yaml");

        // Create ~/.hermes directory and expected subdirectories.
        // Matches the NixOS module's native-mode directory structure.
        for subdir in &["", "cron", "sessions", "logs", "memories", "plugins"] {
            let dir = hh.join(subdir);
            if !dir.exists() {
                std::fs::create_dir_all(&dir)
                    .with_context(|| format!("failed to create {}", dir.display()))?;
            }
        }

        // Create workspace directory for agent execution
        let workspace = hh.parent().unwrap_or(Path::new("/root")).join(".hermes-workspace");
        if !workspace.exists() {
            std::fs::create_dir_all(&workspace)
                .context("failed to create workspace directory")?;
        }

        if !cfg_path.exists() {
            tracing::info!("hermes config not found, writing default config.yaml");
            sentry_ext::breadcrumb(
                "setup",
                "writing default hermes config.yaml",
                &[("service", "hermes")],
            );
            // Write a minimal default config matching hermes DEFAULT_CONFIG
            std::fs::write(
                &cfg_path,
                "model: ''\ntoolsets:\n  - hermes-cli\nagent:\n  max_turns: 90\n",
            )
            .context("failed to write default config.yaml")?;
        } else {
            tracing::info!("hermes config found at {}", cfg_path.display());
        }

        // Ensure .env exists
        let env = hh.join(".env");
        if !env.exists() {
            std::fs::write(&env, "").context("failed to create ~/.hermes/.env")?;
        }

        // Auto-generate API_SERVER_KEY if missing
        self.ensure_api_server_key()?;

        self.apply_config_patch()?;

        Ok(())
    }

    fn configure(&self) -> Result<()> {
        tracing::info!("hermes: re-applying config (hot reload)");
        self.apply_config_patch()?;
        Ok(())
    }

    fn supports_hot_reload(&self) -> bool {
        true
    }

    fn spawn_spec(&self) -> crate::managed_service::SpawnSpec {
        let hh = hermes_home();
        let workspace = hh
            .parent()
            .unwrap_or(Path::new("/root"))
            .join(".hermes-workspace");

        let mut env = std::collections::HashMap::new();
        env.insert("HERMES_MANAGED".into(), "1".into());
        env.insert("HERMES_HOME".into(), hh.to_string_lossy().into_owned());
        env.insert(
            "MESSAGING_CWD".into(),
            workspace.to_string_lossy().into_owned(),
        );

        // Inject API_SERVER_KEY so the HTTP API is enabled
        if let Some(key) = read_env_var("API_SERVER_KEY") {
            env.insert("API_SERVER_KEY".into(), key);
        }

        crate::managed_service::SpawnSpec {
            program: "hermes".into(),
            // --replace kills any existing gateway on the same port
            args: vec!["gateway".into(), "run".into(), "--replace".into()],
            env,
        }
    }

    fn check_health(&self) -> Result<bool> {
        let host = self.gateway_host();
        let port = self.gateway_port();
        let url = format!("http://{host}:{port}/health");

        let mut cmd = std::process::Command::new("curl");
        cmd.args(["-sf", "--max-time", "10", &url]);
        // Pass API_SERVER_KEY as bearer auth if available
        if let Some(key) = read_env_var("API_SERVER_KEY") {
            cmd.args(["-H", &format!("Authorization: Bearer {key}")]);
        }

        let output = crate::cmd::output_with_timeout(&mut cmd, crate::cmd::DEFAULT_TIMEOUT)
            .context("failed to check hermes health")?;

        if !output.status.success() {
            tracing::warn!("hermes health check failed with status {}", output.status);
            return Ok(false);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        tracing::debug!("hermes health output: {stdout}");

        // Parse response and check for ok status
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
            let status = json
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            return Ok(status == "ok");
        }

        // If we got a 200 response, consider it healthy even if not valid JSON
        Ok(true)
    }

    fn repair(&self) -> Result<()> {
        Self::stop_existing_gateway("repair");

        tracing::info!("running hermes doctor");
        sentry_ext::breadcrumb(
            "repair",
            "running hermes doctor",
            &[("service", "hermes")],
        );

        let output = crate::cmd::output_with_timeout(
            std::process::Command::new("hermes").arg("doctor"),
            std::time::Duration::from_secs(60),
        )
        .context("failed to run hermes doctor")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            tracing::info!("hermes doctor completed successfully");
        } else {
            tracing::warn!(
                "hermes doctor exited with status {}: {}",
                output.status,
                stderr.trim()
            );
            sentry_ext::capture_cmd_failure(
                "hermes doctor",
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
        let upgradable = crate::nix::packages_with_upgrades(&["hermes-agent"])?;

        if !upgradable.iter().any(|name| name == "hermes-agent") {
            return Ok(false);
        }

        tracing::info!("upgrading hermes-agent via nix");
        sentry_ext::breadcrumb(
            "upgrade",
            "upgrading hermes-agent via nix",
            &[("service", "hermes")],
        );
        crate::nix::profile_install("hermes-agent", true)?;
        tracing::info!("hermes-agent upgraded, restart pending");
        Ok(true)
    }

    fn is_busy(&self) -> Result<bool> {
        Ok(false)
    }

    fn data_paths(&self, home: &std::path::Path) -> Vec<DataPath> {
        vec![
            DataPath {
                name: "config",
                path: home.join(".hermes/config.yaml"),
                backup: true,
            },
            DataPath {
                name: "data",
                path: home.join(".hermes"),
                backup: true,
            },
        ]
    }

    fn expose_tunnels(&self) -> Vec<TunnelDef> {
        vec![TunnelDef {
            name: "hermes".into(),
            host: self.gateway_host(),
            tcp_port: self.gateway_port(),
        }]
    }

    fn expose_shell_commands(&self) -> Vec<crate::managed_service::ShellCommandDef> {
        use crate::managed_service::ShellCommandDef;
        vec![
            ShellCommandDef {
                name: "hermes-status".into(),
                command: "hermes".into(),
                args: vec!["status".into()],
                description: "Check component status".into(),
                arg_template: None,
                timeout_secs: None,
            },
            ShellCommandDef {
                name: "hermes-doctor".into(),
                command: "hermes".into(),
                args: vec!["doctor".into()],
                description: "Run diagnostics".into(),
                arg_template: None,
                timeout_secs: None,
            },
            ShellCommandDef {
                name: "hermes-config".into(),
                command: "hermes".into(),
                args: vec!["config".into(), "show".into()],
                description: "Show configuration".into(),
                arg_template: None,
                timeout_secs: None,
            },
        ]
    }

    fn expose_files(&self) -> Vec<FileTunnelDef> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/root"));
        vec![FileTunnelDef::Folder {
            name: "hermes-config".into(),
            path: home.join(".hermes").to_string_lossy().into(),
            writable: true,
            allow_write: Vec::new(),
            include: Some(vec!["config.yaml".into()]),
            validators: vec![VALIDATOR.clone()],
            description: "Hermes agent configuration".into(),
        }]
    }

    fn service_inventory(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<mac_mgmt_common::InventoryEntry>> + Send + '_>> {
        use mac_mgmt_common::{InventoryEntry, InventoryValueType};
        Box::pin(async move {
            let mut entries = Vec::new();
            if let Ok(out) = crate::cmd::output_with_timeout(
                std::process::Command::new("hermes").arg("--version"),
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

            let has_api_key = read_env_var("API_SERVER_KEY").is_some_and(|k| !k.is_empty());
            findings.push(SecurityFinding {
                id: "hermes_api_key".into(),
                severity: FindingSeverity::Medium,
                message: if has_api_key {
                    "API server key is configured".into()
                } else {
                    "API server key is not set — HTTP API disabled".into()
                },
                pass: has_api_key,
            });

            findings
        })
    }
}
