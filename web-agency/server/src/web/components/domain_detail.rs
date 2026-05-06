use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Badge, BadgeVariant, Button, ButtonVariant, Card, FormField, PageHeader, SectionHeading, Td, Th};

// ── Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DomainData {
    id: Uuid,
    name: String,
    registrar_type: Option<String>,
    registrar_credential_id: Option<Uuid>,
    ssl_mode: String,
    dnssec_enabled: bool,
    cloudflare_zone_id: Option<String>,
    cloudflare_credential_id: Option<Uuid>,
    cloudflare_zone_status: Option<String>,
    cloudflare_nameservers: Vec<String>,
    registered_at: Option<String>,
    expires_at: Option<String>,
    organization_name: String,
    subdomains: Vec<SubdomainData>,
    can_set_nameservers: bool,
    ai_bots_protection: Option<String>,
    dnssec_ds: Option<String>,
    dnssec_key_tag: Option<String>,
    dnssec_algorithm: Option<String>,
    dnssec_digest_type: Option<String>,
    dnssec_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct SubdomainData {
    id: Uuid,
    name: String,
    records: Vec<DnsRecordRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct DnsRecordRow {
    id: Uuid,
    record_type: String,
    record_value: String,
    ttl: Option<i32>,
    proxied: bool,
    cloudflare_record_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CfCredOption { id: Uuid, name: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeployResult {
    zone_id: String,
    status: String,
    nameservers: Vec<String>,
    nameservers_set_at_registrar: bool,
}

// ── Server functions ──────────────────────────────────────────────────

#[server]
async fn get_domain(domain_id: Uuid) -> Result<DomainData, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let row = sqlx::query_as::<_, (Uuid, String, Option<String>, Option<Uuid>, String, bool, Option<String>, Option<Uuid>, Option<chrono::DateTime<chrono::Utc>>, Option<chrono::DateTime<chrono::Utc>>, Uuid, Option<String>)>(
        "SELECT d.id, d.name, d.registrar_type, d.registrar_credential_id, d.ssl_mode, d.dnssec_enabled, d.cloudflare_zone_id, \
         d.cloudflare_credential_id, d.registered_at, d.expires_at, d.organization_id, d.ai_bots_protection \
         FROM domains d WHERE d.id = $1",
    )
    .bind(domain_id).fetch_optional(&pool).await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("domain not found"))?;

    let (id, name, registrar_type, registrar_credential_id, ssl_mode, dnssec_enabled, cloudflare_zone_id, cf_cred_id, registered_at, expires_at, org_id, ai_bots_protection) = row;

    use crate::web::user::WebUserExt;
    user.require_org_read(&org_id)?;

    let org_name = sqlx::query_scalar::<_, String>("SELECT name FROM organizations WHERE id = $1")
        .bind(org_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    // Fetch live zone info + DNSSEC details
    let mut cf_status = None;
    let mut cf_nameservers = Vec::new();
    let mut dnssec_ds = None;
    let mut dnssec_key_tag = None;
    let mut dnssec_algorithm = None;
    let mut dnssec_digest_type = None;
    let mut dnssec_digest = None;
    if let (Some(zone_id), Some(cred_id)) = (&cloudflare_zone_id, cf_cred_id) {
        if let Ok(client) = build_cf_client(&pool, cred_id).await {
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
    ).bind(domain_id).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut subdomains = Vec::new();
    for (sub_id, sub_name) in sub_rows {
        let records = sqlx::query_as::<_, (Uuid, String, String, Option<i32>, bool, Option<String>)>(
            "SELECT id, record_type, record_value, ttl, proxied, cloudflare_record_id \
             FROM dns_records WHERE subdomain_id = $1 ORDER BY record_type, record_value",
        ).bind(sub_id).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

        subdomains.push(SubdomainData {
            id: sub_id,
            name: sub_name,
            records: records.into_iter().map(|(id, record_type, record_value, ttl, proxied, cloudflare_record_id)| {
                DnsRecordRow { id, record_type, record_value, ttl, proxied, cloudflare_record_id }
            }).collect(),
        });
    }

    let can_set_ns = registrar_type.as_deref() == Some("spaceship") && registrar_credential_id.is_some();

    Ok(DomainData {
        id, name, registrar_type, registrar_credential_id, ssl_mode, dnssec_enabled,
        cloudflare_zone_id, cloudflare_credential_id: cf_cred_id,
        cloudflare_zone_status: cf_status, cloudflare_nameservers: cf_nameservers,
        registered_at: registered_at.map(|d| d.format("%Y-%m-%d").to_string()),
        expires_at: expires_at.map(|d| d.format("%Y-%m-%d").to_string()),
        organization_name: org_name, subdomains, can_set_nameservers: can_set_ns,
        ai_bots_protection, dnssec_ds, dnssec_key_tag, dnssec_algorithm, dnssec_digest_type, dnssec_digest,
    })
}

#[server]
async fn list_cf_credentials() -> Result<Vec<CfCredOption>, ServerFnError> {
    let _user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let rows = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, name FROM credentials WHERE credential_type = 'cloudflare' ORDER BY name",
    ).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rows.into_iter().map(|(id, name)| CfCredOption { id, name }).collect())
}

#[server]
async fn deploy_to_cloudflare(domain_id: Uuid, credential_id: Uuid) -> Result<DeployResult, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let (domain_name, org_id) = sqlx::query_as::<_, (String, Uuid)>(
        "SELECT name, organization_id FROM domains WHERE id = $1",
    ).bind(domain_id).fetch_optional(&pool).await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("domain not found"))?;

    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    let client = build_cf_client(&pool, credential_id).await?;
    let account_id = get_cf_account_id(&pool, credential_id).await?;

    let existing = client.list_zones(Some(&domain_name)).await
        .map_err(|e| ServerFnError::new(format!("CF API: {e}")))?;

    let zone = if let Some(z) = existing.into_iter().find(|z| z.name == domain_name) {
        z
    } else {
        client.create_zone(&domain_name, &account_id).await
            .map_err(|e| ServerFnError::new(format!("zone creation failed: {e}")))?
    };

    let zone_id = zone.id.clone();
    let status = zone.status.clone();
    let nameservers = zone.name_servers.clone().unwrap_or_default();

    sqlx::query("UPDATE domains SET cloudflare_zone_id = $1, cloudflare_credential_id = $2, updated_at = now() WHERE id = $3")
        .bind(&zone_id).bind(credential_id).bind(domain_id)
        .execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut ns_set = false;
    if !nameservers.is_empty() {
        if let Ok(true) = try_set_registrar_nameservers(&pool, domain_id, &domain_name, &nameservers).await {
            ns_set = true;
        }
    }

    Ok(DeployResult { zone_id, status, nameservers, nameservers_set_at_registrar: ns_set })
}

#[server]
async fn update_ssl_mode(domain_id: Uuid, ssl_mode: String) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let (zone_id, cred_id, org_id) = sqlx::query_as::<_, (Option<String>, Option<Uuid>, Uuid)>(
        "SELECT cloudflare_zone_id, cloudflare_credential_id, organization_id FROM domains WHERE id = $1",
    ).bind(domain_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    if let (Some(zone_id), Some(cred_id)) = (&zone_id, cred_id) {
        let client = build_cf_client(&pool, cred_id).await?;
        client.set_ssl_mode(zone_id, "custom").await
            .map_err(|e| ServerFnError::new(format!("CF SSL: {e}")))?;
    }

    sqlx::query("UPDATE domains SET ssl_mode = $1, updated_at = now() WHERE id = $2")
        .bind(&ssl_mode).bind(domain_id).execute(&pool).await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn toggle_dnssec(domain_id: Uuid, enable: bool) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let (zone_id, cred_id, org_id) = sqlx::query_as::<_, (Option<String>, Option<Uuid>, Uuid)>(
        "SELECT cloudflare_zone_id, cloudflare_credential_id, organization_id FROM domains WHERE id = $1",
    ).bind(domain_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    if let (Some(zone_id), Some(cred_id)) = (&zone_id, cred_id) {
        let client = build_cf_client(&pool, cred_id).await?;
        client.set_dnssec(zone_id, if enable { "active" } else { "disabled" }).await
            .map_err(|e| ServerFnError::new(format!("CF DNSSEC: {e}")))?;
    }

    sqlx::query("UPDATE domains SET dnssec_enabled = $1, updated_at = now() WHERE id = $2")
        .bind(enable).bind(domain_id).execute(&pool).await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn set_ai_bots_protection(domain_id: Uuid, value: String) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let (zone_id, cred_id, org_id) = sqlx::query_as::<_, (Option<String>, Option<Uuid>, Uuid)>(
        "SELECT cloudflare_zone_id, cloudflare_credential_id, organization_id FROM domains WHERE id = $1",
    ).bind(domain_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    if let (Some(zone_id), Some(cred_id)) = (&zone_id, cred_id) {
        let client = build_cf_client(&pool, cred_id).await?;
        client.set_bot_management(zone_id, &serde_json::json!({"ai_bots_protection": value})).await
            .map_err(|e| ServerFnError::new(format!("CF bot management: {e}")))?;
    }

    sqlx::query("UPDATE domains SET ai_bots_protection = $1, updated_at = now() WHERE id = $2")
        .bind(&value).bind(domain_id).execute(&pool).await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn set_nameservers_at_registrar(domain_id: Uuid, nameservers: Vec<String>) -> Result<String, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let (_, _, org_id) = sqlx::query_as::<_, (String, Option<Uuid>, Uuid)>(
        "SELECT name, registrar_credential_id, organization_id FROM domains WHERE id = $1",
    ).bind(domain_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    let domain_name = sqlx::query_scalar::<_, String>("SELECT name FROM domains WHERE id = $1")
        .bind(domain_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    match try_set_registrar_nameservers(&pool, domain_id, &domain_name, &nameservers).await {
        Ok(true) => Ok("Nameservers updated at registrar".into()),
        Ok(false) => Err(ServerFnError::new("No registrar credential or unsupported registrar")),
        Err(e) => Err(e),
    }
}

/// Create a subdomain entity (e.g. "www", "api", "@").
#[server]
async fn create_subdomain(domain_id: Uuid, name: String) -> Result<Uuid, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>("SELECT organization_id FROM domains WHERE id = $1")
        .bind(domain_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO subdomains (domain_id, name) VALUES ($1, $2) \
         ON CONFLICT (domain_id, name) DO UPDATE SET updated_at = now() RETURNING id",
    ).bind(domain_id).bind(&name).fetch_one(&pool).await
    .map_err(|e| ServerFnError::new(format!("failed to create subdomain: {e}")))?;

    Ok(id)
}

/// Delete a subdomain and all its records (cascades).
#[server]
async fn delete_subdomain(domain_id: Uuid, subdomain_id: Uuid) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>("SELECT organization_id FROM domains WHERE id = $1")
        .bind(domain_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    // Delete CF records first
    let records = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT cloudflare_record_id FROM dns_records WHERE subdomain_id = $1",
    ).bind(subdomain_id).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let cf = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
        "SELECT cloudflare_zone_id, cloudflare_credential_id FROM domains WHERE id = $1",
    ).bind(domain_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    if let (Some(zone_id), Some(cred_id)) = cf {
        if let Ok(client) = build_cf_client(&pool, cred_id).await {
            for (cf_id,) in &records {
                if let Some(cf_id) = cf_id {
                    let _ = client.delete_dns_record(&zone_id, cf_id).await;
                }
            }
        }
    }

    sqlx::query("DELETE FROM subdomains WHERE id = $1 AND domain_id = $2")
        .bind(subdomain_id).bind(domain_id).execute(&pool).await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

/// Add a DNS record to a subdomain (and Cloudflare if deployed).
#[server]
async fn add_dns_record(
    domain_id: Uuid,
    subdomain_id: Uuid,
    record_type: String,
    record_value: String,
    proxied: bool,
) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>("SELECT organization_id FROM domains WHERE id = $1")
        .bind(domain_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    let sub_name = sqlx::query_scalar::<_, String>("SELECT name FROM subdomains WHERE id = $1")
        .bind(subdomain_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut cf_record_id: Option<String> = None;

    let cf = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
        "SELECT cloudflare_zone_id, cloudflare_credential_id FROM domains WHERE id = $1",
    ).bind(domain_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    if let (Some(zone_id), Some(cred_id)) = cf {
        let client = build_cf_client(&pool, cred_id).await?;
        let domain_name = sqlx::query_scalar::<_, String>("SELECT name FROM domains WHERE id = $1")
            .bind(domain_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

        let fqdn = if sub_name == "@" { domain_name } else { format!("{sub_name}.{domain_name}") };
        let record = cloudflare_api::compat::CreateDnsRecord {
            record_type: record_type.clone(), name: fqdn,
            content: Some(record_value.clone()), data: None,
            ttl: Some(1), proxied: Some(proxied), comment: None, priority: None,
        };
        let created = client.create_dns_record(&zone_id, &record).await
            .map_err(|e| ServerFnError::new(format!("CF DNS: {e}")))?;
        cf_record_id = Some(created.id);
    }

    sqlx::query(
        "INSERT INTO dns_records (subdomain_id, domain_id, name, record_type, record_value, proxied, cloudflare_record_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(subdomain_id).bind(domain_id).bind(&sub_name).bind(&record_type)
    .bind(&record_value).bind(proxied).bind(&cf_record_id)
    .execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

/// Delete a DNS record.
#[server]
async fn delete_dns_record(domain_id: Uuid, record_id: Uuid) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>("SELECT organization_id FROM domains WHERE id = $1")
        .bind(domain_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    let cf_id = sqlx::query_scalar::<_, Option<String>>(
        "SELECT cloudflare_record_id FROM dns_records WHERE id = $1",
    ).bind(record_id).fetch_optional(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?.flatten();

    if let Some(cf_id) = cf_id {
        let cf = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
            "SELECT cloudflare_zone_id, cloudflare_credential_id FROM domains WHERE id = $1",
        ).bind(domain_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

        if let (Some(zone_id), Some(cred_id)) = cf {
            if let Ok(client) = build_cf_client(&pool, cred_id).await {
                let _ = client.delete_dns_record(&zone_id, &cf_id).await;
            }
        }
    }

    sqlx::query("DELETE FROM dns_records WHERE id = $1").bind(record_id)
        .execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

/// Sync DNS records from Cloudflare into local database.
/// Fetches all records from the zone and upserts them locally.
#[server]
async fn sync_records_from_cloudflare(domain_id: Uuid) -> Result<String, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let (domain_name, org_id) = sqlx::query_as::<_, (String, Uuid)>(
        "SELECT name, organization_id FROM domains WHERE id = $1",
    ).bind(domain_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    let (zone_id, cred_id) = match sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
        "SELECT cloudflare_zone_id, cloudflare_credential_id FROM domains WHERE id = $1",
    ).bind(domain_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))? {
        (Some(z), Some(c)) => (z, c),
        _ => return Err(ServerFnError::new("domain not deployed to Cloudflare")),
    };

    let client = build_cf_client(&pool, cred_id).await?;
    let cf_records = client.list_dns_records(&zone_id).await
        .map_err(|e| ServerFnError::new(format!("CF API: {e}")))?;

    // Delete existing local records for this domain and re-import from CF
    sqlx::query("DELETE FROM dns_records WHERE domain_id = $1")
        .bind(domain_id).execute(&pool).await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

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
        ).bind(domain_id).bind(&sub_name).fetch_one(&pool).await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

        let content = rec.content.as_deref().unwrap_or("");
        let proxied = rec.proxied.unwrap_or(false);

        sqlx::query(
            "INSERT INTO dns_records (subdomain_id, domain_id, name, record_type, record_value, proxied, cloudflare_record_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(sub_id).bind(domain_id).bind(&sub_name)
        .bind(&rec.record_type).bind(content).bind(proxied).bind(&rec.id)
        .execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

        synced += 1;
    }

    Ok(format!("Synced {synced} records from Cloudflare"))
}

#[server]
async fn delete_domain(domain_id: Uuid) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>("SELECT organization_id FROM domains WHERE id = $1")
        .bind(domain_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    sqlx::query("DELETE FROM domains WHERE id = $1").bind(domain_id)
        .execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────

#[cfg(feature = "server")]
async fn build_cf_client(pool: &sqlx::PgPool, cred_id: Uuid) -> Result<cloudflare_api::compat::SimpleClient, ServerFnError> {
    crate::credentials::cf_client(pool, cred_id).await.map_err(|e| ServerFnError::new(format!("{e}")))
}

#[cfg(feature = "server")]
async fn get_cf_account_id(pool: &sqlx::PgPool, cred_id: Uuid) -> Result<String, ServerFnError> {
    let (_, account_id) = crate::credentials::cf_client_with_account(pool, cred_id).await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;
    Ok(account_id)
}

#[cfg(feature = "server")]
async fn try_set_registrar_nameservers(pool: &sqlx::PgPool, domain_id: Uuid, domain_name: &str, nameservers: &[String]) -> Result<bool, ServerFnError> {
    let row = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
        "SELECT registrar_type, registrar_credential_id FROM domains WHERE id = $1",
    ).bind(domain_id).fetch_optional(pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    match row {
        Some((Some(ref rt), Some(cred_id))) if rt == "spaceship" => {
            let client = crate::credentials::spaceship_client(pool, cred_id).await
                .map_err(|e| ServerFnError::new(format!("{e}")))?;
            client.set_nameservers(domain_name, &spaceship_api::compat::NameserverConfig {
                provider: "custom".into(), hosts: Some(nameservers.to_vec()),
            }).await.map_err(|e| ServerFnError::new(format!("Spaceship NS: {e}")))?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

// ── Components ────────────────────────────────────────────────────────

#[component]
pub fn DomainDetail(id: String) -> Element {
    let domain_id = Uuid::parse_str(&id).ok();
    let domain = use_server_future(move || {
        let did = domain_id;
        async move { match did { Some(id) => get_domain(id).await, None => Err(ServerFnError::new("invalid ID")) } }
    })?;

    let data = match &*domain.read() {
        Some(Ok(d)) => d.clone(),
        Some(Err(e)) => return rsx! { div { class: "text-danger", "Error: {e}" } },
        None => return rsx! { div { class: "text-fg-muted", "Loading..." } },
    };

    let has_cf = data.cloudflare_zone_id.is_some();

    rsx! {
        PageHeader { "{data.name}" }

        Card {
            div { class: "p-6 grid grid-cols-2 md:grid-cols-4 gap-4",
                div { div { class: "text-sm text-fg-muted", "Registrar" } div { class: "font-medium", {data.registrar_type.as_deref().unwrap_or("External")} } }
                div { div { class: "text-sm text-fg-muted", "SSL" } Badge { "{data.ssl_mode}" } }
                div { div { class: "text-sm text-fg-muted", "DNSSEC" } if data.dnssec_enabled { Badge { variant: BadgeVariant::Success, "On" } } else { span { class: "text-fg-muted", "Off" } } }
                div { div { class: "text-sm text-fg-muted", "Expires" } div { {data.expires_at.as_deref().unwrap_or("-")} } }
                div { div { class: "text-sm text-fg-muted", "Organization" } div { "{data.organization_name}" } }
            }
        }

        SectionHeading { class: "mt-6", "Cloudflare" }
        if has_cf {
            CloudflareDeployed {
                domain_id: data.id, zone_id: data.cloudflare_zone_id.clone().unwrap_or_default(),
                zone_status: data.cloudflare_zone_status.clone(), nameservers: data.cloudflare_nameservers.clone(),
                ssl_mode: data.ssl_mode.clone(), dnssec_enabled: data.dnssec_enabled, can_set_nameservers: data.can_set_nameservers,
                ai_bots_protection: data.ai_bots_protection.clone(),
            }
        } else {
            CloudflareDeployForm { domain_id: data.id, domain_name: data.name.clone(), can_set_nameservers: data.can_set_nameservers }
        }

        // DNSSEC DS record info
        if let Some(ref ds) = data.dnssec_ds {
            SectionHeading { class: "mt-6", "DNSSEC DS Record" }
            Card {
                div { class: "p-6 space-y-3",
                    div { class: "text-sm text-fg-muted", "Configure this DS record at your registrar to enable DNSSEC validation:" }
                    div { class: "font-mono text-sm bg-surface-2 px-4 py-2 rounded break-all", "{ds}" }
                    div { class: "grid grid-cols-2 md:grid-cols-4 gap-4 mt-3",
                        if let Some(ref kt) = data.dnssec_key_tag {
                            div { div { class: "text-sm text-fg-muted", "Key Tag" } div { class: "font-mono text-sm", "{kt}" } }
                        }
                        if let Some(ref alg) = data.dnssec_algorithm {
                            div { div { class: "text-sm text-fg-muted", "Algorithm" } div { class: "font-mono text-sm", "{alg}" } }
                        }
                        if let Some(ref dt) = data.dnssec_digest_type {
                            div { div { class: "text-sm text-fg-muted", "Digest Type" } div { class: "font-mono text-sm", "{dt}" } }
                        }
                        if let Some(ref dig) = data.dnssec_digest {
                            div { div { class: "text-sm text-fg-muted", "Digest" } div { class: "font-mono text-sm break-all", "{dig}" } }
                        }
                    }
                }
            }
        }

        // Subdomains & DNS records
        SectionHeading { class: "mt-6", "Subdomains & DNS Records" }
        if has_cf {
            SyncFromCloudflareButton { domain_id: data.id }
        }
        SubdomainsSection { domain_id: data.id, subdomains: data.subdomains.clone() }

        SectionHeading { class: "mt-6", "Danger Zone" }
        Card {
            div { class: "p-6 flex items-center justify-between",
                div {
                    div { class: "font-medium text-danger", "Delete this domain" }
                    div { class: "text-sm text-fg-muted", "Removes domain and all records from the database." }
                }
                DeleteDomainButton { domain_id: data.id }
            }
        }
    }
}

// ── Subdomains section ────────────────────────────────────────────────

#[component]
fn SubdomainsSection(domain_id: Uuid, subdomains: Vec<SubdomainData>) -> Element {
    let mut new_sub_name = use_signal(String::new);
    let mut adding_sub = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    rsx! {
        for sub in &subdomains {
            SubdomainCard { domain_id, subdomain: sub.clone() }
        }

        // Add subdomain form
        Card {
            div { class: "p-4",
                div { class: "flex items-end gap-3",
                    FormField { label: "New Subdomain",
                        input { class: "input w-48", r#type: "text", placeholder: "www, @, api, ...",
                            value: "{new_sub_name}", oninput: move |evt| new_sub_name.set(evt.value()) }
                    }
                    Button {
                        variant: ButtonVariant::Primary,
                        disabled: new_sub_name.read().is_empty() || *adding_sub.read(),
                        onclick: {
                            let did = domain_id;
                            move |_| {
                                let n = new_sub_name.read().clone();
                                adding_sub.set(true);
                                error.set(None);
                                spawn(async move {
                                    match create_subdomain(did, n).await {
                                        Ok(_) => {
                                            new_sub_name.set(String::new());
                                            navigator().replace(crate::web::app::Route::DomainDetail { id: did.to_string() });
                                        }
                                        Err(e) => error.set(Some(format!("{e}"))),
                                    }
                                    adding_sub.set(false);
                                });
                            }
                        },
                        if *adding_sub.read() { "Creating..." } else { "Add Subdomain" }
                    }
                }
                if let Some(err) = &*error.read() {
                    div { class: "mt-2 text-danger text-sm", "{err}" }
                }
            }
        }
    }
}

#[component]
fn SubdomainCard(domain_id: Uuid, subdomain: SubdomainData) -> Element {
    let mut new_type = use_signal(|| "A".to_string());
    let mut new_value = use_signal(String::new);
    let mut new_proxied = use_signal(|| true);
    let mut adding = use_signal(|| false);
    let mut deleting_rec: Signal<Option<Uuid>> = use_signal(|| None);
    let mut deleting_sub = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    let sub_id = subdomain.id;
    let did = domain_id;

    rsx! {
        Card { class: "mb-3",
            div { class: "px-4 py-3 border-b border-line-soft flex items-center justify-between",
                div { class: "flex items-center gap-2",
                    span { class: "font-mono font-semibold", "{subdomain.name}" }
                    Badge { variant: BadgeVariant::Neutral, "{subdomain.records.len()} record(s)" }
                }
                Button {
                    variant: ButtonVariant::Danger,
                    disabled: *deleting_sub.read(),
                    onclick: move |_| {
                        deleting_sub.set(true);
                        spawn(async move {
                            let _ = delete_subdomain(did, sub_id).await;
                            navigator().replace(crate::web::app::Route::DomainDetail { id: did.to_string() });
                        });
                    },
                    if *deleting_sub.read() { "..." } else { "Delete Subdomain" }
                }
            }

            if !subdomain.records.is_empty() {
                div { class: "overflow-x-auto",
                    table { class: "table w-full",
                        thead { tr { Th { "Type" } Th { "Value" } Th { "Proxied" } Th { "CF" } Th { "" } } }
                        tbody {
                            for rec in &subdomain.records {
                                {
                                    let rid = rec.id;
                                    let is_del = *deleting_rec.read() == Some(rid);
                                    rsx! {
                                        tr {
                                            Td { Badge { "{rec.record_type}" } }
                                            Td { class: "font-mono text-sm", "{rec.record_value}" }
                                            Td { if rec.proxied { Badge { variant: BadgeVariant::Success, "Yes" } } else { span { class: "text-fg-muted", "No" } } }
                                            Td { if rec.cloudflare_record_id.is_some() { Badge { variant: BadgeVariant::Info, "Synced" } } else { span { class: "text-fg-muted", "-" } } }
                                            Td {
                                                Button { variant: ButtonVariant::Danger, disabled: is_del,
                                                    onclick: move |_| {
                                                        deleting_rec.set(Some(rid));
                                                        spawn(async move {
                                                            let _ = delete_dns_record(did, rid).await;
                                                            deleting_rec.set(None);
                                                            navigator().replace(crate::web::app::Route::DomainDetail { id: did.to_string() });
                                                        });
                                                    },
                                                    if is_del { "..." } else { "Del" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Add record to this subdomain
            div { class: "p-3 border-t border-line-soft",
                div { class: "flex items-end gap-2 flex-wrap",
                    FormField { label: "Type",
                        select { class: "input w-24", value: "{new_type}", oninput: move |evt| new_type.set(evt.value()),
                            option { value: "A", "A" } option { value: "AAAA", "AAAA" } option { value: "CNAME", "CNAME" }
                            option { value: "MX", "MX" } option { value: "TXT", "TXT" } option { value: "NS", "NS" }
                        }
                    }
                    FormField { label: "Value",
                        input { class: "input w-48 font-mono text-sm", r#type: "text", placeholder: "1.2.3.4",
                            value: "{new_value}", oninput: move |evt| new_value.set(evt.value()) }
                    }
                    FormField { label: "Proxied",
                        input { class: "mt-2", r#type: "checkbox", checked: *new_proxied.read(),
                            oninput: move |evt| new_proxied.set(evt.checked()) }
                    }
                    Button { variant: ButtonVariant::Primary,
                        disabled: new_value.read().is_empty() || *adding.read(),
                        onclick: move |_| {
                            let t = new_type.read().clone();
                            let v = new_value.read().clone();
                            let p = *new_proxied.read();
                            adding.set(true); error.set(None);
                            spawn(async move {
                                match add_dns_record(did, sub_id, t, v, p).await {
                                    Ok(()) => { new_value.set(String::new()); navigator().replace(crate::web::app::Route::DomainDetail { id: did.to_string() }); }
                                    Err(e) => error.set(Some(format!("{e}"))),
                                }
                                adding.set(false);
                            });
                        },
                        if *adding.read() { "..." } else { "Add" }
                    }
                }
                if let Some(err) = &*error.read() {
                    div { class: "mt-1 text-danger text-sm", "{err}" }
                }
            }
        }
    }
}

// ── CF deploy + NS components (same as before) ───────────────────────

#[component]
fn CloudflareDeployForm(domain_id: Uuid, domain_name: String, can_set_nameservers: bool) -> Element {
    let creds = use_server_future(list_cf_credentials)?;
    let cred_list = match &*creds.read() { Some(Ok(c)) => c.clone(), _ => vec![] };
    let mut cred_id = use_signal(|| cred_list.first().map(|c| c.id.to_string()).unwrap_or_default());
    let mut deploying = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut result = use_signal(|| None::<DeployResult>);

    if let Some(res) = &*result.read() {
        let ns_set = res.nameservers_set_at_registrar;
        let ns_list = res.nameservers.clone();
        return rsx! {
            Card { div { class: "p-6",
                div { class: "flex items-center gap-2 mb-3", Badge { variant: BadgeVariant::Success, "Deployed" } span { class: "font-mono text-sm", "Zone: {res.zone_id}" } }
                if !ns_list.is_empty() {
                    div { class: "mt-3", div { class: "text-sm text-fg-muted mb-1", "Nameservers:" }
                        for ns in &ns_list { div { class: "font-mono text-sm bg-surface-2 px-3 py-1 rounded mb-1", "{ns}" } }
                    }
                    if ns_set { div { class: "mt-2", Badge { variant: BadgeVariant::Success, "NS set at registrar" } } }
                    else if can_set_nameservers { SetNsButton { domain_id, nameservers: ns_list.clone() } }
                    else { div { class: "mt-2 text-sm text-fg-muted", "Update nameservers at your registrar." } }
                }
                div { class: "mt-3 text-sm text-fg-muted", "Reload to see updated status." }
            }}
        };
    }

    rsx! {
        Card { div { class: "p-6",
            p { class: "text-fg-muted mb-4", "Deploy " span { class: "font-semibold text-fg", "{domain_name}" } " to Cloudflare." }
            if cred_list.is_empty() {
                p { class: "text-fg-muted", "No CF credentials. " Link { to: crate::web::app::Route::CredentialForm {}, class: "text-brand underline", "Add one" } " first." }
            } else {
                div { class: "flex items-end gap-3",
                    FormField { label: "Credential", select { class: "input", value: "{cred_id}", oninput: move |evt| cred_id.set(evt.value()),
                        for c in &cred_list { option { value: "{c.id}", "{c.name}" } }
                    }}
                    Button { variant: ButtonVariant::Primary, disabled: *deploying.read(),
                        onclick: { let did = domain_id; let cid_str = cred_id.read().clone();
                            move |_| { let cid_str = cid_str.clone(); deploying.set(true); error.set(None);
                                spawn(async move { if let Ok(cid) = uuid::Uuid::parse_str(&cid_str) {
                                    match deploy_to_cloudflare(did, cid).await { Ok(r) => result.set(Some(r)), Err(e) => error.set(Some(format!("{e}"))) }
                                } deploying.set(false); });
                            }
                        },
                        if *deploying.read() { "Deploying..." } else { "Deploy" }
                    }
                }
            }
            if let Some(err) = &*error.read() { div { class: "mt-3 text-danger text-sm", "{err}" } }
        }}
    }
}

#[component]
fn CloudflareDeployed(domain_id: Uuid, zone_id: String, zone_status: Option<String>, nameservers: Vec<String>, ssl_mode: String, dnssec_enabled: bool, can_set_nameservers: bool, ai_bots_protection: Option<String>) -> Element {
    let mut ssl = use_signal(move || ssl_mode.clone());
    let mut dnssec = use_signal(move || dnssec_enabled);
    let mut ai_bots = use_signal(move || ai_bots_protection.unwrap_or_default());
    let mut saving_ssl = use_signal(|| false);
    let mut saving_dnssec = use_signal(|| false);
    let mut saving_ai_bots = use_signal(|| false);
    let mut message = use_signal(|| None::<String>);

    rsx! {
        Card { div { class: "p-6 space-y-4",
            div { class: "flex items-center gap-3 flex-wrap",
                span { class: "text-sm text-fg-muted", "Zone:" } span { class: "font-mono text-sm", "{zone_id}" }
                match zone_status.as_deref() {
                    Some("active") => rsx! { Badge { variant: BadgeVariant::Success, "Active" } },
                    Some("pending") => rsx! { Badge { variant: BadgeVariant::Warn, "Pending" } },
                    Some(s) => rsx! { Badge { "{s}" } }, None => rsx! {},
                }
            }
            if !nameservers.is_empty() {
                div { div { class: "text-sm text-fg-muted mb-1", "Nameservers:" }
                    div { class: "flex gap-2 flex-wrap", for ns in &nameservers { span { class: "font-mono text-sm bg-surface-2 px-3 py-1 rounded", "{ns}" } } }
                    if can_set_nameservers && zone_status.as_deref() == Some("pending") { SetNsButton { domain_id, nameservers: nameservers.clone() } }
                }
            }
            div { class: "flex items-end gap-3",
                FormField { label: "SSL", select { class: "input w-40", value: "{ssl}", oninput: move |evt| ssl.set(evt.value()),
                    option { value: "off", "Off" } option { value: "flexible", "Flexible" } option { value: "full", "Full" } option { value: "strict", "Strict" }
                }}
                Button { variant: ButtonVariant::Secondary, disabled: *saving_ssl.read(),
                    onclick: { let did = domain_id; move |_| { let m = ssl.read().clone(); saving_ssl.set(true); message.set(None);
                        spawn(async move { match update_ssl_mode(did, m).await { Ok(()) => message.set(Some("SSL updated".into())), Err(e) => message.set(Some(format!("{e}"))) } saving_ssl.set(false); }); }},
                    if *saving_ssl.read() { "..." } else { "Update SSL" }
                }
            }
            div { class: "flex items-center gap-3",
                span { class: "text-sm", "DNSSEC:" }
                Button { variant: if *dnssec.read() { ButtonVariant::Danger } else { ButtonVariant::Primary }, disabled: *saving_dnssec.read(),
                    onclick: { let did = domain_id; move |_| { let ns = !*dnssec.read(); saving_dnssec.set(true); message.set(None);
                        spawn(async move { match toggle_dnssec(did, ns).await { Ok(()) => { dnssec.set(ns); message.set(Some(if ns { "DNSSEC enabled" } else { "DNSSEC disabled" }.into())); } Err(e) => message.set(Some(format!("{e}"))) } saving_dnssec.set(false); }); }},
                    if *saving_dnssec.read() { "..." } else if *dnssec.read() { "Disable" } else { "Enable" }
                }
                if *dnssec.read() { Badge { variant: BadgeVariant::Success, "On" } }
            }
            div { class: "flex items-end gap-3",
                FormField { label: "AI Bot Protection",
                    select { class: "input w-48", value: "{ai_bots}", oninput: move |evt| ai_bots.set(evt.value()),
                        option { value: "block", "Block" }
                        option { value: "disabled", "Disabled" }
                    }
                }
                Button { variant: ButtonVariant::Secondary, disabled: *saving_ai_bots.read(),
                    onclick: { let did = domain_id; move |_| { let v = ai_bots.read().clone(); saving_ai_bots.set(true); message.set(None);
                        spawn(async move { match set_ai_bots_protection(did, v).await { Ok(()) => message.set(Some("AI bot protection updated".into())), Err(e) => message.set(Some(format!("{e}"))) } saving_ai_bots.set(false); }); }},
                    if *saving_ai_bots.read() { "..." } else { "Update" }
                }
            }
            if let Some(msg) = &*message.read() { div { class: "text-sm text-fg-muted", "{msg}" } }
        }}
    }
}

#[component]
fn SetNsButton(domain_id: Uuid, nameservers: Vec<String>) -> Element {
    let mut setting = use_signal(|| false);
    let mut msg = use_signal(|| None::<String>);
    rsx! {
        div { class: "mt-2 flex items-center gap-3",
            Button { variant: ButtonVariant::Secondary, disabled: *setting.read(),
                onclick: { let did = domain_id; let ns = nameservers.clone();
                    move |_| { let ns = ns.clone(); setting.set(true); msg.set(None);
                        spawn(async move { match set_nameservers_at_registrar(did, ns).await { Ok(m) => msg.set(Some(m)), Err(e) => msg.set(Some(format!("{e}"))) } setting.set(false); }); }},
                if *setting.read() { "Setting..." } else { "Set NS at Registrar" }
            }
            if let Some(m) = &*msg.read() { span { class: "text-sm text-fg-muted", "{m}" } }
        }
    }
}

#[component]
fn SyncFromCloudflareButton(domain_id: Uuid) -> Element {
    let mut syncing = use_signal(|| false);
    let mut message = use_signal(|| None::<String>);

    rsx! {
        div { class: "mb-3 flex items-center gap-3",
            Button {
                variant: ButtonVariant::Secondary,
                disabled: *syncing.read(),
                onclick: {
                    let did = domain_id;
                    move |_| {
                        syncing.set(true);
                        message.set(None);
                        spawn(async move {
                            match sync_records_from_cloudflare(did).await {
                                Ok(msg) => {
                                    message.set(Some(msg));
                                    navigator().replace(crate::web::app::Route::DomainDetail { id: did.to_string() });
                                }
                                Err(e) => message.set(Some(format!("{e}"))),
                            }
                            syncing.set(false);
                        });
                    }
                },
                if *syncing.read() { "Syncing..." } else { "Sync from Cloudflare" }
            }
            if let Some(msg) = &*message.read() {
                span { class: "text-sm text-fg-muted", "{msg}" }
            }
        }
    }
}

#[component]
fn DeleteDomainButton(domain_id: Uuid) -> Element {
    let mut deleting = use_signal(|| false);
    let mut confirm = use_signal(|| false);
    let nav = use_navigator();

    if !*confirm.read() {
        return rsx! { Button { variant: ButtonVariant::Danger, onclick: move |_| confirm.set(true), "Delete Domain" } };
    }
    rsx! {
        div { class: "flex items-center gap-2",
            Button { variant: ButtonVariant::Danger, disabled: *deleting.read(),
                onclick: { let nav = nav.clone(); let did = domain_id; move |_| { let nav = nav.clone(); deleting.set(true);
                    spawn(async move { let _ = delete_domain(did).await; nav.push(crate::web::app::Route::DomainList {}); }); }},
                if *deleting.read() { "Deleting..." } else { "Confirm Delete" }
            }
            Button { variant: ButtonVariant::Secondary, onclick: move |_| confirm.set(false), "Cancel" }
        }
    }
}
