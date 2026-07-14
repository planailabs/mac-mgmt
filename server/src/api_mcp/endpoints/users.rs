//! User & admin endpoints: list/create/get/delete users, the global-admin
//! flag toggle, and user↔organization membership management. All admin-only
//! (these are the /users admin pages).

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

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UserListInput {}

/// A user with their organization names, as shown in the user list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UserRow {
    pub id: String,
    pub email: String,
    pub name: String,
    pub is_admin: bool,
    pub org_names: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UserGetInput {
    pub id: Uuid,
}

/// Basic user info.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UserInfo {
    pub id: String,
    pub email: String,
    pub name: String,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UserCreateInput {
    pub email: String,
    pub name: String,
    /// Whether the new user is a global admin.
    pub is_admin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UserDeleteInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UserSetAdminInput {
    pub id: Uuid,
    /// New global-admin status for the user.
    pub is_admin: bool,
}

// ── Handlers ────────────────────────────────────────────────────────────

/// was: list_users() in web/components/user_list.rs
#[api_mcp_dioxus_server(server = "list_users")]
pub async fn user_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: UserListInput,
) -> Result<Vec<UserRow>, ApiError> {
    p.require_admin()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        email: String,
        name: String,
        is_admin: bool,
        created_at: DateTime<Utc>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, email, name, is_admin, created_at FROM users ORDER BY email",
    )
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    let mut users = Vec::with_capacity(rows.len());
    for r in rows {
        let org_names: Vec<String> = sqlx::query_scalar(
            "SELECT o.name FROM organizations o \
             JOIN organization_members om ON om.organization_id = o.id \
             WHERE om.user_id = $1 \
             ORDER BY o.name",
        )
        .bind(r.id)
        .fetch_all(pool)
        .await
        .map_err(internal)?;

        users.push(UserRow {
            id: r.id.to_string(),
            email: r.email,
            name: r.name,
            is_admin: r.is_admin,
            org_names,
            created_at: r.created_at,
        });
    }

    Ok(users)
}

/// was: get_user() in web/components/user_detail.rs
#[api_mcp_dioxus_server(server = "get_user")]
pub async fn user_get(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: UserGetInput,
) -> Result<UserInfo, ApiError> {
    p.require_admin()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        email: String,
        name: String,
        is_admin: bool,
        created_at: DateTime<Utc>,
    }

    let row = sqlx::query_as::<_, Row>(
        "SELECT id, email, name, is_admin, created_at FROM users WHERE id = $1",
    )
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or_else(|| ApiError::not_found("user not found"))?;

    Ok(UserInfo {
        id: row.id.to_string(),
        email: row.email,
        name: row.name,
        is_admin: row.is_admin,
        created_at: row.created_at,
    })
}

/// was: create_user() in web/components/user_form.rs
#[api_mcp_dioxus_server(server = "create_user")]
pub async fn user_create(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: UserCreateInput,
) -> Result<String, ApiError> {
    p.require_admin()?;

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, name, is_admin) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(&input.email)
    .bind(&input.name)
    .bind(input.is_admin)
    .fetch_one(pool)
    .await
    .map_err(internal)?;

    Ok(id.to_string())
}

/// Email of the target user, for "acting on yourself" guards (the web
/// principal's subject is the email; token principals have no self).
#[cfg(feature = "server")]
async fn user_email(pool: &sqlx::PgPool, id: Uuid) -> Result<String, ApiError> {
    sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(internal)?
        .ok_or_else(|| ApiError::not_found("user not found"))
}

/// was: delete_user() in web/components/user_detail.rs
#[api_mcp_dioxus_server(server = "delete_user")]
pub async fn user_delete(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: UserDeleteInput,
) -> Result<(), ApiError> {
    p.require_admin()?;

    if user_email(pool, input.id).await? == p.subject {
        return Err(ApiError::forbidden("cannot delete yourself"));
    }

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

/// was: toggle_user_admin() in web/components/user_list.rs (and user_detail.rs dupe)
#[api_mcp_dioxus_server(server = "toggle_user_admin")]
pub async fn user_set_admin(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: UserSetAdminInput,
) -> Result<(), ApiError> {
    p.require_admin()?;

    if !input.is_admin && user_email(pool, input.id).await? == p.subject {
        return Err(ApiError::forbidden("cannot remove your own admin status"));
    }

    sqlx::query("UPDATE users SET is_admin = $2 WHERE id = $1")
        .bind(input.id)
        .bind(input.is_admin)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

// ── Organization memberships ────────────────────────────────────────────

/// An organization the user belongs to, with their role.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UserOrgEntry {
    pub organization_id: String,
    pub name: String,
    pub role: String,
}

/// An organization the user could be added to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgOption {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UserOrgsInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UserAvailableOrgsInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UserOrgAddInput {
    pub id: Uuid,
    pub org_id: Uuid,
    /// Membership role: "admin", "write" or "read".
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UserOrgRemoveInput {
    pub id: Uuid,
    pub org_id: Uuid,
}

/// was: get_user_orgs() in web/components/user_detail.rs
#[api_mcp_dioxus_server(server = "get_user_orgs")]
pub async fn user_orgs(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: UserOrgsInput,
) -> Result<Vec<UserOrgEntry>, ApiError> {
    p.require_admin()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        organization_id: Uuid,
        name: String,
        role: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT om.organization_id, o.name, om.role \
         FROM organization_members om \
         JOIN organizations o ON o.id = om.organization_id \
         WHERE om.user_id = $1 \
         ORDER BY o.name",
    )
    .bind(input.id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    Ok(rows
        .into_iter()
        .map(|r| UserOrgEntry {
            organization_id: r.organization_id.to_string(),
            name: r.name,
            role: r.role,
        })
        .collect())
}

/// was: get_available_orgs_for_user() in web/components/user_detail.rs
#[api_mcp_dioxus_server(server = "get_available_orgs_for_user")]
pub async fn user_available_orgs(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: UserAvailableOrgsInput,
) -> Result<Vec<OrgOption>, ApiError> {
    p.require_admin()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        name: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, name FROM organizations \
         WHERE id NOT IN (SELECT organization_id FROM organization_members WHERE user_id = $1) \
         ORDER BY name",
    )
    .bind(input.id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    Ok(rows
        .into_iter()
        .map(|r| OrgOption {
            id: r.id.to_string(),
            name: r.name,
        })
        .collect())
}

/// was: add_user_to_org() in web/components/user_detail.rs
#[api_mcp_dioxus_server(server = "add_user_to_org")]
pub async fn user_org_add(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: UserOrgAddInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    if !["admin", "write", "read"].contains(&input.role.as_str()) {
        return Err(ApiError::bad_request("invalid role"));
    }

    sqlx::query(
        "INSERT INTO organization_members (user_id, organization_id, role) \
         VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(input.id)
    .bind(input.org_id)
    .bind(&input.role)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

/// was: remove_user_from_org() in web/components/user_detail.rs
#[api_mcp_dioxus_server(server = "remove_user_from_org")]
pub async fn user_org_remove(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: UserOrgRemoveInput,
) -> Result<(), ApiError> {
    p.require_admin()?;

    sqlx::query("DELETE FROM organization_members WHERE user_id = $1 AND organization_id = $2")
        .bind(input.id)
        .bind(input.org_id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}
