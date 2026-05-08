use std::collections::HashMap;

use mac_mgmt_common::{
    FederationCatalog, FederationEvent, McpServerEntry, ResolveSkillsRequest,
    ResolveMcpServersRequest, SkillResolveEntry,
};
use tokio::sync::mpsc;

/// HTTP client for talking to a remote skill center's federation API.
pub struct SkillCenterClient {
    http: reqwest::Client,
    base_url: String,
    token: String,
}

impl SkillCenterClient {
    pub fn new(base_url: String, token: String) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("failed to build HTTP client"),
            base_url: base_url.trim_end_matches('/').to_string(),
            token,
        }
    }

    /// Fetch the full catalog from the skill center.
    pub async fn fetch_catalog(&self) -> Result<FederationCatalog, String> {
        let resp = self
            .http
            .get(format!("{}/api/federation/catalog", self.base_url))
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| format!("federation catalog fetch failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("skill center returned {}", resp.status()));
        }

        resp.json()
            .await
            .map_err(|e| format!("failed to parse catalog response: {e}"))
    }

    /// Resolve skill slugs+channels to nix store paths for a given architecture.
    pub async fn resolve_skills(
        &self,
        skills: &[(String, String)], // (slug, channel) pairs
        arch: &str,
    ) -> Result<HashMap<String, String>, String> {
        let body = ResolveSkillsRequest {
            skills: skills
                .iter()
                .map(|(slug, channel)| SkillResolveEntry {
                    slug: slug.clone(),
                    channel: channel.clone(),
                })
                .collect(),
            arch: arch.to_string(),
        };

        let resp = self
            .http
            .post(format!(
                "{}/api/federation/resolve-skills",
                self.base_url
            ))
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("resolve-skills request failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("skill center returned {}", resp.status()));
        }

        resp.json()
            .await
            .map_err(|e| format!("failed to parse resolve-skills response: {e}"))
    }

    /// Resolve MCP server slugs to their configs and nix packages.
    pub async fn resolve_mcp_servers(
        &self,
        slugs: &[String],
    ) -> Result<HashMap<String, McpServerEntry>, String> {
        let body = ResolveMcpServersRequest {
            slugs: slugs.to_vec(),
        };

        let resp = self
            .http
            .post(format!(
                "{}/api/federation/resolve-mcp-servers",
                self.base_url
            ))
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("resolve-mcp-servers request failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("skill center returned {}", resp.status()));
        }

        resp.json()
            .await
            .map_err(|e| format!("failed to parse resolve-mcp-servers response: {e}"))
    }

    /// Subscribe to the federation SSE event stream.
    /// Spawns a background task that reads SSE events and sends them to the returned receiver.
    /// The task exits when the SSE connection drops.
    pub async fn subscribe_events(
        &self,
    ) -> Result<mpsc::Receiver<Result<FederationEvent, String>>, String> {
        // Use a client without the default 30s timeout for the long-lived SSE connection
        let sse_client = reqwest::Client::builder()
            .build()
            .map_err(|e| format!("failed to build SSE client: {e}"))?;

        let resp = sse_client
            .get(format!("{}/api/federation/events", self.base_url))
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| format!("SSE connection failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("skill center returned {}", resp.status()));
        }

        let (tx, rx) = mpsc::channel(32);

        tokio::spawn(async move {
            use tokio_stream::StreamExt;
            let mut stream = resp.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        if let Ok(text) = std::str::from_utf8(&bytes) {
                            buffer.push_str(text);

                            // Process complete lines
                            while let Some(newline_pos) = buffer.find('\n') {
                                let line = buffer[..newline_pos].trim().to_string();
                                buffer = buffer[newline_pos + 1..].to_string();

                                if let Some(data) = line.strip_prefix("data:") {
                                    let data = data.trim();
                                    if data.is_empty() {
                                        continue;
                                    }
                                    let event = serde_json::from_str::<FederationEvent>(data)
                                        .map_err(|e| format!("failed to parse SSE data: {e}"));
                                    if tx.send(event).await.is_err() {
                                        return; // receiver dropped
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(format!("SSE read error: {e}"))).await;
                        return;
                    }
                }
            }
        });

        Ok(rx)
    }
}
