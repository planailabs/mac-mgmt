//! Config-driven custom service: a [`ManagedService`] whose capabilities are
//! defined entirely in `config.toml` — no Rust code required.
//!
//! If `spawn` is configured the service becomes fully managed (supervised
//! process). Otherwise it runs as an integrated service (no process).

use std::future::Future;
use std::pin::Pin;
use std::process::Command;
use std::time::Duration;

use anyhow::Result;
use mac_mgmt_common::custom_service::{
    CustomFileDef, CustomInventoryDef, CustomSecurityDef, CustomServiceConfig,
};
use mac_mgmt_common::{InventoryEntry, InventoryValueType, SecurityFinding};

use crate::managed_service::{
    FileTunnelDef, ManagedService, ServiceMode, ShellArgTemplate, ShellCommandDef, SpawnSpec,
    TunnelDef,
};

pub struct CustomService {
    config: CustomServiceConfig,
}

impl CustomService {
    pub fn new(config: CustomServiceConfig) -> Self {
        Self { config }
    }
}

impl ManagedService for CustomService {
    fn name(&self) -> &str {
        &self.config.name
    }

    fn service_mode(&self) -> ServiceMode {
        if self.config.spawn.is_some() {
            ServiceMode::Managed
        } else {
            ServiceMode::Integrated
        }
    }

    fn ensure_installed(&self) -> Result<()> {
        Ok(())
    }

    fn ensure_setup(&self) -> Result<()> {
        Ok(())
    }

    fn spawn_spec(&self) -> SpawnSpec {
        match &self.config.spawn {
            Some(spawn) => SpawnSpec {
                program: spawn.command.clone(),
                args: spawn.args.clone(),
                env: spawn.env.clone(),
            },
            None => unreachable!("spawn_spec called on integrated custom service"),
        }
    }

    fn check_health(&self) -> Result<bool> {
        let Some(hc) = &self.config.health_check else {
            return Ok(true);
        };
        let timeout = Duration::from_secs(hc.timeout_secs);
        let out =
            crate::cmd::output_with_timeout(Command::new(&hc.command).args(&hc.args), timeout)?;
        Ok(out.status.success())
    }

    fn check_health_async(&self) -> Pin<Box<dyn Future<Output = Result<bool>> + Send + '_>> {
        Box::pin(async move {
            let Some(hc) = &self.config.health_check else {
                return Ok(true);
            };
            let timeout = Duration::from_secs(hc.timeout_secs);
            let output = tokio::time::timeout(
                timeout,
                tokio::process::Command::new(&hc.command)
                    .args(&hc.args)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .output(),
            )
            .await??;
            Ok(output.status.success())
        })
    }

    fn repair(&self) -> Result<()> {
        Ok(())
    }

    fn check_and_upgrade(&self) -> Result<bool> {
        Ok(false)
    }

    fn expose_tunnels(&self) -> Vec<TunnelDef> {
        self.config
            .tunnels
            .iter()
            .map(|t| TunnelDef {
                name: t.name.clone(),
                host: t.host.clone(),
                tcp_port: t.port,
            })
            .collect()
    }

    fn expose_files(&self) -> Vec<FileTunnelDef> {
        self.config
            .files
            .iter()
            .map(|f| match f {
                CustomFileDef::File {
                    name,
                    path,
                    writable,
                    description,
                } => FileTunnelDef::File {
                    name: name.clone(),
                    path: path.clone(),
                    writable: *writable,
                    description: description.clone(),
                },
                CustomFileDef::Folder {
                    name,
                    path,
                    writable,
                    allow_write,
                    include,
                    description,
                } => FileTunnelDef::Folder {
                    name: name.clone(),
                    path: path.clone(),
                    writable: *writable,
                    allow_write: allow_write.clone(),
                    include: include.clone(),
                    validators: Vec::new(),
                    description: description.clone(),
                },
            })
            .collect()
    }

    fn expose_shell_commands(&self) -> Vec<ShellCommandDef> {
        self.config
            .commands
            .iter()
            .map(|c| ShellCommandDef {
                name: c.name.clone(),
                command: c.command.clone(),
                args: c.args.clone(),
                description: c.description.clone(),
                arg_template: c.arg_template.as_ref().map(|at| ShellArgTemplate {
                    label: at.label.clone(),
                    placeholder: at.placeholder.clone(),
                    validation: at.validation.clone(),
                }),
                timeout_secs: c.timeout_secs,
            })
            .collect()
    }

    fn service_inventory(&self) -> Pin<Box<dyn Future<Output = Vec<InventoryEntry>> + Send + '_>> {
        Box::pin(async move { collect_inventory_entries(&self.config.inventory).await })
    }

    fn service_sample(&self) -> Pin<Box<dyn Future<Output = Vec<InventoryEntry>> + Send + '_>> {
        Box::pin(async move { collect_inventory_entries(&self.config.samples).await })
    }

    fn service_security(&self) -> Pin<Box<dyn Future<Output = Vec<SecurityFinding>> + Send + '_>> {
        Box::pin(async move { collect_security_findings(&self.config.security).await })
    }
}

// ── Inventory/sample collection ──────────────────────────────────────────

async fn collect_inventory_entries(defs: &[CustomInventoryDef]) -> Vec<InventoryEntry> {
    let mut entries = Vec::with_capacity(defs.len());
    for def in defs {
        if let Some(value) = resolve_inventory_value(def).await {
            entries.push(InventoryEntry {
                id: def.id.clone(),
                name: def.name.clone(),
                value,
                value_type: def.value_type.clone(),
            });
        }
    }
    entries
}

async fn resolve_inventory_value(def: &CustomInventoryDef) -> Option<serde_json::Value> {
    if let Some(v) = &def.static_value {
        return Some(v.clone());
    }
    let cmd = def.command.as_ref()?;
    let timeout = Duration::from_secs(cmd.timeout_secs);
    let output = tokio::time::timeout(
        timeout,
        tokio::process::Command::new(&cmd.run)
            .args(&cmd.args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output(),
    )
    .await
    .ok()?
    .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let raw = if let Some(re_str) = &cmd.parse_regex {
        let re = regex::Regex::new(re_str).ok()?;
        let caps = re.captures(&stdout)?;
        caps.get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| caps[0].to_string())
    } else {
        stdout.trim().to_string()
    };

    // Convert based on value_type
    Some(match &def.value_type {
        InventoryValueType::Number => {
            if let Ok(n) = raw.parse::<f64>() {
                serde_json::Value::Number(
                    serde_json::Number::from_f64(n).unwrap_or_else(|| serde_json::Number::from(0)),
                )
            } else {
                serde_json::Value::String(raw)
            }
        }
        InventoryValueType::Bool => {
            serde_json::Value::Bool(matches!(raw.as_str(), "true" | "1" | "yes"))
        }
        InventoryValueType::Json => {
            serde_json::from_str(&raw).unwrap_or(serde_json::Value::String(raw))
        }
        InventoryValueType::String => serde_json::Value::String(raw),
    })
}

// ── Security check collection ────────────────────────────────────────────

async fn collect_security_findings(defs: &[CustomSecurityDef]) -> Vec<SecurityFinding> {
    let mut findings = Vec::with_capacity(defs.len());
    for def in defs {
        let pass = evaluate_security_check(def).await;
        findings.push(SecurityFinding {
            id: def.id.clone(),
            severity: def.severity.clone(),
            message: def.message.clone(),
            pass,
        });
    }
    findings
}

async fn evaluate_security_check(def: &CustomSecurityDef) -> bool {
    if let Some(exec) = &def.exec {
        let timeout = Duration::from_secs(exec.timeout_secs);
        let result = tokio::time::timeout(
            timeout,
            tokio::process::Command::new(&exec.command)
                .args(&exec.args)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output(),
        )
        .await;
        return match result {
            Ok(Ok(output)) => output.status.success(),
            _ => false,
        };
    }
    if let Some(fc) = &def.file_check {
        return evaluate_file_check(fc);
    }
    false
}

fn evaluate_file_check(fc: &mac_mgmt_common::custom_service::SecurityFileCheckDef) -> bool {
    let meta = match std::fs::metadata(&fc.path) {
        Ok(m) => m,
        Err(_) => return !fc.exists, // file doesn't exist — pass only if exists=false
    };

    // File exists check
    if !fc.exists {
        return false; // file exists but we expected it not to
    }

    // Permission + owner checks read Unix metadata (st_mode / st_uid); on
    // non-Unix those fields don't exist, so the existence check above is the
    // only enforceable part.
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        // Permission check
        if let Some(max_mode_str) = &fc.max_mode {
            if let Ok(max_mode) = u32::from_str_radix(max_mode_str.trim_start_matches('0'), 8) {
                let actual_mode = meta.mode() & 0o7777;
                // "not more permissive" = actual must be a subset of max_mode bits
                if actual_mode & !max_mode != 0 {
                    return false;
                }
            }
        }

        // Owner check
        if let Some(expected_owner) = &fc.owner {
            // Resolve username to uid
            let actual_uid = meta.uid();
            let expected_uid = resolve_uid(expected_owner);
            if let Some(uid) = expected_uid {
                if actual_uid != uid {
                    return false;
                }
            }
        }
    }
    #[cfg(not(unix))]
    let _ = &meta;

    true
}

#[cfg(unix)]
fn resolve_uid(username: &str) -> Option<u32> {
    // Try numeric first
    if let Ok(uid) = username.parse::<u32>() {
        return Some(uid);
    }
    // Use getpwnam via libc
    use std::ffi::CString;
    let c_name = CString::new(username).ok()?;
    unsafe {
        let pw = libc::getpwnam(c_name.as_ptr());
        if pw.is_null() {
            None
        } else {
            Some((*pw).pw_uid)
        }
    }
}
