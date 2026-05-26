use anyhow::{Context, Result};

use crate::managed_service::{DataPath, ManagedService, TunnelDef};
use crate::sentry_ext;
use crate::services::hermes::{gateway_api_port, hermes_home, read_env_var};
pub use mac_mgmt_common::HermesDashboardConfig;

pub struct HermesDashboard {
    config: HermesDashboardConfig,
}

impl HermesDashboard {
    pub fn new(config: HermesDashboardConfig) -> Self {
        Self { config }
    }

    fn host(&self) -> String {
        if self.config.host.is_empty() {
            "127.0.0.1".to_string()
        } else {
            self.config.host.clone()
        }
    }

    fn port(&self) -> u16 {
        if self.config.port == 0 {
            9119
        } else {
            self.config.port
        }
    }
}

impl ManagedService for HermesDashboard {
    fn name(&self) -> &str {
        "hermes-dashboard"
    }

    fn binary_name(&self) -> &str {
        "hermes"
    }

    fn ensure_installed(&self) -> Result<()> {
        if crate::nix::is_installed("hermes-agent")? {
            tracing::info!("hermes-agent (dashboard) is already installed");
            return Ok(());
        }

        tracing::info!("hermes-agent not found, installing via nix (for dashboard)");
        sentry_ext::breadcrumb(
            "install",
            "installing hermes-agent via nix (dashboard)",
            &[("service", "hermes-dashboard")],
        );
        crate::nix::profile_install("hermes-agent", false)?;
        Ok(())
    }

    fn ensure_setup(&self) -> Result<()> {
        // The dashboard reads config.yaml, .env, state.db, gateway.pid,
        // sessions, and plugins from HERMES_HOME. It writes to logs/.
        let hh = hermes_home();
        for subdir in &["", "logs"] {
            let dir = hh.join(subdir);
            if !dir.exists() {
                std::fs::create_dir_all(&dir)
                    .with_context(|| format!("failed to create {}", dir.display()))?;
            }
        }
        Ok(())
    }

    fn spawn_spec(&self) -> crate::managed_service::SpawnSpec {
        let hh = hermes_home();

        let mut env = std::collections::HashMap::new();
        env.insert("HERMES_HOME".into(), hh.to_string_lossy().into_owned());
        env.insert("HERMES_MANAGED".into(), "1".into());

        // Tell the dashboard where the gateway API lives so it can probe
        // /health/detailed for cross-process status (unauthenticated).
        if read_env_var("API_SERVER_KEY").is_some() {
            env.insert(
                "GATEWAY_HEALTH_URL".into(),
                format!("http://127.0.0.1:{}", gateway_api_port()),
            );
        }

        crate::managed_service::SpawnSpec {
            program: "hermes".into(),
            args: vec![
                "dashboard".into(),
                "--host".into(),
                self.host(),
                "--port".into(),
                self.port().to_string(),
                "--no-open".into(),
            ],
            env,
        }
    }

    fn check_health(&self) -> Result<bool> {
        // /api/status is a public endpoint (no session token required)
        let url = format!("http://{}:{}/api/status", self.host(), self.port());

        let mut cmd = std::process::Command::new("curl");
        cmd.args(["-sf", "--max-time", "10", &url]);

        let output = crate::cmd::output_with_timeout(&mut cmd, crate::cmd::DEFAULT_TIMEOUT)
            .context("failed to check hermes-dashboard health")?;

        if !output.status.success() {
            tracing::warn!(
                "hermes-dashboard health check failed with status {}",
                output.status
            );
            return Ok(false);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        tracing::debug!("hermes-dashboard status: {stdout}");
        Ok(true)
    }

    fn repair(&self) -> Result<()> {
        tracing::info!("hermes-dashboard: no specific repair steps");
        Ok(())
    }

    fn check_and_upgrade(&self) -> Result<bool> {
        let upgradable = crate::nix::packages_with_upgrades(&["hermes-agent"])?;
        if !upgradable.iter().any(|name| name == "hermes-agent") {
            return Ok(false);
        }

        tracing::info!("upgrading hermes-agent via nix (dashboard)");
        sentry_ext::breadcrumb(
            "upgrade",
            "upgrading hermes-agent via nix (dashboard)",
            &[("service", "hermes-dashboard")],
        );
        crate::nix::profile_install("hermes-agent", true)?;
        Ok(true)
    }

    fn is_busy(&self) -> Result<bool> {
        Ok(false)
    }

    fn data_paths(&self, _home: &std::path::Path) -> Vec<DataPath> {
        Vec::new()
    }

    fn expose_tunnels(&self) -> Vec<TunnelDef> {
        vec![TunnelDef {
            name: "hermes-dashboard".into(),
            host: self.host(),
            tcp_port: self.port(),
        }]
    }

    fn service_inventory(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Vec<mac_mgmt_common::InventoryEntry>> + Send + '_>,
    > {
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
}
