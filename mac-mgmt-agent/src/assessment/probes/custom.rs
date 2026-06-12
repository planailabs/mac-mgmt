//! Config-driven custom probes: HTTP or command-based health checks defined
//! entirely in `config.toml` via `[[custom-service.probes]]`.

use std::process::Stdio;

use anyhow::{Context, Result};
use async_trait::async_trait;
use mac_mgmt_common::custom_service::{CustomProbeDef, CustomProbeKind, CustomServiceConfig};
use tokio::process::Command;

use super::{Probe, ProbeCtx, ProbeKind, ProbeResult, timed};

/// A probe whose behavior is defined by config rather than code.
pub struct CustomProbe {
    /// Leak the name so we can return `&'static str` as required by the trait.
    name: &'static str,
    kind: ProbeKind,
    def: CustomProbeDef,
}

impl CustomProbe {
    pub fn new(service_name: &str, def: CustomProbeDef) -> Self {
        let display_name = format!("{}/{}", service_name, def.name);
        Self {
            name: Box::leak(display_name.into_boxed_str()),
            kind: match def.kind {
                CustomProbeKind::Liveness => ProbeKind::Liveness,
                CustomProbeKind::Functional => ProbeKind::Functional,
            },
            def,
        }
    }

    /// Build all custom probes from a list of custom service configs.
    pub fn from_configs(configs: &[CustomServiceConfig]) -> Vec<Box<dyn Probe>> {
        let mut probes: Vec<Box<dyn Probe>> = Vec::new();
        for svc in configs {
            if !svc.enabled {
                continue;
            }
            for probe_def in &svc.probes {
                probes.push(Box::new(CustomProbe::new(&svc.name, probe_def.clone())));
            }
        }
        probes
    }
}

#[async_trait]
impl Probe for CustomProbe {
    fn name(&self) -> &'static str {
        self.name
    }

    fn kind(&self) -> ProbeKind {
        self.kind
    }

    async fn run(&self, _ctx: &ProbeCtx) -> ProbeResult {
        if let Some(http) = &self.def.http {
            timed(|| run_http(http)).await
        } else if let Some(exec) = &self.def.exec {
            timed(|| run_exec(exec)).await
        } else {
            ProbeResult {
                ok: false,
                error_class: Some("config".into()),
                error_detail: Some("probe has neither http nor exec defined".into()),
                ..Default::default()
            }
        }
    }
}

async fn run_http(def: &mac_mgmt_common::custom_service::HttpProbeDef) -> Result<ProbeResult> {
    let timeout = std::time::Duration::from_secs(def.timeout_secs);
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(timeout)
        .build()?;

    let mut req = match def.method.to_uppercase().as_str() {
        "POST" => client.post(&def.url),
        _ => client.get(&def.url),
    };

    for pair in &def.headers {
        req = req.header(&pair[0], &pair[1]);
    }

    if let Some(body) = &def.body {
        req = req.body(body.clone());
    }

    let resp = req.send().await.context("HTTP request failed")?;
    let status = resp.status().as_u16();
    let ok = status == def.expected_status;

    Ok(ProbeResult {
        ok,
        error_class: if ok { None } else { Some("status".into()) },
        error_detail: if ok {
            None
        } else {
            Some(format!("expected {}, got {status}", def.expected_status))
        },
        ..Default::default()
    })
}

async fn run_exec(def: &mac_mgmt_common::custom_service::ExecProbeDef) -> Result<ProbeResult> {
    let timeout = std::time::Duration::from_secs(def.timeout_secs);
    let output = tokio::time::timeout(
        timeout,
        Command::new(&def.command)
            .args(&def.args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .context("probe command timed out")?
    .context("failed to spawn probe command")?;

    Ok(ProbeResult {
        ok: output.status.success(),
        error_class: if output.status.success() {
            None
        } else {
            Some("error".into())
        },
        error_detail: if output.status.success() {
            None
        } else {
            Some(String::from_utf8_lossy(&output.stderr).trim().to_string())
        },
        ..Default::default()
    })
}
