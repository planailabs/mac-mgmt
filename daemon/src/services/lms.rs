use anyhow::{Context, Result};
use std::process::Command;

use crate::managed_service::{ManagedService, TunnelDef};
use crate::sentry_ext;
pub use mac_mgmt_common::LmsConfig;

pub struct Lms {
    config: LmsConfig,
}

impl Lms {
    pub fn new(config: LmsConfig) -> Self {
        Self { config }
    }

    fn effective_host(&self) -> &str {
        if self.config.host.is_empty() {
            "127.0.0.1"
        } else {
            &self.config.host
        }
    }

    fn effective_port(&self) -> u16 {
        if self.config.port == 0 {
            1234
        } else {
            self.config.port
        }
    }

    fn base_url(&self) -> String {
        format!("http://{}:{}", self.effective_host(), self.effective_port())
    }
}

impl ManagedService for Lms {
    fn name(&self) -> &str {
        "lms"
    }

    fn ensure_installed(&self) -> Result<()> {
        if crate::nix::is_installed("lmstudio")? {
            tracing::info!("lmstudio is already installed");
            return Ok(());
        }

        tracing::info!("lmstudio not found, installing via nix");
        sentry_ext::breadcrumb(
            "install",
            "installing lmstudio via nix",
            &[("service", "lms"), ("package", "lmstudio")],
        );
        crate::nix::profile_install("lmstudio", false)?;
        Ok(())
    }

    fn ensure_setup(&self) -> Result<()> {
        Ok(())
    }

    fn preflight(&self) -> Result<()> {
        // Best-effort: stop any leftover server so spawn() starts cleanly.
        let _ = Command::new("lms").args(["server", "stop"]).status();
        Ok(())
    }

    fn spawn_spec(&self) -> crate::managed_service::SpawnSpec {
        // `lms server start` backgrounds itself, so wrap it in a shell that
        // starts the server and then blocks on sleep — the supervisor treats
        // the sleep as the supervised process.
        crate::managed_service::SpawnSpec {
            program: "lms".into(),
            args: vec!["server".into(), "start".into(), "--foreground".into()],
            env: Default::default(),
        }
    }

    fn check_health(&self) -> Result<bool> {
        // `lms server status --json` prints `{"running": bool, "port": number}`
        // on stdout — see ~/lms/src/subcommands/server.ts.
        let output = crate::cmd::output_with_timeout(
            Command::new("lms").args(["server", "status", "--json"]),
            crate::cmd::DEFAULT_TIMEOUT,
        )
        .context("failed to run `lms server status --json`")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("`lms server status --json` failed: {}", stderr.trim());
            return Ok(false);
        }
        let json: serde_json::Value = serde_json::from_slice(&output.stdout)
            .context("failed to parse `lms server status --json` output")?;
        let healthy = json
            .get("running")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if healthy {
            tracing::debug!("lms is healthy at {}", self.base_url());
        } else {
            tracing::warn!("lms server status reports not running: {}", json);
        }
        Ok(healthy)
    }

    fn repair(&self) -> Result<()> {
        // Stop the server; the supervisor's restart will invoke
        // `lms server start` again via the spawn spec.
        tracing::info!("repairing lms by stopping the server");
        let _ = Command::new("lms").args(["server", "stop"]).status();
        Ok(())
    }

    fn post_start(&self) -> Result<()> {
        load_configured_models(&self.config)
    }

    fn check_and_upgrade(&self) -> Result<bool> {
        let upgradable = crate::nix::packages_with_upgrades(&["lmstudio"])?;
        if !upgradable.iter().any(|name| name == "lmstudio") {
            return Ok(false);
        }
        tracing::info!("upgrading lmstudio via nix");
        sentry_ext::breadcrumb(
            "upgrade",
            "upgrading lmstudio via nix",
            &[("service", "lms"), ("package", "lmstudio")],
        );
        crate::nix::profile_install("lmstudio", true)?;
        tracing::info!("lmstudio upgraded, restart pending");
        Ok(true)
    }

    fn is_busy(&self) -> Result<bool> {
        let output = Command::new("lms")
            .args(["ps", "--json"])
            .output()
            .context("failed to run `lms ps --json`")?;
        if !output.status.success() {
            return Ok(false);
        }
        let json: serde_json::Value = serde_json::from_slice(&output.stdout)
            .context("failed to parse `lms ps --json` output")?;
        let busy = json.as_array().is_some_and(|a| !a.is_empty());
        if busy {
            tracing::info!("lms has models loaded");
        } else {
            tracing::debug!("lms is idle");
        }
        Ok(busy)
    }

    fn expose_shell_commands(&self) -> Vec<crate::managed_service::ShellCommandDef> {
        use crate::managed_service::{ShellArgTemplate, ShellCommandDef};
        vec![
            ShellCommandDef {
                name: "lms-status".into(),
                command: "lms".into(),
                args: vec!["server".into(), "status".into(), "--json".into()],
                description: "Show server status".into(),
                arg_template: None,
                timeout_secs: None,
            },
            ShellCommandDef {
                name: "lms-ps".into(),
                command: "lms".into(),
                args: vec!["ps".into(), "--json".into()],
                description: "List loaded models".into(),
                arg_template: None,
                timeout_secs: None,
            },
            ShellCommandDef {
                name: "lms-ls".into(),
                command: "lms".into(),
                args: vec!["ls".into()],
                description: "List available models".into(),
                arg_template: None,
                timeout_secs: None,
            },
            ShellCommandDef {
                name: "lms-load".into(),
                command: "lms".into(),
                args: vec!["load".into()],
                description: "Load a model".into(),
                arg_template: Some(ShellArgTemplate {
                    label: "Model".into(),
                    placeholder: "model-id".into(),
                    validation: None,
                }),
                timeout_secs: None,
            },
            ShellCommandDef {
                name: "lms-unload-all".into(),
                command: "lms".into(),
                args: vec!["unload".into(), "--all".into()],
                description: "Unload all models".into(),
                arg_template: None,
                timeout_secs: None,
            },
        ]
    }

    fn expose_tunnels(&self) -> Vec<TunnelDef> {
        vec![TunnelDef {
            name: "lms".into(),
            host: self.effective_host().to_string(),
            tcp_port: self.effective_port(),
        }]
    }

    fn service_inventory(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<mac_mgmt_common::InventoryEntry>> + Send + '_>> {
        use mac_mgmt_common::{InventoryEntry, InventoryValueType};
        Box::pin(async move {
            let mut entries = Vec::new();
            if let Ok(out) = crate::cmd::output_with_timeout(
                std::process::Command::new("lms").arg("--version"),
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
        let host = self.effective_host().to_string();
        let port = self.effective_port();
        Box::pin(async move {
            let mut entries = Vec::new();
            if let Ok(body) = super::http_get(&host, port, "/v1/models").await {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                    if let Some(models) = json.get("data").and_then(|d| d.as_array()) {
                        let names: Vec<serde_json::Value> = models.iter()
                            .filter_map(|m| m.get("id").cloned())
                            .collect();
                        entries.push(InventoryEntry {
                            id: "loaded_models".into(),
                            name: "Loaded Models".into(),
                            value: serde_json::Value::Array(names),
                            value_type: InventoryValueType::Json,
                        });
                    }
                }
            }
            entries
        })
    }
}

/// Load every model in `config.models` via `lms load -y`.
/// Idempotent — already-loaded models are a fast no-op. Shared by
/// the managed post_start path and the unmanaged installer.
pub fn load_configured_models(config: &mac_mgmt_common::LmsConfig) -> anyhow::Result<()> {
    use std::process::Command;
    for model in &config.models {
        tracing::info!("loading lms model: {model}");
        crate::sentry_ext::breadcrumb(
            "post_start",
            &format!("loading model {model}"),
            &[("service", "lms"), ("model", model)],
        );
        let output = Command::new("lms")
            .args(["load", model, "-y"])
            .output()
            .with_context(|| format!("failed to run `lms load {model}`"))?;
        if output.status.success() {
            tracing::info!("lms model {model} loaded");
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("`lms load {model}` exited with {}", output.status);
            crate::sentry_ext::capture_cmd_failure(
                &format!("lms load {model}"),
                output.status.code(),
                stderr.trim(),
            );
        }
    }
    Ok(())
}
