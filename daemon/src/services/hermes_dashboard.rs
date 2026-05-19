use anyhow::{Context, Result};

use crate::managed_service::{DataPath, ManagedService, TunnelDef};
use crate::sentry_ext;
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
        // The dashboard is launched via the `hermes` binary (same as the gateway).
        // Store-path drift detection uses this to check the nix profile.
        "hermes"
    }

    fn ensure_installed(&self) -> Result<()> {
        // The dashboard ships inside the hermes-agent package — same binary.
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
        // The dashboard reads from ~/.hermes — ensure directory exists.
        let home = dirs::home_dir().context("HOME not set")?;
        let hermes_home = home.join(".hermes");
        if !hermes_home.exists() {
            std::fs::create_dir_all(&hermes_home)
                .context("failed to create ~/.hermes directory")?;
        }
        Ok(())
    }

    fn spawn_spec(&self) -> crate::managed_service::SpawnSpec {
        let host = self.host();
        let port = self.port();

        let mut env = std::collections::HashMap::new();
        env.insert("HERMES_MANAGED".into(), "1".into());

        crate::managed_service::SpawnSpec {
            program: "hermes".into(),
            args: vec![
                "dashboard".into(),
                "--host".into(),
                host,
                "--port".into(),
                port.to_string(),
                "--no-open".into(),
            ],
            env,
        }
    }

    fn check_health(&self) -> Result<bool> {
        let host = self.host();
        let port = self.port();
        let url = format!("http://{host}:{port}/");

        let mut cmd = std::process::Command::new("curl");
        cmd.args(["-sf", "--max-time", "10", "-o", "/dev/null", "-w", "%{http_code}", &url]);

        let output = crate::cmd::output_with_timeout(&mut cmd, crate::cmd::DEFAULT_TIMEOUT)
            .context("failed to check hermes-dashboard health")?;

        if !output.status.success() {
            tracing::warn!("hermes-dashboard health check failed with status {}", output.status);
            return Ok(false);
        }

        let code = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let ok = code.starts_with('2') || code.starts_with('3');
        if !ok {
            tracing::warn!("hermes-dashboard returned HTTP {code}");
        }
        Ok(ok)
    }

    fn repair(&self) -> Result<()> {
        // No specific repair — the dashboard is stateless.
        tracing::info!("hermes-dashboard: no specific repair steps");
        Ok(())
    }

    fn check_and_upgrade(&self) -> Result<bool> {
        // Shares the hermes-agent nix package with the gateway.
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
        tracing::info!("hermes-agent upgraded (dashboard), restart pending");
        Ok(true)
    }

    fn is_busy(&self) -> Result<bool> {
        Ok(false)
    }

    fn data_paths(&self, _home: &std::path::Path) -> Vec<DataPath> {
        // The dashboard is stateless — it reads ~/.hermes which the gateway
        // service already backs up.
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
}
