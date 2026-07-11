//! Eventual-consistency goal checks. Each is a poll-with-timeout that returns
//! `Ok(())` once the property holds, or an error on timeout.

use anyhow::{Result, bail};

use crate::assert::eventually;
use crate::env::Ctx;

/// Freshness window: a heartbeat newer than this counts the node as alive.
fn is_fresh(reported_at: chrono::DateTime<chrono::Utc>) -> bool {
    (chrono::Utc::now() - reported_at).num_seconds().abs() < 120
}

/// Every expected instance has a fresh, non-chaos-filtered heartbeat.
pub async fn heartbeats_fresh(ctx: &Ctx, instances: &[String]) -> Result<()> {
    let t = &ctx.timeouts;
    eventually("heartbeats_fresh", t.ec, t.poll, || async {
        let machines = ctx.mgmt().list_cluster_machines(ctx.cluster_id).await?;
        for id in instances {
            match machines.iter().find(|m| &m.instance_id == id) {
                Some(m) if is_fresh(m.reported_at) => {}
                Some(_) => bail!("instance {id} heartbeat is stale"),
                None => bail!("instance {id} has no heartbeat yet"),
            }
        }
        Ok(())
    })
    .await
}

/// All reported services on every known instance are healthy.
pub async fn all_services_healthy(ctx: &Ctx) -> Result<()> {
    let t = &ctx.timeouts;
    eventually("all_services_healthy", t.ec, t.poll, || async {
        let machines = ctx.mgmt().list_cluster_machines(ctx.cluster_id).await?;
        let live: Vec<_> = machines
            .iter()
            .filter(|m| is_fresh(m.reported_at))
            .collect();
        if live.is_empty() {
            bail!("no live machines");
        }
        for m in live {
            let Some(ext) = &m.services_extended else {
                continue;
            };
            if let Some(arr) = ext.as_array() {
                for svc in arr {
                    let state = svc.get("state").and_then(|v| v.as_str()).unwrap_or("");
                    // Accept common healthy/terminal-ok states.
                    if matches!(state, "failed" | "crashed" | "errored") {
                        let name = svc.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                        bail!("service {name} on {} is {state}", m.instance_id);
                    }
                }
            }
        }
        Ok(())
    })
    .await
}

/// Every probe reported for the cluster is ok.
pub async fn all_probes_healthy(ctx: &Ctx) -> Result<()> {
    let t = &ctx.timeouts;
    eventually("all_probes_healthy", t.ec, t.poll, || async {
        let probes = ctx.mgmt().list_probes(ctx.cluster_id).await?;
        if let Some(bad) = probes.iter().find(|p| !p.ok) {
            bail!(
                "probe {}/{} failed: {:?}",
                bad.service,
                bad.kind,
                bad.error_class
            );
        }
        Ok(())
    })
    .await
}

/// Every expected skill slug is present in a node's synced state.
pub async fn skills_synced(ctx: &Ctx, prefix: &str, expected: &[String]) -> Result<()> {
    let t = &ctx.timeouts;
    eventually("skills_synced", t.ec, t.poll, || async {
        let out = ctx.relay().shell_exec(prefix, "sync-state", None).await?;
        let json: serde_json::Value = serde_json::from_str(out.stdout().trim())
            .map_err(|e| anyhow::anyhow!("sync-state not JSON: {e}"))?;
        let synced: Vec<String> = json
            .get("skills")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        for want in expected {
            if !synced.iter().any(|s| s.contains(want)) {
                bail!("skill {want} not yet synced (have {synced:?})");
            }
        }
        Ok(())
    })
    .await
}

/// At least `min` MCP servers are present in a node's synced state.
pub async fn mcp_servers_present(ctx: &Ctx, prefix: &str, min: usize) -> Result<()> {
    let t = &ctx.timeouts;
    eventually("mcp_servers_present", t.ec, t.poll, || async {
        let out = ctx.relay().shell_exec(prefix, "sync-state", None).await?;
        let json: serde_json::Value = serde_json::from_str(out.stdout().trim())
            .map_err(|e| anyhow::anyhow!("sync-state not JSON: {e}"))?;
        let count = json
            .get("mcp_servers")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        if count < min {
            bail!("only {count} MCP servers present, want >= {min}");
        }
        Ok(())
    })
    .await
}

/// A memvault key is retrievable from `prefix` (used to assert cross-node sync).
pub async fn memvault_synced(ctx: &Ctx, prefix: &str, key: &str) -> Result<()> {
    let t = &ctx.timeouts;
    eventually("memvault_synced", t.ec, t.poll, || async {
        let out = ctx
            .relay()
            .shell_exec(prefix, "memctl", Some(&format!("get {key}")))
            .await?;
        if out.exit_code == Some(0) && !out.stdout().trim().is_empty() {
            Ok(())
        } else {
            bail!(
                "memvault key {key} not present on {prefix}: {:?}",
                out.error
            )
        }
    })
    .await
}
