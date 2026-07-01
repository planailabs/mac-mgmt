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
pub struct UserInfo {
    pub id: String,
    pub email: String,
    pub name: String,
    pub is_admin: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UserGetInput {
    pub id: String,
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
    .map_err(|e| ApiError::internal(e.to_string()))?;

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
    .map_err(|e| ApiError::internal(e.to_string()))?
    .ok_or_else(|| ApiError::not_found("user not found"))?;

    Ok(UserInfo {
        id: row.0.to_string(),
        email: row.1,
        name: row.2,
        is_admin: row.3,
        created_at: row.4.format("%Y-%m-%d %H:%M").to_string(),
    })
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
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("user not found"))?;

    if email == principal.subject {
        return Err(ApiError::forbidden("cannot delete yourself"));
    }

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(uid)
        .execute(pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(())
}
