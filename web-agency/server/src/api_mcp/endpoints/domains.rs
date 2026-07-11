//! Domain endpoints (HTTP `/api/v1/domains` + MCP `domain_*` tools).
//!
//! Handlers are written once here; the macro-generated `#[server]` wrappers let
//! the Dioxus UI call the same logic.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dioxus::prelude::*;
use plan_ai_api_mcp_macros::api_mcp_dioxus_server;

// Glue the macro-generated `#[server]` wrappers call (server-only).
#[cfg(feature = "server")]
use crate::server_pool;
#[cfg(feature = "server")]
use crate::web::user::{current_user, principal_from, to_serverfn};
#[cfg(feature = "server")]
use plan_ai_api_mcp::{ApiError, Principal};

// ── DTOs (shared client + server) ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DomainRow {
    pub id: Uuid,
    pub name: String,
    pub registrar_type: Option<String>,
    pub ssl_mode: String,
    pub dnssec_enabled: bool,
    pub cloudflare_zone_id: Option<String>,
    pub expires_at: Option<String>,
    pub organization_name: String,
    /// Whether nameservers can be set at the registrar (spaceship + credential).
    pub can_set_nameservers: bool,
    /// Cached NS match status: None = unknown, Some(true) = match, Some(false) = mismatch.
    pub ns_ok: Option<bool>,
    pub has_webspace: bool,
    pub ai_bots_protection: Option<String>,
    pub expires_soon: bool,
}

/// Input for `domain_list`. Results are scoped by the caller's principal;
/// no fields are required.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DomainListInput {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DnsRecordRow {
    pub id: Uuid,
    pub record_type: String,
    pub record_value: String,
    pub ttl: Option<i32>,
    pub proxied: bool,
    pub cloudflare_record_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SubdomainData {
    pub id: Uuid,
    pub name: String,
    pub records: Vec<DnsRecordRow>,
}

/// Full domain detail, incl. subdomains, DNS records and live Cloudflare
/// zone/DNSSEC info.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DomainData {
    pub id: Uuid,
    pub name: String,
    pub registrar_type: Option<String>,
    pub registrar_credential_id: Option<Uuid>,
    pub ssl_mode: String,
    pub dnssec_enabled: bool,
    pub cloudflare_zone_id: Option<String>,
    pub cloudflare_credential_id: Option<Uuid>,
    pub cloudflare_zone_status: Option<String>,
    pub cloudflare_nameservers: Vec<String>,
    pub registered_at: Option<String>,
    pub expires_at: Option<String>,
    pub organization_id: Uuid,
    pub organization_name: String,
    pub subdomains: Vec<SubdomainData>,
    pub can_set_nameservers: bool,
    pub ai_bots_protection: Option<String>,
    pub dnssec_ds: Option<String>,
    pub dnssec_key_tag: Option<String>,
    pub dnssec_algorithm: Option<String>,
    pub dnssec_digest_type: Option<String>,
    pub dnssec_digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeployResult {
    pub zone_id: String,
    pub status: String,
    pub nameservers: Vec<String>,
    pub nameservers_set_at_registrar: bool,
}

/// One failed domain in a bulk operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BulkFailure {
    pub name: String,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BulkOpResult {
    pub succeeded: Vec<String>,
    pub failed: Vec<BulkFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DomainGetInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DomainCreateInput {
    pub org_id: Uuid,
    pub domain_name: String,
    pub cf_credential_id: Option<Uuid>,
    pub registrar_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DomainDeleteInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DomainMoveInput {
    pub id: Uuid,
    pub target_org_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DomainDeployCloudflareInput {
    pub id: Uuid,
    pub credential_id: Uuid,
}

/// Input for `domain_update`. Each provided field is applied (with its
/// Cloudflare call, when the domain is deployed) in the order ssl_mode →
/// dnssec_enabled → ai_bots_protection, failing fast.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DomainUpdateInput {
    pub id: Uuid,
    pub ssl_mode: Option<String>,
    pub dnssec_enabled: Option<bool>,
    pub ai_bots_protection: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DomainSetNameserversInput {
    pub id: Uuid,
    pub nameservers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DomainSyncRecordsInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SubdomainCreateInput {
    pub domain_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SubdomainDeleteInput {
    /// The subdomain id.
    pub id: Uuid,
    pub domain_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DnsRecordCreateInput {
    pub domain_id: Uuid,
    pub subdomain_id: Uuid,
    pub record_type: String,
    pub record_value: String,
    pub proxied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DnsRecordDeleteInput {
    /// The DNS record id.
    pub id: Uuid,
    pub domain_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DomainBulkDeployCloudflareInput {
    pub domain_ids: Vec<Uuid>,
    pub credential_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DomainBulkSetNameserversInput {
    pub domain_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DomainBulkSetSslModeInput {
    pub domain_ids: Vec<Uuid>,
    pub ssl_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DomainBulkSetAiBotsInput {
    pub domain_ids: Vec<Uuid>,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DomainBulkCreatePagesInput {
    pub domain_ids: Vec<Uuid>,
    pub credential_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DomainCheckAvailabilityInput {
    pub credential_id: Uuid,
    pub domain: String,
}

/// Availability + registration pricing for one domain at a registrar.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AvailabilityResult {
    pub domain: String,
    pub available: bool,
    pub price: Option<String>,
    pub currency: Option<String>,
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DomainDiscoverInput {
    pub credential_id: Uuid,
    pub organization_id: Uuid,
}

/// A domain discovered from a credential, ready for import.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DiscoveredDomain {
    pub name: String,
    /// Cloudflare zone ID.
    pub zone_id: Option<String>,
    /// Zone status or registrar lifecycle status.
    pub status: Option<String>,
    pub expires_at: Option<String>,
    /// True if the domain already exists in the organization.
    pub already_imported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DomainImportInput {
    pub credential_id: Uuid,
    pub organization_id: Uuid,
    pub domain_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ImportResult {
    pub imported: u32,
    pub skipped: u32,
    pub errors: Vec<String>,
}

// ── Endpoints ─────────────────────────────────────────────────────────

/// List domains the caller may see (admins: all; otherwise their orgs').
#[api_mcp_dioxus_server(server = "list_domains")]
pub async fn domain_list(
    pool: &sqlx::PgPool,
    principal: &Principal,
    _input: DomainListInput,
) -> Result<Vec<DomainRow>, ApiError> {
    type Row = (
        Uuid,
        String,
        Option<String>,
        String,
        bool,
        Option<String>,
        Option<chrono::DateTime<chrono::Utc>>,
        String,
        Option<String>,
        Option<Uuid>,
        Option<bool>,
        bool,
        Option<String>,
        bool,
    );

    let query = "SELECT d.id, d.name, d.registrar_type, d.ssl_mode, d.dnssec_enabled, d.cloudflare_zone_id, d.expires_at, o.name, \
         d.registrar_type, d.registrar_credential_id, d.ns_ok, \
         EXISTS(SELECT 1 FROM webspace_host_domains whd WHERE whd.domain_id = d.id) AS has_webspace, \
         d.ai_bots_protection, \
         (d.expires_at IS NOT NULL AND d.expires_at < now() + interval '30 days') AS expires_soon \
         FROM domains d JOIN organizations o ON o.id = d.organization_id";

    let rows = match principal.read_filter() {
        None => {
            sqlx::query_as::<_, Row>(&format!("{query} ORDER BY d.name"))
                .fetch_all(pool)
                .await
        }
        Some(org_ids) => {
            sqlx::query_as::<_, Row>(&format!(
                "{query} WHERE d.organization_id = ANY($1) ORDER BY d.name"
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
            |(
                id,
                name,
                registrar_type,
                ssl_mode,
                dnssec_enabled,
                cloudflare_zone_id,
                expires_at,
                organization_name,
                reg_type,
                reg_cred_id,
                ns_ok,
                has_webspace,
                ai_bots_protection,
                expires_soon,
            )| DomainRow {
                id,
                name,
                registrar_type,
                ssl_mode,
                dnssec_enabled,
                cloudflare_zone_id,
                expires_at: expires_at.map(|d| d.format("%Y-%m-%d").to_string()),
                organization_name,
                can_set_nameservers: reg_type.as_deref() == Some("spaceship")
                    && reg_cred_id.is_some(),
                ns_ok,
                has_webspace,
                ai_bots_protection,
                expires_soon,
            },
        )
        .collect())
}

/// Get a domain (requires org read): full detail incl. subdomains, DNS records
/// and live Cloudflare zone/DNSSEC info.
#[api_mcp_dioxus_server(server = "get_domain")]
pub async fn domain_get(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: DomainGetInput,
) -> Result<DomainData, ApiError> {
    let row = sqlx::query_as::<_, (Uuid, String, Option<String>, Option<Uuid>, String, bool, Option<String>, Option<Uuid>, Option<chrono::DateTime<chrono::Utc>>, Option<chrono::DateTime<chrono::Utc>>, Uuid, Option<String>)>(
        "SELECT d.id, d.name, d.registrar_type, d.registrar_credential_id, d.ssl_mode, d.dnssec_enabled, d.cloudflare_zone_id, \
         d.cloudflare_credential_id, d.registered_at, d.expires_at, d.organization_id, d.ai_bots_protection \
         FROM domains d WHERE d.id = $1",
    )
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?
    .ok_or_else(|| ApiError::not_found("domain not found"))?;

    let (
        id,
        name,
        registrar_type,
        registrar_credential_id,
        ssl_mode,
        dnssec_enabled,
        cloudflare_zone_id,
        cf_cred_id,
        registered_at,
        expires_at,
        org_id,
        ai_bots_protection,
    ) = row;

    principal.require_read(&org_id)?;

    let org_name = sqlx::query_scalar::<_, String>("SELECT name FROM organizations WHERE id = $1")
        .bind(org_id)
        .fetch_one(pool)
        .await
        .map_err(super::internal)?;

    // Fetch live zone info + DNSSEC details
    let mut cf_status = None;
    let mut cf_nameservers = Vec::new();
    let mut dnssec_ds = None;
    let mut dnssec_key_tag = None;
    let mut dnssec_algorithm = None;
    let mut dnssec_digest_type = None;
    let mut dnssec_digest = None;
    if let (Some(zone_id), Some(cred_id)) = (&cloudflare_zone_id, cf_cred_id) {
        if let Ok(client) = build_cf_client(pool, cred_id).await {
            if let Ok(zone) = client.get_zone(zone_id).await {
                cf_status = Some(zone.status);
                cf_nameservers = zone.name_servers.unwrap_or_default();
            }
            if let Ok(dnssec) = client.get_dnssec(zone_id).await {
                if dnssec.status.as_deref() == Some("active") {
                    dnssec_ds = dnssec.ds;
                    dnssec_key_tag = dnssec.key_tag.map(|v| v.to_string());
                    dnssec_algorithm = dnssec.algorithm;
                    dnssec_digest_type = dnssec.digest_type;
                    dnssec_digest = dnssec.digest;
                }
            }
        }
    }

    // Load subdomains with their records
    let sub_rows = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, name FROM subdomains WHERE domain_id = $1 ORDER BY name",
    )
    .bind(input.id)
    .fetch_all(pool)
    .await
    .map_err(super::internal)?;

    let mut subdomains = Vec::new();
    for (sub_id, sub_name) in sub_rows {
        let records =
            sqlx::query_as::<_, (Uuid, String, String, Option<i32>, bool, Option<String>)>(
                "SELECT id, record_type, record_value, ttl, proxied, cloudflare_record_id \
             FROM dns_records WHERE subdomain_id = $1 ORDER BY record_type, record_value",
            )
            .bind(sub_id)
            .fetch_all(pool)
            .await
            .map_err(super::internal)?;

        subdomains.push(SubdomainData {
            id: sub_id,
            name: sub_name,
            records: records
                .into_iter()
                .map(
                    |(id, record_type, record_value, ttl, proxied, cloudflare_record_id)| {
                        DnsRecordRow {
                            id,
                            record_type,
                            record_value,
                            ttl,
                            proxied,
                            cloudflare_record_id,
                        }
                    },
                )
                .collect(),
        });
    }

    let can_set_ns =
        registrar_type.as_deref() == Some("spaceship") && registrar_credential_id.is_some();

    Ok(DomainData {
        id,
        name,
        registrar_type,
        registrar_credential_id,
        ssl_mode,
        dnssec_enabled,
        cloudflare_zone_id,
        cloudflare_credential_id: cf_cred_id,
        cloudflare_zone_status: cf_status,
        cloudflare_nameservers: cf_nameservers,
        registered_at: registered_at.map(|d| d.format("%Y-%m-%d").to_string()),
        expires_at: expires_at.map(|d| d.format("%Y-%m-%d").to_string()),
        organization_id: org_id,
        organization_name: org_name,
        subdomains,
        can_set_nameservers: can_set_ns,
        ai_bots_protection,
        dnssec_ds,
        dnssec_key_tag,
        dnssec_algorithm,
        dnssec_digest_type,
        dnssec_digest,
    })
}

/// Add a domain (requires org write): creates a Cloudflare zone (or finds an
/// existing one) when a Cloudflare credential is provided.
#[api_mcp_dioxus_server(server = "add_domain")]
pub async fn domain_create(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: DomainCreateInput,
) -> Result<Uuid, ApiError> {
    principal.require_write(&input.org_id)?;

    let domain_name = &input.domain_name;
    let mut zone_id: Option<String> = None;

    // If a Cloudflare credential is provided, try to add/find the zone
    if let Some(cred_id) = input.cf_credential_id {
        let (client, account_id) = crate::credentials::cf_client_with_account(pool, cred_id)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))?;

        // Check if zone already exists
        let existing = client
            .list_zones(Some(domain_name))
            .await
            .map_err(|e| ApiError::internal(format!("CF API error: {e}")))?;

        if let Some(zone) = existing.first() {
            zone_id = Some(zone.id.clone());
            tracing::info!("domain {domain_name} already exists as zone {}", zone.id);
        } else {
            let zone = client
                .create_zone(domain_name, &account_id)
                .await
                .map_err(|e| ApiError::internal(format!("CF zone creation failed: {e}")))?;
            zone_id = Some(zone.id);
            tracing::info!("created CF zone for {domain_name}");
        }
    }

    let registrar = if input.registrar_type.is_empty() {
        None
    } else {
        Some(input.registrar_type.as_str())
    };

    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO domains (organization_id, name, registrar_type, cloudflare_credential_id, cloudflare_zone_id) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(input.org_id)
    .bind(domain_name)
    .bind(registrar)
    .bind(input.cf_credential_id)
    .bind(&zone_id)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::internal(format!("failed to add domain: {e}")))?;

    Ok(id)
}

/// Update a domain's Cloudflare-backed settings (requires org write). Each
/// provided field is applied with its Cloudflare call + DB update, fail-fast,
/// in the order ssl_mode → dnssec_enabled → ai_bots_protection.
#[api_mcp_dioxus_server(server = "update_domain")]
pub async fn domain_update(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: DomainUpdateInput,
) -> Result<(), ApiError> {
    let (zone_id, cred_id, org_id) = sqlx::query_as::<_, (Option<String>, Option<Uuid>, Uuid)>(
        "SELECT cloudflare_zone_id, cloudflare_credential_id, organization_id FROM domains WHERE id = $1",
    )
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?
    .ok_or_else(|| ApiError::not_found("domain not found"))?;

    principal.require_write(&org_id)?;

    if let Some(ssl_mode) = &input.ssl_mode {
        if let (Some(zone_id), Some(cred_id)) = (&zone_id, cred_id) {
            let client = build_cf_client(pool, cred_id).await?;
            client
                .set_ssl_mode(zone_id, "custom")
                .await
                .map_err(|e| ApiError::internal(format!("CF SSL: {e}")))?;
        }
        sqlx::query("UPDATE domains SET ssl_mode = $1, updated_at = now() WHERE id = $2")
            .bind(ssl_mode)
            .bind(input.id)
            .execute(pool)
            .await
            .map_err(super::internal)?;
    }

    if let Some(enable) = input.dnssec_enabled {
        if let (Some(zone_id), Some(cred_id)) = (&zone_id, cred_id) {
            let client = build_cf_client(pool, cred_id).await?;
            client
                .set_dnssec(zone_id, if enable { "active" } else { "disabled" })
                .await
                .map_err(|e| ApiError::internal(format!("CF DNSSEC: {e}")))?;
        }
        sqlx::query("UPDATE domains SET dnssec_enabled = $1, updated_at = now() WHERE id = $2")
            .bind(enable)
            .bind(input.id)
            .execute(pool)
            .await
            .map_err(super::internal)?;
    }

    if let Some(value) = &input.ai_bots_protection {
        if let (Some(zone_id), Some(cred_id)) = (&zone_id, cred_id) {
            let client = build_cf_client(pool, cred_id).await?;
            client
                .set_bot_management(zone_id, &serde_json::json!({"ai_bots_protection": value}))
                .await
                .map_err(|e| ApiError::internal(format!("CF bot management: {e}")))?;
        }
        sqlx::query("UPDATE domains SET ai_bots_protection = $1, updated_at = now() WHERE id = $2")
            .bind(value)
            .bind(input.id)
            .execute(pool)
            .await
            .map_err(super::internal)?;
    }

    Ok(())
}

/// Delete a domain and all its records (requires org write).
#[api_mcp_dioxus_server(server = "delete_domain")]
pub async fn domain_delete(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: DomainDeleteInput,
) -> Result<(), ApiError> {
    let org_id = super::owning_org(pool, "domains", input.id, "domain").await?;
    principal.require_write(&org_id)?;

    sqlx::query("DELETE FROM domains WHERE id = $1")
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(super::internal)?;
    Ok(())
}

/// Move a domain to another organization (requires write on both orgs).
#[api_mcp_dioxus_server(server = "move_domain")]
pub async fn domain_move(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: DomainMoveInput,
) -> Result<(), ApiError> {
    let org_id = super::owning_org(pool, "domains", input.id, "domain").await?;
    principal.require_write(&org_id)?;
    principal.require_write(&input.target_org_id)?;

    if org_id == input.target_org_id {
        return Err(ApiError::bad_request(
            "domain is already in that organization",
        ));
    }

    // Check for name conflicts in target org
    let conflict = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM domains WHERE organization_id = $1 AND name = (SELECT name FROM domains WHERE id = $2))",
    )
    .bind(input.target_org_id)
    .bind(input.id)
    .fetch_one(pool)
    .await
    .map_err(super::internal)?;
    if conflict {
        return Err(ApiError::conflict(
            "a domain with the same name already exists in the target organization",
        ));
    }

    sqlx::query("UPDATE domains SET organization_id = $1 WHERE id = $2")
        .bind(input.target_org_id)
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(super::internal)?;

    Ok(())
}

/// Deploy a domain to Cloudflare (requires org write): finds or creates the
/// zone, records it, and best-effort sets nameservers at the registrar.
#[api_mcp_dioxus_server(server = "deploy_to_cloudflare")]
pub async fn domain_deploy_cloudflare(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: DomainDeployCloudflareInput,
) -> Result<DeployResult, ApiError> {
    let (domain_name, org_id) = sqlx::query_as::<_, (String, Uuid)>(
        "SELECT name, organization_id FROM domains WHERE id = $1",
    )
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?
    .ok_or_else(|| ApiError::not_found("domain not found"))?;

    principal.require_write(&org_id)?;

    let (client, account_id) =
        crate::credentials::cf_client_with_account(pool, input.credential_id)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))?;

    let zone = deploy_zone(
        &client,
        &account_id,
        pool,
        input.id,
        &domain_name,
        input.credential_id,
    )
    .await
    .map_err(super::internal)?;

    let zone_id = zone.id;
    let status = zone.status;
    let nameservers = zone.name_servers.unwrap_or_default();

    let mut ns_set = false;
    if !nameservers.is_empty() {
        if let Ok(true) =
            try_set_registrar_nameservers(pool, input.id, &domain_name, &nameservers).await
        {
            ns_set = true;
        }
    }

    Ok(DeployResult {
        zone_id,
        status,
        nameservers,
        nameservers_set_at_registrar: ns_set,
    })
}

/// Set the given nameservers at the domain's registrar (requires org write).
#[api_mcp_dioxus_server(server = "set_nameservers_at_registrar")]
pub async fn domain_set_nameservers(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: DomainSetNameserversInput,
) -> Result<String, ApiError> {
    let (domain_name, org_id) = sqlx::query_as::<_, (String, Uuid)>(
        "SELECT name, organization_id FROM domains WHERE id = $1",
    )
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?
    .ok_or_else(|| ApiError::not_found("domain not found"))?;

    principal.require_write(&org_id)?;

    match try_set_registrar_nameservers(pool, input.id, &domain_name, &input.nameservers).await? {
        true => Ok("Nameservers updated at registrar".into()),
        false => Err(ApiError::bad_request(
            "No registrar credential or unsupported registrar",
        )),
    }
}

/// Sync DNS records from Cloudflare into the local database (requires org
/// write). Replaces all local records for the domain with the zone's records.
#[api_mcp_dioxus_server(server = "sync_records_from_cloudflare")]
pub async fn domain_sync_records(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: DomainSyncRecordsInput,
) -> Result<String, ApiError> {
    let (domain_name, org_id, zone_id, cred_id) =
        sqlx::query_as::<_, (String, Uuid, Option<String>, Option<Uuid>)>(
            "SELECT name, organization_id, cloudflare_zone_id, cloudflare_credential_id \
             FROM domains WHERE id = $1",
        )
        .bind(input.id)
        .fetch_optional(pool)
        .await
        .map_err(super::internal)?
        .ok_or_else(|| ApiError::not_found("domain not found"))?;

    principal.require_write(&org_id)?;

    let (zone_id, cred_id) = match (zone_id, cred_id) {
        (Some(z), Some(c)) => (z, c),
        _ => return Err(ApiError::bad_request("domain not deployed to Cloudflare")),
    };

    let client = build_cf_client(pool, cred_id).await?;
    let cf_records = client
        .list_dns_records(&zone_id)
        .await
        .map_err(|e| ApiError::internal(format!("CF API: {e}")))?;

    // Delete existing local records for this domain and re-import from CF
    sqlx::query("DELETE FROM dns_records WHERE domain_id = $1")
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(super::internal)?;

    let mut synced = 0usize;
    for rec in &cf_records {
        // Determine subdomain name from the record FQDN
        let sub_name = if rec.name == domain_name {
            "@".to_string()
        } else if let Some(stripped) = rec.name.strip_suffix(&format!(".{domain_name}")) {
            stripped.to_string()
        } else {
            rec.name.clone()
        };

        // Ensure subdomain entity exists
        let sub_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO subdomains (domain_id, name) VALUES ($1, $2) \
             ON CONFLICT (domain_id, name) DO UPDATE SET updated_at = now() RETURNING id",
        )
        .bind(input.id)
        .bind(&sub_name)
        .fetch_one(pool)
        .await
        .map_err(super::internal)?;

        let content = rec.content.as_deref().unwrap_or("");
        let proxied = rec.proxied.unwrap_or(false);

        sqlx::query(
            "INSERT INTO dns_records (subdomain_id, domain_id, name, record_type, record_value, proxied, cloudflare_record_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(sub_id)
        .bind(input.id)
        .bind(&sub_name)
        .bind(&rec.record_type)
        .bind(content)
        .bind(proxied)
        .bind(&rec.id)
        .execute(pool)
        .await
        .map_err(super::internal)?;

        synced += 1;
    }

    Ok(format!("Synced {synced} records from Cloudflare"))
}

/// Create a subdomain entity (e.g. "www", "api", "@") on a domain (requires
/// org write). Upserts on (domain, name).
#[api_mcp_dioxus_server(server = "create_subdomain")]
pub async fn subdomain_create(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: SubdomainCreateInput,
) -> Result<Uuid, ApiError> {
    let org_id = super::owning_org(pool, "domains", input.domain_id, "domain").await?;
    principal.require_write(&org_id)?;

    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO subdomains (domain_id, name) VALUES ($1, $2) \
         ON CONFLICT (domain_id, name) DO UPDATE SET updated_at = now() RETURNING id",
    )
    .bind(input.domain_id)
    .bind(&input.name)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::internal(format!("failed to create subdomain: {e}")))?;

    Ok(id)
}

/// Delete a subdomain and all its records (requires org write). Best-effort
/// deletes the corresponding Cloudflare records first.
#[api_mcp_dioxus_server(server = "delete_subdomain")]
pub async fn subdomain_delete(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: SubdomainDeleteInput,
) -> Result<(), ApiError> {
    let org_id = super::owning_org(pool, "domains", input.domain_id, "domain").await?;
    principal.require_write(&org_id)?;

    // Delete CF records first
    let records = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT cloudflare_record_id FROM dns_records WHERE subdomain_id = $1",
    )
    .bind(input.id)
    .fetch_all(pool)
    .await
    .map_err(super::internal)?;

    let cf = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
        "SELECT cloudflare_zone_id, cloudflare_credential_id FROM domains WHERE id = $1",
    )
    .bind(input.domain_id)
    .fetch_one(pool)
    .await
    .map_err(super::internal)?;

    if let (Some(zone_id), Some(cred_id)) = cf {
        if let Ok(client) = build_cf_client(pool, cred_id).await {
            for (cf_id,) in &records {
                if let Some(cf_id) = cf_id {
                    let _ = client.delete_dns_record(&zone_id, cf_id).await;
                }
            }
        }
    }

    sqlx::query("DELETE FROM subdomains WHERE id = $1 AND domain_id = $2")
        .bind(input.id)
        .bind(input.domain_id)
        .execute(pool)
        .await
        .map_err(super::internal)?;
    Ok(())
}

/// Add a DNS record to a subdomain (requires org write); also creates it in
/// Cloudflare when the domain is deployed.
#[api_mcp_dioxus_server(server = "add_dns_record")]
pub async fn dns_record_create(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: DnsRecordCreateInput,
) -> Result<(), ApiError> {
    let org_id = super::owning_org(pool, "domains", input.domain_id, "domain").await?;
    principal.require_write(&org_id)?;

    let sub_name = sqlx::query_scalar::<_, String>("SELECT name FROM subdomains WHERE id = $1")
        .bind(input.subdomain_id)
        .fetch_optional(pool)
        .await
        .map_err(super::internal)?
        .ok_or_else(|| ApiError::not_found("subdomain not found"))?;

    let mut cf_record_id: Option<String> = None;

    let cf = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
        "SELECT cloudflare_zone_id, cloudflare_credential_id FROM domains WHERE id = $1",
    )
    .bind(input.domain_id)
    .fetch_one(pool)
    .await
    .map_err(super::internal)?;

    if let (Some(zone_id), Some(cred_id)) = cf {
        let client = build_cf_client(pool, cred_id).await?;
        let domain_name = sqlx::query_scalar::<_, String>("SELECT name FROM domains WHERE id = $1")
            .bind(input.domain_id)
            .fetch_one(pool)
            .await
            .map_err(super::internal)?;

        let fqdn = if sub_name == "@" {
            domain_name
        } else {
            format!("{sub_name}.{domain_name}")
        };
        let record = cloudflare_api::compat::CreateDnsRecord {
            record_type: input.record_type.clone(),
            name: fqdn,
            content: Some(input.record_value.clone()),
            data: None,
            ttl: Some(1),
            proxied: Some(input.proxied),
            comment: None,
            priority: None,
        };
        let created = client
            .create_dns_record(&zone_id, &record)
            .await
            .map_err(|e| ApiError::internal(format!("CF DNS: {e}")))?;
        cf_record_id = Some(created.id);
    }

    sqlx::query(
        "INSERT INTO dns_records (subdomain_id, domain_id, name, record_type, record_value, proxied, cloudflare_record_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(input.subdomain_id)
    .bind(input.domain_id)
    .bind(&sub_name)
    .bind(&input.record_type)
    .bind(&input.record_value)
    .bind(input.proxied)
    .bind(&cf_record_id)
    .execute(pool)
    .await
    .map_err(super::internal)?;
    Ok(())
}

/// Delete a DNS record (requires org write); best-effort deletes it in
/// Cloudflare too.
#[api_mcp_dioxus_server(server = "delete_dns_record")]
pub async fn dns_record_delete(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: DnsRecordDeleteInput,
) -> Result<(), ApiError> {
    let org_id = super::owning_org(pool, "domains", input.domain_id, "domain").await?;
    principal.require_write(&org_id)?;

    let cf_id = sqlx::query_scalar::<_, Option<String>>(
        "SELECT cloudflare_record_id FROM dns_records WHERE id = $1",
    )
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?
    .flatten();

    if let Some(cf_id) = cf_id {
        let cf = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
            "SELECT cloudflare_zone_id, cloudflare_credential_id FROM domains WHERE id = $1",
        )
        .bind(input.domain_id)
        .fetch_one(pool)
        .await
        .map_err(super::internal)?;

        if let (Some(zone_id), Some(cred_id)) = cf {
            if let Ok(client) = build_cf_client(pool, cred_id).await {
                let _ = client.delete_dns_record(&zone_id, &cf_id).await;
            }
        }
    }

    sqlx::query("DELETE FROM dns_records WHERE id = $1")
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(super::internal)?;
    Ok(())
}

// ── Bulk operations (admin only) ──────────────────────────────────────

/// Deploy multiple domains to Cloudflare under one credential (admin only).
/// Unlike the single-domain deploy, this does NOT touch registrar nameservers.
#[api_mcp_dioxus_server(server = "bulk_deploy_to_cloudflare")]
pub async fn domain_bulk_deploy_cloudflare(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: DomainBulkDeployCloudflareInput,
) -> Result<BulkOpResult, ApiError> {
    principal.require_admin()?;

    let (client, account_id) =
        crate::credentials::cf_client_with_account(pool, input.credential_id)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))?;

    let mut succeeded = Vec::new();
    let mut failed = Vec::new();

    for domain_id in &input.domain_ids {
        let row = sqlx::query_as::<_, (String,)>("SELECT name FROM domains WHERE id = $1")
            .bind(domain_id)
            .fetch_optional(pool)
            .await
            .map_err(super::internal)?;
        let Some((domain_name,)) = row else { continue };

        match deploy_zone(
            &client,
            &account_id,
            pool,
            *domain_id,
            &domain_name,
            input.credential_id,
        )
        .await
        {
            Ok(_) => succeeded.push(domain_name),
            Err(e) => failed.push(BulkFailure {
                name: domain_name,
                error: e.to_string(),
            }),
        }
    }

    Ok(BulkOpResult { succeeded, failed })
}

/// Set Cloudflare nameservers at the registrar for domains that have a CF
/// zone (admin only). Domains without a zone are skipped.
#[api_mcp_dioxus_server(server = "bulk_set_nameservers")]
pub async fn domain_bulk_set_nameservers(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: DomainBulkSetNameserversInput,
) -> Result<BulkOpResult, ApiError> {
    principal.require_admin()?;

    let mut succeeded = Vec::new();
    let mut failed = Vec::new();

    for domain_id in &input.domain_ids {
        let row = sqlx::query_as::<_, (String, Option<String>, Option<Uuid>)>(
            "SELECT name, cloudflare_zone_id, cloudflare_credential_id FROM domains WHERE id = $1",
        )
        .bind(domain_id)
        .fetch_optional(pool)
        .await
        .map_err(super::internal)?;

        let Some((domain_name, Some(zone_id), Some(cred_id))) = row else {
            continue; // skip domains without CF zone
        };

        match set_ns_single(pool, *domain_id, &domain_name, &zone_id, cred_id).await {
            Ok(()) => succeeded.push(domain_name),
            Err(e) => failed.push(BulkFailure {
                name: domain_name,
                error: e.to_string(),
            }),
        }
    }

    Ok(BulkOpResult { succeeded, failed })
}

/// Bulk set SSL mode for domains with CF zones (admin only).
#[api_mcp_dioxus_server(server = "bulk_set_ssl_mode")]
pub async fn domain_bulk_set_ssl_mode(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: DomainBulkSetSslModeInput,
) -> Result<BulkOpResult, ApiError> {
    principal.require_admin()?;

    let mut succeeded = Vec::new();
    let mut failed = Vec::new();

    for domain_id in &input.domain_ids {
        let row = sqlx::query_as::<_, (String, Option<String>, Option<Uuid>)>(
            "SELECT name, cloudflare_zone_id, cloudflare_credential_id FROM domains WHERE id = $1",
        )
        .bind(domain_id)
        .fetch_optional(pool)
        .await
        .map_err(super::internal)?;

        let Some((domain_name, Some(zone_id), Some(cred_id))) = row else {
            continue;
        };

        match async {
            let client = build_cf_client(pool, cred_id).await?;
            client
                .set_ssl_mode(&zone_id, &input.ssl_mode)
                .await
                .map_err(super::internal)?;
            sqlx::query("UPDATE domains SET ssl_mode = $1, updated_at = now() WHERE id = $2")
                .bind(&input.ssl_mode)
                .bind(domain_id)
                .execute(pool)
                .await
                .map_err(super::internal)?;
            Ok::<_, ApiError>(())
        }
        .await
        {
            Ok(()) => succeeded.push(domain_name),
            Err(e) => failed.push(BulkFailure {
                name: domain_name,
                error: e.to_string(),
            }),
        }
    }
    Ok(BulkOpResult { succeeded, failed })
}

/// Bulk set AI bot protection for domains with CF zones (admin only).
#[api_mcp_dioxus_server(server = "bulk_set_ai_bots_protection")]
pub async fn domain_bulk_set_ai_bots(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: DomainBulkSetAiBotsInput,
) -> Result<BulkOpResult, ApiError> {
    principal.require_admin()?;

    let mut succeeded = Vec::new();
    let mut failed = Vec::new();

    for domain_id in &input.domain_ids {
        let row = sqlx::query_as::<_, (String, Option<String>, Option<Uuid>)>(
            "SELECT name, cloudflare_zone_id, cloudflare_credential_id FROM domains WHERE id = $1",
        )
        .bind(domain_id)
        .fetch_optional(pool)
        .await
        .map_err(super::internal)?;

        let Some((domain_name, Some(zone_id), Some(cred_id))) = row else {
            continue;
        };

        match async {
            let client = build_cf_client(pool, cred_id).await?;
            client
                .set_bot_management(
                    &zone_id,
                    &serde_json::json!({"ai_bots_protection": input.value}),
                )
                .await
                .map_err(super::internal)?;
            sqlx::query(
                "UPDATE domains SET ai_bots_protection = $1, updated_at = now() WHERE id = $2",
            )
            .bind(&input.value)
            .bind(domain_id)
            .execute(pool)
            .await
            .map_err(super::internal)?;
            Ok::<_, ApiError>(())
        }
        .await
        {
            Ok(()) => succeeded.push(domain_name),
            Err(e) => failed.push(BulkFailure {
                name: domain_name,
                error: e.to_string(),
            }),
        }
    }
    Ok(BulkOpResult { succeeded, failed })
}

/// Bulk create CF Pages direct-upload projects for domains without webspaces
/// (admin only). Creates a cloudflare host + webspace per domain and links it.
#[api_mcp_dioxus_server(server = "bulk_create_pages_project")]
pub async fn domain_bulk_create_pages(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: DomainBulkCreatePagesInput,
) -> Result<BulkOpResult, ApiError> {
    principal.require_admin()?;

    let (client, account_id) =
        crate::credentials::cf_client_with_account(pool, input.credential_id)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))?;

    let mut succeeded = Vec::new();
    let mut failed = Vec::new();

    for domain_id in &input.domain_ids {
        let row = sqlx::query_as::<_, (String, Uuid)>(
            "SELECT name, organization_id FROM domains WHERE id = $1",
        )
        .bind(domain_id)
        .fetch_optional(pool)
        .await
        .map_err(super::internal)?;

        let Some((domain_name, org_id)) = row else {
            continue;
        };

        // Project name: replace dots with hyphens (CF Pages doesn't allow dots)
        let project_name = domain_name.replace('.', "-");

        match async {
            let project = client
                .create_pages_project(&account_id, &project_name, "main")
                .await
                .map_err(super::internal)?;
            let project_id = project.id.clone();

            // Create a cloudflare host with its single main-folder (the Pages project).
            let host_id = sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO webspace_hosts (organization_id, name, kind) VALUES ($1, $2, 'cloudflare') RETURNING id",
            )
            .bind(org_id)
            .bind(&project_name)
            .fetch_one(pool)
            .await
            .map_err(super::internal)?;

            sqlx::query(
                "INSERT INTO webspaces (organization_id, webspace_host_id, name, path_prefix, hosting_type, cloudflare_pages_project, cloudflare_pages_project_id, cloudflare_credential_id) \
                 VALUES ($1, $2, $3, '/', 'cloudflare_pages', $4, $5, $6)",
            )
            .bind(org_id)
            .bind(host_id)
            .bind(&project_name)
            .bind(&project_name)
            .bind(&project_id)
            .bind(input.credential_id)
            .execute(pool)
            .await
            .map_err(super::internal)?;

            // Link domain to the host.
            sqlx::query(
                "INSERT INTO webspace_host_domains (webspace_host_id, domain_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(host_id)
            .bind(domain_id)
            .execute(pool)
            .await
            .map_err(super::internal)?;

            Ok::<_, ApiError>(())
        }
        .await
        {
            Ok(()) => succeeded.push(domain_name),
            Err(e) => failed.push(BulkFailure {
                name: domain_name,
                error: e.to_string(),
            }),
        }
    }
    if !succeeded.is_empty() {
        crate::api::internal::notify_proxy_reload();
    }
    Ok(BulkOpResult { succeeded, failed })
}

// ── Availability / discovery / import ─────────────────────────────────

/// Check a domain's availability and registration pricing via a registrar
/// credential (requires credential read: credential's org, or admin for
/// global credentials).
#[api_mcp_dioxus_server(server = "check_domain_availability")]
pub async fn domain_check_availability(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: DomainCheckAvailabilityInput,
) -> Result<AvailabilityResult, ApiError> {
    super::credentials::require_credential_read(pool, principal, input.credential_id).await?;

    let cred_type =
        sqlx::query_scalar::<_, String>("SELECT credential_type FROM credentials WHERE id = $1")
            .bind(input.credential_id)
            .fetch_optional(pool)
            .await
            .map_err(super::internal)?
            .ok_or_else(|| ApiError::not_found("credential not found"))?;

    match cred_type.as_str() {
        "cloudflare" => {
            let (client, account_id) =
                crate::credentials::cf_client_with_account(pool, input.credential_id)
                    .await
                    .map_err(|e| ApiError::internal(format!("{e}")))?;
            let results = client
                .check_domains(&account_id, &[input.domain.clone()])
                .await
                .map_err(|e| ApiError::internal(format!("CF API: {e}")))?;
            let r = results
                .into_iter()
                .next()
                .ok_or_else(|| ApiError::internal("no result"))?;
            Ok(AvailabilityResult {
                domain: r.name,
                available: r.registrable,
                price: r.pricing.as_ref().and_then(|p| p.registration_cost.clone()),
                currency: r.pricing.map(|p| p.currency),
                provider: "cloudflare".into(),
            })
        }
        "spaceship" => {
            let client = crate::credentials::spaceship_client(pool, input.credential_id)
                .await
                .map_err(|e| ApiError::internal(format!("{e}")))?;
            let r = client
                .check_availability(&input.domain)
                .await
                .map_err(|e| ApiError::internal(format!("Spaceship API: {e}")))?;
            let available = r.status.as_deref() == Some("available");
            let reg_price = r.premium_pricing.iter().find(|p| p.operation == "register");
            Ok(AvailabilityResult {
                domain: r.domain.unwrap_or(input.domain),
                available,
                price: reg_price.map(|p| format!("{:.2}", p.price)),
                currency: reg_price.map(|p| p.currency.clone()),
                provider: "spaceship".into(),
            })
        }
        _ => Err(ApiError::bad_request("unsupported registrar")),
    }
}

/// Discover importable domains on a credential (Cloudflare zones or Spaceship
/// domains), flagging ones already present in the organization (requires org
/// read).
#[api_mcp_dioxus_server(server = "discover_domains")]
pub async fn domain_discover(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: DomainDiscoverInput,
) -> Result<Vec<DiscoveredDomain>, ApiError> {
    principal.require_read(&input.organization_id)?;

    let cred_type =
        sqlx::query_scalar::<_, String>("SELECT credential_type FROM credentials WHERE id = $1")
            .bind(input.credential_id)
            .fetch_optional(pool)
            .await
            .map_err(super::internal)?
            .ok_or_else(|| ApiError::not_found("credential not found"))?;

    // Load existing domains in this org for deduplication
    let existing: Vec<String> =
        sqlx::query_scalar("SELECT name FROM domains WHERE organization_id = $1")
            .bind(input.organization_id)
            .fetch_all(pool)
            .await
            .map_err(super::internal)?;

    let mut discovered = Vec::new();

    match cred_type.as_str() {
        "cloudflare" => {
            let client = build_cf_client(pool, input.credential_id).await?;
            let zones = client
                .list_zones(None)
                .await
                .map_err(|e| ApiError::internal(format!("Cloudflare API error: {e}")))?;

            for zone in zones {
                discovered.push(DiscoveredDomain {
                    already_imported: existing.contains(&zone.name),
                    name: zone.name,
                    zone_id: Some(zone.id),
                    status: Some(zone.status),
                    expires_at: None,
                });
            }
        }
        "spaceship" => {
            let client = crate::credentials::spaceship_client(pool, input.credential_id)
                .await
                .map_err(|e| ApiError::internal(format!("{e}")))?;
            let domains = client
                .list_all_domains()
                .await
                .map_err(|e| ApiError::internal(format!("Spaceship API error: {e}")))?;

            for domain in domains {
                discovered.push(DiscoveredDomain {
                    already_imported: existing.contains(&domain.name),
                    name: domain.name,
                    zone_id: None,
                    status: domain.lifecycle_status,
                    expires_at: domain.expiration_date,
                });
            }
        }
        _ => return Err(ApiError::bad_request("unsupported credential type")),
    }

    Ok(discovered)
}

/// Import domains from a credential into an organization (requires org
/// write). Already-present names are skipped; per-domain insert failures are
/// collected in `errors`.
#[api_mcp_dioxus_server(server = "import_domains")]
pub async fn domain_import(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: DomainImportInput,
) -> Result<ImportResult, ApiError> {
    principal.require_write(&input.organization_id)?;

    let cred_type =
        sqlx::query_scalar::<_, String>("SELECT credential_type FROM credentials WHERE id = $1")
            .bind(input.credential_id)
            .fetch_optional(pool)
            .await
            .map_err(super::internal)?
            .ok_or_else(|| ApiError::not_found("credential not found"))?;

    // Build a map of name -> zone_id for Cloudflare
    let mut zone_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let registrar_type = cred_type.as_str();

    if registrar_type == "cloudflare" {
        let client = build_cf_client(pool, input.credential_id).await?;
        let zones = client
            .list_zones(None)
            .await
            .map_err(|e| ApiError::internal(format!("Cloudflare API error: {e}")))?;
        for zone in zones {
            zone_map.insert(zone.name.clone(), zone.id);
        }
    }

    let mut imported = 0u32;
    let mut skipped = 0u32;
    let mut errors = Vec::new();

    for domain_name in &input.domain_names {
        // Skip if already exists
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM domains WHERE organization_id = $1 AND name = $2)",
        )
        .bind(input.organization_id)
        .bind(domain_name)
        .fetch_one(pool)
        .await
        .map_err(super::internal)?;

        if exists {
            skipped += 1;
            continue;
        }

        let zone_id = zone_map.get(domain_name).cloned();
        let cf_cred = if registrar_type == "cloudflare" {
            Some(input.credential_id)
        } else {
            None
        };
        let reg_cred = match registrar_type {
            "spaceship" => Some(input.credential_id),
            _ => None,
        };

        let result = sqlx::query(
            "INSERT INTO domains (organization_id, name, registrar_type, registrar_credential_id, \
             cloudflare_credential_id, cloudflare_zone_id) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(input.organization_id)
        .bind(domain_name)
        .bind(registrar_type)
        .bind(reg_cred)
        .bind(cf_cred)
        .bind(&zone_id)
        .execute(pool)
        .await;

        match result {
            Ok(_) => imported += 1,
            Err(e) => errors.push(format!("{domain_name}: {e}")),
        }
    }

    Ok(ImportResult {
        imported,
        skipped,
        errors,
    })
}

// ── Helpers (server-only) ─────────────────────────────────────────────

#[cfg(feature = "server")]
async fn build_cf_client(
    pool: &sqlx::PgPool,
    cred_id: Uuid,
) -> Result<cloudflare_api::compat::SimpleClient, ApiError> {
    crate::credentials::cf_client(pool, cred_id)
        .await
        .map_err(|e| ApiError::internal(format!("{e}")))
}

/// Find or create the Cloudflare zone for a domain and record it on the
/// domains row. Shared verbatim by the single and bulk deploy paths (the
/// single path additionally sets nameservers at the registrar afterwards).
#[cfg(feature = "server")]
async fn deploy_zone(
    client: &cloudflare_api::compat::SimpleClient,
    account_id: &str,
    pool: &sqlx::PgPool,
    domain_id: Uuid,
    domain_name: &str,
    credential_id: Uuid,
) -> Result<cloudflare_api::compat::Zone, Box<dyn std::error::Error + Send + Sync>> {
    let existing = client
        .list_zones(Some(domain_name))
        .await
        .map_err(|e| format!("CF API: {e}"))?;
    let zone = if let Some(z) = existing.into_iter().find(|z| z.name == domain_name) {
        z
    } else {
        client
            .create_zone(domain_name, account_id)
            .await
            .map_err(|e| format!("zone creation failed: {e}"))?
    };

    sqlx::query("UPDATE domains SET cloudflare_zone_id = $1, cloudflare_credential_id = $2, updated_at = now() WHERE id = $3")
        .bind(&zone.id)
        .bind(credential_id)
        .bind(domain_id)
        .execute(pool)
        .await?;

    Ok(zone)
}

/// Set nameservers at the domain's registrar if it is a spaceship domain with
/// a credential. `Ok(false)` = no supported registrar/credential (not an error).
#[cfg(feature = "server")]
async fn try_set_registrar_nameservers(
    pool: &sqlx::PgPool,
    domain_id: Uuid,
    domain_name: &str,
    nameservers: &[String],
) -> Result<bool, ApiError> {
    let row = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
        "SELECT registrar_type, registrar_credential_id FROM domains WHERE id = $1",
    )
    .bind(domain_id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?;

    match row {
        Some((Some(ref rt), Some(cred_id))) if rt == "spaceship" => {
            let client = crate::credentials::spaceship_client(pool, cred_id)
                .await
                .map_err(|e| ApiError::internal(format!("{e}")))?;
            client
                .set_nameservers(
                    domain_name,
                    &spaceship_api::compat::NameserverConfig {
                        provider: "custom".into(),
                        hosts: Some(nameservers.to_vec()),
                    },
                )
                .await
                .map_err(|e| ApiError::internal(format!("Spaceship NS: {e}")))?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// Bulk-NS per-domain step: read the zone's nameservers from Cloudflare, then
/// set them at the registrar; errors when the registrar is unsupported.
#[cfg(feature = "server")]
async fn set_ns_single(
    pool: &sqlx::PgPool,
    domain_id: Uuid,
    domain_name: &str,
    zone_id: &str,
    cred_id: Uuid,
) -> Result<(), ApiError> {
    let cf_client = build_cf_client(pool, cred_id).await?;
    let zone = cf_client.get_zone(zone_id).await.map_err(super::internal)?;
    let nameservers = zone.name_servers.unwrap_or_default();
    if nameservers.is_empty() {
        return Err(ApiError::internal("zone has no nameservers"));
    }

    match try_set_registrar_nameservers(pool, domain_id, domain_name, &nameservers).await? {
        true => Ok(()),
        false => Err(ApiError::internal("no supported registrar")),
    }
}
