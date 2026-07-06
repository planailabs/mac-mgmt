//! Concrete `Env` implementations for prod and antithesis, plus a thin mmrcd
//! HTTP client used by the antithesis env.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use antithesis_workloads::env::{Env, NodeKind, Timeouts};
use antithesis_workloads::mgmt::MgmtApi;
use antithesis_workloads::relay::{HostMode, RelayApi};
use async_trait::async_trait;
use uuid::Uuid;

use crate::api::*;

// ── mmrcd HTTP client ───────────────────────────────────────────────

pub struct MmrcdClient {
    http: reqwest::Client,
    base: String,
    token: String,
}

impl MmrcdClient {
    pub fn new(base: &str, token: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            base: base.trim_end_matches('/').to_string(),
            token: token.to_string(),
        }
    }

    fn req(&self, rb: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        rb.bearer_auth(&self.token)
    }

    pub async fn create_run(&self, req: &CreateRunRequest) -> Result<RunInfo> {
        let resp = self
            .req(self.http.post(format!("{}/runs", self.base)))
            .json(req)
            .send()
            .await
            .context("mmrcd create_run")?;
        json_or_bail("create_run", resp).await
    }

    pub async fn spawn_nodes(&self, run_id: &str, req: &SpawnNodesRequest) -> Result<SpawnNodesResponse> {
        let resp = self
            .req(self.http.post(format!("{}/runs/{run_id}/nodes", self.base)))
            .json(req)
            .send()
            .await
            .context("mmrcd spawn_nodes")?;
        json_or_bail("spawn_nodes", resp).await
    }

    pub async fn delete_run(&self, run_id: &str) -> Result<()> {
        self.req(self.http.delete(format!("{}/runs/{run_id}", self.base)))
            .send()
            .await
            .context("mmrcd delete_run")?;
        Ok(())
    }
}

async fn json_or_bail<T: serde::de::DeserializeOwned>(op: &str, resp: reqwest::Response) -> Result<T> {
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!("{op}: mmrcd returned {status}: {body}");
    }
    serde_json::from_str(&body).with_context(|| format!("{op}: bad mmrcd JSON: {body}"))
}

// ── Prod env ────────────────────────────────────────────────────────

pub struct ProdEnv {
    mgmt: MgmtApi,
    relay: RelayApi,
    organization_id: Uuid,
    cluster_name: String,
    timeouts: Timeouts,
}

impl ProdEnv {
    pub fn new(cfg: &crate::config::ProdConfig) -> Result<Self> {
        let mgmt = MgmtApi::new(&cfg.server_url, &cfg.admin_token, cfg.organization_id)?;
        // Prod relay is reached at its real hostname; proxy token minted lazily
        // per-workload would be ideal, but a single token for the run is fine.
        let relay = RelayApi::new(&cfg.relay_url, "", HostMode::Subdomain);
        Ok(Self {
            mgmt,
            relay,
            organization_id: cfg.organization_id,
            cluster_name: cfg.cluster_name.clone(),
            timeouts: Timeouts {
                ec: Duration::from_secs(cfg.ec_timeout_secs),
                poll: Duration::from_secs(5),
            },
        })
    }

    pub fn timeouts(&self) -> Timeouts {
        self.timeouts.clone()
    }
}

#[async_trait]
impl Env for ProdEnv {
    fn name(&self) -> &str {
        "prod"
    }
    fn mgmt(&self) -> &MgmtApi {
        &self.mgmt
    }
    fn relay(&self) -> &RelayApi {
        &self.relay
    }
    fn organization_id(&self) -> Uuid {
        self.organization_id
    }
    fn cluster_name(&self) -> &str {
        &self.cluster_name
    }

    async fn spawn_nodes(&self, _cluster_id: Uuid, _n: usize, _kind: NodeKind) -> Result<Vec<String>> {
        bail!(
            "prod env does not spawn nodes: fleet nodes are provisioned by the mmr \
             runner and the chaos API is disabled in production. Target existing \
             enrolled nodes instead."
        );
    }

    async fn remove_node(&self, _cluster_id: Uuid, _instance_id: &str) -> Result<()> {
        bail!("prod env does not remove nodes");
    }

    fn relay_prefix(&self, instance_id: &str) -> String {
        // Prod relay advertises instances by the first 12 hex of the id.
        instance_id.chars().take(12).collect()
    }
}

// ── Antithesis env ──────────────────────────────────────────────────

pub struct AntithesisEnv {
    mgmt: MgmtApi,
    relay: RelayApi,
    organization_id: Uuid,
    cluster_name: String,
    mmrcd: MmrcdClient,
    run_id: String,
    timeouts: Timeouts,
}

impl AntithesisEnv {
    /// Build against a live mmrcd run. `server_url` / `relay_url` must be
    /// host-reachable (mmrcd returns instance addresses).
    pub fn new(
        cfg: &crate::config::AntithesisConfig,
        run_id: String,
        server_url: &str,
        relay_url: &str,
        proxy_token: &str,
    ) -> Result<Self> {
        let mgmt = MgmtApi::with_opts(
            server_url,
            &cfg.admin_token,
            cfg.organization_id,
            cfg.insecure_tls,
            Some(cfg.server_hostname.clone()),
        )?;
        let relay = RelayApi::with_opts(
            relay_url,
            proxy_token,
            HostMode::Override {
                proxy_hostname: cfg.proxy_hostname.clone(),
            },
            cfg.insecure_tls,
        );
        Ok(Self {
            mgmt,
            relay,
            organization_id: cfg.organization_id,
            cluster_name: cfg.cluster_name.clone(),
            mmrcd: MmrcdClient::new(&cfg.mmrcd_url, &cfg.mmrcd_token),
            run_id,
            timeouts: Timeouts {
                ec: Duration::from_secs(cfg.ec_timeout_secs),
                poll: Duration::from_secs(3),
            },
        })
    }

    pub fn timeouts(&self) -> Timeouts {
        self.timeouts.clone()
    }

    pub fn mmrcd(&self) -> &MmrcdClient {
        &self.mmrcd
    }
    pub fn run_id(&self) -> &str {
        &self.run_id
    }
}

#[async_trait]
impl Env for AntithesisEnv {
    fn name(&self) -> &str {
        "antithesis"
    }
    fn mgmt(&self) -> &MgmtApi {
        &self.mgmt
    }
    fn relay(&self) -> &RelayApi {
        &self.relay
    }
    fn organization_id(&self) -> Uuid {
        self.organization_id
    }
    fn cluster_name(&self) -> &str {
        &self.cluster_name
    }

    async fn spawn_nodes(&self, cluster_id: Uuid, n: usize, kind: NodeKind) -> Result<Vec<String>> {
        // Mint one cluster sync token shared by the new daemons.
        let sync_token = self
            .mgmt
            .create_cluster_token(cluster_id, "sync", &format!("mmrc-{}", self.run_id))
            .await?;

        // Snapshot existing instance ids, spawn, then diff to learn the new ids.
        let before: std::collections::HashSet<String> = self
            .mgmt
            .list_cluster_machines(cluster_id)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|m| m.instance_id)
            .collect();

        self.mmrcd
            .spawn_nodes(
                &self.run_id,
                &SpawnNodesRequest {
                    count: n,
                    kind: match kind {
                        NodeKind::Fleet => "fleet".into(),
                        NodeKind::Chaos => "chaos".into(),
                    },
                    cluster_id,
                    sync_token,
                },
            )
            .await?;

        // Poll for `n` new instance ids to enroll.
        let deadline = tokio::time::Instant::now() + self.timeouts.ec;
        loop {
            let now: Vec<String> = self
                .mgmt
                .list_cluster_machines(cluster_id)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|m| m.instance_id)
                .collect();
            let new: Vec<String> = now.into_iter().filter(|id| !before.contains(id)).collect();
            if new.len() >= n {
                if kind == NodeKind::Chaos {
                    for id in &new {
                        self.mgmt
                            .register_chaos_node(cluster_id, id, "mmrc-chaos")
                            .await
                            .ok();
                    }
                }
                return Ok(new);
            }
            if tokio::time::Instant::now() >= deadline {
                bail!("only {} of {n} spawned nodes enrolled before timeout", new.len());
            }
            tokio::time::sleep(self.timeouts.poll).await;
        }
    }

    async fn remove_node(&self, cluster_id: Uuid, instance_id: &str) -> Result<()> {
        self.mgmt.delete_chaos_node(cluster_id, instance_id).await.ok();
        Ok(())
    }

    fn relay_prefix(&self, instance_id: &str) -> String {
        instance_id.chars().take(12).collect()
    }
}
