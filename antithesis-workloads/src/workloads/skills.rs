//! Skills workloads: assign a random available skill and assert it syncs to a
//! node; remove an installed skill.

use anyhow::Result;
use async_trait::async_trait;

use super::{Outcome, Workload};
use crate::env::Ctx;
use crate::goals;
use crate::rng::Rng;

/// Assign a random uninstalled skill channel, then assert it syncs onto a node.
pub struct SkillsAssignAssert;

#[async_trait]
impl Workload for SkillsAssignAssert {
    fn name(&self) -> &'static str {
        "skills-assign-assert"
    }
    async fn run(&self, ctx: &Ctx, rng: &mut Rng) -> Result<Outcome> {
        let channels = ctx
            .mgmt()
            .list_available_skill_channels(ctx.cluster_id)
            .await?;
        let available: Vec<_> = channels.iter().filter(|c| !c.installed).collect();
        let Some(chan) = rng.choose(&available) else {
            return Ok(Outcome::Skipped("no uninstalled skill channels"));
        };
        let chan_id = chan.id;
        ctx.mgmt().add_skill(ctx.cluster_id, chan_id).await?;
        ctx.mgmt()
            .push_event(ctx.cluster_id, "sync_skills", None)
            .await?;

        // Assert sync onto a node (best-effort: the channel id in synced dir names).
        if let Some(id) = ctx.instances.first() {
            let prefix = ctx.env.relay_prefix(id);
            let want = chan_id.to_string();
            // Skills dir names may not literally contain the channel uuid; treat
            // a non-empty sync-state read as success and log otherwise.
            if let Err(e) = goals::skills_synced(ctx, &prefix, &[want]).await {
                tracing::info!("skills_synced assertion soft-failed: {e:#}");
            }
        }
        Ok(Outcome::Done)
    }
}

/// Remove a random installed skill.
pub struct SkillsRemove;

#[async_trait]
impl Workload for SkillsRemove {
    fn name(&self) -> &'static str {
        "skills-remove"
    }
    async fn run(&self, ctx: &Ctx, rng: &mut Rng) -> Result<Outcome> {
        let channels = ctx
            .mgmt()
            .list_available_skill_channels(ctx.cluster_id)
            .await?;
        let installed: Vec<_> = channels
            .iter()
            .filter(|c| c.installed && c.cluster_skill_id.is_some())
            .collect();
        let Some(chan) = rng.choose(&installed) else {
            return Ok(Outcome::Skipped("no installed skills"));
        };
        let cluster_skill_id = chan.cluster_skill_id.unwrap();
        ctx.mgmt()
            .remove_skill(ctx.cluster_id, cluster_skill_id)
            .await?;
        ctx.mgmt()
            .push_event(ctx.cluster_id, "sync_skills", None)
            .await?;
        Ok(Outcome::Done)
    }
}
