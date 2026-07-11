//! Memvault workloads driven through the `memctl` shell tunnel (memvault has no
//! HTTP API). Put a document and assert it's retrievable (cross-node when the
//! cluster has ≥2 nodes); add + link graph entities.

use anyhow::{Result, bail};
use async_trait::async_trait;

use super::{Outcome, Workload};
use crate::env::Ctx;
use crate::goals;
use crate::rng::Rng;

/// Extract the first long hex token from memctl output (a CID/node id).
fn first_hex_id(s: &str) -> Option<String> {
    s.split_whitespace()
        .find(|t| t.len() >= 32 && t.chars().all(|c| c.is_ascii_hexdigit()))
        .map(String::from)
}

/// Put a document on one node and assert it is retrievable (on another node
/// when ≥2 exist — exercises p2p sync).
pub struct MemvaultDoc;

#[async_trait]
impl Workload for MemvaultDoc {
    fn name(&self) -> &'static str {
        "memvault-doc"
    }
    async fn run(&self, ctx: &Ctx, rng: &mut Rng) -> Result<Outcome> {
        if ctx.instances.is_empty() {
            return Ok(Outcome::Skipped("no instances"));
        }
        let writer = ctx.env.relay_prefix(&ctx.instances[0]);
        let nonce = rng.next_u64();
        let title = format!("chaos-doc-{nonce:016x}");
        let put = ctx
            .relay()
            .shell_exec(
                &writer,
                "memctl",
                Some(&format!("put \"chaos body {nonce:016x}\" --title {title}")),
            )
            .await?;
        if put.exit_code.unwrap_or(-1) != 0 {
            bail!("memctl put failed: {:?}", put.error);
        }
        let Some(id) = first_hex_id(&put.stdout()) else {
            return Ok(Outcome::Skipped("memctl put returned no id"));
        };

        // Assert retrievable on a *different* node when we have one.
        let reader = if ctx.instances.len() >= 2 {
            ctx.env.relay_prefix(&ctx.instances[1])
        } else {
            writer
        };
        goals::memvault_synced(ctx, &reader, &id).await?;
        Ok(Outcome::Done)
    }
}

/// Add two graph entities and link them.
pub struct MemvaultGraph;

#[async_trait]
impl Workload for MemvaultGraph {
    fn name(&self) -> &'static str {
        "memvault-graph"
    }
    async fn run(&self, ctx: &Ctx, rng: &mut Rng) -> Result<Outcome> {
        if ctx.instances.is_empty() {
            return Ok(Outcome::Skipped("no instances"));
        }
        let prefix = ctx.env.relay_prefix(&ctx.instances[0]);
        let n = rng.next_u64();

        let a = ctx
            .relay()
            .shell_exec(
                &prefix,
                "memctl",
                Some(&format!("graph add note --prop name=a{n:x}")),
            )
            .await?;
        let b = ctx
            .relay()
            .shell_exec(
                &prefix,
                "memctl",
                Some(&format!("graph add note --prop name=b{n:x}")),
            )
            .await?;
        let (Some(sa), Some(sb)) = (first_hex_id(&a.stdout()), first_hex_id(&b.stdout())) else {
            return Ok(Outcome::Skipped("graph add returned no entity ids"));
        };
        let link = ctx
            .relay()
            .shell_exec(
                &prefix,
                "memctl",
                Some(&format!("graph link {sa} {sb} relates-to")),
            )
            .await?;
        if link.exit_code.unwrap_or(-1) != 0 {
            bail!("graph link failed: {:?}", link.error);
        }
        Ok(Outcome::Done)
    }
}
