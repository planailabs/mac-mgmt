use anyhow::{Context, Result};
use std::process::Command;

use crate::managed_service::{ManagedService, SpawnSpec, TunnelDef};
use crate::sentry_ext;
pub use mac_mgmt_common::UnslothConfig;

pub struct Unsloth {
    config: UnslothConfig,
}

impl Unsloth {
    pub fn new(config: UnslothConfig) -> Self {
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
            8888
        } else {
            self.config.port
        }
    }

    fn base_url(&self) -> String {
        format!("http://{}:{}", self.effective_host(), self.effective_port())
    }
}

impl ManagedService for Unsloth {
    fn name(&self) -> &str {
        "unsloth"
    }

    fn ensure_installed(&self) -> Result<()> {
        if crate::nix::is_installed("unsloth")? {
            tracing::info!("unsloth is already installed");
            return Ok(());
        }

        tracing::info!("unsloth not found, installing via nix");
        sentry_ext::breadcrumb(
            "install",
            "installing unsloth via nix",
            &[("service", "unsloth"), ("package", "unsloth")],
        );
        crate::nix::profile_install("unsloth", false)?;
        Ok(())
    }

    fn ensure_setup(&self) -> Result<()> {
        Ok(())
    }

    fn preflight(&self) -> Result<()> {
        let _ = Command::new("unsloth").args(["studio", "stop"]).status();
        Ok(())
    }

    fn spawn_spec(&self) -> SpawnSpec {
        SpawnSpec {
            program: "unsloth".into(),
            args: vec![
                "studio".into(),
                "-H".into(),
                self.effective_host().to_string(),
                "-p".into(),
                self.effective_port().to_string(),
            ],
            env: Default::default(),
        }
    }

    fn check_health(&self) -> Result<bool> {
        let output = crate::cmd::output_with_timeout(
            Command::new("curl")
                .args(["-sf", &format!("{}/api/health", self.base_url())]),
            crate::cmd::DEFAULT_TIMEOUT,
        )
        .context("failed to check unsloth health")?;
        if !output.status.success() {
            return Ok(false);
        }
        let json: serde_json::Value = serde_json::from_slice(&output.stdout)
            .context("failed to parse unsloth health response")?;
        let healthy = json
            .get("status")
            .and_then(|v| v.as_str())
            .is_some_and(|s| s == "healthy");
        if healthy {
            tracing::debug!("unsloth is healthy at {}", self.base_url());
        } else {
            tracing::warn!("unsloth health check reports unhealthy: {}", json);
        }
        Ok(healthy)
    }

    fn repair(&self) -> Result<()> {
        tracing::info!("repairing unsloth by stopping the server");
        let _ = Command::new("unsloth").args(["studio", "stop"]).status();
        Ok(())
    }

    fn post_start(&self) -> Result<()> {
        Ok(())
    }

    fn check_and_upgrade(&self) -> Result<bool> {
        let upgradable = crate::nix::packages_with_upgrades(&["unsloth"])?;
        if !upgradable.iter().any(|name| name == "unsloth") {
            return Ok(false);
        }
        tracing::info!("upgrading unsloth via nix");
        sentry_ext::breadcrumb(
            "upgrade",
            "upgrading unsloth via nix",
            &[("service", "unsloth"), ("package", "unsloth")],
        );
        crate::nix::profile_install("unsloth", true)?;
        tracing::info!("unsloth upgraded, restart pending");
        Ok(true)
    }

    fn is_busy(&self) -> Result<bool> {
        // Check if a model is currently loaded for inference
        let output = crate::cmd::output_with_timeout(
            Command::new("curl")
                .args(["-sf", &format!("{}/api/inference/status", self.base_url())]),
            crate::cmd::DEFAULT_TIMEOUT,
        );
        match output {
            Ok(out) if out.status.success() => {
                let json: serde_json::Value =
                    serde_json::from_slice(&out.stdout).unwrap_or_default();
                // If there's a model loaded or inference is active, consider it busy
                let busy = json
                    .get("model_loaded")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if busy {
                    tracing::info!("unsloth has a model loaded");
                } else {
                    tracing::debug!("unsloth is idle");
                }
                Ok(busy)
            }
            _ => Ok(false),
        }
    }

    fn expose_shell_commands(&self) -> Vec<crate::managed_service::ShellCommandDef> {
        use crate::managed_service::ShellCommandDef;
        vec![
            ShellCommandDef {
                name: "unsloth-health".into(),
                command: "curl".into(),
                args: vec!["-sf".into(), format!("{}/api/health", self.base_url())],
                description: "Show Unsloth health status".into(),
                arg_template: None,
                timeout_secs: None,
            },
            ShellCommandDef {
                name: "unsloth-system".into(),
                command: "curl".into(),
                args: vec!["-sf".into(), format!("{}/api/system", self.base_url())],
                description: "Show system info (GPU, memory, etc.)".into(),
                arg_template: None,
                timeout_secs: None,
            },
        ]
    }

    fn expose_tunnels(&self) -> Vec<TunnelDef> {
        vec![TunnelDef {
            name: "unsloth".into(),
            host: self.effective_host().to_string(),
            tcp_port: self.effective_port(),
        }]
    }

    fn service_inventory(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<mac_mgmt_common::InventoryEntry>> + Send + '_>>
    {
        use mac_mgmt_common::{InventoryEntry, InventoryValueType};
        let host = self.effective_host().to_string();
        let port = self.effective_port();
        Box::pin(async move {
            let mut entries = Vec::new();
            if let Ok(body) = super::http_get(&host, port, "/api/health").await {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                    if let Some(version) = json.get("version").and_then(|v| v.as_str()) {
                        entries.push(InventoryEntry {
                            id: "version".into(),
                            name: "Version".into(),
                            value: serde_json::Value::String(version.to_string()),
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
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<mac_mgmt_common::InventoryEntry>> + Send + '_>>
    {
        use mac_mgmt_common::{InventoryEntry, InventoryValueType};
        let host = self.effective_host().to_string();
        let port = self.effective_port();
        Box::pin(async move {
            let mut entries = Vec::new();
            if let Ok(body) = super::http_get(&host, port, "/api/inference/status").await {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                    entries.push(InventoryEntry {
                        id: "inference_status".into(),
                        name: "Inference Status".into(),
                        value: json,
                        value_type: InventoryValueType::Json,
                    });
                }
            }
            entries
        })
    }
}
