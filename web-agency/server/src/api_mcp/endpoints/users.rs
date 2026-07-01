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
