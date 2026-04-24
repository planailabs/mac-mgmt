use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

/// Read an HTTP response and return its body as text, bail! with status +
/// body snippet on non-2xx. Used to produce actionable errors when the
/// server replies with something unexpected (HTML error pages, empty
/// bodies from old server versions missing a route, etc).
async fn read_ok(op: &str, resp: reqwest::Response) -> Result<String> {
    let status = resp.status();
    let ct = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        let snippet = body.chars().take(400).collect::<String>();
        bail!("{op}: status={status} content-type={ct} body={snippet:?}");
    }
    Ok(body)
}

async fn read_json<T: DeserializeOwned>(op: &str, resp: reqwest::Response) -> Result<T> {
    let body = read_ok(op, resp).await?;
    if body.trim().is_empty() {
        bail!("{op}: empty response body (older server missing this route?)");
    }
    serde_json::from_str::<T>(&body).with_context(|| {
        let snippet: String = body.chars().take(400).collect();
        format!("{op}: response was not JSON: {snippet:?}")
    })
}

/// HTTP client against the mgmt admin API.
pub struct MgmtClient {
    http: reqwest::Client,
    base: String,
    admin_token: String,
    organization_id: Uuid,
}

#[derive(Debug, Clone, Serialize)]
struct CreateClusterBody<'a> {
    name: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatedCluster {
    pub id: Uuid,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdminClusterRow {
    pub id: Uuid,
    pub name: String,
    #[allow(dead_code)] // present in API response, kept for structural completeness
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RolloutGroupRow {
    pub id: Uuid,
    pub name: String,
    #[allow(dead_code)] // deserialized from API but not accessed by runner
    #[serde(default)]
    pub description: String,
    #[allow(dead_code)] // deserialized from API but not accessed by runner
    #[serde(default)]
    pub member_count: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SkillChannelRow {
    pub id: Uuid,
    pub installed: bool,
    #[serde(default)]
    pub cluster_skill_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OptionRow {
    pub id: Uuid,
    pub installed: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpServerOptionRow {
    pub id: Uuid,
    pub installed: bool,
    #[serde(default)]
    pub cluster_mcp_server_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClusterBundleRow {
    pub cluster_bundle_id: Uuid,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClusterMcpBundleRow {
    pub cluster_mcp_bundle_id: Uuid,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdminMachineRow {
    pub instance_id: String,
    pub hostname: Option<String>,
    pub version: String,
    pub reported_at: DateTime<Utc>,
    #[serde(default)]
    pub services_extended: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default)]
pub struct CloudInitRequest<'a> {
    pub system: &'a str,
    pub server_url: Option<&'a str>,
    pub daemon_version: Option<&'a str>,
    pub label: Option<&'a str>,
    pub host_key_pem: Option<&'a str>,
    pub instance_id: Option<&'a str>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CloudInitResponse {
    pub cloud_init: String,
    #[allow(dead_code)] // surfaced in logs / future use
    pub sync_token: String,
    #[allow(dead_code)] // runner already knows it; this just echoes
    pub instance_id: Option<String>,
}

impl MgmtClient {
    pub fn new(base: &str, admin_token: &str, organization_id: Uuid) -> Result<Self> {
        let http = reqwest::Client::builder()
            .build()
            .context("building http client")?;
        Ok(Self {
            http,
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
                .context("admin token contains invalid characters")?,
        );
        if let Some(cid) = cluster_id {
            h.insert(
                "X-Cluster-Id",
                HeaderValue::from_str(&cid.to_string()).unwrap(),
            );
        }
        Ok(h)
    }

    /// Preflight: hit /api/self to confirm the URL points at the REST API
    /// (not the OIDC-protected web UI) and that the admin token works.
    /// Called once at startup so errors show up before the first cell boots.
    pub async fn preflight(&self) -> Result<()> {
        let url = format!("{}/api/self", self.base);
        let resp = self
            .http
            .get(&url)
            .headers(self.headers(None)?)
            .send()
            .await
            .with_context(|| format!("preflight: GET {url}"))?;
        let status = resp.status();
        let ct = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            if body.starts_with("<!doctype")
                || body.starts_with("<html")
                || ct.contains("text/html")
            {
                bail!(
                    "mgmt.url ({}) looks like the web UI, not the REST API. \
                     Point it at the server's api.external_url (default port 7378). \
                     Got status={status} content-type={ct}",
                    self.base
                );
            }
            bail!(
                "preflight failed: status={status} content-type={ct} body={:?}",
                body.chars().take(400).collect::<String>()
            );
        }
        #[derive(Deserialize)]
        struct SelfInfo {
            token_kind: String,
        }
        let info: SelfInfo = serde_json::from_str(&body).with_context(|| {
            let snippet: String = body.chars().take(400).collect();
            format!("preflight: /api/self did not return JSON: {snippet:?}")
        })?;
        if info.token_kind != "admin" {
            bail!(
                "mgmt.admin_token is a {:?} token, but the runner requires an admin token",
                info.token_kind
            );
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
        let resp = self
            .http
            .post(format!(
                "{}/api/admin/organizations/{}/clusters",
                self.base, self.organization_id
            ))
            .headers(self.headers(None)?)
            .json(&CreateClusterBody { name })
            .send()
            .await
            .context("creating cluster")?;
        read_json(&format!("create_cluster({name})"), resp).await
    }

    pub async fn delete_cluster(&self, cluster_id: Uuid) -> Result<()> {
        let resp = self
            .http
            .delete(format!("{}/api/admin/clusters/{}", self.base, cluster_id))
            .headers(self.headers(None)?)
            .send()
            .await
            .context("deleting cluster")?;
        let status = resp.status();
        if status.as_u16() == 404 {
            return Ok(());
        }
        let _ = read_ok(&format!("delete_cluster({cluster_id})"), resp).await?;
        Ok(())
    }

    pub async fn put_config(&self, cluster_id: Uuid, config: &serde_json::Value) -> Result<()> {
        // Server-side SetConfigBody uses #[serde(flatten)] so the entire
        // request body IS the ClusterConfig — no envelope.
        let resp = self
            .http
            .put(format!("{}/api/setting/config", self.base))
            .headers(self.headers(Some(cluster_id))?)
            .json(config)
            .send()
            .await
            .context("putting cluster config")?;
        let status = resp.status();
        if !status.is_success() {
            let ct = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            let resp_body = resp.text().await.unwrap_or_default();
            let sent = serde_json::to_string(config).unwrap_or_default();
            bail!(
                "put_config({cluster_id}): status={status} content-type={ct} \
                 response_body={:?} sent_body={sent}",
                resp_body.chars().take(400).collect::<String>()
            );
        }
        Ok(())
    }

    pub async fn get_cloud_init(
        &self,
        cluster_id: Uuid,
        req: &CloudInitRequest<'_>,
    ) -> Result<CloudInitResponse> {
        #[derive(Serialize)]
        struct Body<'a> {
            system: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            server_url: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            daemon_version: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            label: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            host_key_pem: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            instance_id: Option<&'a str>,
        }
        let resp = self
            .http
            .post(format!("{}/api/setting/cloud-init", self.base))
            .headers(self.headers(Some(cluster_id))?)
            .json(&Body {
                system: req.system,
                server_url: req.server_url,
                daemon_version: req.daemon_version,
                label: req.label,
                host_key_pem: req.host_key_pem,
                instance_id: req.instance_id,
            })
            .send()
            .await
            .context("fetching cloud-init")?;
        read_json(&format!("get_cloud_init({cluster_id})"), resp).await
    }

    pub async fn list_rollout_groups(&self) -> Result<Vec<RolloutGroupRow>> {
        let resp = self
            .http
            .get(format!("{}/api/admin/rollout-groups", self.base))
            .headers(self.headers(None)?)
            .send()
            .await
            .context("listing rollout groups")?;
        read_json("list_rollout_groups", resp).await
    }

    pub async fn create_rollout_group(&self, name: &str, description: &str) -> Result<()> {
        #[derive(Serialize)]
        struct Body<'a> {
            name: &'a str,
            description: &'a str,
        }
        let resp = self
            .http
            .post(format!("{}/api/admin/rollout-groups", self.base))
            .headers(self.headers(None)?)
            .json(&Body { name, description })
            .send()
            .await
            .context("creating rollout group")?;
        let _ = read_ok(&format!("create_rollout_group({name})"), resp).await?;
        Ok(())
    }

    pub async fn delete_rollout_group(&self, group_id: Uuid) -> Result<()> {
        let resp = self
            .http
            .delete(format!(
                "{}/api/admin/rollout-groups/{}",
                self.base, group_id
            ))
            .headers(self.headers(None)?)
            .send()
            .await
            .context("deleting rollout group")?;
        if resp.status().as_u16() == 404 {
            return Ok(());
        }
        let _ = read_ok(&format!("delete_rollout_group({group_id})"), resp).await?;
        Ok(())
    }

    pub async fn add_rollout_group_member(&self, group_id: Uuid, cluster_id: Uuid) -> Result<()> {
        #[derive(Serialize)]
        struct Body {
            cluster_id: Uuid,
        }
        let resp = self
            .http
            .post(format!(
                "{}/api/admin/rollout-groups/{}/members",
                self.base, group_id
            ))
            .headers(self.headers(None)?)
            .json(&Body { cluster_id })
            .send()
            .await
            .context("adding rollout group member")?;
        // 409 = already a member, which is fine for idempotent reconcile.
        let status = resp.status();
        if status.is_success() || status.as_u16() == 409 {
            return Ok(());
        }
        let body = resp.text().await.unwrap_or_default();
        bail!(
            "add_rollout_group_member(group={group_id}, cluster={cluster_id}): \
             status={status} body={:?}",
            body.chars().take(300).collect::<String>()
        );
    }

    // ── Setting: skills/bundles/mcp-servers/mcp-bundles ──────────────

    pub async fn list_available_skill_channels(
        &self,
        cluster_id: Uuid,
    ) -> Result<Vec<SkillChannelRow>> {
        let resp = self
            .http
            .get(format!(
                "{}/api/setting/available/skill-channels",
                self.base
            ))
            .headers(self.headers(Some(cluster_id))?)
            .send()
            .await?;
        read_json(&format!("available_skill_channels({cluster_id})"), resp).await
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
            .await?;
        let _ = read_ok(
            &format!("add_skill({cluster_id}, {skill_channel_id})"),
            resp,
        )
        .await?;
        Ok(())
    }

    pub async fn remove_skill(&self, cluster_id: Uuid, cluster_skill_id: Uuid) -> Result<()> {
        let resp = self
            .http
            .delete(format!(
                "{}/api/setting/skills/{}",
                self.base, cluster_skill_id
            ))
            .headers(self.headers(Some(cluster_id))?)
            .send()
            .await?;
        let _ = read_ok(&format!("remove_skill({cluster_skill_id})"), resp).await?;
        Ok(())
    }

    pub async fn list_available_bundles(&self, cluster_id: Uuid) -> Result<Vec<OptionRow>> {
        let resp = self
            .http
            .get(format!("{}/api/setting/available/bundles", self.base))
            .headers(self.headers(Some(cluster_id))?)
            .send()
            .await?;
        read_json(&format!("available_bundles({cluster_id})"), resp).await
    }

    pub async fn list_cluster_bundles(&self, cluster_id: Uuid) -> Result<Vec<ClusterBundleRow>> {
        let resp = self
            .http
            .get(format!("{}/api/setting/bundles", self.base))
            .headers(self.headers(Some(cluster_id))?)
            .send()
            .await?;
        read_json(&format!("cluster_bundles({cluster_id})"), resp).await
    }

    pub async fn add_bundle(&self, cluster_id: Uuid, bundle_id: Uuid) -> Result<()> {
        #[derive(Serialize)]
        struct Body {
            bundle_id: Uuid,
        }
        let resp = self
            .http
            .post(format!("{}/api/setting/bundles", self.base))
            .headers(self.headers(Some(cluster_id))?)
            .json(&Body { bundle_id })
            .send()
            .await?;
        let _ = read_ok(&format!("add_bundle({cluster_id}, {bundle_id})"), resp).await?;
        Ok(())
    }

    pub async fn remove_bundle(&self, cluster_id: Uuid, cluster_bundle_id: Uuid) -> Result<()> {
        let resp = self
            .http
            .delete(format!(
                "{}/api/setting/bundles/{}",
                self.base, cluster_bundle_id
            ))
            .headers(self.headers(Some(cluster_id))?)
            .send()
            .await?;
        let _ = read_ok(&format!("remove_bundle({cluster_bundle_id})"), resp).await?;
        Ok(())
    }

    pub async fn list_available_mcp_servers(
        &self,
        cluster_id: Uuid,
    ) -> Result<Vec<McpServerOptionRow>> {
        let resp = self
            .http
            .get(format!("{}/api/setting/available/mcp-servers", self.base))
            .headers(self.headers(Some(cluster_id))?)
            .send()
            .await?;
        read_json(&format!("available_mcp_servers({cluster_id})"), resp).await
    }

    pub async fn add_mcp_server(&self, cluster_id: Uuid, mcp_server_id: Uuid) -> Result<()> {
        #[derive(Serialize)]
        struct Body {
            mcp_server_id: Uuid,
        }
        let resp = self
            .http
            .post(format!("{}/api/setting/mcp-servers", self.base))
            .headers(self.headers(Some(cluster_id))?)
            .json(&Body { mcp_server_id })
            .send()
            .await?;
        let _ = read_ok(
            &format!("add_mcp_server({cluster_id}, {mcp_server_id})"),
            resp,
        )
        .await?;
        Ok(())
    }

    pub async fn remove_mcp_server(
        &self,
        cluster_id: Uuid,
        cluster_mcp_server_id: Uuid,
    ) -> Result<()> {
        let resp = self
            .http
            .delete(format!(
                "{}/api/setting/mcp-servers/{}",
                self.base, cluster_mcp_server_id
            ))
            .headers(self.headers(Some(cluster_id))?)
            .send()
            .await?;
        let _ = read_ok(&format!("remove_mcp_server({cluster_mcp_server_id})"), resp).await?;
        Ok(())
    }

    pub async fn list_available_mcp_bundles(&self, cluster_id: Uuid) -> Result<Vec<OptionRow>> {
        let resp = self
            .http
            .get(format!("{}/api/setting/available/mcp-bundles", self.base))
            .headers(self.headers(Some(cluster_id))?)
            .send()
            .await?;
        read_json(&format!("available_mcp_bundles({cluster_id})"), resp).await
    }

    pub async fn list_cluster_mcp_bundles(
        &self,
        cluster_id: Uuid,
    ) -> Result<Vec<ClusterMcpBundleRow>> {
        let resp = self
            .http
            .get(format!("{}/api/setting/mcp-bundles", self.base))
            .headers(self.headers(Some(cluster_id))?)
            .send()
            .await?;
        read_json(&format!("cluster_mcp_bundles({cluster_id})"), resp).await
    }

    pub async fn add_mcp_bundle(&self, cluster_id: Uuid, bundle_id: Uuid) -> Result<()> {
        #[derive(Serialize)]
        struct Body {
            bundle_id: Uuid,
        }
        let resp = self
            .http
            .post(format!("{}/api/setting/mcp-bundles", self.base))
            .headers(self.headers(Some(cluster_id))?)
            .json(&Body { bundle_id })
            .send()
            .await?;
        let _ = read_ok(&format!("add_mcp_bundle({cluster_id}, {bundle_id})"), resp).await?;
        Ok(())
    }

    pub async fn remove_mcp_bundle(
        &self,
        cluster_id: Uuid,
        cluster_mcp_bundle_id: Uuid,
    ) -> Result<()> {
        let resp = self
            .http
            .delete(format!(
                "{}/api/setting/mcp-bundles/{}",
                self.base, cluster_mcp_bundle_id
            ))
            .headers(self.headers(Some(cluster_id))?)
            .send()
            .await?;
        let _ = read_ok(&format!("remove_mcp_bundle({cluster_mcp_bundle_id})"), resp).await?;
        Ok(())
    }

    pub async fn list_cluster_machines(&self, cluster_id: Uuid) -> Result<Vec<AdminMachineRow>> {
        let resp = self
            .http
            .get(format!(
                "{}/api/admin/clusters/{}/machines",
                self.base, cluster_id
            ))
            .headers(self.headers(None)?)
            .send()
            .await
            .context("listing cluster machines")?;
        read_json(&format!("list_cluster_machines({cluster_id})"), resp).await
    }
}
