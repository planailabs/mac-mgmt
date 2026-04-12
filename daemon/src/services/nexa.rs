use anyhow::{Context, Result};
use std::process::Command;

use crate::managed_service::{ManagedService, TunnelDef};
use crate::sentry_ext;
pub use mac_mgmt_common::NexaConfig;

pub struct Nexa {
    config: NexaConfig,
}

impl Nexa {
    pub fn new(config: NexaConfig) -> Self {
        Self { config }
    }

    fn base_url(&self) -> String {
        format!("http://{}:{}", self.config.host, self.config.port)
    }

    fn http_get(&self, path: &str) -> Result<String> {
        super::http_get(&self.config.host, self.config.port, path)
    }
}

impl ManagedService for Nexa {
    fn name(&self) -> &str {
        "nexa"
    }

    fn ensure_installed(&self) -> Result<()> {
        if crate::nix::is_installed("nexa")? {
            tracing::info!("nexa is already installed");
            return Ok(());
        }

        tracing::info!("nexa not found, installing via nix");
        sentry_ext::breadcrumb("install", "installing nexa via nix", &[
            ("service", "nexa"),
            ("package", "nexa"),
        ]);
        crate::nix::profile_install("nexa", false)?;
        Ok(())
    }

    fn ensure_setup(&self) -> Result<()> {
        Ok(())
    }

    fn spawn_spec(&self) -> crate::service_ipc::protocol::SpawnSpec {
        crate::service_ipc::protocol::SpawnSpec {
            program: "nexa".into(),
            args: vec![
                "serve".into(),
                "--host".into(),
                format!("{}:{}", self.config.host, self.config.port),
                "--skip-update".into(),
            ],
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
            .context("failed to start nexa serve")?;
        tracing::info!("nexa serve started (pid: {})", child.id());
        sentry_ext::breadcrumb("spawn", "nexa serve started", &[
            ("service", "nexa"),
            ("pid", &child.id().to_string()),
        ]);
        Ok(child)
    }

    fn check_health(&self) -> Result<bool> {
        match self.http_get("/") {
            Ok(body) if body.contains("Nexa SDK is running") => {
                tracing::debug!("nexa is healthy at {}", self.base_url());
                Ok(true)
            }
            Ok(body) => {
                tracing::warn!("nexa unexpected response: {body}");
                Ok(false)
            }
            Err(e) => {
                tracing::warn!("nexa health check failed at {}: {e}", self.base_url());
                Ok(false)
            }
        }
    }

    fn repair(&self) -> Result<()> {
        Ok(())
    }

    fn post_start(&self) -> Result<()> {
        for model in &self.config.models {
            tracing::info!("pulling nexa model: {model}");
            sentry_ext::breadcrumb("post_start", &format!("pulling model {model}"), &[
                ("service", "nexa"),
                ("model", model),
            ]);
            let output = Command::new("nexa")
                .args(["pull", model, "--model-type", "llm", "--skip-update"])
                .output()
                .with_context(|| format!("failed to run nexa pull {model}"))?;

            if output.status.success() {
                tracing::info!("nexa model {model} pulled successfully");
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::warn!("nexa pull {model} exited with {}", output.status);
                sentry_ext::capture_cmd_failure(
                    &format!("nexa pull {model}"),
                    output.status.code(),
                    stderr.trim(),
                );
            }
        }

        Ok(())
    }

    fn check_and_upgrade(&self) -> Result<bool> {
        let upgradable = crate::nix::packages_with_upgrades(&["nexa"])?;

        if !upgradable.iter().any(|name| name == "nexa") {
            return Ok(false);
        }

        tracing::info!("upgrading nexa via nix");
        sentry_ext::breadcrumb("upgrade", "upgrading nexa via nix", &[
            ("service", "nexa"),
            ("package", "nexa"),
        ]);
        crate::nix::profile_install("nexa", true)?;
        tracing::info!("nexa upgraded, restart pending");
        Ok(true)
    }

    fn is_busy(&self) -> Result<bool> {
        let body = self.http_get("/v1/models")?;
        let json: serde_json::Value =
            serde_json::from_str(&body).context("failed to parse nexa /v1/models")?;

        let busy = json
            .get("data")
            .and_then(|d| d.as_array())
            .is_some_and(|models| !models.is_empty());

        if busy {
            tracing::info!("nexa has models loaded");
        } else {
            tracing::debug!("nexa is idle");
        }

        Ok(busy)
    }

    fn expose_tunnels(&self) -> Vec<TunnelDef> {
        vec![TunnelDef {
            name: "nexa".into(),
            host: self.config.host.clone(),
            tcp_port: self.config.port,
        }]
    }
}
