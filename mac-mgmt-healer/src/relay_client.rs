use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::sync::broadcast;

use crate::session::HealerEvent;

const DAEMON_RECONNECT_TIMEOUT: Duration = Duration::from_secs(600);
const DAEMON_POLL_INTERVAL: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// HTTP client for relay proxy endpoints.
///
/// All relay-facing methods automatically wait for the daemon to reconnect
/// if it bounces off (502/503/504). Waits up to 10 minutes, polling every 5s.
/// On timeout, the **original** error is returned.
///
/// When an `events_tx` is set, broadcasts status messages about daemon
/// connectivity so the UI can show the waiting state.
pub struct RelayClient {
    http: reqwest::Client,
    relay_url: String,
    proxy_token: String,
    events_tx: Option<broadcast::Sender<HealerEvent>>,
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

pub struct FileReadResult {
    pub content: Vec<u8>,
    pub mtime: Option<i64>,
}

impl RelayClient {
    pub fn new(relay_url: String, proxy_token: String) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .unwrap_or_default(),
            relay_url: relay_url.trim_end_matches('/').to_string(),
            proxy_token,
            events_tx: None,
        }
    }

    /// Attach an event broadcaster so daemon connectivity changes
    /// are visible in the UI.
    pub fn with_events(mut self, tx: broadcast::Sender<HealerEvent>) -> Self {
        self.events_tx = Some(tx);
        self
    }

    fn broadcast_status(&self, message: &str) {
        if let Some(tx) = &self.events_tx {
            let _ = tx.send(HealerEvent::Status {
                message: message.to_string(),
            });
        }
    }

    fn instance_url(&self, instance_prefix: &str) -> String {
        if let Some(rest) = self.relay_url.strip_prefix("https://") {
            format!("https://{instance_prefix}.{rest}")
        } else if let Some(rest) = self.relay_url.strip_prefix("http://") {
            format!("http://{instance_prefix}.{rest}")
        } else {
            format!("https://{instance_prefix}.{}", self.relay_url)
        }
    }

    fn req(&self, url: &str) -> reqwest::RequestBuilder {
        self.http.get(url).bearer_auth(&self.proxy_token)
    }

    fn post_req(&self, url: &str) -> reqwest::RequestBuilder {
        self.http.post(url).bearer_auth(&self.proxy_token)
    }

    // ── Daemon connectivity ────────────────────────────────────────────

    /// Check if the daemon is connected to the relay.
    /// Uses the lightweight /api/ping endpoint — no forwarding to the daemon,
    /// just a registry lookup on the relay side.
    pub async fn is_daemon_online(&self, instance_prefix: &str) -> bool {
        let base = self.instance_url(instance_prefix);
        let url = format!("{base}/api/ping");
        match self
            .http
            .get(&url)
            .bearer_auth(&self.proxy_token)
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            Ok(resp) => resp.status() == reqwest::StatusCode::OK,
            Err(_) => false,
        }
    }

    /// Wait for the daemon to reconnect to the relay (up to 10 minutes).
    pub async fn wait_for_daemon(&self, instance_prefix: &str) -> Result<()> {
        let deadline = tokio::time::Instant::now() + DAEMON_RECONNECT_TIMEOUT;
        tracing::warn!(
            instance = instance_prefix,
            "daemon appears offline, waiting up to 10m for reconnect"
        );
        self.broadcast_status(
            "Daemon disconnected from relay. Waiting for reconnect (up to 10 minutes)...",
        );
        let mut attempt = 0u32;
        loop {
            tokio::time::sleep(DAEMON_POLL_INTERVAL).await;
            attempt += 1;
            if self.is_daemon_online(instance_prefix).await {
                let secs = attempt * 5;
                tracing::info!(
                    instance = instance_prefix,
                    wait_secs = secs,
                    "daemon back online"
                );
                self.broadcast_status(&format!("Daemon reconnected after {secs}s."));
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                self.broadcast_status("Daemon did not reconnect within 10 minutes.");
                anyhow::bail!(
                    "daemon {} did not reconnect within 10 minutes",
                    instance_prefix
                );
            }
            if attempt % 12 == 0 {
                let elapsed = attempt * 5;
                tracing::debug!(
                    instance = instance_prefix,
                    elapsed_secs = elapsed,
                    "still waiting..."
                );
                self.broadcast_status(&format!("Still waiting for daemon... ({elapsed}s elapsed)"));
            }
        }
    }

    /// Execute a request with automatic daemon reconnect guard.
    /// If the first attempt gets a 502/503/504 or connection error,
    /// waits for the daemon and retries once. On timeout, returns the original error.
    async fn guarded_request<F, Fut>(
        &self,
        instance_prefix: &str,
        label: &str,
        make_request: F,
    ) -> Result<reqwest::Response>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<reqwest::Response, reqwest::Error>>,
    {
        match make_request().await {
            Ok(resp) if is_daemon_offline_status(resp.status()) => {
                let original = format!("{label} returned {}", resp.status());
                if self.wait_for_daemon(instance_prefix).await.is_ok() {
                    make_request()
                        .await
                        .context(format!("{label} failed after reconnect"))
                } else {
                    anyhow::bail!("{original} (daemon did not reconnect)")
                }
            }
            Ok(resp) => Ok(resp),
            Err(e) if is_connection_error(&e) => {
                let original = e.to_string();
                if self.wait_for_daemon(instance_prefix).await.is_ok() {
                    make_request()
                        .await
                        .context(format!("{label} failed after reconnect"))
                } else {
                    anyhow::bail!("{label}: {original} (daemon did not reconnect)")
                }
            }
            Err(e) => Err(e).context(format!("{label} request failed")),
        }
    }

    // ── Public API ─────────────────────────────────────────────────────

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
            .guarded_request(instance_prefix, "file_list", || self.req(&url).send())
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("file_list returned {status}: {body}");
        }
        resp.json()
            .await
            .context("file_list: invalid JSON response")
    }

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
            .guarded_request(instance_prefix, "file_read", || self.req(&url).send())
            .await?;
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
        let bytes = resp
            .bytes()
            .await
            .context("file_read: failed to read body")?;
        Ok(FileReadResult {
            content: bytes.to_vec(),
            mtime,
        })
    }

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
        let content = content.to_vec();
        let resp = self
            .guarded_request(instance_prefix, "file_write", || {
                self.post_req(&url)
                    .header("content-type", "application/octet-stream")
                    .body(content.clone())
                    .send()
            })
            .await?;
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
            .guarded_request(instance_prefix, "shell_exec", || {
                self.post_req(&url).json(&body).send()
            })
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("shell_exec returned {status}: {text}");
        }

        let text = resp
            .text()
            .await
            .context("shell_exec: failed to read body")?;
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
            .guarded_request(instance_prefix, "log_fetch", || self.req(&url).send())
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("log_fetch returned {status}: {body}");
        }
        resp.json()
            .await
            .context("log_fetch: invalid JSON response")
    }
}

fn is_daemon_offline_status(status: reqwest::StatusCode) -> bool {
    // 404 = daemon not in relay registry (disconnected)
    // 502/503/504 = relay can't reach daemon's control channel
    matches!(status.as_u16(), 404 | 502 | 503 | 504)
}

fn is_connection_error(e: &reqwest::Error) -> bool {
    e.is_connect() || e.is_timeout() || {
        let msg = e.to_string().to_lowercase();
        msg.contains("connection refused")
            || msg.contains("connection reset")
            || msg.contains("broken pipe")
    }
}
