use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Persisted fleet state. One entry per matrix cell that is (or should be) alive.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FleetState {
    pub cells: Vec<CellState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellState {
    /// Stable identifier derived from the matrix key (e.g. "openclaw-cloud-anthropic").
    pub key: String,
    /// Cluster UUID registered on the mgmt server.
    pub cluster_id: Uuid,
    /// Incus instance name (identical to cluster name; derives from key + prefix).
    pub instance_name: String,
    /// Pregenerated daemon instance_id (hex SHA-256 of the SSH-wire ed25519 public key).
    /// Used to match heartbeats without waiting for the daemon to pick one itself.
    pub instance_id: String,
    /// When the instance was created.
    pub created_at: DateTime<Utc>,
    /// When the instance was last re-provisioned.
    pub last_reprovisioned_at: DateTime<Utc>,
    /// Set while we are still waiting for the first heartbeat from this instance.
    /// Cleared when a matching heartbeat arrives. A value older than the configured
    /// deploy_timeout triggers a teardown + retry.
    #[serde(default)]
    pub pending_since: Option<DateTime<Utc>>,
    /// Count of consecutive deploy timeouts. Resets to zero after a successful
    /// heartbeat; once it hits `max_deploy_retries` the cell is parked.
    #[serde(default)]
    pub deploy_failures: u32,
    /// When true, the cell has exceeded `max_deploy_retries` and will not be
    /// auto-retried until the operator runs `reprovision`.
    #[serde(default)]
    pub parked: bool,
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
