//! HTTP client for the mac-mgmt server admin/setting API.
//!
//! Adapted from `runner/src/mgmt.rs` (MgmtClient) — reimplemented rather than
//! extracted so this crate stays dependency-light and can add the endpoints the
//! runner doesn't use (admin push, probes, chaos-node registration).

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

async fn read_ok(op: &str, resp: reqwest::Response) -> Result<String> {
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        let snippet = body.chars().take(400).collect::<String>();
        bail!("{op}: status={status} body={snippet:?}");
    }
    Ok(body)
}

async fn read_json<T: DeserializeOwned>(op: &str, resp: reqwest::Response) -> Result<T> {
    let body = read_ok(op, resp).await?;
    if body.trim().is_empty() {
        bail!("{op}: empty response body");
    }
    serde_json::from_str::<T>(&body)
        .with_context(|| format!("{op}: response was not JSON: {:?}", body.chars().take(400).collect::<String>()))
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdminClusterRow {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatedCluster {
    pub id: Uuid,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatedToken {
    pub token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProxyTokenResponse {
    pub token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MachineRow {
    pub instance_id: String,
    #[serde(default)]
    pub hostname: Option<String>,
    pub version: String,
    pub reported_at: DateTime<Utc>,
    #[serde(default)]
    pub services_extended: Option<serde_json::Value>,
    #[serde(default)]
    pub chaos: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SkillChannelRow {
    pub id: Uuid,
    pub installed: bool,
    #[serde(default)]
    pub cluster_skill_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProbeRow {
    pub instance_id: String,
    pub service: String,
    pub kind: String,
    pub ok: bool,
    #[serde(default)]
    pub error_class: Option<String>,
    #[serde(default)]
    pub error_detail: Option<String>,
    pub collected_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdminPushResult {
    pub ok: bool,
    #[serde(default)]
    pub receivers: usize,
}

/// Admin/setting API client.
pub struct MgmtApi {
    http: reqwest::Client,
    base: String,
    admin_token: String,
    organization_id: Uuid,
}

impl MgmtApi {
    pub fn new(base: &str, admin_token: &str, organization_id: Uuid) -> Result<Self> {
        Ok(Self {
            http: reqwest::Client::builder()
                .build()
                .context("building http client")?,
            base: base.trim_end_matches('/').to_string(),
            admin_token: admin_token.to_string(),
            organization_id,
        })
    }

    fn headers(&self, cluster_id: Option<Uuid>) -> Result<HeaderMap> {
        let mut h = HeaderMap::new();
        h.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.admin_token))
                .context("admin token has invalid characters")?,
        );
        if let Some(cid) = cluster_id {
            h.insert("X-Cluster-Id", HeaderValue::from_str(&cid.to_string()).unwrap());
        }
        Ok(h)
    }

    /// Confirm the URL is the REST API and the token is an admin token.
    pub async fn preflight(&self) -> Result<()> {
        let resp = self
            .http
            .get(format!("{}/api/self", self.base))
            .headers(self.headers(None)?)
            .send()
            .await
            .context("preflight GET /api/self")?;
        #[derive(Deserialize)]
        struct SelfInfo {
            token_kind: String,
        }
        let info: SelfInfo = read_json("preflight", resp).await?;
        if info.token_kind != "admin" {
            bail!("token is a {:?} token, need admin", info.token_kind);
        }
        Ok(())
    }

    pub async fn list_clusters(&self) -> Result<Vec<AdminClusterRow>> {
        let resp = self
            .http
            .get(format!("{}/api/admin/clusters", self.base))
            .headers(self.headers(None)?)
            .send()
            .await
            .context("listing clusters")?;
        read_json("list_clusters", resp).await
    }

    pub async fn create_cluster(&self, name: &str) -> Result<CreatedCluster> {
        #[derive(Serialize)]
        struct Body<'a> {
            name: &'a str,
        }
        let resp = self
            .http
            .post(format!(
                "{}/api/admin/organizations/{}/clusters",
                self.base, self.organization_id
            ))
            .headers(self.headers(None)?)
            .json(&Body { name })
            .send()
            .await
            .context("creating cluster")?;
        read_json(&format!("create_cluster({name})"), resp).await
    }

    /// Find a cluster by name, or create it.
    pub async fn ensure_cluster(&self, name: &str) -> Result<Uuid> {
        if let Some(c) = self.list_clusters().await?.into_iter().find(|c| c.name == name) {
            return Ok(c.id);
        }
        Ok(self.create_cluster(name).await?.id)
    }

    pub async fn put_config(&self, cluster_id: Uuid, config: &serde_json::Value) -> Result<()> {
        let resp = self
            .http
            .put(format!("{}/api/setting/config", self.base))
            .headers(self.headers(Some(cluster_id))?)
            .json(config)
            .send()
            .await
            .context("putting cluster config")?;
        let _ = read_ok(&format!("put_config({cluster_id})"), resp).await?;
        Ok(())
    }

    /// Mint a token for the cluster. `kind` is "sync" or "setting".
    pub async fn create_cluster_token(
        &self,
        cluster_id: Uuid,
        kind: &str,
        label: &str,
    ) -> Result<String> {
        #[derive(Serialize)]
        struct Body<'a> {
            label: &'a str,
            kind: &'a str,
        }
        let resp = self
            .http
            .post(format!("{}/api/admin/clusters/{cluster_id}/tokens", self.base))
            .headers(self.headers(None)?)
            .json(&Body { label, kind })
            .send()
            .await
            .context("creating cluster token")?;
        let t: CreatedToken = read_json("create_cluster_token", resp).await?;
        Ok(t.token)
    }

    /// Mint a short-lived proxy token for relay tunnel access. Empty `scopes`
    /// means all scopes.
    pub async fn create_proxy_token(&self, cluster_id: Uuid, scopes: &[&str]) -> Result<String> {
        #[derive(Serialize)]
        struct Body<'a> {
            scopes: &'a [&'a str],
        }
        let resp = self
            .http
            .post(format!("{}/api/proxy-token", self.base))
            .headers(self.headers(Some(cluster_id))?)
            .json(&Body { scopes })
            .send()
            .await
            .context("creating proxy token")?;
        let t: ProxyTokenResponse = read_json("create_proxy_token", resp).await?;
        Ok(t.token)
    }

    pub async fn list_cluster_machines(&self, cluster_id: Uuid) -> Result<Vec<MachineRow>> {
        let resp = self
            .http
            .get(format!("{}/api/admin/clusters/{cluster_id}/machines", self.base))
            .headers(self.headers(None)?)
            .send()
            .await
            .context("listing machines")?;
        read_json("list_cluster_machines", resp).await
    }

    pub async fn list_probes(&self, cluster_id: Uuid) -> Result<Vec<ProbeRow>> {
        let resp = self
            .http
            .get(format!("{}/api/admin/clusters/{cluster_id}/probes", self.base))
            .headers(self.headers(None)?)
            .send()
            .await
            .context("listing probes")?;
        read_json("list_probes", resp).await
    }

    /// Push an SSE event to the cluster, or a single instance when `instance_id`
    /// is set. `event` is a snake_case name (e.g. "sync_skills").
    pub async fn push_event(
        &self,
        cluster_id: Uuid,
        event: &str,
        instance_id: Option<&str>,
    ) -> Result<AdminPushResult> {
        #[derive(Serialize)]
        struct Body<'a> {
            event: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            instance_id: Option<&'a str>,
        }
        let resp = self
            .http
            .post(format!("{}/api/admin/clusters/{cluster_id}/push", self.base))
            .headers(self.headers(None)?)
            .json(&Body { event, instance_id })
            .send()
            .await
            .context("pushing event")?;
        read_json("push_event", resp).await
    }

    // ── Chaos-node registration (antithesis server only) ────────────────

    pub async fn register_chaos_node(
        &self,
        cluster_id: Uuid,
        instance_id: &str,
        label: &str,
    ) -> Result<()> {
        #[derive(Serialize)]
        struct Body<'a> {
            instance_id: &'a str,
            label: &'a str,
        }
        let resp = self
            .http
            .post(format!("{}/api/admin/clusters/{cluster_id}/chaos-nodes", self.base))
            .headers(self.headers(None)?)
            .json(&Body { instance_id, label })
            .send()
            .await
            .context("registering chaos node")?;
        let _ = read_ok("register_chaos_node", resp).await?;
        Ok(())
    }

    pub async fn delete_chaos_node(&self, cluster_id: Uuid, instance_id: &str) -> Result<()> {
        let resp = self
            .http
            .delete(format!(
                "{}/api/admin/clusters/{cluster_id}/chaos-nodes/{instance_id}",
                self.base
            ))
            .headers(self.headers(None)?)
            .send()
            .await
            .context("deleting chaos node")?;
        if resp.status().as_u16() == 404 {
            return Ok(());
        }
        let _ = read_ok("delete_chaos_node", resp).await?;
        Ok(())
    }

    // ── Skills ──────────────────────────────────────────────────────────

    pub async fn list_available_skill_channels(
        &self,
        cluster_id: Uuid,
    ) -> Result<Vec<SkillChannelRow>> {
        let resp = self
            .http
            .get(format!("{}/api/setting/available/skill-channels", self.base))
            .headers(self.headers(Some(cluster_id))?)
            .send()
            .await
            .context("listing skill channels")?;
        read_json("list_available_skill_channels", resp).await
    }

    pub async fn add_skill(&self, cluster_id: Uuid, skill_channel_id: Uuid) -> Result<()> {
        #[derive(Serialize)]
        struct Body {
            skill_channel_id: Uuid,
        }
        let resp = self
            .http
            .post(format!("{}/api/setting/skills", self.base))
            .headers(self.headers(Some(cluster_id))?)
            .json(&Body { skill_channel_id })
            .send()
            .await
            .context("adding skill")?;
        let _ = read_ok("add_skill", resp).await?;
        Ok(())
    }

    pub async fn remove_skill(&self, cluster_id: Uuid, cluster_skill_id: Uuid) -> Result<()> {
        let resp = self
            .http
            .delete(format!("{}/api/setting/skills/{cluster_skill_id}", self.base))
            .headers(self.headers(Some(cluster_id))?)
            .send()
            .await
            .context("removing skill")?;
        let _ = read_ok("remove_skill", resp).await?;
        Ok(())
    }
}
