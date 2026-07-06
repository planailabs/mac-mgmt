//! Setup workloads: ensure a cluster exists (with selected managed services)
//! and ensure N attached nodes are heartbeating. Run before random workloads.

use anyhow::Result;
use uuid::Uuid;

use crate::env::{Ctx, Env, NodeKind};
use crate::goals;

/// Feature toggles for `ensure_cluster`'s config.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClusterServices {
    pub ollama: bool,
    pub openclaw: bool,
    pub memvault: bool,
}

/// Find-or-create the cluster and apply a config enabling the selected services.
pub async fn ensure_cluster(env: &dyn Env, services: ClusterServices) -> Result<Uuid> {
    let cluster_id = env.mgmt().ensure_cluster(env.cluster_name()).await?;
    let config = serde_json::json!({
        "ollama": { "enabled": services.ollama },
        "openclaw": { "enabled": services.openclaw },
        "memvault": { "enabled": services.memvault },
    });
    // put_config tolerates partial configs (server merges into defaults).
    if let Err(e) = env.mgmt().put_config(cluster_id, &config).await {
        tracing::warn!("ensure_cluster: put_config failed (continuing): {e:#}");
    }
    Ok(cluster_id)
}

/// Ensure `n` nodes of `kind` exist and are heartbeating. Returns instance ids.
pub async fn ensure_nodes(ctx: &mut Ctx, n: usize, kind: NodeKind) -> Result<Vec<String>> {
    let ids = ctx.env.spawn_nodes(ctx.cluster_id, n, kind).await?;
    goals::heartbeats_fresh(ctx, &ids).await?;
    for id in &ids {
        if !ctx.instances.contains(id) {
            ctx.instances.push(id.clone());
        }
    }
    Ok(ids)
}
