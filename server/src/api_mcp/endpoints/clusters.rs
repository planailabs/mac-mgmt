//! Cluster endpoints: list/get/create/rename/delete plus cluster-scoped
//! settings (nixpkgs pin, daemon version pin, cloud-init).

use dioxus::prelude::*;
use plan_ai_api_mcp_macros::api_mcp_dioxus_server;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "server")]
use super::internal;
#[cfg(feature = "server")]
use crate::api_mcp::access;
#[cfg(feature = "server")]
use crate::server_pool;
#[cfg(feature = "server")]
use crate::web::user::{current_user, principal_from, to_serverfn};
#[cfg(feature = "server")]
use plan_ai_api_mcp::{ApiError, Principal};

// ── DTOs ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterListInput {}

/// A cluster with its owning organizations, as shown in the cluster list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterRow {
    pub id: String,
    pub name: String,
    pub org_names: Vec<String>,
    pub pinned_version: Option<String>,
    pub nixpkgs_commit: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// ── Handlers ────────────────────────────────────────────────────────────

/// was: list_clusters() in web/components/cluster_list.rs
#[api_mcp_dioxus_server(server = "list_clusters")]
pub async fn cluster_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: ClusterListInput,
) -> Result<Vec<ClusterRow>, ApiError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        name: String,
        org_names: Vec<String>,
        pinned_version: Option<String>,
        nixpkgs_commit: Option<String>,
        created_at: chrono::DateTime<chrono::Utc>,
    }

    const BASE: &str = "SELECT c.id, c.name, \
         COALESCE(array_agg(DISTINCT o.name) FILTER (WHERE o.name IS NOT NULL), '{}') AS org_names, \
         c.pinned_version, c.nixpkgs_commit, c.created_at \
         FROM clusters c \
         LEFT JOIN organization_clusters oc ON oc.cluster_id = c.id \
         LEFT JOIN organizations o ON o.id = oc.organization_id";

    let rows = if let Some(ids) = access::accessible_cluster_ids(pool, p).await? {
        sqlx::query_as::<_, Row>(&format!(
            "{BASE} WHERE c.id = ANY($1) GROUP BY c.id ORDER BY c.name"
        ))
        .bind(&ids)
        .fetch_all(pool)
        .await
        .map_err(internal)?
    } else {
        sqlx::query_as::<_, Row>(&format!("{BASE} GROUP BY c.id ORDER BY c.name"))
            .fetch_all(pool)
            .await
            .map_err(internal)?
    };

    Ok(rows
        .into_iter()
        .map(|r| ClusterRow {
            id: r.id.to_string(),
            name: r.name,
            org_names: r.org_names,
            pinned_version: r.pinned_version,
            nixpkgs_commit: r.nixpkgs_commit,
            created_at: r.created_at,
        })
        .collect())
}
