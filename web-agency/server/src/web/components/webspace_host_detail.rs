//! Webspace-host detail page.
//!
//! A webspace-host owns the hostname: domain bindings, the CNAME flow, and
//! ChangeDetection all live here. Its `webspace` children are path-mounted
//! folders. `kind` is either `proxy` (folders route through web-agency-proxy;
//! CNAME → agency_domain) or `cloudflare` (a single Pages-project folder at `/`;
//! CNAME → {project}.pages.dev).

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{
    Badge, BadgeVariant, Button, ButtonVariant, Card, FormField, PageHeader, SectionHeading, Td,
    TdMuted, Th,
};

// ── Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WebspaceHostData {
    id: Uuid,
    name: String,
    kind: String,
    organization_id: Uuid,
    organization_name: String,
    is_org_admin: bool,
    folders: Vec<FolderRow>,
    bindings: Vec<DomainBinding>,
    changedetection_credential_id: Option<Uuid>,
    changedetection_credential_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct FolderRow {
    id: Uuid,
    name: String,
    path_prefix: String,
    hosting_type: String,
    runtime: Option<String>,
    local_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct DomainBinding {
    binding_id: Uuid,
    domain_id: Uuid,
    subdomain_id: Option<Uuid>,
    domain_name: String,
    subdomain_name: Option<String>,
    hostname: String,
    cname_ok: bool,
    /// CF Pages custom domain verification status (cloudflare hosts only).
    cf_domain_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredOption {
    id: Uuid,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DomainOption {
    id: Uuid,
    name: String,
    cloudflare_zone_id: Option<String>,
    subdomains: Vec<SubdomainOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SubdomainOption {
    id: Uuid,
    name: String,
}

// ── Server helpers (server-only) ──────────────────────────────────────

/// The Cloudflare Pages project + credential for a cloudflare host live on its
/// single main-folder webspace.
#[cfg(feature = "server")]
async fn cloudflare_folder(
    pool: &sqlx::PgPool,
    host_id: Uuid,
) -> Result<(String, Uuid), ServerFnError> {
    let row = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
        "SELECT cloudflare_pages_project, cloudflare_credential_id FROM webspaces \
         WHERE webspace_host_id = $1 AND path_prefix = '/'",
    )
    .bind(host_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("cloudflare host has no main-folder"))?;
    match row {
        (Some(project), Some(cred)) => Ok((project, cred)),
        _ => Err(ServerFnError::new("cloudflare host folder is not deployed")),
    }
}

/// Create a CNAME record on the bound domain's Cloudflare zone (used for both
/// proxy hosts → agency_domain and cloudflare hosts → {project}.pages.dev).
#[cfg(feature = "server")]
async fn create_host_cname(
    pool: &sqlx::PgPool,
    domain_id: Uuid,
    subdomain_id: Option<Uuid>,
    hostname: &str,
    cname_target: &str,
    comment: &str,
) -> Result<(), ServerFnError> {
    let domain_cf = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
        "SELECT cloudflare_zone_id, cloudflare_credential_id FROM domains WHERE id = $1",
    )
    .bind(domain_id)
    .fetch_one(pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let (zone_id, domain_cred_id) = match domain_cf {
        (Some(z), Some(c)) => (z, c),
        _ => return Err(ServerFnError::new("domain not deployed to Cloudflare")),
    };

    let client = crate::credentials::cf_client(pool, domain_cred_id)
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;

    let record = cloudflare_api::compat::CreateDnsRecord {
        record_type: "CNAME".into(),
        name: hostname.to_string(),
        content: Some(cname_target.to_string()),
        data: None,
        ttl: Some(1),
        proxied: Some(true),
        comment: Some(comment.to_string()),
        priority: None,
    };

    let created = match client.create_dns_record(&zone_id, &record).await {
        Ok(created) => {
            tracing::info!("created CNAME {hostname} → {cname_target} (CF record {})", created.id);
            created
        }
        Err(e) => {
            tracing::warn!("failed to create CNAME for {hostname}: {e}");
            return Ok(());
        }
    };

    // Mirror into dns_records, ensuring the subdomain entity exists.
    let sub_id = if let Some(sid) = subdomain_id {
        sid
    } else {
        sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO subdomains (domain_id, name) VALUES ($1, '@') \
             ON CONFLICT (domain_id, name) DO UPDATE SET updated_at = now() RETURNING id",
        )
        .bind(domain_id)
        .fetch_one(pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };
    let sub_name = if let Some(sid) = subdomain_id {
        sqlx::query_scalar::<_, String>("SELECT name FROM subdomains WHERE id = $1")
            .bind(sid)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "@".into())
    } else {
        "@".into()
    };
    let _ = sqlx::query(
        "INSERT INTO dns_records (subdomain_id, domain_id, name, record_type, record_value, proxied, cloudflare_record_id) \
         VALUES ($1, $2, $3, 'CNAME', $4, true, $5) ON CONFLICT DO NOTHING",
    )
    .bind(sub_id).bind(domain_id).bind(&sub_name).bind(cname_target).bind(&created.id)
    .execute(pool).await;

    Ok(())
}

/// Remove a CNAME record pointing to `cname_target` from the domain's zone.
#[cfg(feature = "server")]
async fn remove_host_cname(pool: &sqlx::PgPool, domain_id: Uuid, hostname: &str, cname_target: &str) {
    let domain_cf = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
        "SELECT cloudflare_zone_id, cloudflare_credential_id FROM domains WHERE id = $1",
    )
    .bind(domain_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    if let Some((Some(zone_id), Some(domain_cred_id))) = domain_cf {
        if let Ok(client) = crate::credentials::cf_client(pool, domain_cred_id).await {
            if let Ok(records) = client.list_dns_records(&zone_id).await {
                for rec in records {
                    if rec.record_type == "CNAME"
                        && rec.content.as_deref() == Some(cname_target)
                        && (rec.name == hostname || rec.name.ends_with(&format!(".{hostname}")))
                    {
                        let _ = client.delete_dns_record(&zone_id, &rec.id).await;
                        tracing::info!("removed CNAME {} → {cname_target}", rec.name);
                        let _ = sqlx::query("DELETE FROM dns_records WHERE cloudflare_record_id = $1")
                            .bind(&rec.id)
                            .execute(pool)
                            .await;
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(feature = "server")]
fn agency_domain() -> Option<String> {
    crate::config::config()
        .proxy
        .as_ref()
        .map(|p| p.agency_domain.clone())
}

// ── Server functions ──────────────────────────────────────────────────

#[server]
async fn get_host(host_id: Uuid) -> Result<WebspaceHostData, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let (id, name, kind, org_id, cd_cred_id) =
        sqlx::query_as::<_, (Uuid, String, String, Uuid, Option<Uuid>)>(
            "SELECT id, name, kind, organization_id, changedetection_credential_id \
             FROM webspace_hosts WHERE id = $1",
        )
        .bind(host_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("host not found"))?;

    use crate::web::user::WebUserExt;
    user.require_org_read(&org_id)?;

    let org_name = sqlx::query_scalar::<_, String>("SELECT name FROM organizations WHERE id = $1")
        .bind(org_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let folders = sqlx::query_as::<_, (Uuid, String, String, String, Option<String>, Option<String>)>(
        "SELECT id, name, path_prefix, hosting_type, runtime, local_status \
         FROM webspaces WHERE webspace_host_id = $1 ORDER BY path_prefix",
    )
    .bind(host_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .into_iter()
    .map(|(id, name, path_prefix, hosting_type, runtime, local_status)| FolderRow {
        id,
        name,
        path_prefix,
        hosting_type,
        runtime,
        local_status,
    })
    .collect::<Vec<_>>();

    let binding_rows = sqlx::query_as::<_, (Uuid, Uuid, Option<Uuid>, String, Option<String>)>(
        "SELECT whd.id, whd.domain_id, whd.subdomain_id, d.name, s.name \
         FROM webspace_host_domains whd \
         JOIN domains d ON d.id = whd.domain_id \
         LEFT JOIN subdomains s ON s.id = whd.subdomain_id \
         WHERE whd.webspace_host_id = $1 ORDER BY d.name",
    )
    .bind(host_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let is_cloudflare = kind == "cloudflare";

    let mut bindings = Vec::new();
    for (binding_id, domain_id, subdomain_id, domain_name, subdomain_name) in binding_rows {
        let hostname = match &subdomain_name {
            Some(sub) if sub != "@" => format!("{sub}.{domain_name}"),
            _ => domain_name.clone(),
        };
        let sub_name = subdomain_name.as_deref().unwrap_or("@");
        // proxy hosts: CNAME → agency_domain; cloudflare hosts: CNAME → *.pages.dev.
        let cname_ok = if is_cloudflare {
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM dns_records WHERE domain_id = $1 AND name = $2 \
                 AND record_type = 'CNAME' AND record_value LIKE '%.pages.dev')",
            )
            .bind(domain_id)
            .bind(sub_name)
            .fetch_one(&pool)
            .await
            .unwrap_or(false)
        } else if let Some(ad) = agency_domain() {
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM dns_records WHERE domain_id = $1 AND name = $2 \
                 AND record_type = 'CNAME' AND record_value = $3)",
            )
            .bind(domain_id)
            .bind(sub_name)
            .bind(&ad)
            .fetch_one(&pool)
            .await
            .unwrap_or(false)
        } else {
            false
        };

        bindings.push(DomainBinding {
            binding_id,
            domain_id,
            subdomain_id,
            domain_name,
            subdomain_name,
            hostname,
            cname_ok,
            cf_domain_status: None,
        });
    }

    // For cloudflare hosts, fetch live custom-domain verification statuses.
    if is_cloudflare {
        if let Ok((project_name, cred_id)) = cloudflare_folder(&pool, host_id).await {
            if let Ok((client, account_id)) =
                crate::credentials::cf_client_with_account(&pool, cred_id).await
            {
                if let Ok(cf_domains) = client
                    .list_pages_custom_domains(&account_id, &project_name)
                    .await
                {
                    for binding in &mut bindings {
                        let hostname_lower = binding.hostname.to_lowercase();
                        if let Some(cf_dom) = cf_domains
                            .iter()
                            .find(|d| d.name.to_lowercase() == hostname_lower)
                        {
                            binding.cf_domain_status = cf_dom.status.clone();
                        }
                    }
                }
            }
        }
    }

    let cd_cred_name = if let Some(cid) = cd_cred_id {
        sqlx::query_scalar::<_, String>("SELECT name FROM credentials WHERE id = $1")
            .bind(cid)
            .fetch_optional(&pool)
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    Ok(WebspaceHostData {
        id,
        name,
        kind,
        organization_id: org_id,
        organization_name: org_name,
        is_org_admin: user.is_org_admin(&org_id),
        folders,
        bindings,
        changedetection_credential_id: cd_cred_id,
        changedetection_credential_name: cd_cred_name,
    })
}

#[cfg(feature = "server")]
async fn host_org(pool: &sqlx::PgPool, host_id: Uuid) -> Result<(Uuid, String), ServerFnError> {
    sqlx::query_as::<_, (Uuid, String)>(
        "SELECT organization_id, kind FROM webspace_hosts WHERE id = $1",
    )
    .bind(host_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("host not found"))
}

#[server]
async fn list_domains_for_binding(host_id: Uuid) -> Result<Vec<DomainOption>, ServerFnError> {
    let _user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let (org_id, _kind) = host_org(&pool, host_id).await?;

    let rows = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
        "SELECT id, name, cloudflare_zone_id FROM domains WHERE organization_id = $1 ORDER BY name",
    )
    .bind(org_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut domains = Vec::new();
    for (id, name, cloudflare_zone_id) in rows {
        let subs = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, name FROM subdomains WHERE domain_id = $1 ORDER BY name",
        )
        .bind(id)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
        domains.push(DomainOption {
            id,
            name,
            cloudflare_zone_id,
            subdomains: subs
                .into_iter()
                .map(|(sid, sname)| SubdomainOption { id: sid, name: sname })
                .collect(),
        });
    }
    Ok(domains)
}

#[server]
async fn list_cd_creds() -> Result<Vec<CredOption>, ServerFnError> {
    let _user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let rows = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, name FROM credentials WHERE credential_type = 'changedetection' ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rows.into_iter().map(|(id, name)| CredOption { id, name }).collect())
}

#[server]
async fn set_host_changedetection(
    host_id: Uuid,
    credential_id: Option<Uuid>,
) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let (org_id, _) = host_org(&pool, host_id).await?;
    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    sqlx::query(
        "UPDATE webspace_hosts SET changedetection_credential_id = $1, updated_at = now() WHERE id = $2",
    )
    .bind(credential_id)
    .bind(host_id)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

/// Bind a domain (or subdomain) to this host and create the appropriate CNAME.
#[server]
async fn bind_domain(
    host_id: Uuid,
    domain_id: Uuid,
    subdomain_id: Option<Uuid>,
    hostname: String,
) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let (org_id, kind) = host_org(&pool, host_id).await?;
    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    sqlx::query(
        "INSERT INTO webspace_host_domains (webspace_host_id, domain_id, subdomain_id) \
         VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(host_id)
    .bind(domain_id)
    .bind(subdomain_id)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    if kind == "cloudflare" {
        let (project_name, cred_id) = cloudflare_folder(&pool, host_id).await?;
        let (client, account_id) = crate::credentials::cf_client_with_account(&pool, cred_id)
            .await
            .map_err(|e| ServerFnError::new(format!("{e}")))?;
        if let Err(e) = client
            .add_pages_custom_domain(&account_id, &project_name, &hostname)
            .await
        {
            tracing::warn!("failed to add custom domain {hostname} to Pages: {e}");
        }
        let cname_target = client
            .get_pages_project(&account_id, &project_name)
            .await
            .map_err(|e| ServerFnError::new(format!("failed to fetch Pages project: {e}")))?
            .subdomain
            .ok_or_else(|| ServerFnError::new("Pages project has no subdomain"))?;
        create_host_cname(
            &pool,
            domain_id,
            subdomain_id,
            &hostname,
            &cname_target,
            &format!("Pages: {project_name}"),
        )
        .await?;
    } else if let Some(ad) = agency_domain() {
        create_host_cname(&pool, domain_id, subdomain_id, &hostname, &ad, "Proxy host").await?;
    }

    crate::api::internal::notify_proxy_reload();
    Ok(())
}

/// Re-create the CNAME for a binding (kind-aware).
#[server]
async fn fix_cname(
    host_id: Uuid,
    domain_id: Uuid,
    subdomain_id: Option<Uuid>,
    hostname: String,
) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let (org_id, kind) = host_org(&pool, host_id).await?;
    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    if kind == "cloudflare" {
        let (project_name, cred_id) = cloudflare_folder(&pool, host_id).await?;
        let (client, account_id) = crate::credentials::cf_client_with_account(&pool, cred_id)
            .await
            .map_err(|e| ServerFnError::new(format!("{e}")))?;
        let cname_target = client
            .get_pages_project(&account_id, &project_name)
            .await
            .map_err(|e| ServerFnError::new(format!("failed to fetch Pages project: {e}")))?
            .subdomain
            .ok_or_else(|| ServerFnError::new("Pages project has no subdomain"))?;
        create_host_cname(
            &pool,
            domain_id,
            subdomain_id,
            &hostname,
            &cname_target,
            &format!("Pages: {project_name}"),
        )
        .await
    } else {
        let ad = agency_domain().ok_or_else(|| ServerFnError::new("no proxy config"))?;
        create_host_cname(&pool, domain_id, subdomain_id, &hostname, &ad, "Proxy host").await
    }
}

/// Trigger a recheck of a custom domain's CF Pages verification status.
#[server]
async fn recheck_custom_domain(host_id: Uuid, hostname: String) -> Result<String, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let (org_id, _) = host_org(&pool, host_id).await?;
    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    let (project_name, cred_id) = cloudflare_folder(&pool, host_id).await?;
    let (client, account_id) = crate::credentials::cf_client_with_account(&pool, cred_id)
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;

    match client
        .retry_pages_custom_domain(&account_id, &project_name, &hostname)
        .await
    {
        Ok(dom) => Ok(format!(
            "Validation retried — status: {}",
            dom.status.as_deref().unwrap_or("pending")
        )),
        Err(e) => match client
            .add_pages_custom_domain(&account_id, &project_name, &hostname)
            .await
        {
            Ok(dom) => Ok(format!(
                "Domain added — status: {}",
                dom.status.as_deref().unwrap_or("pending")
            )),
            Err(_) => Err(ServerFnError::new(format!("retry failed: {e}"))),
        },
    }
}

/// Remove a domain binding and its CNAME.
#[server]
async fn unbind_domain(
    host_id: Uuid,
    binding_id: Uuid,
    hostname: String,
) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let (org_id, kind) = host_org(&pool, host_id).await?;
    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    let binding = sqlx::query_as::<_, (Uuid, Option<Uuid>)>(
        "SELECT domain_id, subdomain_id FROM webspace_host_domains WHERE id = $1",
    )
    .bind(binding_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    if let Some((domain_id, _)) = binding {
        if kind == "cloudflare" {
            if let Ok((project_name, cred_id)) = cloudflare_folder(&pool, host_id).await {
                if let Ok((client, account_id)) =
                    crate::credentials::cf_client_with_account(&pool, cred_id).await
                {
                    let _ = client
                        .remove_pages_custom_domain(&account_id, &project_name, &hostname)
                        .await;
                }
                let cname_target = format!("{project_name}.pages.dev");
                remove_host_cname(&pool, domain_id, &hostname, &cname_target).await;
            }
        } else if let Some(ad) = agency_domain() {
            remove_host_cname(&pool, domain_id, &hostname, &ad).await;
        }
    }

    sqlx::query("DELETE FROM webspace_host_domains WHERE id = $1")
        .bind(binding_id)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    crate::api::internal::notify_proxy_reload();
    Ok(())
}

// ── Rename / transfer / delete ─────────────────────────────────────────

#[server]
async fn update_host_settings(host_id: Uuid, name: String) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let (org_id, _) = host_org(&pool, host_id).await?;
    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    // CF Pages project name lives on the folder; the host name is for display.
    sqlx::query("UPDATE webspace_hosts SET name = $1, updated_at = now() WHERE id = $2")
        .bind(&name)
        .bind(host_id)
        .execute(&pool)
        .await
        .map_err(|e| match e.as_database_error() {
            Some(db) if db.is_unique_violation() => {
                ServerFnError::new("a host with that name already exists in this organization")
            }
            _ => ServerFnError::new(e.to_string()),
        })?;
    Ok(())
}

#[server]
async fn list_host_move_target_orgs() -> Result<Vec<crate::web::user::OrgOption>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    crate::web::user::list_user_write_orgs(&user, &pool).await
}

#[server]
async fn move_host(host_id: Uuid, target_org_id: Uuid) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let (org_id, _) = host_org(&pool, host_id).await?;
    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;
    user.require_org_write(&target_org_id)?;

    if org_id == target_org_id {
        return Err(ServerFnError::new("host is already in that organization"));
    }

    // Host and its child folders both carry organization_id; move them together.
    // Their org-scoped UNIQUE(name) constraints catch collisions and roll back.
    // ponytail: domain bindings keep pointing at source-org domains; rebinding is manual.
    let mut tx = pool.begin().await.map_err(|e| ServerFnError::new(e.to_string()))?;
    sqlx::query("UPDATE webspace_hosts SET organization_id = $1, updated_at = now() WHERE id = $2")
        .bind(target_org_id)
        .bind(host_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| match e.as_database_error() {
            Some(db) if db.is_unique_violation() => {
                ServerFnError::new("a host with the same name already exists in the target organization")
            }
            _ => ServerFnError::new(e.to_string()),
        })?;
    sqlx::query("UPDATE webspaces SET organization_id = $1 WHERE webspace_host_id = $2")
        .bind(target_org_id)
        .bind(host_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| match e.as_database_error() {
            Some(db) if db.is_unique_violation() => {
                ServerFnError::new("a folder with the same name already exists in the target organization")
            }
            _ => ServerFnError::new(e.to_string()),
        })?;
    tx.commit().await.map_err(|e| ServerFnError::new(e.to_string()))?;

    crate::api::internal::notify_proxy_reload();
    Ok(())
}

#[server]
async fn delete_host(host_id: Uuid) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let (org_id, kind) = host_org(&pool, host_id).await?;
    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    // Folders, bindings, and ChangeDetection rows cascade on FK delete, but the
    // bindings own external CNAMEs (and CF custom domains) that won't — clean them
    // up first, mirroring unbind_domain. ponytail: CF Pages project itself is left.
    let bindings = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
        "SELECT whd.domain_id, d.name, s.name \
         FROM webspace_host_domains whd \
         JOIN domains d ON d.id = whd.domain_id \
         LEFT JOIN subdomains s ON s.id = whd.subdomain_id \
         WHERE whd.webspace_host_id = $1",
    )
    .bind(host_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let cf = if kind == "cloudflare" {
        cloudflare_folder(&pool, host_id).await.ok()
    } else {
        None
    };

    for (domain_id, domain_name, subdomain_name) in bindings {
        let hostname = match &subdomain_name {
            Some(sub) if sub != "@" => format!("{sub}.{domain_name}"),
            _ => domain_name.clone(),
        };
        if let Some((project_name, cred_id)) = &cf {
            if let Ok((client, account_id)) =
                crate::credentials::cf_client_with_account(&pool, *cred_id).await
            {
                let _ = client
                    .remove_pages_custom_domain(&account_id, project_name, &hostname)
                    .await;
            }
            let cname_target = format!("{project_name}.pages.dev");
            remove_host_cname(&pool, domain_id, &hostname, &cname_target).await;
        } else if let Some(ad) = agency_domain() {
            remove_host_cname(&pool, domain_id, &hostname, &ad).await;
        }
    }

    sqlx::query("DELETE FROM webspace_hosts WHERE id = $1")
        .bind(host_id)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    crate::api::internal::notify_proxy_reload();
    Ok(())
}

// ── Component ─────────────────────────────────────────────────────────

#[component]
pub fn WebspaceHostDetail(id: String) -> Element {
    let host_id = Uuid::parse_str(&id).ok();
    let refresh = use_context_provider(|| Signal::new(0u32));
    let host = use_server_future(move || {
        let hid = host_id;
        let _ = *refresh.read();
        async move {
            match hid {
                Some(id) => get_host(id).await,
                None => Err(ServerFnError::new("invalid host ID")),
            }
        }
    })?;

    let data = match &*host.read() {
        Some(Ok(d)) => d.clone(),
        Some(Err(e)) => return rsx! { div { class: "text-danger", "Error: {e}" } },
        None => return rsx! { div { class: "text-fg-muted", "Loading..." } },
    };

    let is_cloudflare = data.kind == "cloudflare";

    rsx! {
        PageHeader { "{data.name}" }

        Card {
            div { class: "p-6 grid grid-cols-2 md:grid-cols-4 gap-4",
                div {
                    div { class: "text-sm text-fg-muted", "Kind" }
                    if is_cloudflare {
                        Badge { variant: BadgeVariant::Info, "Cloudflare" }
                    } else {
                        Badge { variant: BadgeVariant::Accent, "Proxy" }
                    }
                }
                div {
                    div { class: "text-sm text-fg-muted", "Organization" }
                    div { "{data.organization_name}" }
                }
                div {
                    div { class: "text-sm text-fg-muted", "Folders" }
                    div { "{data.folders.len()}" }
                }
            }
        }

        // Folders
        div { class: "mt-6 flex items-center justify-between",
            SectionHeading { "Folders" }
            if !is_cloudflare {
                Link {
                    to: crate::web::app::Route::WebspaceForm { host_id: data.id.to_string() },
                    class: "btn btn-primary",
                    "Add Folder"
                }
            }
        }
        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead {
                        tr {
                            Th { "Path" }
                            Th { "Name" }
                            Th { "Type" }
                            Th { "Runtime / Status" }
                        }
                    }
                    tbody {
                        if data.folders.is_empty() {
                            tr { td { class: "td text-fg-muted text-center", colspan: "4", "No folders" } }
                        }
                        for f in &data.folders {
                            {
                                let fid = f.id.to_string();
                                rsx! {
                                    tr { class: "cursor-pointer hover:bg-surface-2",
                                        onclick: move |_| { navigator().push(crate::web::app::Route::WebspaceDetail { id: fid.clone() }); },
                                        Td { class: "font-mono", "{f.path_prefix}" }
                                        Td { "{f.name}" }
                                        Td {
                                            match f.hosting_type.as_str() {
                                                "cloudflare_pages" => rsx! { Badge { variant: BadgeVariant::Info, "CF Pages" } },
                                                "local" => rsx! { Badge { "Local" } },
                                                "relay" => rsx! { Badge { variant: BadgeVariant::Accent, "Relay" } },
                                                "tunnel" => rsx! { Badge { variant: BadgeVariant::Accent, "Tunnel" } },
                                                _ => rsx! { span { "-" } },
                                            }
                                        }
                                        Td {
                                            if let Some(ref rt) = f.runtime {
                                                Badge { "{rt}" }
                                                if let Some(ref st) = f.local_status {
                                                    span { class: "text-sm text-fg-muted ml-2", "{st}" }
                                                }
                                            } else {
                                                span { class: "text-fg-muted", "-" }
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

        // Change Detection
        SectionHeading { class: "mt-6", "Change Detection" }
        ChangeDetectionSection {
            host_id: data.id,
            current_credential_id: data.changedetection_credential_id,
            current_credential_name: data.changedetection_credential_name.clone(),
        }
        Card { class: "mt-2",
            div { class: "p-4",
                Link {
                    to: crate::web::app::Route::WebspaceChangedetection { id: data.id.to_string() },
                    class: "text-brand underline text-sm",
                    "Settings & Notifications"
                }
            }
        }

        // Domain bindings
        SectionHeading { class: "mt-6", "Domain Bindings" }
        DomainBindingsSection {
            host_id: data.id,
            kind: data.kind.clone(),
            bindings: data.bindings.clone(),
        }

        // Settings
        SectionHeading { class: "mt-6", "Settings" }
        HostSettingsSection { host_id: data.id, current_name: data.name.clone() }
        MoveHostSection { host_id: data.id, current_org_id: data.organization_id }
        DeleteHostSection { host_id: data.id }
    }
}

#[component]
fn HostSettingsSection(host_id: Uuid, current_name: String) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let mut name = use_signal(move || current_name.clone());
    let mut saving = use_signal(|| false);
    let mut message = use_signal(|| None::<String>);

    rsx! {
        Card {
            div { class: "p-4 flex items-end gap-3",
                FormField { label: "Name",
                    input {
                        class: "input w-64",
                        value: "{name}",
                        oninput: move |evt| name.set(evt.value()),
                    }
                }
                Button {
                    variant: ButtonVariant::Primary,
                    disabled: name.read().is_empty() || *saving.read(),
                    onclick: move |_| {
                        let n = name.read().clone();
                        saving.set(true);
                        message.set(None);
                        spawn(async move {
                            match update_host_settings(host_id, n).await {
                                Ok(()) => { message.set(Some("Saved".into())); refresh += 1; }
                                Err(e) => message.set(Some(format!("{e}"))),
                            }
                            saving.set(false);
                        });
                    },
                    if *saving.read() { "Saving..." } else { "Save" }
                }
                if let Some(ref msg) = *message.read() {
                    span { class: "text-sm text-fg-muted", "{msg}" }
                }
            }
        }
    }
}

#[component]
fn MoveHostSection(host_id: Uuid, current_org_id: Uuid) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let orgs = use_server_future(list_host_move_target_orgs)?;
    let mut selected_org = use_signal(String::new);
    let mut moving = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    let targets: Vec<_> = match &*orgs.read() {
        Some(Ok(list)) => list.iter().filter(|o| o.id != current_org_id).cloned().collect(),
        _ => vec![],
    };
    if targets.is_empty() {
        return rsx! {};
    }

    rsx! {
        Card { class: "mt-2",
            div { class: "p-4 flex items-center justify-between gap-4",
                div {
                    div { class: "font-medium", "Move to another organization" }
                    div { class: "text-sm text-fg-muted", "Transfers this host and its folders to a different organization." }
                }
                div { class: "flex items-center gap-2",
                    select {
                        class: "input w-48",
                        value: "{selected_org}",
                        onchange: move |evt| selected_org.set(evt.value()),
                        option { value: "", "Select org..." }
                        for org in &targets {
                            option { value: "{org.id}", "{org.name}" }
                        }
                    }
                    Button {
                        variant: ButtonVariant::Danger,
                        disabled: selected_org.read().is_empty() || *moving.read(),
                        onclick: move |_| {
                            let target = selected_org.read().clone();
                            if let Ok(tid) = Uuid::parse_str(&target) {
                                moving.set(true);
                                error.set(None);
                                spawn(async move {
                                    match move_host(host_id, tid).await {
                                        Ok(()) => { refresh += 1; }
                                        Err(e) => { error.set(Some(format!("{e}"))); moving.set(false); }
                                    }
                                });
                            }
                        },
                        if *moving.read() { "Moving..." } else { "Move Host" }
                    }
                }
            }
            if let Some(err) = &*error.read() {
                div { class: "px-4 pb-4 text-danger text-sm", "{err}" }
            }
        }
    }
}

#[component]
fn DeleteHostSection(host_id: Uuid) -> Element {
    let mut deleting = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    rsx! {
        Card { class: "mt-2",
            div { class: "p-4 flex items-center justify-between",
                div {
                    div { class: "text-sm font-medium text-danger", "Delete this host" }
                    div { class: "text-sm text-fg-muted", "All folders, domain bindings, and deployments will be removed." }
                }
                Button {
                    variant: ButtonVariant::Danger,
                    disabled: *deleting.read(),
                    onclick: move |_| {
                        deleting.set(true);
                        error.set(None);
                        spawn(async move {
                            match delete_host(host_id).await {
                                Ok(()) => { navigator().push(crate::web::app::Route::WebspaceHostList {}); }
                                Err(e) => { error.set(Some(format!("{e}"))); deleting.set(false); }
                            }
                        });
                    },
                    if *deleting.read() { "Deleting..." } else { "Delete Host" }
                }
            }
            if let Some(err) = &*error.read() {
                div { class: "px-4 pb-4 text-danger text-sm", "{err}" }
            }
        }
    }
}

#[component]
fn ChangeDetectionSection(
    host_id: Uuid,
    current_credential_id: Option<Uuid>,
    current_credential_name: Option<String>,
) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let cd_creds = use_server_future(list_cd_creds)?;
    let cred_list: Vec<CredOption> = match &*cd_creds.read() {
        Some(Ok(c)) => c.clone(),
        _ => vec![],
    };

    let mut selected_cred =
        use_signal(move || current_credential_id.map(|id| id.to_string()).unwrap_or_default());
    let mut saving = use_signal(|| false);
    let mut result_msg = use_signal(|| None::<String>);

    rsx! {
        Card {
            div { class: "p-6 space-y-4",
                div { class: "flex items-center gap-3",
                    span { class: "text-sm text-fg-muted", "Current:" }
                    if let Some(ref name) = current_credential_name {
                        Badge { variant: BadgeVariant::Info, "{name}" }
                    } else {
                        span { class: "text-fg-muted text-sm", "Not configured" }
                    }
                }
                if cred_list.is_empty() {
                    div { class: "text-sm text-fg-muted", "No ChangeDetection.io credentials available. Create one first." }
                } else {
                    div { class: "flex items-end gap-3",
                        FormField { label: "Credential",
                            select {
                                class: "input w-64",
                                value: "{selected_cred}",
                                oninput: move |evt| selected_cred.set(evt.value()),
                                option { value: "", "None" }
                                for c in &cred_list {
                                    option { value: "{c.id}", "{c.name}" }
                                }
                            }
                        }
                        Button {
                            variant: ButtonVariant::Primary,
                            disabled: *saving.read(),
                            onclick: move |_| {
                                let cred_str = selected_cred.read().clone();
                                saving.set(true);
                                result_msg.set(None);
                                spawn(async move {
                                    let cid = uuid::Uuid::parse_str(&cred_str).ok();
                                    match set_host_changedetection(host_id, cid).await {
                                        Ok(()) => { result_msg.set(Some("Saved".into())); *refresh.write() += 1; }
                                        Err(e) => result_msg.set(Some(format!("Error: {e}"))),
                                    }
                                    saving.set(false);
                                });
                            },
                            if *saving.read() { "Saving..." } else { "Save" }
                        }
                    }
                }
                if let Some(ref msg) = *result_msg.read() {
                    div { class: "text-sm text-fg-muted", "{msg}" }
                }
            }
        }
    }
}

#[component]
fn DomainBindingsSection(host_id: Uuid, kind: String, bindings: Vec<DomainBinding>) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let is_cloudflare = kind == "cloudflare";
    let domains = use_server_future(move || async move { list_domains_for_binding(host_id).await })?;
    let domain_list = match &*domains.read() {
        Some(Ok(d)) => d.clone(),
        _ => vec![],
    };

    let mut selected_domain = use_signal(String::new);
    let mut adding = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut removing: Signal<Option<Uuid>> = use_signal(|| None);
    let mut fixing: Signal<Option<Uuid>> = use_signal(|| None);
    let mut rechecking: Signal<Option<Uuid>> = use_signal(|| None);

    rsx! {
        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead {
                        tr {
                            Th { "Hostname" }
                            Th { "Domain" }
                            Th { "CNAME" }
                            if is_cloudflare {
                                Th { "Verification" }
                            }
                            Th { "" }
                        }
                    }
                    tbody {
                        if bindings.is_empty() {
                            tr { td { class: "td text-fg-muted text-center", colspan: if is_cloudflare { "5" } else { "4" }, "No domains bound" } }
                        }
                        for b in &bindings {
                            {
                                let bid = b.binding_id;
                                let did = b.domain_id;
                                let sub_id = b.subdomain_id;
                                let hostname = b.hostname.clone();
                                let cname_ok = b.cname_ok;
                                let cf_status = b.cf_domain_status.clone();
                                let is_removing = *removing.read() == Some(bid);
                                let is_fixing = *fixing.read() == Some(bid);
                                let is_rechecking = *rechecking.read() == Some(bid);
                                rsx! {
                                    tr {
                                        Td { class: "font-mono", "{b.hostname}" }
                                        TdMuted { "{b.domain_name}" }
                                        Td {
                                            if cname_ok {
                                                Badge { variant: BadgeVariant::Success, "OK" }
                                            } else {
                                                div { class: "flex items-center gap-2",
                                                    Badge { variant: BadgeVariant::Danger, "Missing" }
                                                    Button {
                                                        variant: ButtonVariant::Secondary,
                                                        disabled: is_fixing,
                                                        onclick: {
                                                            let hostname = hostname.clone();
                                                            move |_| {
                                                                let hostname = hostname.clone();
                                                                fixing.set(Some(bid));
                                                                spawn(async move {
                                                                    let _ = fix_cname(host_id, did, sub_id, hostname).await;
                                                                    fixing.set(None);
                                                                    refresh += 1;
                                                                });
                                                            }
                                                        },
                                                        if is_fixing { "Fixing..." } else { "Fix" }
                                                    }
                                                }
                                            }
                                        }
                                        if is_cloudflare {
                                            Td {
                                                match cf_status.as_deref() {
                                                    Some("active") => rsx! { Badge { variant: BadgeVariant::Success, "Active" } },
                                                    Some("pending") | Some("verifying") => rsx! {
                                                        div { class: "flex items-center gap-2",
                                                            Badge { variant: BadgeVariant::Warn, {cf_status.as_deref().unwrap_or("pending")} }
                                                            Button {
                                                                variant: ButtonVariant::Secondary,
                                                                disabled: is_rechecking,
                                                                onclick: {
                                                                    let hostname = hostname.clone();
                                                                    move |_| {
                                                                        let hostname = hostname.clone();
                                                                        rechecking.set(Some(bid));
                                                                        spawn(async move {
                                                                            let _ = recheck_custom_domain(host_id, hostname).await;
                                                                            rechecking.set(None);
                                                                            refresh += 1;
                                                                        });
                                                                    }
                                                                },
                                                                if is_rechecking { "..." } else { "Recheck" }
                                                            }
                                                        }
                                                    },
                                                    Some(s) => rsx! { Badge { "{s}" } },
                                                    None => rsx! {
                                                        div { class: "flex items-center gap-2",
                                                            Badge { variant: BadgeVariant::Danger, "Not on CF" }
                                                            Button {
                                                                variant: ButtonVariant::Secondary,
                                                                disabled: is_rechecking,
                                                                onclick: {
                                                                    let hostname = hostname.clone();
                                                                    move |_| {
                                                                        let hostname = hostname.clone();
                                                                        rechecking.set(Some(bid));
                                                                        spawn(async move {
                                                                            let _ = recheck_custom_domain(host_id, hostname).await;
                                                                            rechecking.set(None);
                                                                            refresh += 1;
                                                                        });
                                                                    }
                                                                },
                                                                if is_rechecking { "..." } else { "Register" }
                                                            }
                                                        }
                                                    },
                                                }
                                            }
                                        }
                                        Td {
                                            Button {
                                                variant: ButtonVariant::Danger,
                                                disabled: is_removing,
                                                onclick: {
                                                    let hostname = hostname.clone();
                                                    move |_| {
                                                        let hostname = hostname.clone();
                                                        removing.set(Some(bid));
                                                        spawn(async move {
                                                            let _ = unbind_domain(host_id, bid, hostname).await;
                                                            removing.set(None);
                                                            refresh += 1;
                                                        });
                                                    }
                                                },
                                                if is_removing { "Removing..." } else { "Remove" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            div { class: "p-4 border-t border-line-soft",
                div { class: "flex items-end gap-3",
                    FormField { label: "Bind Domain / Subdomain",
                        select {
                            class: "input",
                            value: "{selected_domain}",
                            oninput: move |evt| selected_domain.set(evt.value()),
                            option { value: "", "Select..." }
                            for d in &domain_list {
                                option { value: "{d.id}||{d.name}", "{d.name} (root)" }
                                for s in &d.subdomains {
                                    {
                                        let hostname = if s.name == "@" { d.name.clone() } else { format!("{}.{}", s.name, d.name) };
                                        rsx! { option { value: "{d.id}|{s.id}|{hostname}", "  {hostname}" } }
                                    }
                                }
                            }
                        }
                    }
                    Button {
                        variant: ButtonVariant::Primary,
                        disabled: selected_domain.read().is_empty() || *adding.read(),
                        onclick: {
                            let sel = selected_domain.read().clone();
                            move |_| {
                                let sel = sel.clone();
                                adding.set(true);
                                error.set(None);
                                spawn(async move {
                                    let parts: Vec<&str> = sel.split('|').collect();
                                    if parts.len() == 3 {
                                        let domain_id = Uuid::parse_str(parts[0]).ok();
                                        let subdomain_id = Uuid::parse_str(parts[1]).ok();
                                        let hostname = parts[2].to_string();
                                        if let Some(domain_id) = domain_id {
                                            match bind_domain(host_id, domain_id, subdomain_id, hostname).await {
                                                Ok(()) => { selected_domain.set(String::new()); refresh += 1; }
                                                Err(e) => error.set(Some(format!("{e}"))),
                                            }
                                        }
                                    }
                                    adding.set(false);
                                });
                            }
                        },
                        if *adding.read() { "Binding..." } else { "Bind" }
                    }
                }
                if let Some(ref e) = *error.read() {
                    div { class: "text-danger text-sm mt-2", "{e}" }
                }
            }
        }
    }
}
