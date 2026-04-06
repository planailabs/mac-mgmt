pub mod mcporter;
pub mod nexa;
pub mod ollama;
pub mod openclaw;

use anyhow::{Context, Result};
use std::time::Duration;

/// Send a blocking HTTP GET and return the response body as a string.
pub fn http_get(host: &str, port: u16, path: &str) -> Result<String> {
    let url = format!("http://{host}:{port}{path}");
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .context("failed to build HTTP client")?
        .get(&url)
        .send()
        .with_context(|| format!("GET {url} failed"))?
        .text()
        .context("failed to read response body")
}
