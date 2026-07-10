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

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BasicAuthGetInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BasicAuthCredentialRow {
    pub id: Uuid,
    pub username: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BasicAuthInfo {
    pub id: Uuid,
    pub name: String,
    pub organization_name: String,
    pub credentials: Vec<BasicAuthCredentialRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BasicAuthAddCredentialInput {
    /// The basic-auth list id.
    pub id: Uuid,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BasicAuthRemoveCredentialInput {
    /// The basic-auth list id.
    pub id: Uuid,
    /// The credential row to remove from the list.
    pub credential_id: Uuid,
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

/// Get a basic-auth list with its credential rows (requires org read).
#[api_mcp_dioxus_server(server = "get_basic_auth_list")]
pub async fn basic_auth_get(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: BasicAuthGetInput,
) -> Result<BasicAuthInfo, ApiError> {
    let row = sqlx::query_as::<_, (Uuid, String, Uuid)>(
        "SELECT id, name, organization_id FROM basic_auth_lists WHERE id = $1",
    )
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?
    .ok_or_else(|| ApiError::not_found("list not found"))?;

    let (id, name, org_id) = row;
    principal.require_read(&org_id)?;

    let org_name = sqlx::query_scalar::<_, String>("SELECT name FROM organizations WHERE id = $1")
        .bind(org_id)
        .fetch_one(pool)
        .await
        .map_err(super::internal)?;

    let creds = sqlx::query_as::<_, (Uuid, String, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, username, created_at FROM basic_auth_credentials WHERE list_id = $1 ORDER BY username",
    )
    .bind(input.id)
    .fetch_all(pool)
    .await
    .map_err(super::internal)?;

    Ok(BasicAuthInfo {
        id,
        name,
        organization_name: org_name,
        credentials: creds
            .into_iter()
            .map(|(id, username, created_at)| BasicAuthCredentialRow {
                id,
                username,
                created_at: created_at.format("%Y-%m-%d %H:%M").to_string(),
            })
            .collect(),
    })
}

/// Add (or replace) a username/password credential on a basic-auth list
/// (requires org write). The password is stored as an Argon2id hash and the
/// proxy is told to reload.
#[api_mcp_dioxus_server(server = "add_basic_auth_credential")]
pub async fn basic_auth_add_credential(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: BasicAuthAddCredentialInput,
) -> Result<(), ApiError> {
    let org_id = super::owning_org(pool, "basic_auth_lists", input.id, "basic-auth list").await?;
    principal.require_write(&org_id)?;

    let hash = crate::api::basic_auth::hash_password(&input.password).map_err(super::internal)?;

    sqlx::query(
        "INSERT INTO basic_auth_credentials (list_id, username, password_hash) VALUES ($1, $2, $3) \
         ON CONFLICT (list_id, username) DO UPDATE SET password_hash = $3",
    )
    .bind(input.id)
    .bind(&input.username)
    .bind(&hash)
    .execute(pool)
    .await
    .map_err(super::internal)?;

    // Credential set changed — tell the proxy to reload its basic-auth lists.
    crate::api::internal::notify_proxy_reload();
    Ok(())
}

/// Remove a credential from a basic-auth list (requires org write). The
/// credential must belong to the given list; the proxy is told to reload.
#[api_mcp_dioxus_server(server = "remove_basic_auth_credential")]
pub async fn basic_auth_remove_credential(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: BasicAuthRemoveCredentialInput,
) -> Result<(), ApiError> {
    let row = sqlx::query_as::<_, (Uuid, Uuid)>(
        "SELECT c.list_id, b.organization_id FROM basic_auth_credentials c \
         JOIN basic_auth_lists b ON b.id = c.list_id WHERE c.id = $1",
    )
    .bind(input.credential_id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?
    .ok_or_else(|| ApiError::not_found("credential not found"))?;

    let (list_id, org_id) = row;
    if list_id != input.id {
        return Err(ApiError::not_found("credential not found in this list"));
    }
    principal.require_write(&org_id)?;

    sqlx::query("DELETE FROM basic_auth_credentials WHERE id = $1")
        .bind(input.credential_id)
        .execute(pool)
        .await
        .map_err(super::internal)?;

    // Credential set changed — tell the proxy to reload its basic-auth lists.
    crate::api::internal::notify_proxy_reload();
    Ok(())
}
