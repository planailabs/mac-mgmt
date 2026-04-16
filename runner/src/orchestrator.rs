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
use rand::Rng;
use rand::seq::SliceRandom;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::config::{RunnerConfig, parse_duration};
use crate::host_key;
use crate::incus::{CreateInstanceSpec, IncusClient};
use crate::matrix::{MatrixCell, generate};
use crate::mgmt::{CloudInitRequest, MgmtClient};
use crate::state::{CellStage, CellState, FleetState, InstanceSpec};

pub struct Orchestrator {
    pub config: RunnerConfig,
    pub mgmt: MgmtClient,
    pub incus: IncusClient,
    pub state: Mutex<FleetState>,
    /// Cached rollout group id for the fleet; populated on first use.
    rollout_group: tokio::sync::OnceCell<Uuid>,
}

impl Orchestrator {
    pub fn new(config: RunnerConfig, mgmt: MgmtClient, incus: IncusClient) -> Result<Self> {
        let state = FleetState::load(&config.fleet.state_path)?;
        Ok(Self {
            config,
            mgmt,
            incus,
            state: Mutex::new(state),
            rollout_group: tokio::sync::OnceCell::new(),
        })
    }

    fn rollout_group_name(&self) -> String {
        format!("{}fleet", self.config.incus.name_prefix)
    }

    /// Resolve the rollout group id for the fleet, creating it on the mgmt
    /// server if it doesn't exist yet. Result is cached for the process
    /// lifetime.
    async fn ensure_rollout_group(&self) -> Result<Uuid> {
        if let Some(id) = self.rollout_group.get() {
            return Ok(*id);
        }
        let name = self.rollout_group_name();
        let groups = self.mgmt.list_rollout_groups().await?;
        let id = if let Some(g) = groups.iter().find(|g| g.name == name) {
            g.id
        } else {
            let desc = format!(
                "mac-mgmt-runner fleet ({} matrix cells)",
                self.matrix().len()
            );
            self.mgmt.create_rollout_group(&name, &desc).await?;
            let groups = self.mgmt.list_rollout_groups().await?;
            groups
                .into_iter()
                .find(|g| g.name == name)
                .map(|g| g.id)
                .ok_or_else(|| anyhow::anyhow!("rollout group {name} missing after create"))?
        };
        let _ = self.rollout_group.set(id);
        Ok(id)
    }

    pub fn matrix(&self) -> Vec<MatrixCell> {
        generate(&self.config.matrix)
    }

    /// Incus instance name for a matrix cell. Kept identical to the
    /// mgmt cluster name so they can be correlated at a glance.
    fn instance_name(&self, key: &str, index: u32, total: u32) -> String {
        let base = format!("{}{}", self.config.incus.name_prefix, key);
        let raw = if total <= 1 {
            base
        } else {
            format!("{base}-{index}")
        };
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

    pub async fn is_paused(&self) -> bool {
        self.state.lock().await.paused
    }

    async fn set_paused(&self, paused: bool) -> Result<()> {
        {
            let mut s = self.state.lock().await;
            s.paused = paused;
        }
        self.save_state().await
    }

    /// Clear the pause flag. Called by explicit operator actions
    /// (provision, reprovision, redeploy) so the automatic loops start
    /// touching the fleet again.
    pub async fn resume(&self) -> Result<()> {
        self.set_paused(false).await
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
                // Throttle: at most `max_concurrent_launches` cells may be in
                // the Launching stage at once. Over budget → stay at
                // ConfigPushed; the next reconcile tick picks up the slack
                // once some cell reaches Running or times out.
                let cap = self.config.fleet.max_concurrent_launches;
                let launching = {
                    let s = self.state.lock().await;
                    s.cells
                        .iter()
                        .filter(|c| matches!(c.stage, CellStage::Launching { .. }))
                        .count()
                };
                if cap > 0 && launching >= cap {
                    tracing::debug!(
                        "cell {}: launch queue full ({}/{}), staying at ConfigPushed",
                        matrix_cell.key,
                        launching,
                        cap
                    );
                    return Ok(current);
                }
                self.enter_launching(matrix_cell, cluster_id).await
            }
            CellStage::Launching {
                cluster_id,
                instances,
                since,
            } => {
                self.poll_launching(matrix_cell, cluster_id, instances, since)
                    .await
            }
        }
    }

    async fn enter_cluster_created(&self, cell: &MatrixCell) -> Result<CellStage> {
        let name = self.cluster_name(&cell.key);

        // A cluster with our name but no entry in local state is an orphan
        // (crashed prior run, operator poking around, gc hasn't run yet).
        // Clusters.name is UNIQUE so we can't create a fresh one alongside
        // — wipe the orphan (and any matching Incus instance) first.
        if let Ok(rows) = self.mgmt.list_clusters().await {
            if let Some(stray) = rows.into_iter().find(|r| r.name == name) {
                tracing::warn!(
                    "cluster {name} already exists ({}); deleting before recreate",
                    stray.id
                );
                let _ = self.incus.delete_instance(&name).await;
                let _ = self.mgmt.delete_cluster(stray.id).await;
            }
        }

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

        // Enrol the new cluster in the fleet's rollout group. Failures here
        // don't abort the cell's state machine — the cluster already exists
        // and the next reconcile tick will retry membership.
        match self.ensure_rollout_group().await {
            Ok(group_id) => {
                if let Err(e) = self
                    .mgmt
                    .add_rollout_group_member(group_id, created.id)
                    .await
                {
                    tracing::warn!(
                        "adding cluster {} to rollout group {group_id}: {e:#}",
                        created.id
                    );
                }
            }
            Err(e) => tracing::warn!("resolving rollout group: {e:#}"),
        }
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
        let node_count = cell.node_count.max(1);

        // Predict instance names up front so stray leftovers from a crashed
        // prior run can be cleaned out before we generate fresh host keys.
        let names: Vec<String> = (1..=node_count)
            .map(|i| self.instance_name(&cell.key, i, node_count))
            .collect();
        for name in &names {
            if self.incus.instance_exists(name).await.unwrap_or(false) {
                tracing::warn!(
                    "instance {name} already exists at launch time; deleting to start clean"
                );
                let _ = self.incus.delete_instance(name).await;
            }
        }

        // Phase 1: generate all host keys + fetch cloud-init for each node,
        // persisting the predicted instance_ids BEFORE any Incus launch so a
        // crash between these two phases can't orphan a VM.
        let mut planned: Vec<(InstanceSpec, String)> = Vec::with_capacity(names.len());
        for (idx, name) in names.iter().enumerate() {
            let hk = host_key::generate().context("generating ed25519 host key")?;
            let label = format!(
                "runner-{}-{}",
                &hk.instance_id[..hk.instance_id.len().min(12)],
                idx + 1
            );
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
            planned.push((
                InstanceSpec {
                    instance_name: name.clone(),
                    instance_id: hk.instance_id,
                },
                resp.cloud_init,
            ));
        }

        let now = Utc::now();
        let instances: Vec<InstanceSpec> =
            planned.iter().map(|(i, _)| i.clone()).collect();
        let stage = CellStage::Launching {
            cluster_id,
            instances: instances.clone(),
            since: now,
        };
        self.persist_cell(self.cell_with_stage(cell, stage.clone(), now).await)
            .await?;

        // Phase 2: create Incus instances. Any failure bubbles out; the
        // state already records what we intended and the next reconcile
        // tick will treat missing VMs as a timeout + retry.
        for (inst, user_data) in planned {
            tracing::info!(
                "launching incus instance {} instance_id={}",
                inst.instance_name,
                inst.instance_id
            );
            let spec = CreateInstanceSpec {
                name: inst.instance_name.clone(),
                instance_type: self.config.incus.instance_type.clone(),
                image_alias: self.config.incus.image_alias.clone(),
                image_server: self.config.incus.image_server.clone(),
                profiles: self.config.incus.profiles.clone(),
                cloud_init_user_data: user_data,
            };
            self.incus
                .create_instance(&spec)
                .await
                .with_context(|| format!("creating incus instance {}", inst.instance_name))?;
        }

        Ok(stage)
    }

    async fn poll_launching(
        &self,
        cell: &MatrixCell,
        cluster_id: Uuid,
        instances: Vec<InstanceSpec>,
        since: DateTime<Utc>,
    ) -> Result<CellStage> {
        // Heartbeat check — have all daemons come up?
        let rows = match self.mgmt.list_cluster_machines(cluster_id).await {
            Ok(rows) => Some(rows),
            Err(e) => {
                tracing::warn!("heartbeat check for {}: {e}", cell.key);
                None
            }
        };
        if let Some(rows) = rows {
            let all_up = instances
                .iter()
                .all(|inst| rows.iter().any(|r| r.instance_id == inst.instance_id));
            if all_up {
                let now = Utc::now();
                let stage = CellStage::Running {
                    cluster_id,
                    instances: instances.clone(),
                    since: now,
                };
                let mut cs = self.cell_with_stage(cell, stage.clone(), now).await;
                cs.deploy_failures = 0;
                self.persist_cell(cs).await?;
                tracing::info!(
                    "cell {} came online ({} node{})",
                    cell.key,
                    instances.len(),
                    if instances.len() == 1 { "" } else { "s" }
                );
                return Ok(stage);
            }
        }

        // Still launching — is it stuck?
        let age = Utc::now() - since;
        if age > self.deploy_timeout() {
            return self
                .on_launch_timeout(cell, cluster_id, &instances, age)
                .await;
        }
        Ok(CellStage::Launching {
            cluster_id,
            instances,
            since,
        })
    }

    async fn on_launch_timeout(
        &self,
        cell: &MatrixCell,
        cluster_id: Uuid,
        instances: &[InstanceSpec],
        age: chrono::Duration,
    ) -> Result<CellStage> {
        let failures = {
            let s = self.state.lock().await;
            s.find(&cell.key).map(|c| c.deploy_failures).unwrap_or(0) + 1
        };
        tracing::warn!(
            "cell {} deploy timed out after {} min, attempt {} ({} nodes)",
            cell.key,
            age.num_minutes(),
            failures,
            instances.len()
        );
        sentry::configure_scope(|scope| {
            scope.set_tag("cell", &cell.key);
            scope.set_tag("failures", failures.to_string());
        });
        sentry::capture_message(
            &format!(
                "mac-mgmt-runner deploy timeout: cell={} attempt={failures} nodes={}",
                cell.key,
                instances.len()
            ),
            sentry::Level::Warning,
        );

        // Tear down every failed VM; leave the cluster in place so put_config
        // state isn't lost (we'll re-enter Launching with fresh host keys on
        // the next reconcile tick).
        for inst in instances {
            if let Err(e) = self.incus.delete_instance(&inst.instance_name).await {
                tracing::warn!("deleting failed instance {}: {e}", inst.instance_name);
            }
        }

        let stage = CellStage::ConfigPushed {
            cluster_id,
            at: Utc::now(),
        };
        let mut cs = self.cell_with_stage(cell, stage.clone(), Utc::now()).await;
        cs.deploy_failures = failures;
        self.persist_cell(cs).await?;
        Ok(stage)
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
                CellStage::Running { .. } | CellStage::Launching { .. } => break,
                _ => continue,
            }
        }
        Ok(last)
    }

    pub async fn reconcile(&self) -> Result<()> {
        if self.is_paused().await {
            tracing::debug!("reconcile: paused, skipping");
            return Ok(());
        }
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

        // Garbage-collect Incus instances and mgmt clusters that share our
        // prefix but aren't tracked in local state (survivors from crashed
        // runs, manual operator edits, etc).
        if let Err(e) = self.gc().await {
            tracing::warn!("gc: {e:#}");
        }
        Ok(())
    }

    /// Garbage-collect drift between local state and the outside world:
    ///
    ///   1. Delete any Incus instance / mgmt cluster whose name starts
    ///      with `incus.name_prefix` but isn't tracked locally (forward
    ///      sweep — catches resources the runner didn't create).
    ///   2. Drop any local cell whose cluster_id no longer exists on
    ///      the mgmt server (reverse sweep — catches state that
    ///      outlived the resource it described, e.g. a cluster deleted
    ///      from the web UI).
    ///
    /// Safe to run alongside reconcile: state is persisted before
    /// external resources are created, so in-flight cells are always in
    /// the tracked set.
    pub async fn gc(&self) -> Result<()> {
        if self.is_paused().await {
            tracing::debug!("gc: paused, skipping");
            return Ok(());
        }
        let prefix = self.config.incus.name_prefix.clone();
        let (tracked_instances, tracked_clusters): (
            std::collections::HashSet<String>,
            std::collections::HashSet<Uuid>,
        ) = {
            let s = self.state.lock().await;
            let mut names = std::collections::HashSet::new();
            let mut ids = std::collections::HashSet::new();
            for cell in &s.cells {
                if let Some(cid) = cell.stage.cluster_id() {
                    ids.insert(cid);
                }
                for inst in cell.stage.instances() {
                    names.insert(inst.instance_name.clone());
                }
            }
            (names, ids)
        };

        let incus_names: std::collections::HashSet<String> =
            match self.incus.list_instances().await {
                Ok(v) => v.into_iter().collect(),
                Err(e) => {
                    tracing::warn!("gc: listing incus instances: {e:#}");
                    return Ok(());
                }
            };
        let server_clusters: Vec<(Uuid, String)> = match self.mgmt.list_clusters().await {
            Ok(rows) => rows.into_iter().map(|r| (r.id, r.name)).collect(),
            Err(e) => {
                tracing::warn!("gc: listing clusters: {e:#}");
                return Ok(());
            }
        };
        let server_cluster_ids: std::collections::HashSet<Uuid> =
            server_clusters.iter().map(|(id, _)| *id).collect();

        // Forward sweep — untracked Incus instances with our prefix.
        for name in &incus_names {
            if !name.starts_with(&prefix) || tracked_instances.contains(name) {
                continue;
            }
            tracing::info!("gc: deleting untracked incus instance {name}");
            if let Err(e) = self.incus.delete_instance(name).await {
                tracing::warn!("gc: deleting incus instance {name}: {e:#}");
            }
        }

        // Forward sweep — untracked clusters with our prefix.
        for (id, name) in &server_clusters {
            if !name.starts_with(&prefix) || tracked_clusters.contains(id) {
                continue;
            }
            tracing::info!("gc: deleting untracked cluster {name} ({id})");
            if let Err(e) = self.mgmt.delete_cluster(*id).await {
                tracing::warn!("gc: deleting cluster {id}: {e:#}");
            }
        }

        // Reverse sweep — stale state entries whose cluster vanished.
        // Pending cells (no cluster_id) are never stale; skip them so a
        // cell that's about to create its cluster doesn't get wiped.
        let stale_keys: Vec<String> = {
            let s = self.state.lock().await;
            s.cells
                .iter()
                .filter_map(|cell| {
                    let cid = cell.stage.cluster_id()?;
                    if server_cluster_ids.contains(&cid) {
                        None
                    } else {
                        Some(cell.key.clone())
                    }
                })
                .collect()
        };
        for key in stale_keys {
            tracing::info!("gc: dropping stale state for {key} (cluster gone from server)");
            // Don't call destroy_cell — the cluster is already gone. Just
            // remove any lingering Incus instance (forward sweep may have
            // missed it if it was added between our snapshot calls) and
            // delete the state entry.
            let inst_names: Vec<String> = {
                let s = self.state.lock().await;
                s.find(&key)
                    .map(|c| {
                        c.stage
                            .instances()
                            .iter()
                            .map(|i| i.instance_name.clone())
                            .collect()
                    })
                    .unwrap_or_default()
            };
            for name in inst_names {
                let _ = self.incus.delete_instance(&name).await;
            }
            {
                let mut s = self.state.lock().await;
                s.remove(&key);
            }
        }
        self.save_state().await?;
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

        for inst in cell.stage.instances() {
            if let Err(e) = self.incus.delete_instance(&inst.instance_name).await {
                tracing::warn!("deleting incus instance {}: {e}", inst.instance_name);
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
        // Explicit operator action — wake the runner back up.
        self.set_paused(false).await?;
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
        // Park the runner. Automatic loops will sit idle until the
        // operator triggers provision / reprovision / redeploy again.
        self.set_paused(true).await?;
        tracing::info!("teardown: runner paused — awaiting explicit resume");
        // Sweep any clusters on the mgmt server with our prefix that aren't
        // in local state (crashed runs, manual interference) so teardown
        // genuinely leaves nothing behind.
        let prefix = self.config.incus.name_prefix.clone();
        if let Ok(rows) = self.mgmt.list_clusters().await {
            for row in rows {
                if !row.name.starts_with(&prefix) {
                    continue;
                }
                tracing::info!("teardown: removing stray cluster {} ({})", row.name, row.id);
                let _ = self.incus.delete_instance(&row.name).await;
                if let Err(e) = self.mgmt.delete_cluster(row.id).await {
                    tracing::warn!("teardown: deleting cluster {}: {e:#}", row.id);
                }
            }
        }
        // Drop the rollout group now that its last member is gone.
        let group_name = self.rollout_group_name();
        if let Ok(groups) = self.mgmt.list_rollout_groups().await {
            if let Some(g) = groups.into_iter().find(|g| g.name == group_name) {
                if let Err(e) = self.mgmt.delete_rollout_group(g.id).await {
                    tracing::warn!("teardown: deleting rollout group {}: {e:#}", g.id);
                }
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
        // teardown left us paused — this is an explicit rebuild, so resume.
        self.set_paused(false).await?;

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

    /// Pick a random Running cluster, pick a random resource type
    /// (skill / bundle / mcp-server / mcp-bundle), and either install a
    /// not-yet-installed item or remove an installed one. No-op if the
    /// fleet has no running cells or the chosen endpoint returned nothing
    /// actionable.
    pub async fn chaos_tick(&self) -> Result<Option<String>> {
        if self.is_paused().await {
            return Ok(None);
        }
        let clusters: Vec<(String, Uuid)> = {
            let s = self.state.lock().await;
            s.cells
                .iter()
                .filter_map(|c| {
                    if c.stage.is_running() {
                        c.stage.cluster_id().map(|cid| (c.key.clone(), cid))
                    } else {
                        None
                    }
                })
                .collect()
        };
        let Some((key, cluster_id)) = clusters.choose(&mut rand::thread_rng()).cloned() else {
            return Ok(None);
        };

        let kind_idx: usize = rand::thread_rng().gen_range(0..4);
        let result = match kind_idx {
            0 => self.chaos_skill(cluster_id).await,
            1 => self.chaos_bundle(cluster_id).await,
            2 => self.chaos_mcp_server(cluster_id).await,
            _ => self.chaos_mcp_bundle(cluster_id).await,
        };

        match result {
            Ok(Some(detail)) => {
                tracing::info!("chaos: cell={key} {detail}");
                Ok(Some(format!("{key}: {detail}")))
            }
            Ok(None) => Ok(None),
            Err(e) => {
                tracing::warn!("chaos: cell={key} failed: {e:#}");
                let _ = sentry::integrations::anyhow::capture_anyhow(&e);
                Err(e)
            }
        }
    }

    /// Decide install vs uninstall weighted by the current install ratio:
    ///   P(uninstall) = installed / total
    ///   P(install)   = not_installed / total
    /// This pulls the fleet toward the middle — if most items are already
    /// installed, chaos leans toward removal; if most are missing, chaos
    /// leans toward installation. Returns true for install.
    fn roll_install(installed: usize, not_installed: usize) -> Option<bool> {
        let total = installed + not_installed;
        if total == 0 {
            return None;
        }
        Some(rand::thread_rng().gen_range(0..total) < not_installed)
    }

    async fn chaos_skill(&self, cluster_id: Uuid) -> Result<Option<String>> {
        let rows = self.mgmt.list_available_skill_channels(cluster_id).await?;
        let installable: Vec<Uuid> =
            rows.iter().filter(|r| !r.installed).map(|r| r.id).collect();
        let uninstallable: Vec<Uuid> = rows
            .iter()
            .filter_map(|r| r.cluster_skill_id.filter(|_| r.installed))
            .collect();
        let Some(install) = Self::roll_install(uninstallable.len(), installable.len()) else {
            return Ok(None);
        };
        if install {
            let Some(id) = installable.choose(&mut rand::thread_rng()).copied() else {
                return Ok(None);
            };
            self.mgmt.add_skill(cluster_id, id).await?;
            Ok(Some(format!("+skill {id}")))
        } else {
            let Some(id) = uninstallable.choose(&mut rand::thread_rng()).copied() else {
                return Ok(None);
            };
            self.mgmt.remove_skill(cluster_id, id).await?;
            Ok(Some(format!("-skill {id}")))
        }
    }

    async fn chaos_bundle(&self, cluster_id: Uuid) -> Result<Option<String>> {
        let rows = self.mgmt.list_available_bundles(cluster_id).await?;
        let installable: Vec<Uuid> =
            rows.iter().filter(|r| !r.installed).map(|r| r.id).collect();
        let installed_count = rows.iter().filter(|r| r.installed).count();
        let Some(install) = Self::roll_install(installed_count, installable.len()) else {
            return Ok(None);
        };
        if install {
            let Some(id) = installable.choose(&mut rand::thread_rng()).copied() else {
                return Ok(None);
            };
            self.mgmt.add_bundle(cluster_id, id).await?;
            Ok(Some(format!("+bundle {id}")))
        } else {
            // bundles need the cluster_bundle_id for DELETE — separate endpoint.
            let cluster_rows = self.mgmt.list_cluster_bundles(cluster_id).await?;
            let ids: Vec<Uuid> = cluster_rows.iter().map(|r| r.cluster_bundle_id).collect();
            let Some(id) = ids.choose(&mut rand::thread_rng()).copied() else {
                return Ok(None);
            };
            self.mgmt.remove_bundle(cluster_id, id).await?;
            Ok(Some(format!("-bundle {id}")))
        }
    }

    async fn chaos_mcp_server(&self, cluster_id: Uuid) -> Result<Option<String>> {
        let rows = self.mgmt.list_available_mcp_servers(cluster_id).await?;
        let installable: Vec<Uuid> =
            rows.iter().filter(|r| !r.installed).map(|r| r.id).collect();
        let uninstallable: Vec<Uuid> = rows
            .iter()
            .filter_map(|r| r.cluster_mcp_server_id.filter(|_| r.installed))
            .collect();
        let Some(install) = Self::roll_install(uninstallable.len(), installable.len()) else {
            return Ok(None);
        };
        if install {
            let Some(id) = installable.choose(&mut rand::thread_rng()).copied() else {
                return Ok(None);
            };
            self.mgmt.add_mcp_server(cluster_id, id).await?;
            Ok(Some(format!("+mcp-server {id}")))
        } else {
            let Some(id) = uninstallable.choose(&mut rand::thread_rng()).copied() else {
                return Ok(None);
            };
            self.mgmt.remove_mcp_server(cluster_id, id).await?;
            Ok(Some(format!("-mcp-server {id}")))
        }
    }

    async fn chaos_mcp_bundle(&self, cluster_id: Uuid) -> Result<Option<String>> {
        let rows = self.mgmt.list_available_mcp_bundles(cluster_id).await?;
        let installable: Vec<Uuid> =
            rows.iter().filter(|r| !r.installed).map(|r| r.id).collect();
        let installed_count = rows.iter().filter(|r| r.installed).count();
        let Some(install) = Self::roll_install(installed_count, installable.len()) else {
            return Ok(None);
        };
        if install {
            let Some(id) = installable.choose(&mut rand::thread_rng()).copied() else {
                return Ok(None);
            };
            self.mgmt.add_mcp_bundle(cluster_id, id).await?;
            Ok(Some(format!("+mcp-bundle {id}")))
        } else {
            let cluster_rows = self.mgmt.list_cluster_mcp_bundles(cluster_id).await?;
            let ids: Vec<Uuid> = cluster_rows
                .iter()
                .map(|r| r.cluster_mcp_bundle_id)
                .collect();
            let Some(id) = ids.choose(&mut rand::thread_rng()).copied() else {
                return Ok(None);
            };
            self.mgmt.remove_mcp_bundle(cluster_id, id).await?;
            Ok(Some(format!("-mcp-bundle {id}")))
        }
    }

    /// Pick a random Running cell and apply either a toggle (flip a
    /// random instance's Incus power state) or a reprovision. Both ops
    /// re-verify the cell is still in the Running stage right before
    /// firing so a cell that slipped into Launching via another loop
    /// isn't caught mid-boot.
    pub async fn chaos_vm_tick(&self) -> Result<Option<String>> {
        if self.is_paused().await {
            return Ok(None);
        }
        let candidates: Vec<(String, Vec<String>)> = {
            let s = self.state.lock().await;
            s.cells
                .iter()
                .filter(|c| c.stage.is_running())
                .map(|c| {
                    (
                        c.key.clone(),
                        c.stage
                            .instances()
                            .iter()
                            .map(|i| i.instance_name.clone())
                            .collect(),
                    )
                })
                .collect()
        };
        let Some((key, instances)) = candidates.choose(&mut rand::thread_rng()).cloned()
        else {
            return Ok(None);
        };

        // 9/10 toggle, 1/10 reprovision — toggles are cheap, reprovisions
        // are expensive (minutes) and disruptive to the whole cell so
        // we want them rare.
        let reprovision = rand::thread_rng().gen_range(0..10) == 0;
        if reprovision {
            tracing::info!("chaos-vm: reprovision {key}");
            self.reprovision_cell(&key).await?;
            return Ok(Some(format!("{key}: reprovision")));
        }

        let Some(name) = instances.choose(&mut rand::thread_rng()).cloned() else {
            return Ok(None);
        };
        let status = self
            .incus
            .instance_status(&name)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        let action = match status.to_ascii_lowercase().as_str() {
            "running" => "stop",
            "stopped" => "start",
            other => {
                tracing::debug!("chaos-vm: {name} has state {other:?}; skipping toggle");
                return Ok(None);
            }
        };
        if !self.cell_still_running(&key).await {
            tracing::debug!("chaos-vm: cell {key} left Running; skipping toggle");
            return Ok(None);
        }
        tracing::info!("chaos-vm: toggle {name} {status} → {action} (cell={key})");
        self.incus
            .set_instance_state(&name, action)
            .await
            .with_context(|| format!("{action} {name}"))?;
        Ok(Some(format!("{key}: toggle {name} {action}")))
    }

    /// Defensive re-check: between the candidate scan and the actual
    /// Incus call, another loop could have reprovisioned the cell and
    /// pushed it back into Launching. Skip the op if so.
    async fn cell_still_running(&self, key: &str) -> bool {
        let s = self.state.lock().await;
        s.find(key).map(|c| c.stage.is_running()).unwrap_or(false)
    }

    pub async fn reprovision_random(&self) -> Result<Option<String>> {
        // Explicit operator action — wake the runner back up.
        self.set_paused(false).await?;
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
                Some(CellStage::ClusterCreated { .. })
                | Some(CellStage::ConfigPushed { .. }) => {
                    (None, Some("provisioning".into()))
                }
                Some(CellStage::Launching { instances, since, .. }) => {
                    let waited = (now - *since).num_seconds().max(0);
                    (
                        None,
                        Some(format!(
                            "launching (waited {waited}s, {} node{})",
                            instances.len(),
                            if instances.len() == 1 { "" } else { "s" }
                        )),
                    )
                }
                Some(CellStage::Running { cluster_id, instances, .. }) => {
                    match self.mgmt.list_cluster_machines(*cluster_id).await {
                        Ok(rows) => {
                            // Pick the oldest heartbeat among our known
                            // instance_ids as the representative.
                            let latest = instances
                                .iter()
                                .filter_map(|inst| {
                                    rows.iter().find(|r| r.instance_id == inst.instance_id)
                                })
                                .min_by_key(|r| r.reported_at);
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

            let instances = s
                .as_ref()
                .map(|c| {
                    c.stage
                        .instances()
                        .iter()
                        .map(|i| CellInstance {
                            instance_name: i.instance_name.clone(),
                            instance_id: i.instance_id.clone(),
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            cells.push(CellStatus {
                key: mc.key.clone(),
                stage: s
                    .as_ref()
                    .map(|c| c.stage.label().to_string())
                    .unwrap_or_else(|| "missing".into()),
                cluster_id: s.as_ref().and_then(|c| c.stage.cluster_id()),
                node_count: mc.node_count,
                instances,
                launching_since: s.as_ref().and_then(|c| c.stage.launching_since()),
                deploy_failures: s.as_ref().map(|c| c.deploy_failures).unwrap_or(0),
                healthy,
                detail,
            });
        }

        StatusSnapshot {
            total_cells: matrix.len(),
            running: state.cells.iter().filter(|c| c.stage.is_running()).count(),
            paused: state.paused,
            matrix_axes: MatrixAxes {
                cluster_sizes: self.config.matrix.cluster_sizes.clone(),
                agents: self
                    .config
                    .matrix
                    .agents
                    .clone()
                    .unwrap_or_else(|| vec!["openclaw".into(), "none".into()]),
                llms: self
                    .config
                    .matrix
                    .llms
                    .clone()
                    .unwrap_or_else(|| vec!["ollama".into(), "lms".into(), "cloud".into()]),
                cloud_providers_configured: self.config.matrix.cloud_api_keys.len(),
                ollama_model: self.config.matrix.ollama_model.clone(),
                lms_model: self.config.matrix.lms_model.clone(),
            },
            runner_version: crate::VERSION.into(),
            runner_git_sha: crate::GIT_SHA.into(),
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
/// Periodically fires a random install/uninstall against a random running
/// cluster. Interval is jittered ±50% around `fleet.chaos_interval`. Set
/// chaos_interval to "0" or "off" to disable the loop entirely.
pub async fn chaos_loop(orch: Arc<Orchestrator>) {
    let raw = orch.config.fleet.chaos_interval.trim();
    if raw.is_empty() || raw == "off" || raw == "0" {
        tracing::info!("chaos loop disabled");
        return;
    }
    let base = match parse_duration(raw) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("invalid chaos_interval {raw:?}: {e}; disabling chaos loop");
            return;
        }
    };
    // Offset the first fire so chaos doesn't pile on top of an initial
    // reconcile wave.
    tokio::time::sleep(base).await;
    loop {
        let base_secs = base.as_secs().max(60) as f64;
        let jitter = {
            let mut rng = rand::thread_rng();
            rng.gen_range(0.5..1.5)
        };
        let delay = Duration::from_secs_f64(base_secs * jitter);
        tokio::time::sleep(delay).await;
        match orch.chaos_tick().await {
            Ok(Some(d)) => tracing::debug!("chaos tick: {d}"),
            Ok(None) => tracing::debug!("chaos tick: nothing to do"),
            Err(e) => tracing::warn!("chaos tick: {e:#}"),
        }
    }
}

/// Periodically picks a random Running cell and applies a VM-level chaos
/// op (start / stop / reprovision). Interval is fleet.vm_chaos_interval
/// (default 30m). No-op if the fleet has no running cells.
pub async fn vm_chaos_loop(orch: Arc<Orchestrator>) {
    let interval = parse_duration(&orch.config.fleet.vm_chaos_interval)
        .unwrap_or(Duration::from_secs(1800));
    tokio::time::sleep(interval / 2).await;
    let mut ticker = tokio::time::interval(interval);
    ticker.tick().await;
    loop {
        ticker.tick().await;
        match orch.chaos_vm_tick().await {
            Ok(Some(k)) => tracing::info!("vm chaos: {k}"),
            Ok(None) => tracing::debug!("vm chaos: no running cells"),
            Err(e) => tracing::error!("vm chaos: {e:#}"),
        }
    }
}

// ── Public types for HTTP API + CLI ───────────────────────────────────

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct StatusSnapshot {
    pub total_cells: usize,
    pub running: usize,
    /// `true` while the runner is parked after a teardown — automatic
    /// loops are idle until an operator triggers provision / reprovision
    /// / redeploy.
    #[serde(default)]
    pub paused: bool,
    /// Effective matrix axes — surfaces the runner's configuration so an
    /// operator can tell at a glance whether `-n2` cells are expected.
    #[serde(default)]
    pub matrix_axes: MatrixAxes,
    /// Runner binary version (CARGO_PKG_VERSION).
    #[serde(default)]
    pub runner_version: String,
    /// Git commit the runner binary was built from (build.rs).
    #[serde(default)]
    pub runner_git_sha: String,
    pub cells: Vec<CellStatus>,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct MatrixAxes {
    pub cluster_sizes: Vec<u32>,
    pub agents: Vec<String>,
    pub llms: Vec<String>,
    pub cloud_providers_configured: usize,
    pub ollama_model: String,
    pub lms_model: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CellStatus {
    pub key: String,
    pub stage: String,
    pub cluster_id: Option<Uuid>,
    pub node_count: u32,
    pub instances: Vec<CellInstance>,
    pub launching_since: Option<DateTime<Utc>>,
    pub deploy_failures: u32,
    pub healthy: Option<bool>,
    pub detail: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CellInstance {
    pub instance_name: String,
    pub instance_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roll_install_corners() {
        for _ in 0..50 {
            assert_eq!(Orchestrator::roll_install(0, 10), Some(true));
        }
        for _ in 0..50 {
            assert_eq!(Orchestrator::roll_install(10, 0), Some(false));
        }
        assert_eq!(Orchestrator::roll_install(0, 0), None);
    }

    /// With 1 installed and 9 not-installed, install should land ~90%
    /// of the time. 500 samples, tolerate ±10%.
    #[test]
    fn roll_install_bias_reflects_ratio() {
        let mut installs = 0;
        let mut uninstalls = 0;
        for _ in 0..500 {
            match Orchestrator::roll_install(1, 9) {
                Some(true) => installs += 1,
                Some(false) => uninstalls += 1,
                None => unreachable!(),
            }
        }
        let ratio = installs as f64 / (installs + uninstalls) as f64;
        assert!(
            (0.80..=0.98).contains(&ratio),
            "expected ~0.90 install ratio for (1 installed, 9 not), got {ratio:.2}"
        );
    }
}
