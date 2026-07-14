//! Organization endpoints: org CRUD, member management, org↔cluster links,
//! picker queries (available users/clusters), permission probes, and
//! org-scoped API tokens.

use chrono::{DateTime, Utc};
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
pub struct OrgListInput {}

/// An organization with member/cluster counts, as shown in the org list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgRow {
    pub id: String,
    pub name: String,
    pub member_count: i64,
    pub cluster_count: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgGetInput {
    pub id: Uuid,
}

/// Basic organization info.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgInfo {
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgCreateInput {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgUpdateInput {
    pub id: Uuid,
    /// New display name for the organization.
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgDeleteInput {
    pub id: Uuid,
}

// ── CRUD handlers ───────────────────────────────────────────────────────

/// was: list_organizations() in web/components/organization_list.rs
#[api_mcp_dioxus_server(server = "list_organizations")]
pub async fn organization_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: OrgListInput,
) -> Result<Vec<OrgRow>, ApiError> {
    p.require_admin()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        name: String,
        member_count: i64,
        cluster_count: i64,
        created_at: DateTime<Utc>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT o.id, o.name, \
         (SELECT COUNT(*) FROM organization_members om WHERE om.organization_id = o.id) AS member_count, \
         (SELECT COUNT(*) FROM organization_clusters oc WHERE oc.organization_id = o.id) AS cluster_count, \
         o.created_at \
         FROM organizations o ORDER BY o.name",
    )
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    Ok(rows
        .into_iter()
        .map(|r| OrgRow {
            id: r.id.to_string(),
            name: r.name,
            member_count: r.member_count,
            cluster_count: r.cluster_count,
            created_at: r.created_at,
        })
        .collect())
}

/// was: get_organization() in web/components/organization_detail.rs
#[api_mcp_dioxus_server(server = "get_organization")]
pub async fn organization_get(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: OrgGetInput,
) -> Result<OrgInfo, ApiError> {
    p.require_read(&input.id)?;

    #[derive(sqlx::FromRow)]
    struct Row {
        name: String,
        created_at: DateTime<Utc>,
    }

    let row = sqlx::query_as::<_, Row>("SELECT name, created_at FROM organizations WHERE id = $1")
        .bind(input.id)
        .fetch_optional(pool)
        .await
        .map_err(internal)?
        .ok_or_else(|| ApiError::not_found("organization not found"))?;

    Ok(OrgInfo {
        name: row.name,
        created_at: row.created_at,
    })
}

/// was: create_organization() in web/components/organization_form.rs
#[api_mcp_dioxus_server(server = "create_organization")]
pub async fn organization_create(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: OrgCreateInput,
) -> Result<String, ApiError> {
    p.require_admin()?;

    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::bad_request("Name is required"));
    }

    let id: Uuid = sqlx::query_scalar("INSERT INTO organizations (name) VALUES ($1) RETURNING id")
        .bind(&name)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            // Postgres unique_violation code is 23505; surface a friendly
            // message instead of the raw constraint error.
            if let sqlx::Error::Database(db_err) = &e {
                if db_err.code().as_deref() == Some("23505") {
                    return ApiError::conflict(format!(
                        "An organization named '{name}' already exists"
                    ));
                }
            }
            internal(e)
        })?;

    Ok(id.to_string())
}

/// was: rename_organization() in web/components/organization_detail.rs
#[api_mcp_dioxus_server(server = "rename_organization")]
pub async fn organization_update(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: OrgUpdateInput,
) -> Result<(), ApiError> {
    access::require_org_admin(p, &input.id)?;
    sqlx::query("UPDATE organizations SET name = $1 WHERE id = $2")
        .bind(&input.name)
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(|e| {
            if e.to_string().contains("23505") {
                ApiError::conflict("An organization with that name already exists")
            } else {
                internal(e)
            }
        })?;
    Ok(())
}

/// was: delete_organization() in web/components/organization_detail.rs
#[api_mcp_dioxus_server(server = "delete_organization")]
pub async fn organization_delete(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: OrgDeleteInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query("DELETE FROM organizations WHERE id = $1")
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

// ── Permission probe ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgPermissionsInput {
    pub id: Uuid,
}

/// What the caller can do on this org.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgPermissions {
    /// mac-mgmt global admin
    pub is_global_admin: bool,
    /// org-level admin (can manage members, roles, tokens)
    pub is_org_admin: bool,
}

/// was: get_org_permissions() in web/components/organization_detail.rs
#[api_mcp_dioxus_server(server = "get_org_permissions")]
pub async fn organization_permissions(
    _pool: &sqlx::PgPool,
    p: &Principal,
    input: OrgPermissionsInput,
) -> Result<OrgPermissions, ApiError> {
    Ok(OrgPermissions {
        is_global_admin: p.admin,
        is_org_admin: access::is_org_admin(p, &input.id),
    })
}

// ── Members ─────────────────────────────────────────────────────────────

/// A user's membership in the organization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MemberEntry {
    pub user_id: String,
    pub email: String,
    pub name: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgMembersInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgMemberAddInput {
    pub id: Uuid,
    pub user_id: Uuid,
    /// Membership role: "admin", "write" or "read".
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgMemberRemoveInput {
    pub id: Uuid,
    pub user_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgMemberSetRoleInput {
    pub id: Uuid,
    pub user_id: Uuid,
    /// New membership role: "admin", "write" or "read".
    pub role: String,
}

#[cfg(feature = "server")]
fn validate_role(role: &str) -> Result<(), ApiError> {
    if ["admin", "write", "read"].contains(&role) {
        Ok(())
    } else {
        Err(ApiError::bad_request("invalid role"))
    }
}

/// was: get_org_members() in web/components/organization_detail.rs
#[api_mcp_dioxus_server(server = "get_org_members")]
pub async fn organization_members(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: OrgMembersInput,
) -> Result<Vec<MemberEntry>, ApiError> {
    p.require_read(&input.id)?;

    #[derive(sqlx::FromRow)]
    struct Row {
        user_id: Uuid,
        email: String,
        name: String,
        role: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT u.id AS user_id, u.email, u.name, om.role \
         FROM users u \
         JOIN organization_members om ON om.user_id = u.id \
         WHERE om.organization_id = $1 \
         ORDER BY u.email",
    )
    .bind(input.id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    Ok(rows
        .into_iter()
        .map(|r| MemberEntry {
            user_id: r.user_id.to_string(),
            email: r.email,
            name: r.name,
            role: r.role,
        })
        .collect())
}

/// was: add_org_member() in web/components/organization_detail.rs
#[api_mcp_dioxus_server(server = "add_org_member")]
pub async fn organization_member_add(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: OrgMemberAddInput,
) -> Result<(), ApiError> {
    access::require_org_admin(p, &input.id)?;
    validate_role(&input.role)?;

    sqlx::query(
        "INSERT INTO organization_members (organization_id, user_id, role) VALUES ($1, $2, $3)",
    )
    .bind(input.id)
    .bind(input.user_id)
    .bind(&input.role)
    .execute(pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("duplicate key") {
            ApiError::conflict("user is already a member of this organization")
        } else {
            internal(e)
        }
    })?;
    Ok(())
}

/// was: remove_org_member() in web/components/organization_detail.rs
#[api_mcp_dioxus_server(server = "remove_org_member")]
pub async fn organization_member_remove(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: OrgMemberRemoveInput,
) -> Result<(), ApiError> {
    access::require_org_admin(p, &input.id)?;
    sqlx::query("DELETE FROM organization_members WHERE organization_id = $1 AND user_id = $2")
        .bind(input.id)
        .bind(input.user_id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

/// was: change_member_role() in web/components/organization_detail.rs
#[api_mcp_dioxus_server(server = "change_member_role")]
pub async fn organization_member_set_role(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: OrgMemberSetRoleInput,
) -> Result<(), ApiError> {
    access::require_org_admin(p, &input.id)?;
    validate_role(&input.role)?;

    sqlx::query(
        "UPDATE organization_members SET role = $3 WHERE organization_id = $1 AND user_id = $2",
    )
    .bind(input.id)
    .bind(input.user_id)
    .bind(&input.role)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

// ── Cluster links ───────────────────────────────────────────────────────

/// A cluster owned by the organization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterEntry {
    pub cluster_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgClustersInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgClusterAddInput {
    pub id: Uuid,
    pub cluster_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgClusterRemoveInput {
    pub id: Uuid,
    pub cluster_id: Uuid,
}

/// was: get_org_clusters() in web/components/organization_detail.rs
#[api_mcp_dioxus_server(server = "get_org_clusters")]
pub async fn organization_clusters(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: OrgClustersInput,
) -> Result<Vec<ClusterEntry>, ApiError> {
    p.require_read(&input.id)?;

    #[derive(sqlx::FromRow)]
    struct Row {
        cluster_id: Uuid,
        name: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT c.id AS cluster_id, c.name \
         FROM clusters c \
         JOIN organization_clusters oc ON oc.cluster_id = c.id \
         WHERE oc.organization_id = $1 \
         ORDER BY c.name",
    )
    .bind(input.id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    Ok(rows
        .into_iter()
        .map(|r| ClusterEntry {
            cluster_id: r.cluster_id.to_string(),
            name: r.name,
        })
        .collect())
}

/// was: add_org_cluster() in web/components/organization_detail.rs
#[api_mcp_dioxus_server(server = "add_org_cluster")]
pub async fn organization_cluster_add(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: OrgClusterAddInput,
) -> Result<(), ApiError> {
    // Global admin only: attaching a cluster grants the whole org access to it.
    p.require_admin()?;
    sqlx::query("INSERT INTO organization_clusters (organization_id, cluster_id) VALUES ($1, $2)")
        .bind(input.id)
        .bind(input.cluster_id)
        .execute(pool)
        .await
        .map_err(|e| {
            if e.to_string().contains("duplicate key") {
                ApiError::conflict("cluster is already linked to this organization")
            } else {
                internal(e)
            }
        })?;
    Ok(())
}

/// was: remove_org_cluster() in web/components/organization_detail.rs
#[api_mcp_dioxus_server(server = "remove_org_cluster")]
pub async fn organization_cluster_remove(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: OrgClusterRemoveInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query("DELETE FROM organization_clusters WHERE organization_id = $1 AND cluster_id = $2")
        .bind(input.id)
        .bind(input.cluster_id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

// ── Pickers (available users / clusters) ────────────────────────────────

/// A user that could be added to the organization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UserOption {
    pub id: String,
    pub email: String,
    pub name: String,
}

/// A cluster that could be linked to the organization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterOption {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgAvailableUsersInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgAvailableClustersInput {
    pub id: Uuid,
}

/// was: get_available_users() in web/components/organization_detail.rs
#[api_mcp_dioxus_server(server = "get_available_users")]
pub async fn organization_available_users(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: OrgAvailableUsersInput,
) -> Result<Vec<UserOption>, ApiError> {
    access::require_org_admin(p, &input.id)?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        email: String,
        name: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, email, name FROM users \
         WHERE id NOT IN (SELECT user_id FROM organization_members WHERE organization_id = $1) \
         ORDER BY email",
    )
    .bind(input.id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    Ok(rows
        .into_iter()
        .map(|r| UserOption {
            id: r.id.to_string(),
            email: r.email,
            name: r.name,
        })
        .collect())
}

/// was: get_available_clusters() in web/components/organization_detail.rs
#[api_mcp_dioxus_server(server = "get_available_clusters")]
pub async fn organization_available_clusters(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: OrgAvailableClustersInput,
) -> Result<Vec<ClusterOption>, ApiError> {
    p.require_admin()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        name: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, name FROM clusters \
         WHERE id NOT IN (SELECT cluster_id FROM organization_clusters WHERE organization_id = $1) \
         ORDER BY name",
    )
    .bind(input.id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    Ok(rows
        .into_iter()
        .map(|r| ClusterOption {
            id: r.id.to_string(),
            name: r.name,
        })
        .collect())
}

// ── Tokens ──────────────────────────────────────────────────────────────

/// An org-scoped API token (hash only; the plaintext is shown once at creation).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgTokenRow {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub revoked: bool,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgTokensListInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgTokenCreateInput {
    pub id: Uuid,
    /// Human-readable label for the token.
    pub label: String,
    /// Optional expiry, in seconds from now; omit for a non-expiring token.
    pub expires_in_secs: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgTokenRevokeInput {
    pub id: Uuid,
    pub token_id: Uuid,
}

/// was: list_org_tokens() in web/components/organization_detail.rs
#[api_mcp_dioxus_server(server = "list_org_tokens")]
pub async fn organization_tokens_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: OrgTokensListInput,
) -> Result<Vec<OrgTokenRow>, ApiError> {
    access::require_org_admin(p, &input.id)?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        label: String,
        kind: String,
        revoked: bool,
        created_at: DateTime<Utc>,
        expires_at: Option<DateTime<Utc>>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, label, kind, revoked, created_at, expires_at FROM tokens \
         WHERE organization_id = $1 ORDER BY created_at DESC",
    )
    .bind(input.id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    Ok(rows
        .into_iter()
        .map(|r| OrgTokenRow {
            id: r.id.to_string(),
            label: r.label,
            kind: r.kind,
            revoked: r.revoked,
            created_at: r.created_at,
            expires_at: r.expires_at,
        })
        .collect())
}

/// was: create_org_token() in web/components/organization_detail.rs
#[api_mcp_dioxus_server(server = "create_org_token")]
pub async fn organization_token_create(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: OrgTokenCreateInput,
) -> Result<String, ApiError> {
    use rand::Rng;
    use sha2::Digest;

    access::require_org_admin(p, &input.id)?;

    let raw_token: String = hex::encode(rand::rng().random::<[u8; 32]>());
    let hash = hex::encode(sha2::Sha256::digest(raw_token.as_bytes()));
    let expires_at = input
        .expires_in_secs
        .map(|s| chrono::Utc::now() + chrono::Duration::seconds(s));

    sqlx::query(
        "INSERT INTO tokens (organization_id, token_hash, label, kind, expires_at) VALUES ($1, $2, $3, 'setting', $4)",
    )
    .bind(input.id)
    .bind(hash)
    .bind(&input.label)
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(internal)?;

    Ok(raw_token)
}

/// was: revoke_org_token() in web/components/organization_detail.rs
#[api_mcp_dioxus_server(server = "revoke_org_token")]
pub async fn organization_token_revoke(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: OrgTokenRevokeInput,
) -> Result<(), ApiError> {
    access::require_org_admin(p, &input.id)?;

    sqlx::query("UPDATE tokens SET revoked = true WHERE id = $1 AND organization_id = $2")
        .bind(input.token_id)
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}
