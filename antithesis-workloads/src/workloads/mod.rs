//! The `Workload` trait and the registry of all workloads.
//!
//! A workload "causes something to happen" on a cluster and then asserts an
//! eventual-consistency property. The emulator picks workloads at random by
//! index into [`registry`]; `ensure-cluster` / `ensure-node` run first as setup.

use async_trait::async_trait;

use crate::env::Ctx;
use crate::rng::Rng;

pub mod cluster;
pub mod memvault;
pub mod node;
pub mod relay;
pub mod skills;
pub mod sse;

/// Outcome of a workload run.
#[derive(Debug)]
pub enum Outcome {
    Done,
    /// Nothing to do (e.g. empty skill catalog); not a failure.
    Skipped(&'static str),
}

#[async_trait]
pub trait Workload: Send + Sync {
    fn name(&self) -> &'static str;
    /// Cause an action and assert its eventual consistency.
    async fn run(&self, ctx: &Ctx, rng: &mut Rng) -> anyhow::Result<Outcome>;
}

/// All non-setup workloads, in a stable order (index = rng pick).
pub fn registry() -> Vec<Box<dyn Workload>> {
    vec![
        Box::new(node::NodeHeartbeat),
        Box::new(node::NodeProbe),
        Box::new(relay::RelayAccess),
        Box::new(relay::RelayFileTunnel),
        Box::new(relay::RelayShellTunnel),
        Box::new(relay::RelayLogs),
        Box::new(relay::RelayTcpTunnel),
        Box::new(sse::SsePush),
        Box::new(skills::SkillsAssignAssert),
        Box::new(skills::SkillsRemove),
        Box::new(memvault::MemvaultDoc),
        Box::new(memvault::MemvaultGraph),
        Box::new(node::ChaosServiceCrash),
    ]
}
