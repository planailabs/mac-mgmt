//! AI proxy liveness probe — HTTP GET `/health` returns 200.

use anyhow::{Context, Result};
use async_trait::async_trait;
use mac_mgmt_common::AiProxyConfig;

use super::{Probe, ProbeCtx, ProbeKind, ProbeResult, timed};

pub struct AiProxyProbe {
    url: String,
}

impl AiProxyProbe {
    pub fn from_config(cfg: &AiProxyConfig) -> Self {
        let host = if cfg.host.is_empty() {
            "127.0.0.1"
        } else {
            &cfg.host
        };
        Self {
            url: format!("http://{host}:{}/health", cfg.port),
        }
    }
}

#[async_trait]
impl Probe for AiProxyProbe {
    fn name(&self) -> &'static str {
        "ai-proxy"
    }

    fn kind(&self) -> ProbeKind {
        ProbeKind::Liveness
    }

    async fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        timed(|| run_impl(&self.url, ctx)).await
    }
}

async fn run_impl(url: &str, ctx: &ProbeCtx) -> Result<ProbeResult> {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(2))
        .timeout(ctx.timeout)
        .build()?;

    let resp = client
        .get(url)
        .send()
        .await
        .context("ai-proxy /health request failed")?;

    let ok = resp.status().is_success();
    Ok(ProbeResult {
        ok,
        error_class: if ok { None } else { Some("unhealthy".into()) },
        error_detail: if ok {
            None
        } else {
            Some(format!("health endpoint returned {}", resp.status()))
        },
        ..Default::default()
    })
}
