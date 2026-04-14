//! OpenClaw probe: parses `openclaw health --json` to catch gateway/subsystem
//! degradations that the current pid-style liveness check misses.
//!
//! A full-prompt round-trip through the openclaw gateway is tracked as a
//! follow-up — wiring that up requires knowing the exact gateway chat
//! endpoint, which is not yet documented in this repo. For now this probe
//! reports `Functional` because it exercises a real subsystem-walk inside
//! openclaw rather than just checking whether the process is alive.

use std::process::Stdio;

use anyhow::{Context, Result};
use async_trait::async_trait;
use mac_mgmt_common::OpenClawConfig;
use tokio::process::Command;

use super::{timed, Probe, ProbeCtx, ProbeKind, ProbeResult};

pub struct OpenClawProbe {
    _gateway_hint: Option<String>,
}

impl OpenClawProbe {
    pub fn from_config(cfg: &OpenClawConfig) -> Self {
        let gateway_hint = cfg.gateway.as_ref().map(|g| {
            let host = if g.host.is_empty() { "127.0.0.1" } else { &g.host };
            format!("http://{host}:{}", g.port)
        });
        Self { _gateway_hint: gateway_hint }
    }

    async fn run_impl(&self, ctx: &ProbeCtx) -> Result<ProbeResult> {
        let out = tokio::time::timeout(
            ctx.timeout,
            Command::new("openclaw")
                .args(["health", "--json"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output(),
        )
        .await
        .context("openclaw health timed out")?
        .context("failed to spawn openclaw health")?;

        let stdout = String::from_utf8_lossy(&out.stdout);
        if !out.status.success() {
            anyhow::bail!(
                "openclaw health exited {}: {}",
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }

        let json: serde_json::Value =
            serde_json::from_str(&stdout).context("failed to parse openclaw health output")?;

        let ok = is_all_healthy(&json);
        let digest = super::digest_hex(stdout.trim().as_bytes());

        Ok(ProbeResult {
            ok,
            canary_digest: Some(digest),
            error_class: if ok { None } else { Some("bad_response".into()) },
            error_detail: if ok {
                None
            } else {
                Some(first_unhealthy(&json).unwrap_or_else(|| "degraded".into()))
            },
            ..Default::default()
        })
    }
}

#[async_trait]
impl Probe for OpenClawProbe {
    fn name(&self) -> &'static str {
        "openclaw"
    }

    fn kind(&self) -> ProbeKind {
        ProbeKind::Functional
    }

    async fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        timed(|| self.run_impl(ctx)).await
    }
}

/// Walk the health JSON and only consider the system healthy if every
/// object with a "status" field reports "ok" or "healthy".
fn is_all_healthy(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Object(map) => {
            if let Some(status) = map.get("status").and_then(|s| s.as_str()) {
                if !matches!(status.to_lowercase().as_str(), "ok" | "healthy" | "ready") {
                    return false;
                }
            }
            map.values().all(is_all_healthy)
        }
        serde_json::Value::Array(arr) => arr.iter().all(is_all_healthy),
        _ => true,
    }
}

fn first_unhealthy(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::Object(map) => {
            if let Some(status) = map.get("status").and_then(|s| s.as_str()) {
                if !matches!(status.to_lowercase().as_str(), "ok" | "healthy" | "ready") {
                    let name = map
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("component");
                    return Some(format!("{name}={status}"));
                }
            }
            map.values().find_map(first_unhealthy)
        }
        serde_json::Value::Array(arr) => arr.iter().find_map(first_unhealthy),
        _ => None,
    }
}
