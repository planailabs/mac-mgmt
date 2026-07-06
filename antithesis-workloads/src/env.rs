//! The `Env` abstraction: what differs between the prod (mmr) cluster and the
//! antithesis (fresh incus) cluster. Concrete impls live in `mmr-causality`.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use crate::mgmt::MgmtApi;
use crate::relay::RelayApi;

/// Kind of node to spawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    /// Regular fleet node (counts toward rollout health).
    Fleet,
    /// Disposable chaos node (excluded from fleet/rollout health).
    Chaos,
}

/// Poll timeouts, tuned per environment (short for local incus, long for prod).
#[derive(Debug, Clone)]
pub struct Timeouts {
    pub ec: Duration,
    pub poll: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            ec: Duration::from_secs(120),
            poll: Duration::from_secs(3),
        }
    }
}

/// Everything an environment exposes to workloads.
#[async_trait]
pub trait Env: Send + Sync {
    fn name(&self) -> &str;
    fn mgmt(&self) -> &MgmtApi;
    fn relay(&self) -> &RelayApi;
    fn organization_id(&self) -> Uuid;
    fn cluster_name(&self) -> &str;

    /// Spawn `n` attached nodes of the given kind, returning their instance ids.
    /// Prod rejects this for both kinds except via the chaos-node path; the
    /// antithesis env spins up fresh daemon containers.
    async fn spawn_nodes(&self, cluster_id: Uuid, n: usize, kind: NodeKind) -> Result<Vec<String>>;

    /// Remove a previously spawned node.
    async fn remove_node(&self, cluster_id: Uuid, instance_id: &str) -> Result<()>;

    /// The relay prefix for an instance (usually the first 12 hex of the id, or
    /// the full id — env decides based on how its relay advertises instances).
    fn relay_prefix(&self, instance_id: &str) -> String {
        instance_id.to_string()
    }
}

/// Per-run context handed to every workload.
pub struct Ctx {
    pub env: Arc<dyn Env>,
    pub cluster_id: Uuid,
    pub timeouts: Timeouts,
    /// Instance ids known to be part of this cluster (populated by ensure-node).
    pub instances: Vec<String>,
}

impl Ctx {
    pub fn new(env: Arc<dyn Env>, cluster_id: Uuid) -> Self {
        Self {
            env,
            cluster_id,
            timeouts: Timeouts::default(),
            instances: Vec::new(),
        }
    }

    pub fn mgmt(&self) -> &MgmtApi {
        self.env.mgmt()
    }
    pub fn relay(&self) -> &RelayApi {
        self.env.relay()
    }
}
