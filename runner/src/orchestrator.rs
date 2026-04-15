//! Reconciliation loop and operations.
//!
//! The orchestrator owns a [`FleetState`] under a mutex. The reconcile loop
//! ensures that for each matrix cell there is exactly one Incus instance and
//! one mgmt cluster, and that the cluster's config matches the cell's spec.
//! A secondary timer picks a random cell at an interval and reprovisions it.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{Utc};
use rand::seq::SliceRandom;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::config::{RunnerConfig, parse_duration};
use crate::incus::{CreateInstanceSpec, IncusClient};
use crate::matrix::{MatrixCell, generate};
use crate::mgmt::MgmtClient;
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

    /// Ensure one cell is present and healthy. Creates cluster + config + incus instance if missing.
    pub async fn ensure_cell(&self, cell: &MatrixCell) -> Result<CellState> {
        let instance_name = self.instance_name(&cell.key);
        let cluster_name = self.cluster_name(&cell.key);

        // Look up existing state.
        let existing = {
            let s = self.state.lock().await;
            s.find(&cell.key).cloned()
        };

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

        // Push config (idempotent — server just appends a new version).
        self.mgmt
            .put_config(cluster_id, &cell.config)
            .await
            .with_context(|| format!("putting config for cluster {cluster_id}"))?;

        // If there's no Incus instance, fetch cloud-init and launch one.
        let exists = self
            .incus
            .instance_exists(&instance_name)
            .await
            .unwrap_or(false);
        if !exists {
            tracing::info!("launching incus instance {instance_name}");
            let yaml = self
                .mgmt
                .get_cloud_init(
                    cluster_id,
                    &self.config.mgmt.system,
                    self.config.mgmt.public_url.as_deref(),
                    self.config.mgmt.daemon_version.as_deref(),
                    Some(&format!("runner-{instance_name}")),
                )
                .await
                .context("fetching cloud-init")?;
            let spec = CreateInstanceSpec {
                name: instance_name.clone(),
                instance_type: self.config.incus.instance_type.clone(),
                image_alias: self.config.incus.image_alias.clone(),
                image_server: self.config.incus.image_server.clone(),
                profiles: self.config.incus.profiles.clone(),
                cloud_init_user_data: yaml,
            };
            self.incus
                .create_instance(&spec)
                .await
                .with_context(|| format!("creating incus instance {instance_name}"))?;
        }

        let now = Utc::now();
        let cell_state = CellState {
            key: cell.key.clone(),
            cluster_id,
            instance_name,
            created_at: existing.as_ref().map(|c| c.created_at).unwrap_or(now),
            last_reprovisioned_at: existing
                .as_ref()
                .map(|c| c.last_reprovisioned_at)
                .unwrap_or(now),
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

    /// Reconcile the whole fleet: ensure each matrix cell is provisioned.
    pub async fn reconcile(&self) -> Result<()> {
        let cells = self.matrix();
        let known_keys: std::collections::HashSet<String> =
            cells.iter().map(|c| c.key.clone()).collect();

        for cell in &cells {
            if let Err(e) = self.ensure_cell(cell).await {
                tracing::error!("ensure_cell {}: {e:#}", cell.key);
            }
        }

        // Drop any state entries no longer in the matrix (e.g. provider was removed).
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
        }
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
            provisioned: s.is_some(),
            cluster_id: s.as_ref().map(|c| c.cluster_id),
            instance_name: s.as_ref().map(|c| c.instance_name.clone()),
            created_at: s.as_ref().map(|c| c.created_at),
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
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    /// None = not yet assessable (still within startup grace); Some(bool) = heartbeat check result.
    pub healthy: Option<bool>,
    /// Human-readable detail for the last heartbeat (version, hostname, age) or error.
    pub detail: Option<String>,
}
