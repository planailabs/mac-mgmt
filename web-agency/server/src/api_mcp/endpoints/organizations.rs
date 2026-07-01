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

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgMemberRow {
    pub user_id: Uuid,
    pub email: String,
    pub name: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgInfo {
    pub id: Uuid,
    pub name: String,
    pub show_billing: bool,
    pub default_changedetection_credential_id: Option<Uuid>,
    pub members: Vec<OrgMemberRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgGetInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgUpdateInput {
    pub id: Uuid,
    pub show_billing: Option<bool>,
    pub default_changedetection_credential_id: Option<Uuid>,
    pub clear_default_changedetection: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgMemberAddInput {
    pub organization_id: Uuid,
    pub user_id: Option<Uuid>,
    pub email: Option<String>,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgMemberRemoveInput {
    pub organization_id: Uuid,
    pub user_id: Uuid,
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
    .map_err(super::internal)?;

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

/// Get an organization by id, including its members (admin only).
#[api_mcp_dioxus_server(server = "get_org")]
pub async fn organization_get(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: OrgGetInput,
) -> Result<OrgInfo, ApiError> {
    principal.require_admin()?;

    let (name, show_billing, default_changedetection_credential_id) =
        sqlx::query_as::<_, (String, bool, Option<Uuid>)>(
            "SELECT name, show_billing, default_changedetection_credential_id \
             FROM organizations WHERE id = $1",
        )
        .bind(input.id)
        .fetch_optional(pool)
        .await
        .map_err(super::internal)?
        .ok_or_else(|| ApiError::not_found("organization not found"))?;

    let members = sqlx::query_as::<_, (Uuid, String, String, String)>(
        "SELECT u.id, u.email, u.name, om.role \
         FROM organization_members om JOIN users u ON u.id = om.user_id \
         WHERE om.organization_id = $1 ORDER BY u.email",
    )
    .bind(input.id)
    .fetch_all(pool)
    .await
    .map_err(super::internal)?
    .into_iter()
    .map(|(user_id, email, name, role)| OrgMemberRow {
        user_id,
        email,
        name,
        role,
    })
    .collect();

    Ok(OrgInfo {
        id: input.id,
        name,
        show_billing,
        default_changedetection_credential_id,
        members,
    })
}

/// Update an organization (admin only). Only the provided fields are applied;
/// `clear_default_changedetection` sets the default credential to NULL.
#[api_mcp_dioxus_server(server = "update_org")]
pub async fn organization_update(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: OrgUpdateInput,
) -> Result<(), ApiError> {
    principal.require_admin()?;

    if let Some(show_billing) = input.show_billing {
        sqlx::query("UPDATE organizations SET show_billing = $2 WHERE id = $1")
            .bind(input.id)
            .bind(show_billing)
            .execute(pool)
            .await
            .map_err(super::internal)?;
    }

    if input.clear_default_changedetection.unwrap_or(false) {
        sqlx::query(
            "UPDATE organizations SET default_changedetection_credential_id = NULL WHERE id = $1",
        )
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(super::internal)?;
    } else if let Some(credential_id) = input.default_changedetection_credential_id {
        sqlx::query(
            "UPDATE organizations SET default_changedetection_credential_id = $2 WHERE id = $1",
        )
        .bind(input.id)
        .bind(credential_id)
        .execute(pool)
        .await
        .map_err(super::internal)?;
    }

    Ok(())
}

/// Add (or re-role) a member in an organization (admin only). Exactly one of
/// `user_id` / `email` selects the user.
#[api_mcp_dioxus_server(server = "add_org_member")]
pub async fn organization_member_add(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: OrgMemberAddInput,
) -> Result<(), ApiError> {
    principal.require_admin()?;

    if !["admin", "write", "read"].contains(&input.role.as_str()) {
        return Err(ApiError::bad_request("invalid role"));
    }

    let user_id = match (input.user_id, input.email) {
        (Some(user_id), None) => user_id,
        (None, Some(email)) => sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE email = $1")
            .bind(&email)
            .fetch_optional(pool)
            .await
            .map_err(super::internal)?
            .ok_or_else(|| ApiError::not_found(format!("user {email} not found")))?,
        _ => {
            return Err(ApiError::bad_request(
                "exactly one of user_id or email must be set",
            ));
        }
    };

    sqlx::query(
        "INSERT INTO organization_members (organization_id, user_id, role) VALUES ($1, $2, $3) \
         ON CONFLICT (organization_id, user_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(input.organization_id)
    .bind(user_id)
    .bind(&input.role)
    .execute(pool)
    .await
    .map_err(super::internal)?;

    Ok(())
}

/// Remove a member from an organization (admin only).
#[api_mcp_dioxus_server(server = "remove_org_member")]
pub async fn organization_member_remove(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: OrgMemberRemoveInput,
) -> Result<(), ApiError> {
    principal.require_admin()?;

    sqlx::query("DELETE FROM organization_members WHERE organization_id = $1 AND user_id = $2")
        .bind(input.organization_id)
        .bind(input.user_id)
        .execute(pool)
        .await
        .map_err(super::internal)?;

    Ok(())
}
