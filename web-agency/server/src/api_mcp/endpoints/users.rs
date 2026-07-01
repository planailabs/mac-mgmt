//! User endpoints.

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
pub struct UserRow {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub is_admin: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UserListInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UserOrgEntry {
    pub organization_id: String,
    pub name: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UserInfo {
    pub id: String,
    pub email: String,
    pub name: String,
    pub is_admin: bool,
    pub created_at: String,
    pub organizations: Vec<UserOrgEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UserGetInput {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UserCreateInput {
    pub email: String,
    pub name: String,
    pub is_admin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UserUpdateInput {
    pub id: String,
    pub is_admin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UserDeleteInput {
    pub id: String,
}

/// List all users (admin only).
#[api_mcp_dioxus_server(server = "list_users")]
pub async fn user_list(
    pool: &sqlx::PgPool,
    principal: &Principal,
    _input: UserListInput,
) -> Result<Vec<UserRow>, ApiError> {
    principal.require_admin()?;
    let rows = sqlx::query_as::<_, (Uuid, String, String, bool, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, email, name, is_admin, created_at FROM users ORDER BY email",
    )
    .fetch_all(pool)
    .await
    .map_err(super::internal)?;

    Ok(rows
        .into_iter()
        .map(|(id, email, name, is_admin, created_at)| UserRow {
            id,
            email,
            name,
            is_admin,
            created_at: created_at.format("%Y-%m-%d %H:%M").to_string(),
        })
        .collect())
}

/// Get a single user by id (admin only).
#[api_mcp_dioxus_server(server = "get_user")]
pub async fn user_get(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: UserGetInput,
) -> Result<UserInfo, ApiError> {
    principal.require_admin()?;
    let uid: Uuid = input
        .id
        .parse()
        .map_err(|_| ApiError::bad_request("invalid user id"))?;

    let row = sqlx::query_as::<_, (Uuid, String, String, bool, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, email, name, is_admin, created_at FROM users WHERE id = $1",
    )
    .bind(uid)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?
    .ok_or_else(|| ApiError::not_found("user not found"))?;

    let orgs = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT om.organization_id, o.name, om.role \
         FROM organization_members om \
         JOIN organizations o ON o.id = om.organization_id \
         WHERE om.user_id = $1 \
         ORDER BY o.name",
    )
    .bind(uid)
    .fetch_all(pool)
    .await
    .map_err(super::internal)?;

    Ok(UserInfo {
        id: row.0.to_string(),
        email: row.1,
        name: row.2,
        is_admin: row.3,
        created_at: row.4.format("%Y-%m-%d %H:%M").to_string(),
        organizations: orgs
            .into_iter()
            .map(|(organization_id, name, role)| UserOrgEntry {
                organization_id: organization_id.to_string(),
                name,
                role,
            })
            .collect(),
    })
}

/// Create a user (admin only). Returns the new user's id.
#[api_mcp_dioxus_server(server = "create_user")]
pub async fn user_create(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: UserCreateInput,
) -> Result<Uuid, ApiError> {
    principal.require_admin()?;
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO users (email, name, is_admin) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(&input.email)
    .bind(&input.name)
    .bind(input.is_admin)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::internal(format!("failed to create user: {e}")))
}

/// Set a user's admin flag (admin only). Refuses to remove the calling
/// admin's own admin status (matched by email, like user_delete).
#[api_mcp_dioxus_server(server = "set_user_admin")]
pub async fn user_update(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: UserUpdateInput,
) -> Result<(), ApiError> {
    principal.require_admin()?;
    let uid: Uuid = input
        .id
        .parse()
        .map_err(|_| ApiError::bad_request("invalid user id"))?;

    let email = sqlx::query_scalar::<_, String>("SELECT email FROM users WHERE id = $1")
        .bind(uid)
        .fetch_optional(pool)
        .await
        .map_err(super::internal)?
        .ok_or_else(|| ApiError::not_found("user not found"))?;

    if email == principal.subject && !input.is_admin {
        return Err(ApiError::forbidden("cannot remove your own admin status"));
    }

    sqlx::query("UPDATE users SET is_admin = $2 WHERE id = $1")
        .bind(uid)
        .bind(input.is_admin)
        .execute(pool)
        .await
        .map_err(super::internal)?;
    Ok(())
}

/// Delete a user (admin only). Refuses to delete the calling admin (matched by
/// email); token-admins have no "self" so this guard only applies to web users.
#[api_mcp_dioxus_server(server = "delete_user")]
pub async fn user_delete(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: UserDeleteInput,
) -> Result<(), ApiError> {
    principal.require_admin()?;
    let uid: Uuid = input
        .id
        .parse()
        .map_err(|_| ApiError::bad_request("invalid user id"))?;

    let email = sqlx::query_scalar::<_, String>("SELECT email FROM users WHERE id = $1")
        .bind(uid)
        .fetch_optional(pool)
        .await
        .map_err(super::internal)?
        .ok_or_else(|| ApiError::not_found("user not found"))?;

    if email == principal.subject {
        return Err(ApiError::forbidden("cannot delete yourself"));
    }

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(uid)
        .execute(pool)
        .await
        .map_err(super::internal)?;
    Ok(())
}
