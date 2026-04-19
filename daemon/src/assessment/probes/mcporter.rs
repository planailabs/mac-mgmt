//! McPorter liveness probe — binary presence + `--version` succeeds.

use std::process::Stdio;

use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::process::Command;

use super::{Probe, ProbeCtx, ProbeKind, ProbeResult, timed};

pub struct McPorterProbe;

#[async_trait]
impl Probe for McPorterProbe {
    fn name(&self) -> &'static str {
        "mcporter"
    }

    fn kind(&self) -> ProbeKind {
        ProbeKind::Liveness
    }

    async fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        timed(|| run_impl(ctx)).await
    }
}

async fn run_impl(ctx: &ProbeCtx) -> Result<ProbeResult> {
    let out = tokio::time::timeout(
        ctx.timeout,
        Command::new("mcporter")
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .context("mcporter --version timed out")?
    .context("failed to spawn mcporter")?;

    Ok(ProbeResult {
        ok: out.status.success(),
        error_class: if out.status.success() {
            None
        } else {
            Some("error".into())
        },
        error_detail: if out.status.success() {
            None
        } else {
            Some(String::from_utf8_lossy(&out.stderr).trim().to_string())
        },
        ..Default::default()
    })
}
