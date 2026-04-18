//! OpenCode functional probe.
//!
//! Sends a health check HTTP request to the OpenCode server's endpoint
//! to verify it's running and responsive.

use anyhow::{Context, Result};
use async_trait::async_trait;
use mac_mgmt_common::OpencodeConfig;
use reqwest::Client;

use super::{digest_hex, timed, Probe, ProbeCtx, ProbeKind, ProbeResult};

pub struct OpencodeProbe {
    base_url: String,
}

impl OpencodeProbe {
    pub fn from_config(cfg: &OpencodeConfig) -> Self {
        let host = if cfg.host.is_empty() { "127.0.0.1" } else { &cfg.host };
        Self {
            base_url: format!("http://{host}:{}", cfg.port),
        }
    }

    async fn run_impl(&self, ctx: &ProbeCtx) -> Result<ProbeResult> {
        let client = Client::builder().timeout(ctx.timeout).build()?;
        let resp = client
            .get(&self.base_url)
            .send()
            .await
            .context("opencode server unreachable")?;

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        // Any HTTP response (even 404) means the server is running.
        let ok = status.is_success() || status.as_u16() == 404;

        Ok(ProbeResult {
            ok,
            canary_digest: Some(digest_hex(body.trim().as_bytes())),
            error_class: if ok { None } else { Some("bad_status".into()) },
            error_detail: if ok { None } else { Some(format!("HTTP {status}")) },
            ..Default::default()
        })
    }
}

#[async_trait]
impl Probe for OpencodeProbe {
    fn name(&self) -> &'static str {
        "opencode"
    }

    fn kind(&self) -> ProbeKind {
        ProbeKind::Liveness
    }

    async fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        timed(|| self.run_impl(ctx)).await
    }
}
