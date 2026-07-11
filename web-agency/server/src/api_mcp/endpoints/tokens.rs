//! API-token endpoints: one unified `tokens` resource covering the admin token
//! manager (admin/api/metrics/deploy kinds) and webspace-scoped deploy tokens.

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
pub struct TokenInfo {
    pub id: Uuid,
    pub label: String,
    pub kind: String,
    pub revoked: bool,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub expired: bool,
    /// Resolved scope: the webspace a deploy token is bound to, else the org.
    pub scope: Option<String>,
    /// Link to the scoped resource's page.
    pub scope_href: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TokenListInput {
    /// When set, list only the deploy tokens scoped to this webspace (requires
    /// org admin of the webspace's org). Otherwise list all tokens (admin only).
    #[serde(default)]
    pub webspace_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TokenCreateInput {
    /// "api", "admin", "metrics" or "deploy".
    pub kind: String,
    pub label: String,
    /// Org scope for api/metrics tokens. For deploy tokens the org is derived
    /// from the webspace; when given it must match.
    #[serde(default)]
    pub organization_id: Option<Uuid>,
    /// For deploy tokens: the webspace the token is bound to.
    #[serde(default)]
    pub webspace_id: Option<Uuid>,
    /// For org-scoped api tokens: "read" or "write".
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub expires_in_secs: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TokenCreateResult {
    /// The plaintext token — shown only once; the DB stores its SHA-256 hash.
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TokenRevokeInput {
    pub id: String,
}

/// Require org-admin role. The web session exposed org-admin directly; a
/// `Principal` doesn't, so look the caller's org role up by subject (email) —
/// same pattern as `webspaces::webspace_get`. Token principals have no users
/// row and pass only when globally admin.
#[cfg(feature = "server")]
async fn require_org_admin(
    pool: &sqlx::PgPool,
    principal: &Principal,
    org_id: &Uuid,
) -> Result<(), ApiError> {
    if principal.admin {
        return Ok(());
    }
    let is_org_admin = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM organization_members om \
         JOIN users u ON u.id = om.user_id \
         WHERE u.email = $1 AND om.organization_id = $2 AND om.role = 'admin')",
    )
    .bind(&principal.subject)
    .bind(org_id)
    .fetch_one(pool)
    .await
    .map_err(super::internal)?;
    if is_org_admin {
        Ok(())
    } else {
        Err(ApiError::forbidden("organization admin required"))
    }
}

/// List tokens. With `webspace_id`: that webspace's deploy tokens (org admin
/// of the webspace's org). Without: all tokens, newest first, capped at 100
/// (admin only).
#[api_mcp_dioxus_server(server = "list_tokens")]
pub async fn token_list(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: TokenListInput,
) -> Result<Vec<TokenInfo>, ApiError> {
    let now = chrono::Utc::now();

    if let Some(wid) = input.webspace_id {
        let org_id = super::owning_org(pool, "webspaces", wid, "webspace").await?;
        require_org_admin(pool, principal, &org_id).await?;

        let rows = sqlx::query_as::<
            _,
            (
                Uuid,
                String,
                bool,
                chrono::DateTime<chrono::Utc>,
                Option<chrono::DateTime<chrono::Utc>>,
            ),
        >(
            "SELECT id, label, revoked, created_at, expires_at FROM tokens \
             WHERE kind = 'deploy' AND organization_id = $1 \
               AND (scopes->>'webspace_id')::uuid = $2 \
             ORDER BY created_at DESC",
        )
        .bind(org_id)
        .bind(wid)
        .fetch_all(pool)
        .await
        .map_err(super::internal)?;

        return Ok(rows
            .into_iter()
            .map(|(id, label, revoked, created_at, expires_at)| TokenInfo {
                id,
                label,
                kind: "deploy".to_string(),
                revoked,
                created_at: created_at.format("%Y-%m-%d %H:%M").to_string(),
                expires_at: expires_at.map(|d| d.format("%Y-%m-%d %H:%M").to_string()),
                expired: expires_at.is_some_and(|e| e < now),
                scope: None,
                scope_href: None,
            })
            .collect());
    }

    principal.require_admin()?;

    let rows = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            String,
            bool,
            chrono::DateTime<chrono::Utc>,
            Option<chrono::DateTime<chrono::Utc>>,
            Option<Uuid>,
            Option<String>,
            Option<Uuid>,
            Option<String>,
        ),
    >(
        "SELECT t.id, t.label, t.kind, t.revoked, t.created_at, t.expires_at, \
                w.id AS ws_id, w.name AS ws_name, t.organization_id AS org_id, o.name AS org_name \
         FROM tokens t \
         LEFT JOIN webspaces w ON w.id = (t.scopes->>'webspace_id')::uuid \
         LEFT JOIN organizations o ON o.id = t.organization_id \
         ORDER BY t.created_at DESC LIMIT 100",
    )
    .fetch_all(pool)
    .await
    .map_err(super::internal)?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                label,
                kind,
                revoked,
                created_at,
                expires_at,
                ws_id,
                ws_name,
                org_id,
                org_name,
            )| {
                TokenInfo {
                    id,
                    label,
                    kind,
                    revoked,
                    created_at: created_at.format("%Y-%m-%d %H:%M").to_string(),
                    expires_at: expires_at.map(|d| d.format("%Y-%m-%d %H:%M").to_string()),
                    expired: expires_at.is_some_and(|e| e < now),
                    scope: ws_name
                        .map(|n| format!("webspace: {n}"))
                        .or_else(|| org_name.map(|n| format!("org: {n}"))),
                    scope_href: ws_id
                        .map(|id| format!("/webspaces/{id}"))
                        .or_else(|| org_id.map(|id| format!("/organizations/{id}"))),
                }
            },
        )
        .collect())
}

/// Create a token; returns the plaintext secret (shown only once).
/// kind "deploy" + `webspace_id` → webspace-bound deploy token, requires org
/// admin of the webspace's org. All other kinds (and unscoped deploy) require
/// global admin; org-scoped "api" tokens take a "read"/"write" role scope.
#[api_mcp_dioxus_server(server = "create_token")]
pub async fn token_create(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: TokenCreateInput,
) -> Result<TokenCreateResult, ApiError> {
    use rand::Rng;
    use sha2::{Digest, Sha256};

    let token_bytes: [u8; 32] = rand::rng().random();
    let token = hex::encode(token_bytes);
    let hash = hex::encode(Sha256::digest(token.as_bytes()));
    let expires_at = input
        .expires_in_secs
        .map(|s| chrono::Utc::now() + chrono::Duration::seconds(s));

    // Webspace-bound deploy token: org admin of the webspace's org suffices.
    if input.kind == "deploy" {
        if let Some(wid) = input.webspace_id {
            let ws_org = super::owning_org(pool, "webspaces", wid, "webspace").await?;
            if input.organization_id.is_some_and(|oid| oid != ws_org) {
                return Err(ApiError::bad_request(
                    "webspace does not belong to this organization",
                ));
            }
            require_org_admin(pool, principal, &ws_org).await?;

            let scopes = serde_json::json!({ "webspace_id": wid.to_string() });
            sqlx::query(
                "INSERT INTO tokens (organization_id, token_hash, label, kind, scopes, expires_at) \
                 VALUES ($1, $2, $3, 'deploy', $4, $5)",
            )
            .bind(ws_org)
            .bind(&hash)
            .bind(&input.label)
            .bind(&scopes)
            .bind(expires_at)
            .execute(pool)
            .await
            .map_err(super::internal)?;

            return Ok(TokenCreateResult { token });
        }
    }

    // Everything else (admin/api/metrics, or a deploy token valid for all
    // webspaces) is global-admin only.
    principal.require_admin()?;

    // Org-scoped api → role scope (read/write); other kinds carry no scopes here.
    let scopes = match input.kind.as_str() {
        "api" => input
            .role
            .as_deref()
            .filter(|r| !r.is_empty())
            .map(|r| serde_json::json!({ "role": r })),
        _ => None,
    };

    sqlx::query(
        "INSERT INTO tokens (organization_id, token_hash, label, kind, scopes, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(input.organization_id)
    .bind(&hash)
    .bind(&input.label)
    .bind(&input.kind)
    .bind(&scopes)
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(super::internal)?;

    Ok(TokenCreateResult { token })
}

/// Revoke a token (sets `revoked`, keeping the row for the audit trail).
/// Webspace-bound deploy tokens: org admin of the owning org; all others:
/// global admin.
#[api_mcp_dioxus_server(server = "revoke_token")]
pub async fn token_revoke(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: TokenRevokeInput,
) -> Result<(), ApiError> {
    let id: Uuid = input
        .id
        .parse()
        .map_err(|_| ApiError::bad_request("invalid token id"))?;

    let (kind, org_id, scopes) =
        sqlx::query_as::<_, (String, Option<Uuid>, Option<serde_json::Value>)>(
            "SELECT kind, organization_id, scopes FROM tokens WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(super::internal)?
        .ok_or_else(|| ApiError::not_found("token not found"))?;

    let webspace_bound = kind == "deploy"
        && scopes
            .as_ref()
            .and_then(|s| s.get("webspace_id"))
            .and_then(|v| v.as_str())
            .is_some();

    match (webspace_bound, org_id) {
        (true, Some(oid)) => require_org_admin(pool, principal, &oid).await?,
        _ => principal.require_admin()?,
    }

    sqlx::query("UPDATE tokens SET revoked = true WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(super::internal)?;
    Ok(())
}
