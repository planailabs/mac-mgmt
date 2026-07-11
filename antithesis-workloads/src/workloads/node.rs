//! Node-facing workloads: heartbeat check, assessment probe, service-crash
//! recovery.

use anyhow::{Result, bail};
use async_trait::async_trait;

use super::{Outcome, Workload};
use crate::env::Ctx;
use crate::goals;
use crate::rng::Rng;

/// Assert a randomly chosen known instance has a fresh heartbeat.
pub struct NodeHeartbeat;

#[async_trait]
impl Workload for NodeHeartbeat {
    fn name(&self) -> &'static str {
        "node-heartbeat"
    }
    async fn run(&self, ctx: &Ctx, rng: &mut Rng) -> Result<Outcome> {
        let Some(id) = rng.choose(&ctx.instances).cloned() else {
            return Ok(Outcome::Skipped("no known instances"));
        };
        goals::heartbeats_fresh(ctx, &[id]).await?;
        Ok(Outcome::Done)
    }
}

/// Request an assessment, then assert probes go healthy.
pub struct NodeProbe;

#[async_trait]
impl Workload for NodeProbe {
    fn name(&self) -> &'static str {
        "node-probe"
    }
    async fn run(&self, ctx: &Ctx, _rng: &mut Rng) -> Result<Outcome> {
        ctx.mgmt()
            .push_event(ctx.cluster_id, "request_assessment", None)
            .await?;
        goals::all_probes_healthy(ctx).await?;
        Ok(Outcome::Done)
    }
}

/// Crash a service (or the daemon) on a random node and assert recovery.
pub struct ChaosServiceCrash;

#[async_trait]
impl Workload for ChaosServiceCrash {
    fn name(&self) -> &'static str {
        "chaos-service-crash"
    }
    async fn run(&self, ctx: &Ctx, rng: &mut Rng) -> Result<Outcome> {
        let Some(id) = rng.choose(&ctx.instances).cloned() else {
            return Ok(Outcome::Skipped("no known instances"));
        };
        let prefix = ctx.env.relay_prefix(&id);
        // Restart the daemon itself — the service manager brings it back.
        let out = ctx
            .relay()
            .shell_exec(&prefix, "restart-daemon", None)
            .await?;
        if out.exit_code.unwrap_or(0) != 0 {
            bail!("restart-daemon failed: {:?}", out.error);
        }
        // Recovery: the node heartbeats again and services are healthy.
        goals::heartbeats_fresh(ctx, &[id]).await?;
        goals::all_services_healthy(ctx).await?;
        Ok(Outcome::Done)
    }
}
