//! HTTP client for relay proxy endpoints (file/shell/log tunnels, TCP tunnels).
//!
//! Adapted from `mac-mgmt-healer/src/relay_client.rs`, decoupled from the
//! healer's event/instance types and extended with a Host-override mode so the
//! relay can be reached over a forwarded port (antithesis env) as well as by
//! subdomain (prod).

use anyhow::{Context, Result};
use reqwest::header::HeaderValue;

/// How to address a specific daemon instance through the relay.
#[derive(Clone)]
pub enum HostMode {
    /// Prod: relay is reachable at its real hostname; instances are selected by
    /// subdomain prefix — `https://{prefix}.relay.example.com`.
    Subdomain,
    /// Antithesis: relay is reached at a fixed base URL (a forwarded port) and
    /// the instance is selected via an explicit Host header
    /// `{prefix}.{proxy_hostname}` (the relay strips the port and matches).
    Override { proxy_hostname: String },
}

/// A single shell-exec output line.
#[derive(Debug, Clone)]
pub struct ShellLine {
    pub stream: String,
    pub data: String,
}

/// Result of a shell-tunnel exec.
#[derive(Debug, Clone)]
pub struct ShellOutput {
    pub lines: Vec<ShellLine>,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
}

impl ShellOutput {
    /// Concatenated stdout lines.
    pub fn stdout(&self) -> String {
        self.lines
            .iter()
            .filter(|l| l.stream == "stdout")
            .map(|l| l.data.as_str())
            .collect::<Vec<_>>()
            .join("")
    }
}

pub struct RelayApi {
    http: reqwest::Client,
    base: String,
    proxy_token: String,
    mode: HostMode,
}

impl RelayApi {
    pub fn new(base: &str, proxy_token: &str, mode: HostMode) -> Self {
        Self::with_opts(base, proxy_token, mode, false)
    }

    /// `insecure_tls` accepts self-signed certs (the antithesis test images use
    /// a baked self-signed CA).
    pub fn with_opts(base: &str, proxy_token: &str, mode: HostMode, insecure_tls: bool) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .danger_accept_invalid_certs(insecure_tls)
                .build()
                .unwrap_or_default(),
            base: base.trim_end_matches('/').to_string(),
            proxy_token: proxy_token.to_string(),
            mode,
        }
    }

    /// Build the request URL + the Host header override (if any) for an
    /// instance prefix (optionally with a tunnel segment, `prefix-tunnel`).
    fn target(&self, prefix: &str, path: &str) -> (String, Option<HeaderValue>) {
        match &self.mode {
            HostMode::Subdomain => {
                let url = if let Some(rest) = self.base.strip_prefix("https://") {
                    format!("https://{prefix}.{rest}{path}")
                } else if let Some(rest) = self.base.strip_prefix("http://") {
                    format!("http://{prefix}.{rest}{path}")
                } else {
                    format!("https://{prefix}.{}{path}", self.base)
                };
                (url, None)
            }
            HostMode::Override { proxy_hostname } => {
                let host = format!("{prefix}.{proxy_hostname}");
                (
                    format!("{}{path}", self.base),
                    HeaderValue::from_str(&host).ok(),
                )
            }
        }
    }

    fn get(&self, prefix: &str, path: &str) -> reqwest::RequestBuilder {
        let (url, host) = self.target(prefix, path);
        let mut rb = self
            .http
            .get(url)
            .header("x-proxy-token", &self.proxy_token);
        if let Some(h) = host {
            rb = rb.header(reqwest::header::HOST, h);
        }
        rb
    }

    fn post(&self, prefix: &str, path: &str) -> reqwest::RequestBuilder {
        let (url, host) = self.target(prefix, path);
        let mut rb = self
            .http
            .post(url)
            .header("x-proxy-token", &self.proxy_token);
        if let Some(h) = host {
            rb = rb.header(reqwest::header::HOST, h);
        }
        rb
    }

    /// Is the daemon connected to the relay? (lightweight, relay-side only)
    pub async fn is_online(&self, prefix: &str) -> bool {
        match self.get(prefix, "/api/ping").send().await {
            Ok(r) => r.status().is_success(),
            Err(_) => false,
        }
    }

    /// List entries of a file tunnel.
    pub async fn file_list(&self, prefix: &str, tunnel: &str) -> Result<serde_json::Value> {
        let resp = self
            .get(prefix, &format!("/api/files/{tunnel}"))
            .send()
            .await
            .context("file_list")?;
        let status = resp.status();
        if !status.is_success() {
            anyhow::bail!("file_list {tunnel} returned {status}");
        }
        resp.json().await.context("file_list: invalid JSON")
    }

    /// Run a shell tunnel command. `user_arg` is the optional argument string.
    pub async fn shell_exec(
        &self,
        prefix: &str,
        command_name: &str,
        user_arg: Option<&str>,
    ) -> Result<ShellOutput> {
        let body = match user_arg {
            Some(arg) => serde_json::json!({ "user_arg": arg }),
            None => serde_json::json!({}),
        };
        let resp = self
            .post(prefix, &format!("/api/shell/{command_name}/exec"))
            .json(&body)
            .send()
            .await
            .context("shell_exec")?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("shell_exec {command_name} returned {status}: {text}");
        }
        let text = resp.text().await.context("shell_exec: read body")?;
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
            } else if let (Some(stream), Some(d)) = (
                obj.get("stream").and_then(|v| v.as_str()),
                obj.get("data").and_then(|v| v.as_str()),
            ) {
                lines.push(ShellLine {
                    stream: stream.to_string(),
                    data: d.to_string(),
                });
            }
        }
        Ok(ShellOutput {
            lines,
            exit_code,
            error,
        })
    }

    /// Fetch recent logs.
    pub async fn logs(&self, prefix: &str, n: Option<usize>) -> Result<serde_json::Value> {
        let path = match n {
            Some(n) => format!("/api/logs?n={n}"),
            None => "/api/logs".to_string(),
        };
        let resp = self.get(prefix, &path).send().await.context("logs")?;
        let status = resp.status();
        if !status.is_success() {
            anyhow::bail!("logs returned {status}");
        }
        resp.json().await.context("logs: invalid JSON")
    }

    /// List the daemon's advertised TCP tunnels.
    pub async fn tunnels(&self, prefix: &str) -> Result<serde_json::Value> {
        let resp = self
            .get(prefix, "/api/tunnels")
            .send()
            .await
            .context("tunnels")?;
        let status = resp.status();
        if !status.is_success() {
            anyhow::bail!("tunnels returned {status}");
        }
        resp.json().await.context("tunnels: invalid JSON")
    }
}
