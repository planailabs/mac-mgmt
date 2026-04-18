pub mod apprise;
pub mod gpu_tool_common;
pub mod lms;
pub mod mcporter;
pub mod nvidia_smi;
pub mod ollama;
pub mod openclaw;
pub mod opencode;
pub mod rocm_smi;

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
