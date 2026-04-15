//! Cell lifecycle state machine and the loops that drive it.
//!
//! Every matrix cell has exactly one [`CellStage`] at any time. The
//! orchestrator never mutates fields in place; it produces a new stage
//! and persists it before returning. That makes crash-resumption a
//! no-op — on startup we reload state and drive every non-terminal
//! cell forward one step at a time until it reaches `Running` (or
//! `Parked`).
//!
//! Transitions:
//!
//! ```text
//! Pending
//!   └─ create_cluster ─▶ ClusterCreated
//!                          └─ put_config ─▶ ConfigPushed
//!                                             └─ launch ─▶ Launching
//!                                                           ├─ heartbeat ─▶ Running
//!                                                           └─ timeout  ─▶ (destroy) ─▶ Pending
//!                                                                                      or Parked
//! Running
//!   └─ reprovision ─▶ (destroy) ─▶ Pending
//!
//! any ─ teardown ─▶ (destroy + remove)
//! ```

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rand::seq::SliceRandom;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::config::{RunnerConfig, parse_duration};
use crate::host_key;
use crate::incus::{CreateInstanceSpec, IncusClient};
use crate::matrix::{MatrixCell, generate};
use crate::mgmt::{CloudInitRequest, MgmtClient};
use crate::state::{CellStage, CellState, FleetState};

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

    /// Incus instance name for a matrix cell. Kept identical to the
    /// mgmt cluster name so they can be correlated at a glance.
    fn instance_name(&self, key: &str) -> String {
        let raw = format!("{}{}", self.config.incus.name_prefix, key);
        let sanitized: String = raw
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();
        if sanitized.len() > 63 { sanitized[..63].to_string() } else { sanitized }
    }

    fn cluster_name(&self, key: &str) -> String {
        format!("{}{}", self.config.incus.name_prefix, key)
    }

    pub async fn save_state(&self) -> Result<()> {
        let s = self.state.lock().await;
        s.save(&self.config.fleet.state_path)
    }

    /// Upsert + fsync. Always use this to persist a cell — never touch
    /// state.cells directly outside the helpers in this module.
    async fn persist_cell(&self, cell: CellState) -> Result<()> {
        {
            let mut s = self.state.lock().await;
            s.upsert(cell);
        }
        self.save_state().await
    }

    fn max_retries(&self) -> u32 {
        self.config.fleet.max_deploy_retries
    }

    fn deploy_timeout(&self) -> chrono::Duration {
        let std = parse_duration(&self.config.fleet.deploy_timeout)
            .unwrap_or(Duration::from_secs(15 * 60));
        chrono::Duration::from_std(std).unwrap_or(chrono::Duration::minutes(15))
    }

    // ── Transitions ───────────────────────────────────────────────────

    /// Drive one cell forward by exactly one stage. Returns the stage
    /// after the transition (or the same stage if already terminal).
    /// Reconcile calls this repeatedly until every cell settles.
    pub async fn drive_cell(&self, matrix_cell: &MatrixCell) -> Result<CellStage> {
        let current = {
            let s = self.state.lock().await;
            s.find(&matrix_cell.key)
                .map(|c| c.stage.clone())
                .unwrap_or(CellStage::Pending)
        };

        match current.clone() {
            CellStage::Parked { .. } => Ok(current),
            CellStage::Running { .. } => {
                // Keep the server's config in sync in case the matrix spec changed.
                if let Some(cid) = current.cluster_id() {
                    if let Err(e) = self.mgmt.put_config(cid, &matrix_cell.config).await {
                        tracing::warn!("running cell {}: put_config drift: {e:#}", matrix_cell.key);
                    }
                }
                Ok(current)
            }
            CellStage::Pending => self.enter_cluster_created(matrix_cell).await,
            CellStage::ClusterCreated { cluster_id, .. } => {
                self.enter_config_pushed(matrix_cell, cluster_id).await
            }
            CellStage::ConfigPushed { cluster_id, .. } => {
                self.enter_launching(matrix_cell, cluster_id).await
            }
            CellStage::Launching {
                cluster_id,
                instance_id,
                instance_name,
                since,
            } => {
                self.poll_launching(matrix_cell, cluster_id, &instance_id, &instance_name, since)
                    .await
            }
        }
    }

    async fn enter_cluster_created(&self, cell: &MatrixCell) -> Result<CellStage> {
        let name = self.cluster_name(&cell.key);
        tracing::info!("creating cluster {name}");
        let created = self
            .mgmt
            .create_cluster(&name)
            .await
            .with_context(|| format!("creating cluster {name}"))?;
        let now = Utc::now();
        let stage = CellStage::ClusterCreated { cluster_id: created.id, at: now };
        self.persist_cell(self.cell_with_stage(cell, stage.clone(), now).await)
            .await?;
        Ok(stage)
    }

    async fn enter_config_pushed(
        &self,
        cell: &MatrixCell,
        cluster_id: Uuid,
    ) -> Result<CellStage> {
        self.mgmt
            .put_config(cluster_id, &cell.config)
            .await
            .with_context(|| format!("putting config for cluster {cluster_id}"))?;
        let now = Utc::now();
        let stage = CellStage::ConfigPushed { cluster_id, at: now };
        self.persist_cell(self.cell_with_stage(cell, stage.clone(), now).await)
            .await?;
        Ok(stage)
    }

    async fn enter_launching(
        &self,
        cell: &MatrixCell,
        cluster_id: Uuid,
    ) -> Result<CellStage> {
        let instance_name = self.instance_name(&cell.key);

        // If an Incus instance is already there from a crashed prior run
        // (and we're about to generate a new host key), wipe it first.
        if self.incus.instance_exists(&instance_name).await.unwrap_or(false) {
            tracing::warn!(
                "instance {instance_name} already exists at launch time; deleting to start clean"
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

        // Persist the predicted instance_id before issuing the Incus call so
        // a crash between the two leaves state pointing at the VM we are
        // about to create rather than an orphan.
        let now = Utc::now();
        let stage_pre = CellStage::Launching {
            cluster_id,
            instance_name: instance_name.clone(),
            instance_id: hk.instance_id.clone(),
            since: now,
        };
        self.persist_cell(self.cell_with_stage(cell, stage_pre.clone(), now).await)
            .await?;

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

        Ok(stage_pre)
    }

    async fn poll_launching(
        &self,
        cell: &MatrixCell,
        cluster_id: Uuid,
        instance_id: &str,
        instance_name: &str,
        since: DateTime<Utc>,
    ) -> Result<CellStage> {
        // Heartbeat check — has the daemon come up?
        match self.mgmt.list_cluster_machines(cluster_id).await {
            Ok(rows) if rows.iter().any(|r| r.instance_id == instance_id) => {
                let now = Utc::now();
                let stage = CellStage::Running {
                    cluster_id,
                    instance_name: instance_name.to_string(),
                    instance_id: instance_id.to_string(),
                    since: now,
                };
                let mut cs = self.cell_with_stage(cell, stage.clone(), now).await;
                cs.deploy_failures = 0;
                self.persist_cell(cs).await?;
                tracing::info!("cell {} came online (instance_id={})", cell.key, instance_id);
                return Ok(stage);
            }
            Err(e) => {
                tracing::warn!("heartbeat check for {}: {e}", cell.key);
            }
            _ => {}
        }

        // Still launching — is it stuck?
        let age = Utc::now() - since;
        if age > self.deploy_timeout() {
            return self
                .on_launch_timeout(cell, cluster_id, instance_name, age)
                .await;
        }
        Ok(CellStage::Launching {
            cluster_id,
            instance_name: instance_name.to_string(),
            instance_id: instance_id.to_string(),
            since,
        })
    }

    async fn on_launch_timeout(
        &self,
        cell: &MatrixCell,
        cluster_id: Uuid,
        instance_name: &str,
        age: chrono::Duration,
    ) -> Result<CellStage> {
        let failures = {
            let s = self.state.lock().await;
            s.find(&cell.key).map(|c| c.deploy_failures).unwrap_or(0) + 1
        };
        tracing::warn!(
            "cell {} deploy timed out after {} min, attempt {}/{}",
            cell.key,
            age.num_minutes(),
            failures,
            self.max_retries()
        );
        sentry::configure_scope(|scope| {
            scope.set_tag("cell", &cell.key);
            scope.set_tag("failures", failures.to_string());
        });
        sentry::capture_message(
            &format!(
                "mac-mgmt-runner deploy timeout: cell={} attempt={failures}",
                cell.key
            ),
            sentry::Level::Warning,
        );

        // Tear down the failed VM; leave the cluster in place so put_config
        // state isn't lost (we'll re-enter Launching with a fresh host key).
        if let Err(e) = self.incus.delete_instance(instance_name).await {
            tracing::warn!("deleting failed instance {instance_name}: {e}");
        }

        if failures >= self.max_retries() {
            let reason = format!(
                "exceeded {} consecutive deploy timeouts",
                self.max_retries()
            );
            let stage = CellStage::Parked {
                cluster_id: Some(cluster_id),
                instance_name: Some(instance_name.to_string()),
                reason: reason.clone(),
            };
            let mut cs = self.cell_with_stage(cell, stage.clone(), Utc::now()).await;
            cs.deploy_failures = failures;
            self.persist_cell(cs).await?;
            tracing::error!("cell {} parked: {reason}", cell.key);
            sentry::capture_message(
                &format!(
                    "mac-mgmt-runner cell parked: {} after {} failures",
                    cell.key, failures
                ),
                sentry::Level::Error,
            );
            Ok(stage)
        } else {
            // Fall back to ConfigPushed so the next reconcile tick re-enters Launching.
            let stage = CellStage::ConfigPushed {
                cluster_id,
                at: Utc::now(),
            };
            let mut cs = self.cell_with_stage(cell, stage.clone(), Utc::now()).await;
            cs.deploy_failures = failures;
            self.persist_cell(cs).await?;
            Ok(stage)
        }
    }

    /// Build a CellState for `cell` carrying `stage`, preserving
    /// deploy_failures + last_reprovisioned_at from any existing entry.
    async fn cell_with_stage(
        &self,
        cell: &MatrixCell,
        stage: CellStage,
        fallback_reprovisioned_at: DateTime<Utc>,
    ) -> CellState {
        let s = self.state.lock().await;
        let existing = s.find(&cell.key);
        CellState {
            key: cell.key.clone(),
            stage,
            deploy_failures: existing.map(|e| e.deploy_failures).unwrap_or(0),
            last_reprovisioned_at: existing
                .map(|e| e.last_reprovisioned_at)
                .unwrap_or(fallback_reprovisioned_at),
        }
    }

    // ── Reconcile + teardown ──────────────────────────────────────────

    /// Drive every matrix cell forward until settled. Each invocation
    /// of drive_cell advances one stage, so reconcile_cell retries up to
    /// a small budget before returning.
    pub async fn reconcile_cell(&self, cell: &MatrixCell) -> Result<CellStage> {
        // At most one transition per stage — Pending → ClusterCreated →
        // ConfigPushed → Launching → (wait) Running.
        const MAX_STEPS: usize = 5;
        let mut last = CellStage::Pending;
        for _ in 0..MAX_STEPS {
            last = self.drive_cell(cell).await?;
            match last {
                CellStage::Running { .. }
                | CellStage::Parked { .. }
                | CellStage::Launching { .. } => break,
                _ => continue,
            }
        }
        Ok(last)
    }

    pub async fn reconcile(&self) -> Result<()> {
        let cells = self.matrix();
        let known_keys: std::collections::HashSet<String> =
            cells.iter().map(|c| c.key.clone()).collect();

        for cell in &cells {
            if let Err(e) = self.reconcile_cell(cell).await {
                tracing::error!("reconcile_cell {}: {e:#}", cell.key);
                let _ = sentry::integrations::anyhow::capture_anyhow(&e);
            }
        }

        // Sweep cells no longer in the matrix spec.
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

    /// Destroy a cell and drop it from state. Best-effort: logs but
    /// does not fail on individual Incus/mgmt errors so a partially-
    /// created cell can always be cleaned up.
    pub async fn destroy_cell(&self, key: &str) -> Result<()> {
        let existing = {
            let s = self.state.lock().await;
            s.find(key).cloned()
        };
        let Some(cell) = existing else {
            return Ok(());
        };
        tracing::info!("destroying cell {key} (stage={})", cell.stage.label());

        if let Some(name) = cell.stage.instance_name() {
            if let Err(e) = self.incus.delete_instance(name).await {
                tracing::warn!("deleting incus instance {name}: {e}");
            }
        }
        if let Some(cid) = cell.stage.cluster_id() {
            if let Err(e) = self.mgmt.delete_cluster(cid).await {
                tracing::warn!("deleting mgmt cluster {cid}: {e}");
            }
        }
        {
            let mut s = self.state.lock().await;
            s.remove(key);
        }
        self.save_state().await?;
        Ok(())
    }

    /// Reprovision: destroy + reset to Pending so reconcile drives it
    /// back up. Clears deploy_failures and marks last_reprovisioned_at.
    pub async fn reprovision_cell(&self, key: &str) -> Result<()> {
        let cells = self.matrix();
        let Some(cell) = cells.iter().find(|c| c.key == key) else {
            anyhow::bail!("unknown matrix cell: {key}");
        };
        self.destroy_cell(key).await?;
        let now = Utc::now();
        self.persist_cell(CellState {
            key: cell.key.clone(),
            stage: CellStage::Pending,
            deploy_failures: 0,
            last_reprovisioned_at: now,
        })
        .await?;
        let s = self.reconcile_cell(cell).await?;
        tracing::info!("reprovisioned {key} — stage={}", s.label());
        Ok(())
    }

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

    /// Wipe the fleet and redeploy from scratch. Destroys everything
    /// tracked locally AND every orphan cluster on the mgmt server whose
    /// name starts with `incus.name_prefix`, then reconciles fresh.
    pub async fn redeploy(&self) -> Result<()> {
        tracing::info!("redeploy: teardown + orphan sweep + reconcile");
        self.teardown().await?;

        let prefix = self.config.incus.name_prefix.clone();
        match self.mgmt.list_clusters().await {
            Ok(rows) => {
                for row in rows {
                    if !row.name.starts_with(&prefix) {
                        continue;
                    }
                    tracing::info!("redeploy: removing orphan cluster {} ({})", row.name, row.id);
                    if let Err(e) = self.incus.delete_instance(&row.name).await {
                        tracing::warn!("redeploy: deleting incus instance {}: {e}", row.name);
                    }
                    if let Err(e) = self.mgmt.delete_cluster(row.id).await {
                        tracing::warn!("redeploy: deleting cluster {}: {e}", row.id);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("redeploy: listing clusters for orphan sweep: {e}");
            }
        }

        {
            let mut s = self.state.lock().await;
            s.cells.clear();
        }
        self.save_state().await?;
        self.reconcile().await
    }

    pub async fn reprovision_random(&self) -> Result<Option<String>> {
        let keys: Vec<String> = {
            let s = self.state.lock().await;
            s.cells
                .iter()
                .filter(|c| c.stage.is_running())
                .map(|c| c.key.clone())
                .collect()
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

    // ── Status snapshot ───────────────────────────────────────────────

    pub async fn snapshot(&self) -> StatusSnapshot {
        let matrix = self.matrix();
        let state = self.state.lock().await.clone();
        let stale_after = parse_duration(&self.config.fleet.heartbeat_stale_after)
            .unwrap_or(Duration::from_secs(300));
        let now = Utc::now();

        let mut cells = Vec::new();
        for mc in &matrix {
            let s = state.cells.iter().find(|c| c.key == mc.key).cloned();

            let (healthy, detail) = match s.as_ref().map(|c| &c.stage) {
                None | Some(CellStage::Pending) => (None, None),
                Some(CellStage::Parked { reason, .. }) => {
                    (Some(false), Some(reason.clone()))
                }
                Some(CellStage::ClusterCreated { .. })
                | Some(CellStage::ConfigPushed { .. }) => {
                    (None, Some("provisioning".into()))
                }
                Some(CellStage::Launching { instance_id, since, .. }) => {
                    let waited = (now - *since).num_seconds().max(0);
                    (
                        None,
                        Some(format!(
                            "launching (waited {waited}s, iid={})",
                            &instance_id[..instance_id.len().min(12)]
                        )),
                    )
                }
                Some(CellStage::Running { cluster_id, instance_id, .. }) => {
                    match self.mgmt.list_cluster_machines(*cluster_id).await {
                        Ok(rows) => {
                            let latest = rows.iter().find(|r| &r.instance_id == instance_id);
                            let heartbeat_fresh = latest
                                .map(|r| {
                                    (now - r.reported_at).to_std().unwrap_or_default()
                                        < stale_after
                                })
                                .unwrap_or(false);
                            let probes_ok = latest
                                .and_then(|r| r.services_extended.as_ref())
                                .and_then(probes_healthy)
                                .unwrap_or(true);
                            let detail = latest.map(|r| {
                                let probes = probe_summary(r.services_extended.as_ref());
                                format!(
                                    "v{} {} ({}s ago){}",
                                    r.version,
                                    r.hostname.clone().unwrap_or_default(),
                                    (now - r.reported_at).num_seconds().max(0),
                                    probes,
                                )
                            });
                            (Some(heartbeat_fresh && probes_ok), detail)
                        }
                        Err(e) => (Some(false), Some(format!("mgmt error: {e}"))),
                    }
                }
            };

            cells.push(CellStatus {
                key: mc.key.clone(),
                stage: s
                    .as_ref()
                    .map(|c| c.stage.label().to_string())
                    .unwrap_or_else(|| "missing".into()),
                cluster_id: s.as_ref().and_then(|c| c.stage.cluster_id()),
                instance_name: s
                    .as_ref()
                    .and_then(|c| c.stage.instance_name().map(|v| v.to_string())),
                instance_id: s
                    .as_ref()
                    .and_then(|c| c.stage.instance_id().map(|v| v.to_string())),
                launching_since: s.as_ref().and_then(|c| c.stage.launching_since()),
                deploy_failures: s.as_ref().map(|c| c.deploy_failures).unwrap_or(0),
                parked: s.as_ref().map(|c| c.stage.is_parked()).unwrap_or(false),
                healthy,
                detail,
            });
        }

        StatusSnapshot {
            total_cells: matrix.len(),
            running: state.cells.iter().filter(|c| c.stage.is_running()).count(),
            cells,
        }
    }
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

// ── Background loops ──────────────────────────────────────────────────

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

/// Periodically picks a random Running cell and reprovisions it.
pub async fn reprovision_loop(orch: Arc<Orchestrator>) {
    let interval = parse_duration(&orch.config.fleet.random_reprovision_interval)
        .unwrap_or(Duration::from_secs(1800));
    tokio::time::sleep(interval / 2).await;
    let mut ticker = tokio::time::interval(interval);
    ticker.tick().await;
    loop {
        ticker.tick().await;
        match orch.reprovision_random().await {
            Ok(Some(k)) => tracing::info!("random reprovision: {k}"),
            Ok(None) => tracing::debug!("random reprovision: no running cells to cycle"),
            Err(e) => tracing::error!("random reprovision: {e:#}"),
        }
    }
}

// ── Public types for HTTP API + CLI ───────────────────────────────────

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct StatusSnapshot {
    pub total_cells: usize,
    pub running: usize,
    pub cells: Vec<CellStatus>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CellStatus {
    pub key: String,
    pub stage: String,
    pub cluster_id: Option<Uuid>,
    pub instance_name: Option<String>,
    pub instance_id: Option<String>,
    pub launching_since: Option<DateTime<Utc>>,
    pub deploy_failures: u32,
    pub parked: bool,
    pub healthy: Option<bool>,
    pub detail: Option<String>,
}
