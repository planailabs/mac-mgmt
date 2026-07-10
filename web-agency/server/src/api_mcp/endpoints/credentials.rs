//! Credential endpoints.

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
pub struct CredentialRow {
    pub id: Uuid,
    pub name: String,
    pub credential_type: String,
    pub organization_name: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CredentialListInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CredentialInfo {
    pub id: Uuid,
    pub name: String,
    pub credential_type: String,
    pub organization_id: Option<Uuid>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CredentialGetInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CredentialCreateInput {
    /// Owning organization; `None` makes a global credential (admin only).
    pub organization_id: Option<Uuid>,
    pub name: String,
    pub credential_type: String,
    /// Secret payload as a JSON document; encrypted at rest.
    pub data_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CredentialUpdateInput {
    pub id: Uuid,
    pub name: String,
    /// New owning organization; `None` makes it global (admin only).
    pub organization_id: Option<Uuid>,
    /// Replacement secret JSON; `None` or blank keeps the current data.
    pub new_data_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CredentialDeleteInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CredentialTestInput {
    pub id: Uuid,
}

/// Require read access to the credential's organization: admins always pass;
/// global (NULL-org) credentials are admin-only; else org read membership.
/// Shared with sibling endpoint modules (e.g. domain availability checks).
#[cfg(feature = "server")]
pub(super) async fn require_credential_read(
    pool: &sqlx::PgPool,
    principal: &Principal,
    credential_id: Uuid,
) -> Result<(), ApiError> {
    if principal.admin {
        return Ok(());
    }
    // Non-admins: collapse missing / global / cross-org into a single generic
    // "access denied" so credential existence cannot be probed by comparing
    // 404 (bogus id) against 403 (real cross-org id) — an IDOR enumeration.
    match credential_org_opt(pool, credential_id).await? {
        Some(Some(oid)) => principal
            .require_read(&oid)
            .map_err(|_| ApiError::forbidden("access denied")),
        _ => Err(ApiError::forbidden("access denied")),
    }
}

/// Require write access to the credential's organization: admins always pass;
/// global (NULL-org) credentials are admin-only; else org write membership.
#[cfg(feature = "server")]
async fn require_credential_write(
    pool: &sqlx::PgPool,
    principal: &Principal,
    credential_id: Uuid,
) -> Result<(), ApiError> {
    if principal.admin {
        return Ok(());
    }
    match credential_org_opt(pool, credential_id).await? {
        Some(Some(oid)) => principal
            .require_write(&oid)
            .map_err(|_| ApiError::forbidden("access denied")),
        _ => Err(ApiError::forbidden("access denied")),
    }
}

/// Look up a credential's owning org without leaking existence via a 404:
///   `None`        → no such credential
///   `Some(None)`  → exists, global (no org)
///   `Some(Some())`→ exists, owned by that org
#[cfg(feature = "server")]
async fn credential_org_opt(
    pool: &sqlx::PgPool,
    credential_id: Uuid,
) -> Result<Option<Option<Uuid>>, ApiError> {
    sqlx::query_scalar::<_, Option<Uuid>>(
        "SELECT organization_id FROM credentials WHERE id = $1",
    )
    .bind(credential_id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)
}

/// Fetch a credential's (nullable) owning org, or 404.
/// List credentials the caller may see (admins: all; else their orgs' + global).
#[api_mcp_dioxus_server(server = "list_credentials")]
pub async fn credential_list(
    pool: &sqlx::PgPool,
    principal: &Principal,
    _input: CredentialListInput,
) -> Result<Vec<CredentialRow>, ApiError> {
    type Row = (
        Uuid,
        String,
        String,
        Option<String>,
        chrono::DateTime<chrono::Utc>,
    );
    let base = "SELECT c.id, c.name, c.credential_type, o.name, c.created_at \
                FROM credentials c LEFT JOIN organizations o ON o.id = c.organization_id";

    let rows = match principal.read_filter() {
        None => {
            sqlx::query_as::<_, Row>(&format!("{base} ORDER BY c.created_at DESC"))
                .fetch_all(pool)
                .await
        }
        Some(org_ids) => {
            // Non-admins also see global (NULL org) credentials.
            sqlx::query_as::<_, Row>(&format!(
                "{base} WHERE c.organization_id = ANY($1) OR c.organization_id IS NULL \
                 ORDER BY c.created_at DESC"
            ))
            .bind(org_ids)
            .fetch_all(pool)
            .await
        }
    }
    .map_err(super::internal)?;

    Ok(rows
        .into_iter()
        .map(
            |(id, name, credential_type, organization_name, created_at)| CredentialRow {
                id,
                name,
                credential_type,
                organization_name,
                created_at: created_at.format("%Y-%m-%d %H:%M").to_string(),
            },
        )
        .collect())
}

/// Get a credential's metadata (admins, or members of the credential's org;
/// global credentials are admin-only). Never returns the secret data.
#[api_mcp_dioxus_server(server = "get_credential")]
pub async fn credential_get(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: CredentialGetInput,
) -> Result<CredentialInfo, ApiError> {
    // Authorize before disclosing existence: otherwise a caller can probe the
    // UUID namespace (403 for a real cross-org id vs 404 for a bogus one) to
    // enumerate credentials belonging to other organizations (IDOR).
    require_credential_read(pool, principal, input.id).await?;

    let row = sqlx::query_as::<_, (Uuid, String, String, Option<Uuid>, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, name, credential_type, organization_id, created_at FROM credentials WHERE id = $1",
    )
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?
    .ok_or_else(|| ApiError::not_found("credential not found"))?;

    let (id, name, credential_type, organization_id, created_at) = row;
    Ok(CredentialInfo {
        id,
        name,
        credential_type,
        organization_id,
        created_at: created_at.format("%Y-%m-%d %H:%M").to_string(),
    })
}

/// Create a credential. Global (no org) credentials require admin; org-owned
/// credentials require org write. The JSON payload is encrypted at rest.
/// Returns the new id.
#[api_mcp_dioxus_server(server = "create_credential")]
pub async fn credential_create(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: CredentialCreateInput,
) -> Result<Uuid, ApiError> {
    match input.organization_id {
        None => principal.require_admin()?,
        Some(oid) => principal.require_write(&oid)?,
    }

    let _: serde_json::Value = serde_json::from_str(&input.data_json)
        .map_err(|e| ApiError::bad_request(format!("invalid JSON: {e}")))?;

    let encrypted = crate::crypto::encrypt(input.data_json.as_bytes())
        .map_err(|e| ApiError::internal(format!("encryption failed: {e}")))?;

    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO credentials (organization_id, name, credential_type, encrypted_data) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(input.organization_id)
    .bind(&input.name)
    .bind(&input.credential_type)
    .bind(&encrypted)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::internal(format!("failed to create credential: {e}")))
}

/// Update a credential's name/org and optionally replace its secret data
/// (requires credential write plus write on the target org; moving to global
/// requires admin).
#[api_mcp_dioxus_server(server = "update_credential")]
pub async fn credential_update(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: CredentialUpdateInput,
) -> Result<(), ApiError> {
    require_credential_write(pool, principal, input.id).await?;

    // Validate target org assignment.
    match input.organization_id {
        None => principal.require_admin()?,
        Some(oid) => principal.require_write(&oid)?,
    }

    sqlx::query(
        "UPDATE credentials SET name = $1, organization_id = $2, updated_at = now() WHERE id = $3",
    )
    .bind(&input.name)
    .bind(input.organization_id)
    .bind(input.id)
    .execute(pool)
    .await
    .map_err(super::internal)?;

    if let Some(json) = input.new_data_json {
        if !json.trim().is_empty() {
            let _: serde_json::Value = serde_json::from_str(&json)
                .map_err(|e| ApiError::bad_request(format!("invalid JSON: {e}")))?;
            let encrypted = crate::crypto::encrypt(json.as_bytes())
                .map_err(|e| ApiError::internal(format!("encryption failed: {e}")))?;
            sqlx::query(
                "UPDATE credentials SET encrypted_data = $1, updated_at = now() WHERE id = $2",
            )
            .bind(&encrypted)
            .bind(input.id)
            .execute(pool)
            .await
            .map_err(super::internal)?;
        }
    }

    Ok(())
}

/// Delete a credential (requires credential write).
#[api_mcp_dioxus_server(server = "delete_credential")]
pub async fn credential_delete(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: CredentialDeleteInput,
) -> Result<(), ApiError> {
    require_credential_write(pool, principal, input.id).await?;

    sqlx::query("DELETE FROM credentials WHERE id = $1")
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(super::internal)?;
    Ok(())
}

/// Test a credential against its upstream API (requires credential read).
/// Returns a human-readable status string.
#[api_mcp_dioxus_server(server = "test_credential_conn")]
pub async fn credential_test(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: CredentialTestInput,
) -> Result<String, ApiError> {
    require_credential_read(pool, principal, input.id).await?;

    let cred_type =
        sqlx::query_scalar::<_, String>("SELECT credential_type FROM credentials WHERE id = $1")
            .bind(input.id)
            .fetch_optional(pool)
            .await
            .map_err(super::internal)?
            .ok_or_else(|| ApiError::not_found("credential not found"))?;

    match cred_type.as_str() {
        "cloudflare" => {
            let client = crate::credentials::cf_client(pool, input.id)
                .await
                .map_err(|e| ApiError::internal(format!("{e}")))?;
            let zones = client
                .list_zones(None)
                .await
                .map_err(|e| ApiError::internal(format!("Cloudflare API error: {e}")))?;
            Ok(format!("OK — {} zone(s) accessible", zones.len()))
        }
        "spaceship" => {
            let client = crate::credentials::spaceship_client(pool, input.id)
                .await
                .map_err(|e| ApiError::internal(format!("{e}")))?;
            let resp = client
                .list_domains(0, 1)
                .await
                .map_err(|e| ApiError::internal(format!("Spaceship API error: {e}")))?;
            Ok(format!(
                "OK — {} domain(s) in account",
                resp.total_count.unwrap_or(0)
            ))
        }
        "changedetection" => {
            let (client, _group) = crate::credentials::changedetection_client(pool, input.id)
                .await
                .map_err(|e| ApiError::internal(format!("{e}")))?;
            let info = client
                .get_system_info()
                .await
                .map_err(|e| ApiError::internal(format!("ChangeDetection API error: {e}")))?;
            let info = info.into_inner();
            Ok(format!(
                "OK — v{}, {} watch(es)",
                info.version.as_deref().unwrap_or("?"),
                info.watch_count.unwrap_or(0),
            ))
        }
        _ => Err(ApiError::bad_request("unknown credential type")),
    }
}
