//! Persisted fleet state.
//!
//! Each matrix cell is represented by a [`CellState`] whose lifecycle is
//! encoded as a [`CellStage`] sum type. Every externally-visible
//! transition (cluster created on the mgmt server, Incus instance
//! launched, heartbeat arrived) corresponds to a new `CellStage` variant,
//! and state is flushed to disk after every transition so a crash can
//! resume exactly where the previous run left off.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FleetState {
    pub cells: Vec<CellState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellState {
    /// Stable identifier derived from the matrix key (e.g. "openclaw-ollama").
    pub key: String,
    /// Current position in the cell lifecycle.
    #[serde(flatten)]
    pub stage: CellStage,
    /// Count of consecutive deploy timeouts. Resets to zero after the cell
    /// reaches `Running`. Shown in status output for observability.
    #[serde(default)]
    pub deploy_failures: u32,
    /// When the cell was last explicitly reprovisioned.
    pub last_reprovisioned_at: DateTime<Utc>,
}

/// One Incus instance belonging to a cell. Multi-node cells carry several.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceSpec {
    /// Incus instance name (also used as the daemon hostname).
    pub instance_name: String,
    /// Predicted daemon fingerprint; the cell transitions to `Running`
    /// when every instance's id shows up in heartbeats.
    pub instance_id: String,
}

/// Cell lifecycle. Each variant encodes precisely which external
/// resources exist. Serialized with `#[serde(tag = "stage")]` so the
/// JSON on disk reads naturally.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum CellStage {
    /// Nothing has been created anywhere yet.
    Pending,
    /// Cluster row exists on the mgmt server; no config, no VM.
    ClusterCreated {
        cluster_id: Uuid,
        at: DateTime<Utc>,
    },
    /// Cluster has a config applied; no VM yet.
    ConfigPushed {
        cluster_id: Uuid,
        at: DateTime<Utc>,
    },
    /// Incus instances created and booting. The cell transitions to
    /// `Running` when every `instances[].instance_id` shows up in the
    /// cluster's heartbeats, or back to `ConfigPushed` after the
    /// deploy_timeout expires (retry indefinitely).
    Launching {
        cluster_id: Uuid,
        instances: Vec<InstanceSpec>,
        /// When the Incus launches were issued — drives the watchdog timeout.
        since: DateTime<Utc>,
    },
    /// Every expected instance has heartbeated.
    Running {
        cluster_id: Uuid,
        instances: Vec<InstanceSpec>,
        /// When the cell first transitioned into Running.
        since: DateTime<Utc>,
    },
}

impl CellStage {
    pub fn cluster_id(&self) -> Option<Uuid> {
        match self {
            CellStage::Pending => None,
            CellStage::ClusterCreated { cluster_id, .. }
            | CellStage::ConfigPushed { cluster_id, .. }
            | CellStage::Launching { cluster_id, .. }
            | CellStage::Running { cluster_id, .. } => Some(*cluster_id),
        }
    }

    pub fn instances(&self) -> &[InstanceSpec] {
        match self {
            CellStage::Launching { instances, .. } | CellStage::Running { instances, .. } => {
                instances
            }
            _ => &[],
        }
    }

    pub fn launching_since(&self) -> Option<DateTime<Utc>> {
        match self {
            CellStage::Launching { since, .. } => Some(*since),
            _ => None,
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(self, CellStage::Running { .. })
    }

    /// Short human label for CLI output.
    pub fn label(&self) -> &'static str {
        match self {
            CellStage::Pending => "pending",
            CellStage::ClusterCreated { .. } => "cluster_created",
            CellStage::ConfigPushed { .. } => "config_pushed",
            CellStage::Launching { .. } => "launching",
            CellStage::Running { .. } => "running",
        }
    }
}

impl FleetState {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let s = std::fs::read_to_string(path)
            .with_context(|| format!("reading state {}", path.display()))?;
        let v: FleetState = serde_json::from_str(&s)
            .with_context(|| format!("parsing state {}", path.display()))?;
        Ok(v)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating state dir {}", parent.display()))?;
        }
        let tmp: PathBuf = path.with_extension("json.tmp");
        let s = serde_json::to_string_pretty(self)?;
        std::fs::write(&tmp, s)
            .with_context(|| format!("writing state tmp {}", tmp.display()))?;
        std::fs::rename(&tmp, path)
            .with_context(|| format!("renaming state file to {}", path.display()))?;
        Ok(())
    }

    pub fn find(&self, key: &str) -> Option<&CellState> {
        self.cells.iter().find(|c| c.key == key)
    }

    pub fn upsert(&mut self, cell: CellState) {
        if let Some(existing) = self.cells.iter_mut().find(|c| c.key == cell.key) {
            *existing = cell;
        } else {
            self.cells.push(cell);
        }
    }

    pub fn remove(&mut self, key: &str) {
        self.cells.retain(|c| c.key != key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_stage_roundtrips_through_json() {
        let now = Utc::now();
        let cid = Uuid::new_v4();
        let insts = vec![
            InstanceSpec {
                instance_name: "mmr-foo-1".into(),
                instance_id: "abc123".into(),
            },
            InstanceSpec {
                instance_name: "mmr-foo-2".into(),
                instance_id: "def456".into(),
            },
        ];
        let stages = vec![
            CellStage::Pending,
            CellStage::ClusterCreated { cluster_id: cid, at: now },
            CellStage::ConfigPushed { cluster_id: cid, at: now },
            CellStage::Launching {
                cluster_id: cid,
                instances: insts.clone(),
                since: now,
            },
            CellStage::Running {
                cluster_id: cid,
                instances: insts.clone(),
                since: now,
            },
        ];
        for stage in stages {
            let cell = CellState {
                key: "k".into(),
                stage: stage.clone(),
                deploy_failures: 2,
                last_reprovisioned_at: now,
            };
            let s = serde_json::to_string(&cell).expect("serialize");
            let back: CellState = serde_json::from_str(&s).expect("deserialize");
            assert_eq!(cell.stage.label(), back.stage.label());
            assert_eq!(cell.stage.cluster_id(), back.stage.cluster_id());
            assert_eq!(cell.stage.instances().len(), back.stage.instances().len());
        }
    }
}
