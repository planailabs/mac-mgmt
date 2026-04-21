//! Bridge between the daemon and the healer crate.
//!
//! Provides local implementations of the healer's abstract traits so the
//! healer can operate directly on this daemon's file tunnels, shell commands,
//! log buffer, and assessment data — no relay or database queries needed.

use std::sync::{Arc, RwLock};

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;

use mac_mgmt_healer::instance_access::{
    FileReadResult, InstanceAccess, ShellLine, ShellOutput,
};
use mac_mgmt_healer::instance_data::InstanceDataSource;
use mac_mgmt_healer::store::{
    ClusterInstance, Inventory, ProbeHistoryEntry, ProbeStatus, ServiceState, SystemSample,
    VersionInfo,
};
use mac_mgmt_healer::{HealerSession, SessionAccess, SessionFactory};

use crate::assessment::Assessor;
use crate::file_tunnels::FileTunnelRegistry;
use crate::log_buffer::LogBuffer;
use crate::shell_tunnels::ShellTunnelRegistry;

// ── LocalInstanceAccess ───────────���────────────────────────────────────

/// [`InstanceAccess`] that calls the daemon's tunnel registries directly.
pub struct LocalInstanceAccess {
    file_tunnels: Arc<RwLock<FileTunnelRegistry>>,
    shell_tunnels: Arc<RwLock<ShellTunnelRegistry>>,
    log_buffer: LogBuffer,
}

impl LocalInstanceAccess {
    pub fn new(
        file_tunnels: Arc<RwLock<FileTunnelRegistry>>,
        shell_tunnels: Arc<RwLock<ShellTunnelRegistry>>,
        log_buffer: LogBuffer,
    ) -> Self {
        Self {
            file_tunnels,
            shell_tunnels,
            log_buffer,
        }
    }
}

#[async_trait]
impl InstanceAccess for LocalInstanceAccess {
    async fn file_list(
        &self,
        tunnel_name: &str,
        path: Option<&str>,
    ) -> Result<serde_json::Value> {
        let tunnel = {
            let registry = self.file_tunnels.read().unwrap();
            registry
                .get(tunnel_name)
                .ok_or_else(|| anyhow::anyhow!("file tunnel '{tunnel_name}' not found"))?
                .clone()
        };
        let (status, body) = crate::file_tunnels::handle_list(&tunnel, path);
        if status == 200 {
            Ok(body)
        } else {
            let err = body
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            anyhow::bail!("file_list returned {status}: {err}")
        }
    }

    async fn file_read(&self, tunnel_name: &str, path: &str) -> Result<FileReadResult> {
        let tunnel = {
            let registry = self.file_tunnels.read().unwrap();
            registry
                .get(tunnel_name)
                .ok_or_else(|| anyhow::anyhow!("file tunnel '{tunnel_name}' not found"))?
                .clone()
        };
        let (content, mtime) = crate::file_tunnels::read_file(&tunnel, Some(path))
            .map_err(|(_status, msg)| anyhow::anyhow!("{msg}"))?;
        Ok(FileReadResult { content, mtime })
    }

    async fn file_write(
        &self,
        tunnel_name: &str,
        path: &str,
        content: &[u8],
        expected_mtime: Option<i64>,
    ) -> Result<serde_json::Value> {
        let tunnel = {
            let registry = self.file_tunnels.read().unwrap();
            registry
                .get(tunnel_name)
                .ok_or_else(|| anyhow::anyhow!("file tunnel '{tunnel_name}' not found"))?
                .clone()
        };
        let mtime = crate::file_tunnels::write_file(&tunnel, Some(path), content, expected_mtime)
            .map_err(|(_status, msg)| anyhow::anyhow!("{msg}"))?;
        Ok(serde_json::json!({ "status": 200, "mtime": mtime }))
    }

    async fn shell_exec(
        &self,
        command_name: &str,
        user_arg: Option<&str>,
    ) -> Result<ShellOutput> {
        use tokio::io::AsyncBufReadExt;

        let (tunnel, virtual_handler) = {
            let registry = self.shell_tunnels.read().unwrap();
            let tunnel = registry
                .get(command_name)
                .ok_or_else(|| anyhow::anyhow!("shell command '{command_name}' not found"))?
                .clone();
            let vh = registry.get_virtual(command_name).cloned();
            (tunnel, vh)
        };

        // Validate user argument (shared with WS handler)
        crate::shell_tunnels::validate_args(&tunnel, user_arg)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        // Virtual handler
        if let Some(handler) = virtual_handler {
            let output = handler(user_arg);
            return Ok(ShellOutput {
                lines: output
                    .lines
                    .into_iter()
                    .map(|(stream, data)| ShellLine { stream, data })
                    .collect(),
                exit_code: Some(output.exit_code),
                error: None,
            });
        }

        // Build command (shared with WS handler)
        let mut cmd = crate::shell_tunnels::build_command(&tunnel, user_arg);

        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let mut stdout_lines = tokio::io::BufReader::new(stdout).lines();
        let mut stderr_lines = tokio::io::BufReader::new(stderr).lines();

        let exec_timeout = tunnel
            .def
            .timeout_secs
            .unwrap_or(crate::shell_tunnels::DEFAULT_EXEC_SECS);
        let timeout = tokio::time::sleep(std::time::Duration::from_secs(exec_timeout));
        tokio::pin!(timeout);

        let mut lines = Vec::new();

        loop {
            tokio::select! {
                line = stdout_lines.next_line() => match line {
                    Ok(Some(data)) => lines.push(ShellLine { stream: "stdout".into(), data }),
                    Ok(None) => {
                        while let Ok(Some(data)) = stderr_lines.next_line().await {
                            lines.push(ShellLine { stream: "stderr".into(), data });
                        }
                        break;
                    }
                    Err(_) => break,
                },
                line = stderr_lines.next_line() => match line {
                    Ok(Some(data)) => lines.push(ShellLine { stream: "stderr".into(), data }),
                    Ok(None) => {
                        while let Ok(Some(data)) = stdout_lines.next_line().await {
                            lines.push(ShellLine { stream: "stdout".into(), data });
                        }
                        break;
                    }
                    Err(_) => break,
                },
                _ = &mut timeout => {
                    let _ = child.kill().await;
                    return Ok(ShellOutput {
                        lines,
                        exit_code: Some(-1),
                        error: Some(format!("command timed out after {exec_timeout}s")),
                    });
                }
            }
        }

        let exit_code = child.wait().await.ok().and_then(|s| s.code()).unwrap_or(-1);
        Ok(ShellOutput {
            lines,
            exit_code: Some(exit_code),
            error: None,
        })
    }

    async fn log_fetch(
        &self,
        n: Option<usize>,
        service: Option<&str>,
        _after: Option<usize>,
    ) -> Result<serde_json::Value> {
        let lines = self.log_buffer.tail(n.unwrap_or(200));
        let filtered: Vec<&String> = if let Some(svc) = service {
            lines.iter().filter(|l| l.contains(svc)).collect()
        } else {
            lines.iter().collect()
        };
        Ok(serde_json::json!({ "lines": filtered }))
    }

    async fn is_online(&self) -> bool {
        true
    }

    async fn wait_until_online(&self, _max_secs: u64) -> Result<()> {
        Ok(())
    }
}

// ── LocalInstanceDataSource ────────────────────────────────────────────

/// [`InstanceDataSource`] that reads from the daemon's in-memory [`Assessor`].
pub struct LocalInstanceDataSource {
    assessor: Arc<Assessor>,
    instance_id: String,
}

impl LocalInstanceDataSource {
    pub fn new(assessor: Arc<Assessor>, instance_id: String) -> Self {
        Self {
            assessor,
            instance_id,
        }
    }
}

#[async_trait]
impl InstanceDataSource for LocalInstanceDataSource {
    async fn get_probe_status(&self, _instance_id: &str) -> Result<Option<ProbeStatus>> {
        let probes = self.assessor.latest_probes_snapshot();
        let sample = self.assessor.latest_sample_snapshot();
        Ok(Some(ProbeStatus {
            services_extended: Some(serde_json::to_value(&probes)?),
            sample: sample
                .as_ref()
                .map(|s| serde_json::to_value(s))
                .transpose()?,
            reported_at: Utc::now(),
        }))
    }

    async fn get_system_sample(&self, _instance_id: &str) -> Result<Option<SystemSample>> {
        let sample = self.assessor.latest_sample_snapshot();
        Ok(Some(SystemSample {
            sample: sample
                .as_ref()
                .map(|s| serde_json::to_value(s))
                .transpose()?,
            reported_at: Utc::now(),
        }))
    }

    async fn get_inventory(&self, _instance_id: &str) -> Result<Option<Inventory>> {
        Ok(None)
    }

    async fn get_probe_history(
        &self,
        _instance_id: &str,
        _service: Option<&str>,
        _limit: i64,
    ) -> Result<Vec<ProbeHistoryEntry>> {
        Ok(Vec::new())
    }

    async fn get_heartbeat_json(&self, _instance_id: &str) -> Result<Option<serde_json::Value>> {
        let sample = self.assessor.latest_sample_snapshot();
        let probes = self.assessor.latest_probes_snapshot();
        Ok(Some(serde_json::json!({
            "instance_id": self.instance_id,
            "version": env!("CARGO_PKG_VERSION"),
            "sample": sample,
            "services_extended": probes,
        })))
    }

    async fn get_version_info(&self, _instance_id: &str) -> Result<VersionInfo> {
        Ok(VersionInfo {
            heartbeat: Some(mac_mgmt_healer::store::HeartbeatVersion {
                version: env!("CARGO_PKG_VERSION").to_string(),
                git_sha: option_env!("GIT_SHA").map(String::from),
                reported_at: Utc::now(),
            }),
            daemon_versions: Vec::new(),
        })
    }

    async fn get_service_state(&self, _instance_id: &str) -> Result<Option<ServiceState>> {
        let probes = self.assessor.latest_probes_snapshot();
        Ok(Some(ServiceState {
            services: serde_json::to_value(&probes)?,
            services_extended: Some(serde_json::to_value(&probes)?),
            reported_at: Utc::now(),
        }))
    }

    async fn get_cluster_instances(
        &self,
        _cluster_id: uuid::Uuid,
    ) -> Result<Vec<ClusterInstance>> {
        Ok(Vec::new())
    }

    async fn has_recent_heartbeat(&self, _instance_id: &str) -> Result<bool> {
        Ok(true)
    }

    async fn get_relay_proxy_url(&self, _instance_id: &str) -> Result<Option<String>> {
        Ok(None)
    }
}

// ── LocalSessionFactory ─────────────────────────────────────────���──────

/// [`SessionFactory`] for daemon-local sessions. Always returns local access.
pub struct LocalSessionFactory {
    file_tunnels: Arc<RwLock<FileTunnelRegistry>>,
    shell_tunnels: Arc<RwLock<ShellTunnelRegistry>>,
    log_buffer: LogBuffer,
}

impl LocalSessionFactory {
    pub fn new(
        file_tunnels: Arc<RwLock<FileTunnelRegistry>>,
        shell_tunnels: Arc<RwLock<ShellTunnelRegistry>>,
        log_buffer: LogBuffer,
    ) -> Self {
        Self {
            file_tunnels,
            shell_tunnels,
            log_buffer,
        }
    }
}

#[async_trait]
impl SessionFactory for LocalSessionFactory {
    async fn build_access(&self, _session: &HealerSession) -> Result<SessionAccess> {
        let instance: mac_mgmt_healer::DynInstanceAccess = Arc::new(LocalInstanceAccess::new(
            self.file_tunnels.clone(),
            self.shell_tunnels.clone(),
            self.log_buffer.clone(),
        ));
        Ok(SessionAccess {
            instance,
            cluster: None,
            metrics_url: None,
            proxy_expires: None,
        })
    }
}
