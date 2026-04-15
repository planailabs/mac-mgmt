use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use mac_mgmt_common::ClusterConfig;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("create_cluster({name}): {status} {body}");
        }
        Ok(resp.json().await.context("decoding created cluster")?)
    }

    pub async fn delete_cluster(&self, cluster_id: Uuid) -> Result<()> {
        let resp = self
            .http
            .delete(format!("{}/api/admin/clusters/{}", self.base, cluster_id))
            .headers(self.headers(None)?)
            .send()
            .await
            .context("deleting cluster")?;
        if !resp.status().is_success() && resp.status().as_u16() != 404 {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("delete_cluster({cluster_id}): {status} {body}");
        }
        Ok(())
    }

    pub async fn put_config(&self, cluster_id: Uuid, config: &ClusterConfig) -> Result<()> {
        #[derive(Serialize)]
        struct Body<'a> {
            config: &'a ClusterConfig,
        }
        let resp = self
            .http
            .put(format!("{}/api/setting/config", self.base))
            .headers(self.headers(Some(cluster_id))?)
            .json(&Body { config })
            .send()
            .await
            .context("putting cluster config")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("put_config({cluster_id}): {status} {body}");
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
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("get_cloud_init({cluster_id}): {status} {body}");
        }
        Ok(resp.json().await.context("decoding cloud-init response")?)
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
        if !resp.status().is_success() {
            let status = resp.status();
            bail!("list_cluster_machines: {status}");
        }
        Ok(resp.json().await.context("decoding machines")?)
    }
}
