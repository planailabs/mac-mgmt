//! Daemon-version endpoints: the catalog of daemon versions known to the
//! server (synced from xzar pins), plus per-version views used by the web
//! UI (store paths, clusters running/pinned to a version, rollouts).
//!
//! All endpoints are admin-only, matching the legacy `#[server]` functions.

use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use plan_ai_api_mcp_macros::api_mcp_dioxus_server;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "server")]
use super::internal;
#[cfg(feature = "server")]
use crate::server_pool;
#[cfg(feature = "server")]
use crate::web::user::{current_user, principal_from, to_serverfn};
#[cfg(feature = "server")]
use plan_ai_api_mcp::{ApiError, Principal};

// ── DTOs ────────────────────────────────────────────────────────────────

/// One known daemon version and when it was first seen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DaemonVersionRow {
    pub version: String,
    pub created_at: DateTime<Utc>,
}

/// Result of syncing the daemon-version catalog from xzar.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DaemonSyncResult {
    /// Versions newly added to the catalog.
    pub created: u32,
    /// Versions removed because they no longer have an xzar pin.
    pub removed: u32,
}

/// A cluster with daemons currently heartbeating a given version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct VersionCluster {
    pub id: Uuid,
    pub name: String,
    /// Number of daemon instances on this version in the cluster.
    pub instances: i64,
}

/// A rollout targeting a given daemon version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct VersionRollout {
    pub id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// A cluster whose daemon version is pinned to a given version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PinnedCluster {
    pub id: Uuid,
    pub name: String,
}

/// A per-system Nix store path for one daemon version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DaemonStorePath {
    /// Nix system, e.g. `aarch64-darwin`.
    pub system: String,
    /// Full `/nix/store/...` path of the daemon build.
    pub store_path: String,
}

// ── List ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DaemonVersionsListInput {}

/// was: list_daemon_versions() in web/components/daemon_version_list.rs
#[api_mcp_dioxus_server(server = "list_daemon_versions")]
pub async fn daemon_version_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: DaemonVersionsListInput,
) -> Result<Vec<DaemonVersionRow>, ApiError> {
    p.require_admin()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        version: String,
        created_at: DateTime<Utc>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT version, created_at FROM daemon_versions ORDER BY version DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    Ok(rows
        .into_iter()
        .map(|r| DaemonVersionRow {
            version: r.version,
            created_at: r.created_at,
        })
        .collect())
}

// ── Sync from xzar ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DaemonVersionsSyncInput {}

/// was: sync_daemon_versions_from_xzar() in web/components/daemon_version_list.rs
///
/// Fetch xzar pins, parse `daemon/{version}/{system}` and upsert the
/// distinct versions into `daemon_versions`. Store paths are resolved
/// live from xzar on each /api/update call, so they are not stored.
#[api_mcp_dioxus_server(server = "sync_daemon_versions_from_xzar")]
pub async fn daemon_version_sync_from_xzar(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: DaemonVersionsSyncInput,
) -> Result<DaemonSyncResult, ApiError> {
    p.require_admin()?;
    let cfg = crate::config::config();

    let xzar = cfg
        .xzar
        .as_ref()
        .ok_or_else(|| internal("xzar not configured"))?;
    let pins = crate::xzar::fetch_pins(&xzar.url, &xzar.token)
        .await
        .map_err(|e| internal(format!("xzar error: {e}")))?;

    let mut created: u32 = 0;
    let mut valid: std::collections::HashSet<String> = std::collections::HashSet::new();

    for pin in &pins {
        if pin.abandoned || pin.roots.is_empty() {
            continue;
        }
        let parts: Vec<&str> = pin.name.splitn(3, '/').collect();
        if parts.len() != 3 || parts[0] != "daemon" {
            continue;
        }
        let version = parts[1].to_string();
        if !valid.insert(version.clone()) {
            continue;
        }

        let inserted = sqlx::query_scalar::<_, bool>(
            "INSERT INTO daemon_versions (version) VALUES ($1) \
             ON CONFLICT (version) DO NOTHING \
             RETURNING true",
        )
        .bind(&version)
        .fetch_optional(pool)
        .await
        .map_err(internal)?;

        if inserted.is_some() {
            created += 1;
        }
    }

    let valid_vec: Vec<String> = valid.into_iter().collect();
    let removed = sqlx::query_scalar::<_, i64>(
        "WITH deleted AS ( \
             DELETE FROM daemon_versions dv \
             WHERE NOT (dv.version = ANY($1::text[])) \
             RETURNING dv.id \
         ) SELECT count(*) FROM deleted",
    )
    .bind(&valid_vec)
    .fetch_one(pool)
    .await
    .map_err(internal)?;

    Ok(DaemonSyncResult {
        created,
        removed: removed as u32,
    })
}

// ── Per-version views ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DaemonStorePathsInput {
    /// Daemon version to resolve store paths for.
    pub version: String,
}

/// was: get_daemon_store_paths() in web/components/daemon_version_detail.rs
///
/// Fetch all `daemon/{version}/{system}` pins from xzar live and return
/// the (system, store_path) pairs for the requested version.
#[api_mcp_dioxus_server(server = "get_daemon_store_paths")]
pub async fn daemon_version_store_paths(
    _pool: &sqlx::PgPool,
    p: &Principal,
    input: DaemonStorePathsInput,
) -> Result<Vec<DaemonStorePath>, ApiError> {
    p.require_admin()?;
    let cfg = crate::config::config();
    let xzar = cfg
        .xzar
        .as_ref()
        .ok_or_else(|| internal("xzar not configured"))?;
    let pins = crate::xzar::fetch_pins(&xzar.url, &xzar.token)
        .await
        .map_err(|e| internal(format!("xzar error: {e}")))?;

    let prefix = format!("daemon/{}/", input.version);
    let mut out: Vec<DaemonStorePath> = Vec::new();
    for pin in &pins {
        if pin.abandoned || pin.roots.is_empty() {
            continue;
        }
        let Some(system) = pin.name.strip_prefix(&prefix) else {
            continue;
        };
        if system.contains('/') {
            continue;
        }
        let raw = &pin.roots[0].drv_full;
        let store_path = if raw.starts_with("/nix/store/") {
            raw.clone()
        } else {
            format!("/nix/store/{raw}")
        };
        out.push(DaemonStorePath {
            system: system.to_string(),
            store_path,
        });
    }
    out.sort_by(|a, b| a.system.cmp(&b.system));
    Ok(out)
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClustersOnVersionInput {
    /// Daemon version to look up.
    pub version: String,
}

/// was: get_clusters_on_version() in web/components/daemon_version_detail.rs
#[api_mcp_dioxus_server(server = "get_clusters_on_version")]
pub async fn daemon_version_clusters_on(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClustersOnVersionInput,
) -> Result<Vec<VersionCluster>, ApiError> {
    p.require_admin()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        name: String,
        instances: i64,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT c.id, c.name, COUNT(*)::bigint AS instances \
         FROM daemon_heartbeats h \
         JOIN clusters c ON c.id = h.cluster_id \
         WHERE h.version = $1 \
         GROUP BY c.id, c.name \
         ORDER BY c.name",
    )
    .bind(&input.version)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    Ok(rows
        .into_iter()
        .map(|r| VersionCluster {
            id: r.id,
            name: r.name,
            instances: r.instances,
        })
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClustersPinnedToInput {
    /// Daemon version to look up.
    pub version: String,
}

/// was: get_clusters_pinned_to() in web/components/daemon_version_detail.rs
#[api_mcp_dioxus_server(server = "get_clusters_pinned_to")]
pub async fn daemon_version_clusters_pinned(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClustersPinnedToInput,
) -> Result<Vec<PinnedCluster>, ApiError> {
    p.require_admin()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        name: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, name FROM clusters WHERE pinned_version = $1 ORDER BY name",
    )
    .bind(&input.version)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    Ok(rows
        .into_iter()
        .map(|r| PinnedCluster {
            id: r.id,
            name: r.name,
        })
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RolloutsForVersionInput {
    /// Daemon version to look up.
    pub version: String,
}

/// was: get_rollouts_for_version() in web/components/daemon_version_detail.rs
#[api_mcp_dioxus_server(server = "get_rollouts_for_version")]
pub async fn daemon_version_rollouts(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: RolloutsForVersionInput,
) -> Result<Vec<VersionRollout>, ApiError> {
    p.require_admin()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        status: String,
        created_at: DateTime<Utc>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, status, created_at FROM rollouts \
         WHERE target_version = $1 \
         ORDER BY created_at DESC",
    )
    .bind(&input.version)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    Ok(rows
        .into_iter()
        .map(|r| VersionRollout {
            id: r.id,
            status: r.status,
            created_at: r.created_at,
        })
        .collect())
}

// ── Registration ────────────────────────────────────────────────────────
