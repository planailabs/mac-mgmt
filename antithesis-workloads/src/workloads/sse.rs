//! SSE push workload: push a random event (cluster- or instance-targeted) and,
//! for events with an observable effect, assert the matching EC goal.

use anyhow::Result;
use async_trait::async_trait;

use super::{Outcome, Workload};
use crate::env::Ctx;
use crate::goals;
use crate::rng::Rng;

const EVENTS: &[&str] = &[
    "ping",
    "sync_config",
    "sync_skills",
    "sync_mcp_servers",
    "sync_packages",
    "request_assessment",
];

pub struct SsePush;

#[async_trait]
impl Workload for SsePush {
    fn name(&self) -> &'static str {
        "sse-push"
    }
    async fn run(&self, ctx: &Ctx, rng: &mut Rng) -> Result<Outcome> {
        let event = *rng.choose(EVENTS).unwrap_or(&"ping");
        // Half the time, target a single instance if we have one.
        let target = if rng.below(2) == 0 {
            rng.choose(&ctx.instances).cloned()
        } else {
            None
        };
        let result = ctx
            .mgmt()
            .push_event(ctx.cluster_id, event, target.as_deref())
            .await?;
        if !result.ok {
            return Ok(Outcome::Skipped("server rejected event"));
        }
        // For observable events, verify the resulting state.
        match event {
            "request_assessment" => goals::all_probes_healthy(ctx).await?,
            "sync_config" | "sync_mcp_servers" | "sync_skills" | "sync_packages" => {
                goals::all_services_healthy(ctx).await?
            }
            _ => {}
        }
        Ok(Outcome::Done)
    }
}
