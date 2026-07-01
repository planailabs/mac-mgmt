//! Basic-auth list endpoints.

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
pub struct BasicAuthListRow {
    pub id: Uuid,
    pub name: String,
    pub organization_name: String,
    pub credential_count: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BasicAuthListInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BasicAuthCreateInput {
    pub organization_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BasicAuthDeleteInput {
    pub id: Uuid,
}

/// List basic-auth lists the caller may see (admins: all; else their orgs').
#[api_mcp_dioxus_server(server = "list_basic_auth_lists")]
pub async fn basic_auth_list(
    pool: &sqlx::PgPool,
    principal: &Principal,
    _input: BasicAuthListInput,
) -> Result<Vec<BasicAuthListRow>, ApiError> {
    let base = "SELECT b.id, b.name, o.name, \
                (SELECT count(*) FROM basic_auth_credentials c WHERE c.list_id = b.id) \
                FROM basic_auth_lists b JOIN organizations o ON o.id = b.organization_id";

    let rows = match principal.read_filter() {
        None => {
            sqlx::query_as::<_, (Uuid, String, String, i64)>(&format!("{base} ORDER BY b.name"))
                .fetch_all(pool)
                .await
        }
        Some(org_ids) => {
            sqlx::query_as::<_, (Uuid, String, String, i64)>(&format!(
                "{base} WHERE b.organization_id = ANY($1) ORDER BY b.name"
            ))
            .bind(org_ids)
            .fetch_all(pool)
            .await
        }
    }
    .map_err(super::internal)?;

    Ok(rows
        .into_iter()
        .map(|(id, name, organization_name, credential_count)| BasicAuthListRow {
            id,
            name,
            organization_name,
            credential_count,
        })
        .collect())
}

/// Create a basic-auth list in an organization (requires org write). Returns the new id.
#[api_mcp_dioxus_server(server = "create_basic_auth_list")]
pub async fn basic_auth_create(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: BasicAuthCreateInput,
) -> Result<Uuid, ApiError> {
    principal.require_write(&input.organization_id)?;
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO basic_auth_lists (organization_id, name) VALUES ($1, $2) RETURNING id",
    )
    .bind(input.organization_id)
    .bind(&input.name)
    .fetch_one(pool)
    .await
    .map_err(super::internal)
}

/// Delete a basic-auth list (requires org write). Clears webspace references
/// and notifies the proxy.
#[api_mcp_dioxus_server(server = "delete_basic_auth_list")]
pub async fn basic_auth_delete(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: BasicAuthDeleteInput,
) -> Result<(), ApiError> {
    let org_id = super::owning_org(pool, "basic_auth_lists", input.id, "basic-auth list").await?;
    principal.require_write(&org_id)?;

    sqlx::query(
        "UPDATE webspaces SET auth_mode = 'none', auth_basic_list_id = NULL \
         WHERE auth_basic_list_id = $1",
    )
    .bind(input.id)
    .execute(pool)
    .await
    .map_err(super::internal)?;

    sqlx::query("DELETE FROM basic_auth_lists WHERE id = $1")
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(super::internal)?;

    crate::api::internal::notify_proxy_reload();
    Ok(())
}
