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
use crate::managed_service::FileTunnelDef;
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
        let resolved = crate::file_tunnels::resolve_path(&tunnel, Some(path))
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        if matches!(&tunnel.def, FileTunnelDef::Folder { .. }) {
            let name = resolved.file_name().and_then(|n| n.to_str()).unwrap_or("");
            anyhow::ensure!(
                crate::file_tunnels::matches_include(&tunnel, name),
                "file not included in tunnel filter"
            );
        }

        let meta = std::fs::metadata(&resolved)?;
        anyhow::ensure!(meta.is_file(), "path is not a file");

        let content = std::fs::read(&resolved)?;
        let mtime = crate::file_tunnels::mtime_secs(&resolved);
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
        anyhow::ensure!(tunnel.writable(), "tunnel is read-only");

        let resolved = crate::file_tunnels::resolve_path(&tunnel, Some(path))
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let filename = resolved
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if matches!(&tunnel.def, FileTunnelDef::Folder { .. }) {
            anyhow::ensure!(
                crate::file_tunnels::matches_include(&tunnel, filename),
                "file not included in tunnel filter"
            );
        }
        anyhow::ensure!(
            crate::file_tunnels::matches_allow_write(&tunnel, filename),
            "file not allowed by write filter"
        );
        anyhow::ensure!(
            (content.len() as u64) <= crate::file_tunnels::MAX_FILE_SIZE,
            "file too large"
        );

        // Optimistic concurrency
        if let Some(expected) = expected_mtime {
            if let Some(actual) = crate::file_tunnels::mtime_secs(&resolved) {
                anyhow::ensure!(
                    actual == expected,
                    "file modified since last read (expected mtime {expected}, actual {actual})"
                );
            }
        }

        // Atomic write: tmp → rename, with backup + rollback on validation failure
        let tmp = resolved.with_extension("tmp.file-tunnel");
        let backup = resolved.with_extension("bak.file-tunnel");
        std::fs::write(&tmp, content)?;

        let had_original = resolved.exists();
        if had_original {
            if let Err(e) = std::fs::copy(&resolved, &backup) {
                let _ = std::fs::remove_file(&tmp);
                anyhow::bail!("backup failed: {e}");
            }
        }

        if let Err(e) = std::fs::rename(&tmp, &resolved) {
            let _ = std::fs::remove_file(&tmp);
            anyhow::bail!("rename failed: {e}");
        }

        // Run validators
        if let Some(v) = crate::file_tunnels::find_validator(&tunnel, &resolved) {
            let rollback = || {
                if had_original {
                    let _ = std::fs::rename(&backup, &resolved);
                } else {
                    let _ = std::fs::remove_file(&resolved);
                }
            };
            if let Some(name) = &v.builtin {
                if let Err(msg) = crate::managed_service::run_builtin_validator(name, &resolved) {
                    rollback();
                    anyhow::bail!("validation failed: {msg}");
                }
            }
            if !v.command.is_empty() {
                match std::process::Command::new(&v.command[0])
                    .args(&v.command[1..])
                    .output()
                {
                    Ok(out) if !out.status.success() => {
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        rollback();
                        anyhow::bail!("validation failed: {stderr}");
                    }
                    Err(e) if v.builtin.is_some() => {
                        tracing::warn!("validation command not available ({e}), builtin passed");
                    }
                    Err(e) => {
                        rollback();
                        anyhow::bail!("validation command failed: {e}");
                    }
                    Ok(_) => {}
                }
            }
        }

        let _ = std::fs::remove_file(&backup);
        let mtime = crate::file_tunnels::mtime_secs(&resolved).unwrap_or(0);
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

        // Validate user argument
        if let Some(ref tmpl) = tunnel.def.arg_template {
            if let Some(arg) = user_arg {
                if let Some(ref pattern) = tmpl.validation {
                    let re = regex::Regex::new(pattern)
                        .map_err(|e| anyhow::anyhow!("invalid validation regex: {e}"))?;
                    anyhow::ensure!(
                        re.is_match(arg),
                        "argument does not match required pattern: {pattern}"
                    );
                }
            }
        } else if user_arg.is_some() {
            anyhow::bail!("this command does not accept arguments");
        }

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

        // Spawn process
        let mut cmd = tokio::process::Command::new(&tunnel.def.command);
        cmd.args(&tunnel.def.args);
        if let Some(arg) = user_arg {
            if !arg.is_empty() {
                cmd.arg(arg);
            }
        }
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        cmd.stdin(std::process::Stdio::null());

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
