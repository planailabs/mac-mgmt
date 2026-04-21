use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::sync::broadcast;

use crate::instance_access::{FileReadResult, ShellLine, ShellOutput};
use crate::instance_data::DynInstanceData;
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
    /// Instance data source + full instance_id for heartbeat-based fallback checks.
    heartbeat_ctx: Option<(DynInstanceData, String)>,
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
            heartbeat_ctx: None,
        }
    }

    /// Attach an event broadcaster so daemon connectivity changes
    /// are visible in the UI.
    pub fn with_events(mut self, tx: broadcast::Sender<HealerEvent>) -> Self {
        self.events_tx = Some(tx);
        self
    }

    /// Attach an instance data source + instance_id for heartbeat-based connectivity fallback.
    pub fn with_heartbeat_ctx(mut self, data: DynInstanceData, instance_id: String) -> Self {
        self.heartbeat_ctx = Some((data, instance_id));
        self
    }

    fn broadcast_status(&self, message: &str) {
        if let Some(tx) = &self.events_tx {
            let _ = tx.send(HealerEvent::Status {
                message: message.to_string(),
            });
        }
    }

    /// The relay's base URL (e.g. "https://relay.example.com").
    pub fn relay_base_url(&self) -> &str {
        &self.relay_url
    }

    /// The underlying HTTP client.
    pub fn http_client(&self) -> &reqwest::Client {
        &self.http
    }

    /// The proxy/bearer token.
    pub fn proxy_token(&self) -> &str {
        &self.proxy_token
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
        tracing::debug!(url = %url, instance = instance_prefix, "checking daemon connectivity");
        match self
            .http
            .get(&url)
            .bearer_auth(&self.proxy_token)
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            Ok(resp) => {
                let online = resp.status() == reqwest::StatusCode::OK;
                tracing::debug!(
                    instance = instance_prefix,
                    status = %resp.status(),
                    online,
                    "ping response"
                );
                online
            }
            Err(e) => {
                tracing::debug!(
                    instance = instance_prefix,
                    err = %e,
                    "ping request failed"
                );
                false
            }
        }
    }

    /// Check if we've received a recent heartbeat (within 2 minutes).
    /// The daemon may be alive and heartbeating to the server even when
    /// its relay WS connection is down.
    async fn check_recent_heartbeat(&self, data: &dyn crate::instance_data::InstanceDataSource, instance_id: &str) -> bool {
        data.has_recent_heartbeat(instance_id).await.unwrap_or(false)
    }

    /// Wait for the daemon to reconnect to the relay (up to 10 minutes).
    /// If instance data + instance_id are provided, also checks heartbeat
    /// freshness — a daemon that's sending heartbeats but not connected
    /// to the relay gets a shorter wait and a more helpful status message.
    pub async fn wait_for_daemon_with_heartbeat(
        &self,
        instance_prefix: &str,
        data: Option<&dyn crate::instance_data::InstanceDataSource>,
        instance_id: Option<&str>,
    ) -> Result<()> {
        let deadline = tokio::time::Instant::now() + DAEMON_RECONNECT_TIMEOUT;
        tracing::warn!(
            instance = instance_prefix,
            "daemon appears offline from relay, waiting up to 10m for reconnect"
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
                    "daemon back online on relay"
                );
                self.broadcast_status(&format!("Daemon reconnected after {secs}s."));
                return Ok(());
            }

            // Check heartbeat as secondary signal
            let heartbeat_alive = match (data, instance_id) {
                (Some(d), Some(iid)) => self.check_recent_heartbeat(d, iid).await,
                _ => false,
            };

            if tokio::time::Instant::now() >= deadline {
                if heartbeat_alive {
                    self.broadcast_status(
                        "Daemon is sending heartbeats but not connected to relay. \
                         Relay-dependent tools (file editing, shell commands) are unavailable.",
                    );
                    anyhow::bail!(
                        "daemon {} is alive (heartbeating) but not connected to relay",
                        instance_prefix
                    );
                }
                self.broadcast_status("Daemon did not reconnect within 10 minutes.");
                anyhow::bail!(
                    "daemon {} did not reconnect within 10 minutes",
                    instance_prefix
                );
            }
            if attempt % 12 == 0 {
                let elapsed = attempt * 5;
                let extra = if heartbeat_alive {
                    " (daemon is heartbeating but relay WS is down)"
                } else {
                    ""
                };
                tracing::debug!(
                    instance = instance_prefix,
                    elapsed_secs = elapsed,
                    heartbeat_alive,
                    "still waiting for relay reconnect..."
                );
                self.broadcast_status(&format!(
                    "Still waiting for relay reconnect... ({elapsed}s elapsed){extra}"
                ));
            }
        }
    }

    /// Wait for the daemon to reconnect to the relay (up to 10 minutes).
    /// Automatically uses heartbeat context if configured.
    pub async fn wait_for_daemon(&self, instance_prefix: &str) -> Result<()> {
        let (data, iid): (Option<&dyn crate::instance_data::InstanceDataSource>, _) =
            match &self.heartbeat_ctx {
                Some((d, i)) => (Some(d.as_ref()), Some(i.as_str())),
                None => (None, None),
            };
        self.wait_for_daemon_with_heartbeat(instance_prefix, data, iid)
            .await
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
        tracing::debug!(instance = instance_prefix, label, "guarded_request: sending");
        match make_request().await {
            Ok(resp) if is_daemon_offline_status(resp.status()) => {
                tracing::warn!(
                    instance = instance_prefix,
                    label,
                    status = %resp.status(),
                    "guarded_request: daemon offline status, entering wait loop"
                );
                let original = format!("{label} returned {}", resp.status());
                if self.wait_for_daemon(instance_prefix).await.is_ok() {
                    tracing::debug!(label, "guarded_request: retrying after reconnect");
                    make_request()
                        .await
                        .context(format!("{label} failed after reconnect"))
                } else {
                    anyhow::bail!("{original} (daemon did not reconnect)")
                }
            }
            Ok(resp) => {
                tracing::debug!(
                    instance = instance_prefix,
                    label,
                    status = %resp.status(),
                    "guarded_request: ok"
                );
                Ok(resp)
            }
            Err(e) if is_connection_error(&e) => {
                tracing::warn!(
                    instance = instance_prefix,
                    label,
                    err = %e,
                    "guarded_request: connection error, entering wait loop"
                );
                let original = e.to_string();
                if self.wait_for_daemon(instance_prefix).await.is_ok() {
                    tracing::debug!(label, "guarded_request: retrying after reconnect");
                    make_request()
                        .await
                        .context(format!("{label} failed after reconnect"))
                } else {
                    anyhow::bail!("{label}: {original} (daemon did not reconnect)")
                }
            }
            Err(e) => {
                tracing::debug!(
                    instance = instance_prefix,
                    label,
                    err = %e,
                    "guarded_request: non-connection error"
                );
                Err(e).context(format!("{label} request failed"))
            }
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

// ── InstanceAccess adapter ─────────────────────────────────────────────

/// [`InstanceAccess`] implementation that routes through a relay proxy.
/// The `instance_prefix` is baked in at construction.
pub struct RelayInstanceAccess {
    relay: Arc<RelayClient>,
    instance_prefix: String,
}

impl RelayInstanceAccess {
    pub fn new(relay: Arc<RelayClient>, instance_prefix: String) -> Self {
        Self {
            relay,
            instance_prefix,
        }
    }

    /// Access the underlying relay client (e.g. for metrics endpoint).
    pub fn relay(&self) -> &RelayClient {
        &self.relay
    }
}

#[async_trait::async_trait]
impl crate::instance_access::InstanceAccess for RelayInstanceAccess {
    async fn file_list(
        &self,
        tunnel_name: &str,
        path: Option<&str>,
    ) -> Result<serde_json::Value> {
        self.relay
            .file_list(&self.instance_prefix, tunnel_name, path)
            .await
    }

    async fn file_read(
        &self,
        tunnel_name: &str,
        path: &str,
    ) -> Result<FileReadResult> {
        self.relay
            .file_read(&self.instance_prefix, tunnel_name, path)
            .await
    }

    async fn file_write(
        &self,
        tunnel_name: &str,
        path: &str,
        content: &[u8],
        expected_mtime: Option<i64>,
    ) -> Result<serde_json::Value> {
        self.relay
            .file_write(&self.instance_prefix, tunnel_name, path, content, expected_mtime)
            .await
    }

    async fn shell_exec(
        &self,
        command_name: &str,
        user_arg: Option<&str>,
    ) -> Result<ShellOutput> {
        self.relay
            .shell_exec(&self.instance_prefix, command_name, user_arg)
            .await
    }

    async fn log_fetch(
        &self,
        n: Option<usize>,
        service: Option<&str>,
        after: Option<usize>,
    ) -> Result<serde_json::Value> {
        self.relay
            .log_fetch(&self.instance_prefix, n, service, after)
            .await
    }

    async fn is_online(&self) -> bool {
        self.relay.is_daemon_online(&self.instance_prefix).await
    }

    async fn wait_until_online(&self, _max_secs: u64) -> Result<()> {
        self.relay.wait_for_daemon(&self.instance_prefix).await
    }
}

/// [`ClusterAccess`] implementation that creates [`RelayInstanceAccess`] for peer nodes.
pub struct RelayClusterAccess {
    relay: Arc<RelayClient>,
}

impl RelayClusterAccess {
    pub fn new(relay: Arc<RelayClient>) -> Self {
        Self { relay }
    }
}

#[async_trait::async_trait]
impl crate::instance_access::ClusterAccess for RelayClusterAccess {
    async fn instance_access(
        &self,
        instance_prefix: &str,
    ) -> Result<crate::instance_access::DynInstanceAccess> {
        Ok(Arc::new(RelayInstanceAccess::new(
            self.relay.clone(),
            instance_prefix.to_string(),
        )))
    }
}
