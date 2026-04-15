//! Reconciliation loop and operations.
//!
//! The orchestrator owns a [`FleetState`] under a mutex. The reconcile loop
//! ensures that for each matrix cell there is exactly one Incus instance and
//! one mgmt cluster, and that the cluster's config matches the cell's spec.
//! A secondary timer picks a random cell at an interval and reprovisions it.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use rand::seq::SliceRandom;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::config::{RunnerConfig, parse_duration};
use crate::host_key;
use crate::incus::{CreateInstanceSpec, IncusClient};
use crate::matrix::{MatrixCell, generate};
use crate::mgmt::{CloudInitRequest, MgmtClient};
use crate::state::{CellState, FleetState};

pub struct Orchestrator {
    pub config: RunnerConfig,
    pub mgmt: MgmtClient,
    pub incus: IncusClient,
    pub state: Mutex<FleetState>,
}

impl Orchestrator {
    pub fn new(config: RunnerConfig, mgmt: MgmtClient, incus: IncusClient) -> Result<Self> {
        let state = FleetState::load(&config.fleet.state_path)?;
        Ok(Self {
            config,
            mgmt,
            incus,
            state: Mutex::new(state),
        })
    }

    pub fn matrix(&self) -> Vec<MatrixCell> {
        generate(&self.config.matrix)
    }

    fn instance_name(&self, key: &str) -> String {
        let mut s = format!("{}{}", self.config.incus.name_prefix, key);
        // Incus instance names: lowercase letters, digits, hyphens; start with letter; ≤63 chars.
        s = s
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c.to_ascii_lowercase() } else { '-' })
            .collect();
        if s.len() > 63 {
            s.truncate(63);
        }
        s
    }

    fn cluster_name(&self, key: &str) -> String {
        format!("{}{}", self.config.incus.name_prefix, key)
    }

    pub async fn save_state(&self) -> Result<()> {
        let s = self.state.lock().await;
        s.save(&self.config.fleet.state_path)
    }

    /// Ensure one cell is present. If an instance is already running, this is
    /// a no-op beyond re-pushing the cluster config. Otherwise a fresh host
    /// key is generated, its instance_id persisted, and an Incus instance is
    /// launched with cloud-init carrying that key.
    pub async fn ensure_cell(&self, cell: &MatrixCell) -> Result<CellState> {
        let instance_name = self.instance_name(&cell.key);
        let cluster_name = self.cluster_name(&cell.key);

        let existing = {
            let s = self.state.lock().await;
            s.find(&cell.key).cloned()
        };

        if let Some(s) = existing.as_ref() {
            if s.parked {
                tracing::debug!("cell {} is parked; skipping ensure", cell.key);
                return Ok(s.clone());
            }
        }

        let cluster_id = match existing.as_ref() {
            Some(c) => c.cluster_id,
            None => {
                tracing::info!("creating cluster {cluster_name}");
                let created = self
                    .mgmt
                    .create_cluster(&cluster_name)
                    .await
                    .with_context(|| format!("creating cluster {cluster_name}"))?;
                created.id
            }
        };

        self.mgmt
            .put_config(cluster_id, &cell.config)
            .await
            .with_context(|| format!("putting config for cluster {cluster_id}"))?;

        let incus_exists = self
            .incus
            .instance_exists(&instance_name)
            .await
            .unwrap_or(false);

        let (instance_id, created_at, pending_since) = if incus_exists && existing.is_some() {
            let c = existing.as_ref().unwrap();
            (c.instance_id.clone(), c.created_at, c.pending_since)
        } else {
            if incus_exists && existing.is_none() {
                // Stray instance from a prior run without state — destroy it so we start clean.
                tracing::warn!(
                    "instance {instance_name} exists but no runner state; recreating"
                );
                let _ = self.incus.delete_instance(&instance_name).await;
            }
            let hk = host_key::generate().context("generating ed25519 host key")?;
            tracing::info!(
                "launching incus instance {instance_name} instance_id={}",
                hk.instance_id
            );
            let label = format!("runner-{}", &hk.instance_id[..hk.instance_id.len().min(12)]);
            let req = CloudInitRequest {
                system: &self.config.mgmt.system,
                server_url: self.config.mgmt.public_url.as_deref(),
                daemon_version: self.config.mgmt.daemon_version.as_deref(),
                label: Some(&label),
                host_key_pem: Some(&hk.private_pem),
                instance_id: Some(&hk.instance_id),
            };
            let resp = self
                .mgmt
                .get_cloud_init(cluster_id, &req)
                .await
                .context("fetching cloud-init")?;
            let spec = CreateInstanceSpec {
                name: instance_name.clone(),
                instance_type: self.config.incus.instance_type.clone(),
                image_alias: self.config.incus.image_alias.clone(),
                image_server: self.config.incus.image_server.clone(),
                profiles: self.config.incus.profiles.clone(),
                cloud_init_user_data: resp.cloud_init,
            };
            self.incus
                .create_instance(&spec)
                .await
                .with_context(|| format!("creating incus instance {instance_name}"))?;
            let now = Utc::now();
            (hk.instance_id, now, Some(now))
        };

        let cell_state = CellState {
            key: cell.key.clone(),
            cluster_id,
            instance_name,
            instance_id,
            created_at,
            last_reprovisioned_at: existing
                .as_ref()
                .map(|c| c.last_reprovisioned_at)
                .unwrap_or(created_at),
            pending_since,
            deploy_failures: existing.as_ref().map(|c| c.deploy_failures).unwrap_or(0),
            parked: false,
        };
        {
            let mut s = self.state.lock().await;
            s.upsert(cell_state.clone());
        }
        self.save_state().await?;
        Ok(cell_state)
    }

    /// Destroy a cell: stop+delete Incus instance and delete mgmt cluster.
    pub async fn destroy_cell(&self, key: &str) -> Result<()> {
        let existing = {
            let s = self.state.lock().await;
            s.find(key).cloned()
        };
        let Some(cell) = existing else {
            return Ok(());
        };
        tracing::info!("destroying cell {key}");
        if let Err(e) = self.incus.delete_instance(&cell.instance_name).await {
            tracing::warn!("deleting incus instance {}: {e}", cell.instance_name);
        }
        if let Err(e) = self.mgmt.delete_cluster(cell.cluster_id).await {
            tracing::warn!("deleting mgmt cluster {}: {e}", cell.cluster_id);
        }
        {
            let mut s = self.state.lock().await;
            s.remove(key);
        }
        self.save_state().await?;
        Ok(())
    }

    /// Reprovision one cell: destroy + re-create. Uses same matrix spec so config comes back identical.
    pub async fn reprovision_cell(&self, key: &str) -> Result<()> {
        let cells = self.matrix();
        let Some(cell) = cells.iter().find(|c| c.key == key) else {
            anyhow::bail!("unknown matrix cell: {key}");
        };
        self.destroy_cell(key).await?;
        let s = self.ensure_cell(cell).await?;
        tracing::info!("reprovisioned {key} — instance {}", s.instance_name);
        Ok(())
    }

    /// Reconcile the whole fleet: ensure each matrix cell is provisioned,
    /// clear pending_since on cells whose expected heartbeat has arrived,
    /// and drop state entries no longer in the matrix.
    pub async fn reconcile(&self) -> Result<()> {
        let cells = self.matrix();
        let known_keys: std::collections::HashSet<String> =
            cells.iter().map(|c| c.key.clone()).collect();

        for cell in &cells {
            if let Err(e) = self.ensure_cell(cell).await {
                tracing::error!("ensure_cell {}: {e:#}", cell.key);
                let _ = sentry::integrations::anyhow::capture_anyhow(&e);
            }
        }

        self.refresh_heartbeats().await;

        let stale: Vec<String> = {
            let s = self.state.lock().await;
            s.cells
                .iter()
                .filter(|c| !known_keys.contains(&c.key))
                .map(|c| c.key.clone())
                .collect()
        };
        for key in stale {
            if let Err(e) = self.destroy_cell(&key).await {
                tracing::warn!("destroy stale cell {key}: {e:#}");
            }
        }
        Ok(())
    }

    /// For every pending cell, check whether its pregenerated instance_id has
    /// reported a heartbeat; clear `pending_since` and `deploy_failures` if so.
    pub async fn refresh_heartbeats(&self) {
        let pending: Vec<(String, Uuid, String)> = {
            let s = self.state.lock().await;
            s.cells
                .iter()
                .filter(|c| c.pending_since.is_some())
                .map(|c| (c.key.clone(), c.cluster_id, c.instance_id.clone()))
                .collect()
        };
        for (key, cluster_id, expected) in pending {
            match self.mgmt.list_cluster_machines(cluster_id).await {
                Ok(rows) => {
                    if rows.iter().any(|r| r.instance_id == expected) {
                        let mut s = self.state.lock().await;
                        if let Some(cell) = s.cells.iter_mut().find(|c| c.key == key) {
                            cell.pending_since = None;
                            cell.deploy_failures = 0;
                            tracing::info!("cell {key} came online (instance_id={expected})");
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("heartbeat check for {key}: {e}");
                }
            }
        }
        if let Err(e) = self.save_state().await {
            tracing::warn!("saving state after heartbeat refresh: {e}");
        }
    }

    /// Find cells whose pending_since exceeds `deploy_timeout`, destroy them,
    /// and recreate with fresh keys. After `max_deploy_retries` failures the
    /// cell is parked and no longer auto-retried.
    pub async fn reap_stuck_deploys(&self) {
        let deploy_timeout = parse_duration(&self.config.fleet.deploy_timeout)
            .unwrap_or(Duration::from_secs(15 * 60));
        let deploy_timeout = chrono::Duration::from_std(deploy_timeout)
            .unwrap_or(chrono::Duration::minutes(15));
        let max_retries = self.config.fleet.max_deploy_retries;
        let now = Utc::now();

        let stuck: Vec<(String, u32)> = {
            let s = self.state.lock().await;
            s.cells
                .iter()
                .filter_map(|c| {
                    let p = c.pending_since?;
                    if c.parked {
                        return None;
                    }
                    if (now - p) > deploy_timeout {
                        Some((c.key.clone(), c.deploy_failures))
                    } else {
                        None
                    }
                })
                .collect()
        };

        for (key, prior_failures) in stuck {
            let failures = prior_failures + 1;
            tracing::warn!(
                "cell {key} deploy timed out (>{} min, attempt {}/{})",
                deploy_timeout.num_minutes(),
                failures,
                max_retries
            );
            sentry::configure_scope(|scope| {
                scope.set_tag("cell", &key);
                scope.set_tag("failures", failures.to_string());
            });
            sentry::capture_message(
                &format!("mac-mgmt-runner deploy timeout: cell={key} attempt={failures}"),
                sentry::Level::Warning,
            );

            if let Err(e) = self.destroy_cell(&key).await {
                tracing::error!("teardown failed deploy {key}: {e:#}");
                let _ = sentry::integrations::anyhow::capture_anyhow(&e);
                continue;
            }

            if failures >= max_retries {
                // Record the failure on a freshly-created state entry so
                // the operator can see it parked.
                let mut s = self.state.lock().await;
                s.upsert(CellState {
                    key: key.clone(),
                    cluster_id: Uuid::nil(),
                    instance_name: String::new(),
                    instance_id: String::new(),
                    created_at: now,
                    last_reprovisioned_at: now,
                    pending_since: None,
                    deploy_failures: failures,
                    parked: true,
                });
                drop(s);
                let _ = self.save_state().await;
                tracing::error!(
                    "cell {key} parked after {failures} consecutive deploy failures"
                );
                sentry::capture_message(
                    &format!("mac-mgmt-runner cell parked: {key} after {failures} failures"),
                    sentry::Level::Error,
                );
                continue;
            }

            // Retry — ensure_cell will generate a fresh host key and relaunch.
            if let Some(cell) = self.matrix().into_iter().find(|c| c.key == key) {
                match self.ensure_cell(&cell).await {
                    Ok(new_state) => {
                        let mut s = self.state.lock().await;
                        if let Some(cs) = s.cells.iter_mut().find(|c| c.key == key) {
                            cs.deploy_failures = failures;
                            cs.instance_id = new_state.instance_id.clone();
                        }
                        drop(s);
                        let _ = self.save_state().await;
                    }
                    Err(e) => {
                        tracing::error!("recreating cell {key}: {e:#}");
                        let _ = sentry::integrations::anyhow::capture_anyhow(&e);
                    }
                }
            }
        }
    }

    /// Destroy every cell.
    pub async fn teardown(&self) -> Result<()> {
        let keys: Vec<String> = {
            let s = self.state.lock().await;
            s.cells.iter().map(|c| c.key.clone()).collect()
        };
        for key in keys {
            if let Err(e) = self.destroy_cell(&key).await {
                tracing::warn!("destroy {key}: {e:#}");
            }
        }
        Ok(())
    }

    /// Pick a random provisioned cell and reprovision it.
    pub async fn reprovision_random(&self) -> Result<Option<String>> {
        let keys: Vec<String> = {
            let s = self.state.lock().await;
            s.cells.iter().map(|c| c.key.clone()).collect()
        };
        let key = {
            let mut rng = rand::thread_rng();
            let Some(k) = keys.choose(&mut rng) else {
                return Ok(None);
            };
            k.clone()
        };
        self.reprovision_cell(&key).await?;
        Ok(Some(key))
    }
}

/// Background task: reconcile on a fixed interval.
pub async fn reconcile_loop(orch: Arc<Orchestrator>) {
    let interval = parse_duration(&orch.config.fleet.reconcile_interval)
        .unwrap_or(Duration::from_secs(60));
    let mut ticker = tokio::time::interval(interval);
    loop {
        ticker.tick().await;
        if let Err(e) = orch.reconcile().await {
            tracing::error!("reconcile: {e:#}");
            let _ = sentry::integrations::anyhow::capture_anyhow(&e);
        }
    }
}

/// Background task: tears down deployments that fail to heartbeat within
/// `deploy_timeout` and recreates them with fresh keys, parking the cell
/// after `max_deploy_retries` consecutive failures.
pub async fn deploy_watchdog_loop(orch: Arc<Orchestrator>) {
    let interval = parse_duration(&orch.config.fleet.reconcile_interval)
        .unwrap_or(Duration::from_secs(60));
    let mut ticker = tokio::time::interval(interval);
    // Skew so it doesn't fire at the same time as reconcile.
    tokio::time::sleep(interval / 3).await;
    loop {
        ticker.tick().await;
        orch.reap_stuck_deploys().await;
    }
}

/// Background task: reprovision a random cell periodically.
pub async fn reprovision_loop(orch: Arc<Orchestrator>) {
    let interval = parse_duration(&orch.config.fleet.random_reprovision_interval)
        .unwrap_or(Duration::from_secs(1800));
    // Offset so the first fire isn't simultaneous with the reconcile tick.
    tokio::time::sleep(interval / 2).await;
    let mut ticker = tokio::time::interval(interval);
    ticker.tick().await;
    loop {
        ticker.tick().await;
        match orch.reprovision_random().await {
            Ok(Some(k)) => tracing::info!("random reprovision: {k}"),
            Ok(None) => tracing::debug!("random reprovision: no cells to reprovision"),
            Err(e) => tracing::error!("random reprovision: {e:#}"),
        }
    }
}

/// Compose a status snapshot for the HTTP API.
pub async fn status_snapshot(orch: &Orchestrator) -> StatusSnapshot {
    let matrix = orch.matrix();
    let state = orch.state.lock().await.clone();
    let stale_after = parse_duration(&orch.config.fleet.heartbeat_stale_after)
        .unwrap_or(Duration::from_secs(300));
    let grace = parse_duration(&orch.config.fleet.startup_grace)
        .unwrap_or(Duration::from_secs(180));
    let now = Utc::now();

    let mut cells = Vec::new();
    for mc in &matrix {
        let s = state.cells.iter().find(|c| c.key == mc.key).cloned();

        let (healthy, detail) = match &s {
            None => (None, None),
            Some(cs) => {
                match orch.mgmt.list_cluster_machines(cs.cluster_id).await {
                    Err(e) => (Some(false), Some(format!("mgmt error: {e}"))),
                    Ok(rows) => {
                        let age_ok = (now - cs.created_at).to_std().unwrap_or_default() >= grace;
                        let latest = rows.first();
                        let heartbeat_fresh = latest
                            .map(|r| (now - r.reported_at).to_std().unwrap_or_default() < stale_after)
                            .unwrap_or(false);
                        let probes_ok = latest
                            .and_then(|r| r.services_extended.as_ref())
                            .and_then(probes_healthy)
                            .unwrap_or(true);
                        let detail = latest.map(|r| {
                            let probes = probe_summary(r.services_extended.as_ref());
                            format!(
                                "v{} inst={} @ {} ({}s ago){}",
                                r.version,
                                r.instance_id,
                                r.hostname.clone().unwrap_or_default(),
                                (now - r.reported_at).num_seconds().max(0),
                                probes,
                            )
                        });
                        if !age_ok && latest.is_none() {
                            (None, Some("starting".into()))
                        } else {
                            (Some(heartbeat_fresh && probes_ok), detail)
                        }
                    }
                }
            }
        };

        cells.push(CellStatus {
            key: mc.key.clone(),
            provisioned: s.is_some() && !s.as_ref().map(|c| c.parked).unwrap_or(false),
            cluster_id: s.as_ref().map(|c| c.cluster_id),
            instance_name: s.as_ref().map(|c| c.instance_name.clone()),
            instance_id: s.as_ref().map(|c| c.instance_id.clone()),
            created_at: s.as_ref().map(|c| c.created_at),
            pending_since: s.as_ref().and_then(|c| c.pending_since),
            deploy_failures: s.as_ref().map(|c| c.deploy_failures).unwrap_or(0),
            parked: s.as_ref().map(|c| c.parked).unwrap_or(false),
            healthy,
            detail,
        });
    }

    StatusSnapshot {
        total_cells: matrix.len(),
        provisioned: state.cells.len(),
        cells,
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct StatusSnapshot {
    pub total_cells: usize,
    pub provisioned: usize,
    pub cells: Vec<CellStatus>,
}

fn probes_healthy(v: &serde_json::Value) -> Option<bool> {
    let map = v.as_object()?;
    let mut any = false;
    for (_, svc) in map {
        any = true;
        let h = svc.get("healthy").and_then(|b| b.as_bool()).unwrap_or(false);
        let probe = svc.get("last_probe_ok").and_then(|b| b.as_bool()).unwrap_or(false);
        if !(h && probe) {
            return Some(false);
        }
    }
    if any { Some(true) } else { None }
}

fn probe_summary(v: Option<&serde_json::Value>) -> String {
    let Some(map) = v.and_then(|v| v.as_object()) else {
        return String::new();
    };
    if map.is_empty() {
        return String::new();
    }
    let mut parts: Vec<String> = Vec::new();
    for (name, svc) in map {
        let h = svc.get("healthy").and_then(|b| b.as_bool()).unwrap_or(false);
        let mark = if h { "✔" } else { "✗" };
        parts.push(format!("{name}{mark}"));
    }
    format!(" [{}]", parts.join(","))
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CellStatus {
    pub key: String,
    pub provisioned: bool,
    pub cluster_id: Option<Uuid>,
    pub instance_name: Option<String>,
    pub instance_id: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub pending_since: Option<chrono::DateTime<chrono::Utc>>,
    pub deploy_failures: u32,
    pub parked: bool,
    /// None = not yet assessable (still within startup grace); Some(bool) = heartbeat check result.
    pub healthy: Option<bool>,
    /// Human-readable detail for the last heartbeat (version, hostname, age) or error.
    pub detail: Option<String>,
}
