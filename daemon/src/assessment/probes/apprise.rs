//! Apprise liveness probe — `apprise --version` succeeds.
//!
//! We don't dry-run a notification against configured URLs here because that
//! would either (a) fail harmlessly for URLs that reject empty messages or
//! (b) succeed by sending a real message. Both are worse than not testing.
//! A real dry-run requires a dedicated apprise endpoint that doesn't transmit.

use std::process::Stdio;

use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::process::Command;

use super::{Probe, ProbeCtx, ProbeKind, ProbeResult, timed};

pub struct AppriseProbe;

#[async_trait]
impl Probe for AppriseProbe {
    fn name(&self) -> &'static str {
        "apprise"
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
        Command::new("apprise")
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .context("apprise --version timed out")?
    .context("failed to spawn apprise")?;

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
