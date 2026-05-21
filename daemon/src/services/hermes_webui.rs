use anyhow::{Context, Result};

use crate::managed_service::{DataPath, FileTunnelDef, ManagedService, TunnelDef};
use crate::sentry_ext;
use crate::services::hermes::hermes_home;
pub use mac_mgmt_common::HermesWebuiConfig;

/// nesquena/hermes-webui — three-panel WebUI (sessions / chat / workspace)
/// for the Hermes agent. The Nix derivation reuses hermes-agent's uv2nix venv
/// for both the Python interpreter and the `agent`/`run_agent` modules the
/// WebUI imports at module load.
pub struct HermesWebui {
    config: HermesWebuiConfig,
}

impl HermesWebui {
    pub fn new(config: HermesWebuiConfig) -> Self {
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
            8787
        } else {
            self.config.port
        }
    }

    fn state_dir(&self) -> std::path::PathBuf {
        hermes_home().join("webui")
    }
}

impl ManagedService for HermesWebui {
    fn name(&self) -> &str {
        "hermes-webui"
    }

    fn binary_name(&self) -> &str {
        "hermes-webui"
    }

    fn ensure_installed(&self) -> Result<()> {
        if crate::nix::is_installed("hermes-webui")? {
            tracing::info!("hermes-webui is already installed");
            return Ok(());
        }

        tracing::info!("hermes-webui not found, installing via nix");
        sentry_ext::breadcrumb(
            "install",
            "installing hermes-webui via nix",
            &[("service", "hermes-webui")],
        );
        crate::nix::profile_install("hermes-webui", false)?;
        Ok(())
    }

    fn ensure_setup(&self) -> Result<()> {
        // WebUI persists sessions/workspaces/profiles under $HERMES_HOME/webui
        // (default for HERMES_WEBUI_STATE_DIR). bootstrap.py would create it
        // on first launch, but pre-creating means file tunnels can register
        // the path immediately and avoids a first-run race.
        let dir = self.state_dir();
        if !dir.exists() {
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("failed to create {}", dir.display()))?;
        }
        Ok(())
    }

    fn spawn_spec(&self) -> crate::managed_service::SpawnSpec {
        let hh = hermes_home();

        let mut env = std::collections::HashMap::new();
        env.insert("HERMES_HOME".into(), hh.to_string_lossy().into_owned());
        env.insert("HERMES_MANAGED".into(), "1".into());
        // Both the .env auto-loader and the venv-creation branch in bootstrap
        // expect to write under REPO_ROOT (read-only Nix store). Force the
        // managed-mode invariants explicitly so a packaging change can't
        // silently re-enable either path.
        env.insert("HERMES_WEBUI_AUTO_INSTALL".into(), "0".into());
        env.insert(
            "HERMES_WEBUI_STATE_DIR".into(),
            self.state_dir().to_string_lossy().into_owned(),
        );

        if let Some(pw) = &self.config.password {
            let v = pw.expose();
            if !v.is_empty() {
                env.insert("HERMES_WEBUI_PASSWORD".into(), v.to_string());
            }
        }
        if let Some(ws) = &self.config.default_workspace {
            if !ws.is_empty() {
                env.insert("HERMES_WEBUI_DEFAULT_WORKSPACE".into(), ws.clone());
            }
        }

        // bootstrap.py CLI: positional `port`, named `--host`. The Nix wrapper
        // already injects `--no-browser --skip-agent-install`, so we only need
        // to add host + port here.
        crate::managed_service::SpawnSpec {
            program: "hermes-webui".into(),
            args: vec![
                "--host".into(),
                self.host(),
                self.port().to_string(),
            ],
            env,
        }
    }

    fn check_health(&self) -> Result<bool> {
        // server.py mounts `/health` (not `/api/...`) — see api/routes.py.
        // Public endpoint; no auth required even when HERMES_WEBUI_PASSWORD is set.
        let url = format!("http://{}:{}/health", self.host(), self.port());

        let mut cmd = std::process::Command::new("curl");
        cmd.args(["-sf", "--max-time", "10", &url]);

        let output = crate::cmd::output_with_timeout(&mut cmd, crate::cmd::DEFAULT_TIMEOUT)
            .context("failed to check hermes-webui health")?;

        if !output.status.success() {
            tracing::warn!(
                "hermes-webui health check failed with status {}",
                output.status
            );
            return Ok(false);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        tracing::debug!("hermes-webui /health: {stdout}");
        Ok(true)
    }

    fn repair(&self) -> Result<()> {
        tracing::info!("hermes-webui: no specific repair steps");
        Ok(())
    }

    fn check_and_upgrade(&self) -> Result<bool> {
        let upgradable = crate::nix::packages_with_upgrades(&["hermes-webui"])?;
        if !upgradable.iter().any(|name| name == "hermes-webui") {
            return Ok(false);
        }

        tracing::info!("upgrading hermes-webui via nix");
        sentry_ext::breadcrumb(
            "upgrade",
            "upgrading hermes-webui via nix",
            &[("service", "hermes-webui")],
        );
        crate::nix::profile_install("hermes-webui", true)?;
        Ok(true)
    }

    fn is_busy(&self) -> Result<bool> {
        Ok(false)
    }

    fn data_paths(&self, _home: &std::path::Path) -> Vec<DataPath> {
        // Sessions, profiles, settings.json, audio/image caches all live under
        // $HERMES_HOME/webui. Worth backing up — the agent's own DataPaths in
        // hermes.rs already cover $HERMES_HOME root, but listing the webui
        // subdir explicitly keeps inventory accurate when only the WebUI is
        // enabled on a host.
        vec![DataPath {
            name: "webui-state",
            path: self.state_dir(),
            backup: true,
        }]
    }

    fn expose_tunnels(&self) -> Vec<TunnelDef> {
        vec![TunnelDef {
            name: "hermes-webui".into(),
            host: self.host(),
            tcp_port: self.port(),
        }]
    }

    fn expose_files(&self) -> Vec<FileTunnelDef> {
        // Expose the WebUI state dir as a writable folder so the WebUI's
        // settings.json / profiles can be edited remotely through the relay.
        // No validators — these are managed by the WebUI itself, not us.
        vec![FileTunnelDef::Folder {
            name: "hermes-webui-state".into(),
            path: self.state_dir().to_string_lossy().into(),
            writable: true,
            allow_write: Vec::new(),
            include: Some(vec![
                "settings.json".into(),
                "profiles/**".into(),
                ".env".into(),
            ]),
            validators: Vec::new(),
            description: "Hermes WebUI settings, profiles, and .env".into(),
        }]
    }

    fn service_inventory(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<mac_mgmt_common::InventoryEntry>> + Send + '_>> {
        use mac_mgmt_common::{InventoryEntry, InventoryValueType};
        let host = self.host();
        let port = self.port();
        Box::pin(async move {
            let mut entries = Vec::new();
            // hermes-webui has no --version flag, but server.py serves a
            // version string at /version (see api/updates.py:WEBUI_VERSION).
            // Probe over loopback to avoid the curl roundtrip cost in inventory.
            if let Ok(body) = super::http_get(&host, port, "/version").await {
                let trimmed = body.trim();
                if !trimmed.is_empty() && trimmed.len() < 256 {
                    entries.push(InventoryEntry {
                        id: "version".into(),
                        name: "Version".into(),
                        value: serde_json::Value::String(trimmed.to_string()),
                        value_type: InventoryValueType::String,
                    });
                }
            }
            entries
        })
    }
}
