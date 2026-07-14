//! SSH key and client certificate/CA endpoints.
//!
//! SSH keys are cluster-scoped. Client certificates and CAs live in the
//! `client_certificates` table at three scopes: cluster, organization and
//! admin (server-wide). Cluster-level changes require org-admin of an owning
//! org; org-level changes require org-admin; admin-level requires global
//! admin.

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

/// An authorized SSH public key on a cluster.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct SshKeyEntry {
    pub id: Uuid,
    pub fingerprint: String,
    pub comment: String,
    pub created_at: DateTime<Utc>,
}

/// A client certificate or CA (fingerprint + label; the PEM is never listed).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct CertEntry {
    pub id: Uuid,
    pub fingerprint: String,
    pub label: String,
    pub created_at: DateTime<Utc>,
}

/// Require org-admin of at least one organization owning the cluster.
/// A cluster with no owning organizations is unmanageable by anyone
/// (including global admins) — matching the legacy web checks.
#[cfg(feature = "server")]
async fn require_cluster_org_admin(
    pool: &sqlx::PgPool,
    p: &Principal,
    cluster_id: Uuid,
) -> Result<(), ApiError> {
    let org_ids = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM organization_clusters WHERE cluster_id = $1",
    )
    .bind(cluster_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    if org_ids.is_empty() || !org_ids.iter().any(|oid| access::is_org_admin(p, oid)) {
        return Err(ApiError::forbidden("organization admin access required"));
    }
    Ok(())
}

/// Resolve (fingerprint, stored PEM) from the add-cert inputs: a PEM wins and
/// derives the fingerprint; else a bare fingerprint is accepted.
#[cfg(feature = "server")]
fn resolve_cert_fp(
    fingerprint: &str,
    certificate_pem: String,
) -> Result<(String, Option<String>), ApiError> {
    if !certificate_pem.trim().is_empty() {
        let fp = crate::api::routes::fingerprint_from_pem_str(&certificate_pem)
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        Ok((fp, Some(certificate_pem)))
    } else if !fingerprint.trim().is_empty() {
        Ok((fingerprint.trim().to_lowercase(), None))
    } else {
        Err(ApiError::bad_request(
            "fingerprint or certificate PEM required",
        ))
    }
}

// ── SSH keys (cluster-scoped) ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SshKeysListInput {
    pub cluster_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SshKeyAddInput {
    pub cluster_id: Uuid,
    /// OpenSSH public key line ("<type> <base64> [comment]").
    pub public_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SshKeyRemoveInput {
    /// SSH key row id.
    pub id: Uuid,
}

/// was: list_ssh_keys() in web/components/cluster_ssh_keys.rs
#[api_mcp_dioxus_server(server = "list_ssh_keys")]
pub async fn ssh_key_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: SshKeysListInput,
) -> Result<Vec<SshKeyEntry>, ApiError> {
    access::require_cluster_read(pool, p, input.cluster_id).await?;
    let keys = sqlx::query_as::<_, SshKeyEntry>(
        "SELECT id, fingerprint, comment, created_at \
         FROM cluster_ssh_keys WHERE cluster_id = $1 ORDER BY created_at",
    )
    .bind(input.cluster_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(keys)
}

/// was: add_ssh_key() in web/components/cluster_ssh_keys.rs
#[api_mcp_dioxus_server(server = "add_ssh_key")]
pub async fn ssh_key_add(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: SshKeyAddInput,
) -> Result<(), ApiError> {
    use base64::Engine;
    use sha2::{Digest, Sha256};

    let cid = input.cluster_id;
    require_cluster_org_admin(pool, p, cid).await?;

    let trimmed = input.public_key.trim();
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(ApiError::bad_request("Invalid SSH public key format"));
    }
    let b64_data = base64::engine::general_purpose::STANDARD
        .decode(parts[1])
        .map_err(|_| ApiError::bad_request("Invalid base64 in SSH key"))?;
    let fingerprint = format!(
        "SHA256:{}",
        base64::engine::general_purpose::STANDARD.encode(Sha256::digest(&b64_data))
    );
    let comment = if parts.len() > 2 {
        parts[2..].join(" ")
    } else {
        String::new()
    };

    sqlx::query(
        "INSERT INTO cluster_ssh_keys (cluster_id, public_key, comment, fingerprint) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(cid)
    .bind(trimmed)
    .bind(&comment)
    .bind(&fingerprint)
    .execute(pool)
    .await
    .map_err(internal)?;
    crate::api::push::notify_global(cid, crate::api::push::PushMessage::SyncSshKeys).await;
    Ok(())
}

/// was: remove_ssh_key() in web/components/cluster_ssh_keys.rs
#[api_mcp_dioxus_server(server = "remove_ssh_key")]
pub async fn ssh_key_remove(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: SshKeyRemoveInput,
) -> Result<(), ApiError> {
    let owner_cid = sqlx::query_scalar::<_, Uuid>(
        "SELECT cluster_id FROM cluster_ssh_keys WHERE id = $1",
    )
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?;
    if let Some(owner_cid) = owner_cid {
        require_cluster_org_admin(pool, p, owner_cid).await?;
    }
    let cid = sqlx::query_scalar::<_, Uuid>(
        "DELETE FROM cluster_ssh_keys WHERE id = $1 RETURNING cluster_id",
    )
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?;
    if let Some(cid) = cid {
        crate::api::push::notify_global(cid, crate::api::push::PushMessage::SyncSshKeys).await;
    }
    Ok(())
}

// ── Cluster client certificates ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterCertsListInput {
    pub cluster_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterCertAddInput {
    pub cluster_id: Uuid,
    /// Certificate fingerprint (hex); ignored when a PEM is supplied.
    pub fingerprint: String,
    /// Certificate PEM; when non-empty the fingerprint is derived from it.
    pub certificate_pem: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterCertRemoveInput {
    /// Certificate row id.
    pub id: Uuid,
}

/// was: list_client_certs() in web/components/cluster_client_certs.rs
#[api_mcp_dioxus_server(server = "list_client_certs")]
pub async fn cert_cluster_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterCertsListInput,
) -> Result<Vec<CertEntry>, ApiError> {
    access::require_cluster_read(pool, p, input.cluster_id).await?;
    let certs = sqlx::query_as::<_, CertEntry>(
        "SELECT id, fingerprint, label, created_at \
         FROM client_certificates \
         WHERE scope = 'cluster' AND scope_id = $1 AND is_ca = false \
         ORDER BY created_at",
    )
    .bind(input.cluster_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(certs)
}

/// was: add_client_cert() in web/components/cluster_client_certs.rs
#[api_mcp_dioxus_server(server = "add_client_cert")]
pub async fn cert_cluster_add(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterCertAddInput,
) -> Result<(), ApiError> {
    require_cluster_org_admin(pool, p, input.cluster_id).await?;

    let lbl = input.label.trim().to_string();
    let (fp, pem) = resolve_cert_fp(&input.fingerprint, input.certificate_pem)?;

    sqlx::query(
        "INSERT INTO client_certificates (scope, scope_id, is_ca, fingerprint, certificate_pem, label) \
         VALUES ('cluster', $1, false, $2, $3, $4)",
    )
    .bind(input.cluster_id)
    .bind(&fp)
    .bind(&pem)
    .bind(&lbl)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

/// was: remove_client_cert() in web/components/cluster_client_certs.rs
#[api_mcp_dioxus_server(server = "remove_client_cert")]
pub async fn cert_cluster_remove(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterCertRemoveInput,
) -> Result<(), ApiError> {
    let owner_cid = sqlx::query_scalar::<_, Uuid>(
        "SELECT scope_id FROM client_certificates WHERE id = $1 AND scope = 'cluster' AND is_ca = false",
    )
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?;
    if let Some(owner_cid) = owner_cid {
        require_cluster_org_admin(pool, p, owner_cid).await?;
    }
    sqlx::query(
        "DELETE FROM client_certificates WHERE id = $1 AND scope = 'cluster' AND is_ca = false",
    )
    .bind(input.id)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

// ── Cluster client CAs ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterCasListInput {
    pub cluster_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterCaAddInput {
    pub cluster_id: Uuid,
    /// CA certificate PEM (required; the fingerprint is derived from it).
    pub certificate_pem: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterCaRemoveInput {
    /// CA row id.
    pub id: Uuid,
}

/// was: list_cluster_cas() in web/components/cluster_client_cas.rs
#[api_mcp_dioxus_server(server = "list_cluster_cas")]
pub async fn ca_cluster_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterCasListInput,
) -> Result<Vec<CertEntry>, ApiError> {
    access::require_cluster_read(pool, p, input.cluster_id).await?;
    let cas = sqlx::query_as::<_, CertEntry>(
        "SELECT id, fingerprint, label, created_at \
         FROM client_certificates \
         WHERE scope = 'cluster' AND scope_id = $1 AND is_ca = true \
         ORDER BY created_at",
    )
    .bind(input.cluster_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(cas)
}

/// was: add_cluster_ca() in web/components/cluster_client_cas.rs
#[api_mcp_dioxus_server(server = "add_cluster_ca")]
pub async fn ca_cluster_add(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterCaAddInput,
) -> Result<(), ApiError> {
    require_cluster_org_admin(pool, p, input.cluster_id).await?;

    let fp = crate::api::routes::fingerprint_from_pem_str(&input.certificate_pem)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let lbl = input.label.trim().to_string();

    sqlx::query(
        "INSERT INTO client_certificates (scope, scope_id, is_ca, fingerprint, certificate_pem, label) \
         VALUES ('cluster', $1, true, $2, $3, $4)",
    )
    .bind(input.cluster_id)
    .bind(&fp)
    .bind(&input.certificate_pem)
    .bind(&lbl)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

/// was: remove_cluster_ca() in web/components/cluster_client_cas.rs
#[api_mcp_dioxus_server(server = "remove_cluster_ca")]
pub async fn ca_cluster_remove(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterCaRemoveInput,
) -> Result<(), ApiError> {
    let owner_cid = sqlx::query_scalar::<_, Uuid>(
        "SELECT scope_id FROM client_certificates WHERE id = $1 AND scope = 'cluster' AND is_ca = true",
    )
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?;
    if let Some(owner_cid) = owner_cid {
        require_cluster_org_admin(pool, p, owner_cid).await?;
    }
    sqlx::query(
        "DELETE FROM client_certificates WHERE id = $1 AND scope = 'cluster' AND is_ca = true",
    )
    .bind(input.id)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

// ── Organization client certificates ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgCertsListInput {
    pub organization_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgCertAddInput {
    pub organization_id: Uuid,
    /// Certificate fingerprint (hex); ignored when a PEM is supplied.
    pub fingerprint: String,
    /// Certificate PEM; when non-empty the fingerprint is derived from it.
    pub certificate_pem: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgCertRemoveInput {
    pub organization_id: Uuid,
    /// Certificate row id.
    pub id: Uuid,
}

/// was: list_org_certs() in web/components/organization_client_certs.rs
#[api_mcp_dioxus_server(server = "list_org_certs")]
pub async fn cert_org_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: OrgCertsListInput,
) -> Result<Vec<CertEntry>, ApiError> {
    p.require_read(&input.organization_id)?;
    let certs = sqlx::query_as::<_, CertEntry>(
        "SELECT id, fingerprint, label, created_at \
         FROM client_certificates \
         WHERE scope = 'organization' AND scope_id = $1 AND is_ca = false \
         ORDER BY created_at",
    )
    .bind(input.organization_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(certs)
}

/// was: add_org_cert() in web/components/organization_client_certs.rs
#[api_mcp_dioxus_server(server = "add_org_cert")]
pub async fn cert_org_add(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: OrgCertAddInput,
) -> Result<(), ApiError> {
    access::require_org_admin(p, &input.organization_id)?;

    let lbl = input.label.trim().to_string();
    let (fp, pem) = resolve_cert_fp(&input.fingerprint, input.certificate_pem)?;

    sqlx::query(
        "INSERT INTO client_certificates (scope, scope_id, is_ca, fingerprint, certificate_pem, label) \
         VALUES ('organization', $1, false, $2, $3, $4)",
    )
    .bind(input.organization_id)
    .bind(&fp)
    .bind(&pem)
    .bind(&lbl)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

/// was: remove_org_cert() in web/components/organization_client_certs.rs
#[api_mcp_dioxus_server(server = "remove_org_cert")]
pub async fn cert_org_remove(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: OrgCertRemoveInput,
) -> Result<(), ApiError> {
    access::require_org_admin(p, &input.organization_id)?;
    sqlx::query(
        "DELETE FROM client_certificates \
         WHERE id = $1 AND scope = 'organization' AND scope_id = $2 AND is_ca = false",
    )
    .bind(input.id)
    .bind(input.organization_id)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

// ── Organization client CAs ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgCasListInput {
    pub organization_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgCaAddInput {
    pub organization_id: Uuid,
    /// CA certificate PEM (required; the fingerprint is derived from it).
    pub certificate_pem: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrgCaRemoveInput {
    pub organization_id: Uuid,
    /// CA row id.
    pub id: Uuid,
}

/// was: list_org_cas() in web/components/organization_client_cas.rs
#[api_mcp_dioxus_server(server = "list_org_cas")]
pub async fn ca_org_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: OrgCasListInput,
) -> Result<Vec<CertEntry>, ApiError> {
    p.require_read(&input.organization_id)?;
    let cas = sqlx::query_as::<_, CertEntry>(
        "SELECT id, fingerprint, label, created_at \
         FROM client_certificates \
         WHERE scope = 'organization' AND scope_id = $1 AND is_ca = true \
         ORDER BY created_at",
    )
    .bind(input.organization_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(cas)
}

/// was: add_org_ca() in web/components/organization_client_cas.rs
#[api_mcp_dioxus_server(server = "add_org_ca")]
pub async fn ca_org_add(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: OrgCaAddInput,
) -> Result<(), ApiError> {
    access::require_org_admin(p, &input.organization_id)?;

    let fp = crate::api::routes::fingerprint_from_pem_str(&input.certificate_pem)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let lbl = input.label.trim().to_string();

    sqlx::query(
        "INSERT INTO client_certificates (scope, scope_id, is_ca, fingerprint, certificate_pem, label) \
         VALUES ('organization', $1, true, $2, $3, $4)",
    )
    .bind(input.organization_id)
    .bind(&fp)
    .bind(&input.certificate_pem)
    .bind(&lbl)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

/// was: remove_org_ca() in web/components/organization_client_cas.rs
#[api_mcp_dioxus_server(server = "remove_org_ca")]
pub async fn ca_org_remove(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: OrgCaRemoveInput,
) -> Result<(), ApiError> {
    access::require_org_admin(p, &input.organization_id)?;
    sqlx::query(
        "DELETE FROM client_certificates \
         WHERE id = $1 AND scope = 'organization' AND scope_id = $2 AND is_ca = true",
    )
    .bind(input.id)
    .bind(input.organization_id)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

// ── Admin (server-wide) client certificates ─────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AdminCertsListInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AdminCertAddInput {
    /// Certificate fingerprint (hex); ignored when a PEM is supplied.
    pub fingerprint: String,
    /// Certificate PEM; when non-empty the fingerprint is derived from it.
    pub certificate_pem: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AdminCertRemoveInput {
    /// Certificate row id.
    pub id: Uuid,
}

/// was: list_admin_certs() in web/components/admin_client_certs_page.rs
#[api_mcp_dioxus_server(server = "list_admin_certs")]
pub async fn cert_admin_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: AdminCertsListInput,
) -> Result<Vec<CertEntry>, ApiError> {
    p.require_admin()?;
    let certs = sqlx::query_as::<_, CertEntry>(
        "SELECT id, fingerprint, label, created_at \
         FROM client_certificates \
         WHERE scope = 'admin' AND is_ca = false \
         ORDER BY created_at",
    )
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(certs)
}

/// was: add_admin_cert() in web/components/admin_client_certs_page.rs
#[api_mcp_dioxus_server(server = "add_admin_cert")]
pub async fn cert_admin_add(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: AdminCertAddInput,
) -> Result<(), ApiError> {
    p.require_admin()?;

    let lbl = input.label.trim().to_string();
    let (fp, pem) = resolve_cert_fp(&input.fingerprint, input.certificate_pem)?;

    sqlx::query(
        "INSERT INTO client_certificates (scope, scope_id, is_ca, fingerprint, certificate_pem, label) \
         VALUES ('admin', NULL, false, $1, $2, $3)",
    )
    .bind(&fp)
    .bind(&pem)
    .bind(&lbl)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

/// was: remove_admin_cert() in web/components/admin_client_certs_page.rs
#[api_mcp_dioxus_server(server = "remove_admin_cert")]
pub async fn cert_admin_remove(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: AdminCertRemoveInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query(
        "DELETE FROM client_certificates WHERE id = $1 AND scope = 'admin' AND is_ca = false",
    )
    .bind(input.id)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

// ── Admin (server-wide) client CAs ──────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AdminCasListInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AdminCaAddInput {
    /// CA certificate PEM (required; the fingerprint is derived from it).
    pub certificate_pem: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AdminCaRemoveInput {
    /// CA row id.
    pub id: Uuid,
}

/// was: list_admin_cas() in web/components/admin_client_cas_page.rs
#[api_mcp_dioxus_server(server = "list_admin_cas")]
pub async fn ca_admin_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: AdminCasListInput,
) -> Result<Vec<CertEntry>, ApiError> {
    p.require_admin()?;
    let cas = sqlx::query_as::<_, CertEntry>(
        "SELECT id, fingerprint, label, created_at \
         FROM client_certificates \
         WHERE scope = 'admin' AND is_ca = true \
         ORDER BY created_at",
    )
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(cas)
}

/// was: add_admin_ca() in web/components/admin_client_cas_page.rs
#[api_mcp_dioxus_server(server = "add_admin_ca")]
pub async fn ca_admin_add(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: AdminCaAddInput,
) -> Result<(), ApiError> {
    p.require_admin()?;

    let fp = crate::api::routes::fingerprint_from_pem_str(&input.certificate_pem)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let lbl = input.label.trim().to_string();

    sqlx::query(
        "INSERT INTO client_certificates (scope, scope_id, is_ca, fingerprint, certificate_pem, label) \
         VALUES ('admin', NULL, true, $1, $2, $3)",
    )
    .bind(&fp)
    .bind(&input.certificate_pem)
    .bind(&lbl)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

/// was: remove_admin_ca() in web/components/admin_client_cas_page.rs
#[api_mcp_dioxus_server(server = "remove_admin_ca")]
pub async fn ca_admin_remove(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: AdminCaRemoveInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query(
        "DELETE FROM client_certificates WHERE id = $1 AND scope = 'admin' AND is_ca = true",
    )
    .bind(input.id)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}
