//! Organization endpoints.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dioxus::prelude::*;
use plan_ai_api_mcp_macros::api_mcp_dioxus_server;

#[cfg(feature = "server")]
use crate::server_pool;
#[cfg(feature = "server")]
use crate::web::user::{current_user, principal_from, to_serverfn};
#[cfg(feature = "server")]
use plan_ai_api_mcp::{ApiError, Principal};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgRow {
    pub id: Uuid,
    pub name: String,
    pub member_count: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgListInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgCreateInput {
    pub name: String,
}

/// List all organizations (admin only).
#[api_mcp_dioxus_server(server = "list_organizations")]
pub async fn organization_list(
    pool: &sqlx::PgPool,
    principal: &Principal,
    _input: OrgListInput,
) -> Result<Vec<OrgRow>, ApiError> {
    principal.require_admin()?;
    let rows = sqlx::query_as::<_, (Uuid, String, i64, chrono::DateTime<chrono::Utc>)>(
        "SELECT o.id, o.name, \
         (SELECT count(*) FROM organization_members om WHERE om.organization_id = o.id), \
         o.created_at FROM organizations o ORDER BY o.name",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|(id, name, member_count, created_at)| OrgRow {
            id,
            name,
            member_count,
            created_at: created_at.format("%Y-%m-%d").to_string(),
        })
        .collect())
}

/// Create an organization (admin only). Returns the new id.
#[api_mcp_dioxus_server(server = "create_organization")]
pub async fn organization_create(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: OrgCreateInput,
) -> Result<Uuid, ApiError> {
    principal.require_admin()?;
    sqlx::query_scalar::<_, Uuid>("INSERT INTO organizations (name) VALUES ($1) RETURNING id")
        .bind(&input.name)
        .fetch_one(pool)
        .await
        .map_err(|e| ApiError::internal(format!("failed to create organization: {e}")))
}
