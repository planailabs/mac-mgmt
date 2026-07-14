//! Token endpoints: cluster-scoped sync/setting tokens, global admin and
//! federation tokens, plus the admin-only master list and revoke-any.
//!
//! Token values are stored hashed; every `*_create` handler returns the
//! plaintext token exactly once.

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

/// A token as shown in the token lists (hash only; the plaintext is shown
/// once at creation and never stored).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TokenEntry {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub revoked: bool,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[cfg(feature = "server")]
#[derive(sqlx::FromRow)]
struct TokenDbRow {
    id: Uuid,
    label: String,
    kind: String,
    revoked: bool,
    created_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
}

#[cfg(feature = "server")]
impl From<TokenDbRow> for TokenEntry {
    fn from(r: TokenDbRow) -> Self {
        TokenEntry {
            id: r.id.to_string(),
            label: r.label,
            kind: r.kind,
            revoked: r.revoked,
            created_at: r.created_at,
            expires_at: r.expires_at,
        }
    }
}

#[cfg(feature = "server")]
const TOKEN_COLS: &str = "id, label, kind, revoked, created_at, expires_at";

/// Mint a fresh token: (plaintext, sha256-hex hash).
#[cfg(feature = "server")]
fn mint_token(prefix: Option<&str>) -> (String, String) {
    use rand::Rng;
    use sha2::{Digest, Sha256};
    let raw = hex::encode(rand::rng().random::<[u8; 32]>());
    let raw = match prefix {
        Some(p) => format!("{p}{raw}"),
        None => raw,
    };
    let hash = hex::encode(Sha256::digest(raw.as_bytes()));
    (raw, hash)
}

// ── Cluster sync tokens ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SyncTokensListInput {
    pub cluster_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SyncTokenCreateInput {
    pub cluster_id: Uuid,
    /// Human-readable label for the token (required).
    pub label: String,
    /// Optional expiry, in seconds from now; omit for a non-expiring token.
    pub expires_in_secs: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SyncTokenRevokeInput {
    /// Token id to revoke.
    pub id: Uuid,
}

/// was: list_tokens() in web/components/token_list.rs
#[api_mcp_dioxus_server(server = "list_tokens")]
pub async fn token_sync_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: SyncTokensListInput,
) -> Result<Vec<TokenEntry>, ApiError> {
    access::require_cluster_read(pool, p, input.cluster_id).await?;
    let rows = sqlx::query_as::<_, TokenDbRow>(&format!(
        "SELECT {TOKEN_COLS} FROM tokens \
         WHERE cluster_id = $1 AND kind = 'sync' ORDER BY created_at DESC"
    ))
    .bind(input.cluster_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// was: create_token() in web/components/token_list.rs
#[api_mcp_dioxus_server(server = "create_token")]
pub async fn token_sync_create(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: SyncTokenCreateInput,
) -> Result<String, ApiError> {
    let label = input.label.trim().to_string();
    if label.is_empty() {
        return Err(ApiError::bad_request("label is required"));
    }
    access::require_cluster_write(pool, p, input.cluster_id).await?;

    let (raw_token, hash) = mint_token(None);
    let expires_at = input
        .expires_in_secs
        .map(|s| chrono::Utc::now() + chrono::Duration::seconds(s));

    sqlx::query(
        "INSERT INTO tokens (cluster_id, token_hash, label, kind, expires_at) VALUES ($1, $2, $3, 'sync', $4)",
    )
    .bind(input.cluster_id)
    .bind(&hash)
    .bind(&label)
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(internal)?;

    Ok(raw_token)
}

/// was: revoke_token() in web/components/token_list.rs
#[api_mcp_dioxus_server(server = "revoke_token")]
pub async fn token_sync_revoke(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: SyncTokenRevokeInput,
) -> Result<(), ApiError> {
    revoke_cluster_token(pool, p, input.id).await
}

/// Shared revoke path for cluster-bound (sync/setting) tokens: check write
/// access to the owning cluster, then mark revoked.
#[cfg(feature = "server")]
async fn revoke_cluster_token(
    pool: &sqlx::PgPool,
    p: &Principal,
    token_id: Uuid,
) -> Result<(), ApiError> {
    let owner_cid = sqlx::query_scalar::<_, Uuid>("SELECT cluster_id FROM tokens WHERE id = $1")
        .bind(token_id)
        .fetch_optional(pool)
        .await
        .map_err(internal)?;
    if let Some(owner_cid) = owner_cid {
        if let Some(ids) = access::writable_cluster_ids(pool, p).await? {
            if !ids.contains(&owner_cid) {
                return Err(ApiError::forbidden("access denied"));
            }
        }
    }
    sqlx::query("UPDATE tokens SET revoked = true WHERE id = $1")
        .bind(token_id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

// ── Setting tokens (cluster-scoped) ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SettingTokensListInput {
    pub cluster_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SettingTokenCreateInput {
    pub cluster_id: Uuid,
    /// Human-readable label for the token (required).
    pub label: String,
    /// Optional expiry, in seconds from now; omit for a non-expiring token.
    pub expires_in_secs: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SettingTokenRevokeInput {
    /// Token id to revoke.
    pub id: Uuid,
}

/// was: list_setting_tokens() in web/components/setting_token_list.rs
#[api_mcp_dioxus_server(server = "list_setting_tokens")]
pub async fn token_setting_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: SettingTokensListInput,
) -> Result<Vec<TokenEntry>, ApiError> {
    access::require_cluster_read(pool, p, input.cluster_id).await?;
    let rows = sqlx::query_as::<_, TokenDbRow>(&format!(
        "SELECT {TOKEN_COLS} FROM tokens \
         WHERE cluster_id = $1 AND kind = 'setting' ORDER BY created_at DESC"
    ))
    .bind(input.cluster_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// was: create_setting_token() in web/components/setting_token_list.rs
#[api_mcp_dioxus_server(server = "create_setting_token")]
pub async fn token_setting_create(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: SettingTokenCreateInput,
) -> Result<String, ApiError> {
    let label = input.label.trim().to_string();
    if label.is_empty() {
        return Err(ApiError::bad_request("label is required"));
    }
    access::require_cluster_write(pool, p, input.cluster_id).await?;

    let (raw_token, hash) = mint_token(None);
    let expires_at = input
        .expires_in_secs
        .map(|s| chrono::Utc::now() + chrono::Duration::seconds(s));

    sqlx::query(
        "INSERT INTO tokens (cluster_id, token_hash, label, kind, expires_at) VALUES ($1, $2, $3, 'setting', $4)",
    )
    .bind(input.cluster_id)
    .bind(&hash)
    .bind(&label)
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(internal)?;

    Ok(raw_token)
}

/// was: revoke_setting_token() in web/components/setting_token_list.rs
#[api_mcp_dioxus_server(server = "revoke_setting_token")]
pub async fn token_setting_revoke(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: SettingTokenRevokeInput,
) -> Result<(), ApiError> {
    revoke_cluster_token(pool, p, input.id).await
}

// ── Admin tokens (global) ───────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AdminTokensListInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AdminTokenCreateInput {
    /// Human-readable label for the token (required).
    pub label: String,
    /// Optional expiry, in seconds from now; omit for a non-expiring token.
    pub expires_in_secs: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AdminTokenRevokeInput {
    /// Token id to revoke.
    pub id: Uuid,
}

/// was: list_admin_tokens() in web/components/admin_token_list.rs
#[api_mcp_dioxus_server(server = "list_admin_tokens")]
pub async fn token_admin_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: AdminTokensListInput,
) -> Result<Vec<TokenEntry>, ApiError> {
    p.require_admin()?;
    let rows = sqlx::query_as::<_, TokenDbRow>(&format!(
        "SELECT {TOKEN_COLS} FROM tokens WHERE kind = 'admin' ORDER BY created_at DESC"
    ))
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// was: create_admin_token() in web/components/admin_token_list.rs
#[api_mcp_dioxus_server(server = "create_admin_token")]
pub async fn token_admin_create(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: AdminTokenCreateInput,
) -> Result<String, ApiError> {
    p.require_admin()?;

    let label = input.label.trim().to_string();
    if label.is_empty() {
        return Err(ApiError::bad_request("label is required"));
    }

    let (raw_token, hash) = mint_token(None);
    let expires_at = input
        .expires_in_secs
        .map(|s| chrono::Utc::now() + chrono::Duration::seconds(s));

    sqlx::query(
        "INSERT INTO tokens (cluster_id, token_hash, label, kind, expires_at) VALUES (NULL, $1, $2, 'admin', $3)",
    )
    .bind(&hash)
    .bind(&label)
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(internal)?;

    Ok(raw_token)
}

/// was: revoke_admin_token() in web/components/admin_token_list.rs
#[api_mcp_dioxus_server(server = "revoke_admin_token")]
pub async fn token_admin_revoke(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: AdminTokenRevokeInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query("UPDATE tokens SET revoked = true WHERE id = $1")
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

// ── Federation tokens (global) ──────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FederationTokensListInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FederationTokenCreateInput {
    /// Human-readable label for the token (required).
    pub label: String,
    /// Optional expiry, in seconds from now; omit for a non-expiring token.
    pub expires_in_secs: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FederationTokenRevokeInput {
    /// Token id to revoke.
    pub id: Uuid,
}

/// was: list_federation_tokens() in web/components/federation_token_list.rs
#[api_mcp_dioxus_server(server = "list_federation_tokens")]
pub async fn token_federation_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: FederationTokensListInput,
) -> Result<Vec<TokenEntry>, ApiError> {
    p.require_admin()?;
    let rows = sqlx::query_as::<_, TokenDbRow>(&format!(
        "SELECT {TOKEN_COLS} FROM tokens WHERE kind = 'federation' ORDER BY created_at DESC"
    ))
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// was: create_federation_token() in web/components/federation_token_list.rs
#[api_mcp_dioxus_server(server = "create_federation_token")]
pub async fn token_federation_create(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: FederationTokenCreateInput,
) -> Result<String, ApiError> {
    p.require_admin()?;

    let label = input.label.trim().to_string();
    if label.is_empty() {
        return Err(ApiError::bad_request("label is required"));
    }

    let (raw_token, hash) = mint_token(Some("fed_"));
    let expires_at = input
        .expires_in_secs
        .map(|s| chrono::Utc::now() + chrono::Duration::seconds(s));

    sqlx::query(
        "INSERT INTO tokens (cluster_id, token_hash, label, kind, expires_at) VALUES (NULL, $1, $2, 'federation', $3)",
    )
    .bind(&hash)
    .bind(&label)
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(internal)?;

    Ok(raw_token)
}

/// was: revoke_federation_token() in web/components/federation_token_list.rs
#[api_mcp_dioxus_server(server = "revoke_federation_token")]
pub async fn token_federation_revoke(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: FederationTokenRevokeInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query("UPDATE tokens SET revoked = true WHERE id = $1")
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

// ── Master list / revoke-any (admin) ────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AllTokensListInput {}

/// One token row for the master list, with its scope resolved to a
/// human-readable string (e.g. "org: Acme", "cluster: edge-1").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AllTokenEntry {
    pub id: String,
    /// Raw label; may be empty.
    pub label: String,
    pub kind: String,
    pub scope: Option<String>,
    /// Web-UI path of the org/cluster the token is bound to, if any.
    pub scope_href: Option<String>,
    pub revoked: bool,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub expired: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AnyTokenRevokeInput {
    /// Token id to revoke.
    pub id: Uuid,
}

/// was: list_all_tokens() in web/components/all_tokens_list.rs
#[api_mcp_dioxus_server(server = "list_all_tokens")]
pub async fn token_list_all(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: AllTokensListInput,
) -> Result<Vec<AllTokenEntry>, ApiError> {
    p.require_admin()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        label: String,
        kind: String,
        revoked: bool,
        created_at: DateTime<Utc>,
        expires_at: Option<DateTime<Utc>>,
        organization_id: Option<Uuid>,
        org_name: Option<String>,
        cluster_id: Option<Uuid>,
        cluster_name: Option<String>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT t.id, t.label, t.kind, t.revoked, t.created_at, t.expires_at, \
                t.organization_id, o.name AS org_name, t.cluster_id, c.name AS cluster_name \
         FROM tokens t \
         LEFT JOIN organizations o ON o.id = t.organization_id \
         LEFT JOIN clusters c ON c.id = t.cluster_id \
         ORDER BY t.created_at DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    let now = chrono::Utc::now();
    Ok(rows
        .into_iter()
        .map(|r| {
            // Resolve scope: admin/federation are global kinds; otherwise show
            // the specific org or cluster the token is bound to.
            let scope = match r.kind.as_str() {
                "admin" => Some("admin".to_string()),
                "federation" => Some("federation".to_string()),
                _ => r
                    .org_name
                    .map(|n| format!("org: {n}"))
                    .or_else(|| r.cluster_name.map(|n| format!("cluster: {n}"))),
            };
            // Link admin/federation scopes nowhere; org/cluster link to their page.
            let scope_href = match r.kind.as_str() {
                "admin" | "federation" => None,
                _ => r
                    .organization_id
                    .map(|id| format!("/organizations/{id}"))
                    .or_else(|| r.cluster_id.map(|id| format!("/clusters/{id}"))),
            };
            AllTokenEntry {
                id: r.id.to_string(),
                label: r.label,
                kind: r.kind,
                scope,
                scope_href,
                revoked: r.revoked,
                created_at: r.created_at,
                expires_at: r.expires_at,
                expired: r.expires_at.is_some_and(|e| e < now),
            }
        })
        .collect())
}

/// was: revoke_any_token() in web/components/all_tokens_list.rs
#[api_mcp_dioxus_server(server = "revoke_any_token")]
pub async fn token_revoke_any(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: AnyTokenRevokeInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query("UPDATE tokens SET revoked = true WHERE id = $1")
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}
