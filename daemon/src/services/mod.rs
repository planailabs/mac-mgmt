pub mod ai_proxy_svc;
pub mod apprise;
pub mod custom_svc;
#[cfg(feature = "memvault")]
pub mod memvault_svc;
pub mod gpu_tool_common;
pub mod litellm;
pub mod lms;
pub mod mcporter;
pub mod nvidia_smi;
pub mod ollama;
pub mod openclaw;
pub mod opencode;
#[cfg(feature = "relay")]
pub mod relay_svc;
pub mod restic;
pub mod rocm_smi;
#[cfg(feature = "relay")]
pub mod swarm_svc;
pub mod unsloth;

use anyhow::Result;
use std::time::Duration;

/// Non-blocking HTTP GET using reqwest. Must be called from an async context.
pub async fn http_get(host: &str, port: u16, path: &str) -> Result<String> {
    let url = format!("http://{host}:{port}{path}");
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(5))
        .build()?;
    let resp = client.get(&url).send().await?;
    Ok(resp.text().await?)
}
