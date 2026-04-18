use anyhow::{Context, Result};
use serde::Deserialize;

/// HTTP client for relay proxy endpoints.
/// Talks to file tunnels, shell tunnels, and log endpoints on remote daemon instances
/// through the relay's proxy layer.
pub struct RelayClient {
    http: reqwest::Client,
    relay_url: String,
    proxy_token: String,
}

#[derive(Debug, Deserialize)]
pub struct ShellOutput {
    pub lines: Vec<ShellLine>,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ShellLine {
    pub stream: String,
    pub data: String,
}

impl RelayClient {
    pub fn new(relay_url: String, proxy_token: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            relay_url: relay_url.trim_end_matches('/').to_string(),
            proxy_token,
        }
    }

    /// Build the base URL for a specific instance, using subdomain routing.
    /// relay_url = "https://relay.plan.ai" + instance_prefix = "a1b2c3d4e5f6"
    /// → "https://a1b2c3d4e5f6.relay.plan.ai"
    fn instance_url(&self, instance_prefix: &str) -> String {
        // Parse the relay URL to extract scheme and host
        if let Some(rest) = self.relay_url.strip_prefix("https://") {
            format!("https://{instance_prefix}.{rest}")
        } else if let Some(rest) = self.relay_url.strip_prefix("http://") {
            format!("http://{instance_prefix}.{rest}")
        } else {
            format!("https://{instance_prefix}.{}", self.relay_url)
        }
    }

    /// List files in a file tunnel directory.
    pub async fn file_list(
        &self,
        instance_prefix: &str,
        tunnel_name: &str,
        path: Option<&str>,
    ) -> Result<serde_json::Value> {
        let base = self.instance_url(instance_prefix);
        let mut url = format!("{base}/api/files/{tunnel_name}");
        if let Some(p) = path {
            url.push_str(&format!("?path={}", urlencoding::encode(p)));
        }
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&self.proxy_token)
            .send()
            .await
            .context("file_list request failed")?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("file_list returned {status}: {body}");
        }
        resp.json().await.context("file_list: invalid JSON response")
    }

    /// Read a file from a file tunnel.
    pub async fn file_read(
        &self,
        instance_prefix: &str,
        tunnel_name: &str,
        path: &str,
    ) -> Result<FileReadResult> {
        let base = self.instance_url(instance_prefix);
        let url = format!(
            "{base}/api/files/{tunnel_name}/read?path={}",
            urlencoding::encode(path)
        );
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&self.proxy_token)
            .send()
            .await
            .context("file_read request failed")?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("file_read returned {status}: {body}");
        }
        let mtime = resp
            .headers()
            .get("x-file-mtime")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<i64>().ok());
        let bytes = resp.bytes().await.context("file_read: failed to read body")?;
        Ok(FileReadResult {
            content: bytes.to_vec(),
            mtime,
        })
    }

    /// Write a file to a file tunnel.
    pub async fn file_write(
        &self,
        instance_prefix: &str,
        tunnel_name: &str,
        path: &str,
        content: &[u8],
        expected_mtime: Option<i64>,
    ) -> Result<serde_json::Value> {
        let base = self.instance_url(instance_prefix);
        let mut url = format!(
            "{base}/api/files/{tunnel_name}/write?path={}",
            urlencoding::encode(path)
        );
        if let Some(mtime) = expected_mtime {
            url.push_str(&format!("&expected_mtime={mtime}"));
        }
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.proxy_token)
            .header("content-type", "application/octet-stream")
            .body(content.to_vec())
            .send()
            .await
            .context("file_write request failed")?;
        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .unwrap_or_else(|_| serde_json::json!({"status": status.as_u16()}));
        if !status.is_success() {
            anyhow::bail!("file_write returned {status}: {body}");
        }
        Ok(body)
    }

    /// Execute a predefined shell command and collect all output.
    /// The relay returns SSE events; we collect them into a single ShellOutput.
    pub async fn shell_exec(
        &self,
        instance_prefix: &str,
        command_name: &str,
        user_arg: Option<&str>,
    ) -> Result<ShellOutput> {
        let base = self.instance_url(instance_prefix);
        let url = format!("{base}/api/shell/{command_name}/exec");
        let body = match user_arg {
            Some(arg) => serde_json::json!({"user_arg": arg}),
            None => serde_json::json!({}),
        };
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.proxy_token)
            .json(&body)
            .send()
            .await
            .context("shell_exec request failed")?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("shell_exec returned {status}: {text}");
        }

        // Parse SSE stream
        let text = resp.text().await.context("shell_exec: failed to read body")?;
        let mut lines = Vec::new();
        let mut exit_code = None;
        let mut error = None;

        for line in text.lines() {
            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };
            let Ok(obj) = serde_json::from_str::<serde_json::Value>(data) else {
                continue;
            };
            if let Some(code) = obj.get("exit_code").and_then(|v| v.as_i64()) {
                exit_code = Some(code as i32);
                error = obj.get("error").and_then(|v| v.as_str()).map(String::from);
            } else if let (Some(stream), Some(data)) = (
                obj.get("stream").and_then(|v| v.as_str()),
                obj.get("data").and_then(|v| v.as_str()),
            ) {
                lines.push(ShellLine {
                    stream: stream.to_string(),
                    data: data.to_string(),
                });
            }
        }

        Ok(ShellOutput {
            lines,
            exit_code,
            error,
        })
    }

    /// Fetch logs from a daemon instance.
    pub async fn log_fetch(
        &self,
        instance_prefix: &str,
        n: Option<usize>,
        service: Option<&str>,
        after: Option<usize>,
    ) -> Result<serde_json::Value> {
        let base = self.instance_url(instance_prefix);
        let mut params = Vec::new();
        if let Some(n) = n {
            params.push(format!("n={n}"));
        }
        if let Some(svc) = service {
            params.push(format!("service={}", urlencoding::encode(svc)));
        }
        if let Some(after) = after {
            params.push(format!("after={after}"));
        }
        let query = if params.is_empty() {
            String::new()
        } else {
            format!("?{}", params.join("&"))
        };
        let url = format!("{base}/api/logs{query}");
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&self.proxy_token)
            .send()
            .await
            .context("log_fetch request failed")?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("log_fetch returned {status}: {body}");
        }
        resp.json().await.context("log_fetch: invalid JSON response")
    }
}

pub struct FileReadResult {
    pub content: Vec<u8>,
    pub mtime: Option<i64>,
}
