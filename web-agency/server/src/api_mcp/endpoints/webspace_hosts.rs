//! Webspace-host endpoints.

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
pub struct HostRow {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub organization_name: String,
    pub folder_count: i64,
    pub hostname: Option<String>,
    pub has_changedetection: bool,
    pub has_missing_cname: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HostListInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HostCreateInput {
    pub organization_id: Uuid,
    pub name: String,
    /// "proxy" or "cloudflare".
    pub kind: String,
    /// Required when `kind == "cloudflare"`.
    #[serde(default)]
    pub cloudflare_credential_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebspaceHostData {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub organization_id: Uuid,
    pub organization_name: String,
    pub is_org_admin: bool,
    pub folders: Vec<FolderRow>,
    pub bindings: Vec<DomainBinding>,
    pub changedetection_credential_id: Option<Uuid>,
    pub changedetection_credential_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FolderRow {
    pub id: Uuid,
    pub name: String,
    pub path_prefix: String,
    pub hosting_type: String,
    pub runtime: Option<String>,
    pub local_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DomainBinding {
    pub binding_id: Uuid,
    pub domain_id: Uuid,
    pub subdomain_id: Option<Uuid>,
    pub domain_name: String,
    pub subdomain_name: Option<String>,
    pub hostname: String,
    pub cname_ok: bool,
    /// CF Pages custom domain verification status (cloudflare hosts only).
    pub cf_domain_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HostGetInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HostUpdateInput {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HostMoveInput {
    pub id: Uuid,
    pub target_org_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HostDeleteInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HostBindDomainInput {
    pub id: Uuid,
    pub domain_id: Uuid,
    #[serde(default)]
    pub subdomain_id: Option<Uuid>,
    pub hostname: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HostUnbindDomainInput {
    pub id: Uuid,
    pub binding_id: Uuid,
    pub hostname: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HostFixCnameInput {
    pub id: Uuid,
    pub domain_id: Uuid,
    #[serde(default)]
    pub subdomain_id: Option<Uuid>,
    pub hostname: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HostRecheckCustomDomainInput {
    pub id: Uuid,
    pub hostname: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HostSetChangedetectionInput {
    pub id: Uuid,
    /// `None` clears the assignment.
    #[serde(default)]
    pub credential_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HostBulkSetChangedetectionInput {
    pub host_ids: Vec<Uuid>,
    pub credential_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HostBulkClearChangedetectionInput {
    pub host_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BulkOpResult {
    pub succeeded: usize,
    pub failed: usize,
}

/// List webspace hosts the caller may see (admins: all; else their orgs').
#[api_mcp_dioxus_server(server = "list_hosts")]
pub async fn host_list(
    pool: &sqlx::PgPool,
    principal: &Principal,
    _input: HostListInput,
) -> Result<Vec<HostRow>, ApiError> {
    // hostname: all bound (domain, subdomain) FQDNs for the host, comma-joined.
    let hostname_subquery = "(SELECT string_agg(CASE WHEN s.name IS NOT NULL AND s.name != '@' \
            THEN s.name || '.' || d.name ELSE d.name END, ', ' ORDER BY d.name) \
         FROM webspace_host_domains whd \
         JOIN domains d ON d.id = whd.domain_id \
         LEFT JOIN subdomains s ON s.id = whd.subdomain_id \
         WHERE whd.webspace_host_id = h.id)";

    // has_missing_cname: at least one domain binding without a matching CNAME record.
    let missing_cname_subquery = "\
        EXISTS( \
            SELECT 1 FROM webspace_host_domains whd2 \
            JOIN domains d2 ON d2.id = whd2.domain_id \
            LEFT JOIN subdomains s2 ON s2.id = whd2.subdomain_id \
            WHERE whd2.webspace_host_id = h.id \
              AND NOT EXISTS( \
                  SELECT 1 FROM dns_records dr \
                  WHERE dr.domain_id = d2.id \
                    AND dr.record_type = 'CNAME' \
                    AND dr.name = COALESCE(s2.name, '@') \
              ) \
        )";

    type Row = (
        Uuid,
        String,
        String,
        String,
        i64,
        Option<String>,
        bool,
        bool,
    );
    let query = format!(
        "SELECT h.id, h.name, h.kind, o.name, \
         (SELECT count(*) FROM webspaces w WHERE w.webspace_host_id = h.id), \
         {hostname_subquery}, \
         h.changedetection_credential_id IS NOT NULL, \
         {missing_cname_subquery} \
         FROM webspace_hosts h JOIN organizations o ON o.id = h.organization_id"
    );

    let rows = match principal.read_filter() {
        None => {
            sqlx::query_as::<_, Row>(&format!("{query} ORDER BY h.name"))
                .fetch_all(pool)
                .await
        }
        Some(org_ids) => {
            sqlx::query_as::<_, Row>(&format!(
                "{query} WHERE h.organization_id = ANY($1) ORDER BY h.name"
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
                kind,
                organization_name,
                folder_count,
                hostname,
                has_changedetection,
                has_missing_cname,
            )| HostRow {
                id,
                name,
                kind,
                organization_name,
                folder_count,
                hostname,
                has_changedetection,
                has_missing_cname,
            },
        )
        .collect())
}

/// Create a webspace host (requires org write). For `kind == "cloudflare"` a
/// Cloudflare Pages project is found-or-created and a root folder is added.
/// Returns the new host id.
#[api_mcp_dioxus_server(server = "create_host")]
pub async fn host_create(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: HostCreateInput,
) -> Result<Uuid, ApiError> {
    principal.require_write(&input.organization_id)?;

    if input.kind != "proxy" && input.kind != "cloudflare" {
        return Err(ApiError::bad_request(
            "invalid kind (expected 'proxy' or 'cloudflare')",
        ));
    }

    let host_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO webspace_hosts (organization_id, name, kind) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(input.organization_id)
    .bind(&input.name)
    .bind(&input.kind)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::internal(format!("failed to create host: {e}")))?;

    if input.kind == "cloudflare" {
        let cred_id = input
            .cloudflare_credential_id
            .ok_or_else(|| ApiError::bad_request("Cloudflare credential required"))?;
        let (client, account_id) = crate::credentials::cf_client_with_account(pool, cred_id)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))?;
        let project = match client.get_pages_project(&account_id, &input.name).await {
            Ok(p) => p,
            Err(_) => client
                .create_pages_project(&account_id, &input.name, "main")
                .await
                .map_err(|e| ApiError::internal(format!("failed to create Pages project: {e}")))?,
        };
        sqlx::query(
            "INSERT INTO webspaces (organization_id, webspace_host_id, name, path_prefix, hosting_type, cloudflare_pages_project, cloudflare_pages_project_id, cloudflare_credential_id) \
             VALUES ($1, $2, $3, '/', 'cloudflare_pages', $4, $5, $6)",
        )
        .bind(input.organization_id)
        .bind(host_id)
        .bind(&input.name)
        .bind(&input.name)
        .bind(&project.id)
        .bind(cred_id)
        .execute(pool)
        .await
        .map_err(super::internal)?;
    }

    crate::api::internal::notify_proxy_reload();
    Ok(host_id)
}

// ── Server helpers ────────────────────────────────────────────────────

/// The host's owning organization + kind, or 404. Complements
/// `super::owning_org` where the handler also needs `kind`.
#[cfg(feature = "server")]
pub(crate) async fn host_org(
    pool: &sqlx::PgPool,
    host_id: Uuid,
) -> Result<(Uuid, String), ApiError> {
    sqlx::query_as::<_, (Uuid, String)>(
        "SELECT organization_id, kind FROM webspace_hosts WHERE id = $1",
    )
    .bind(host_id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?
    .ok_or_else(|| ApiError::not_found("host not found"))
}

/// The Cloudflare Pages project + credential for a cloudflare host live on its
/// single main-folder webspace.
#[cfg(feature = "server")]
async fn cloudflare_folder(pool: &sqlx::PgPool, host_id: Uuid) -> Result<(String, Uuid), ApiError> {
    let row = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
        "SELECT cloudflare_pages_project, cloudflare_credential_id FROM webspaces \
         WHERE webspace_host_id = $1 AND path_prefix = '/'",
    )
    .bind(host_id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?
    .ok_or_else(|| ApiError::internal("cloudflare host has no main-folder"))?;
    match row {
        (Some(project), Some(cred)) => Ok((project, cred)),
        _ => Err(ApiError::internal("cloudflare host folder is not deployed")),
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
) -> Result<(), ApiError> {
    let domain_cf = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
        "SELECT cloudflare_zone_id, cloudflare_credential_id FROM domains WHERE id = $1",
    )
    .bind(domain_id)
    .fetch_one(pool)
    .await
    .map_err(super::internal)?;

    let (zone_id, domain_cred_id) = match domain_cf {
        (Some(z), Some(c)) => (z, c),
        _ => return Err(ApiError::bad_request("domain not deployed to Cloudflare")),
    };

    let client = crate::credentials::cf_client(pool, domain_cred_id)
        .await
        .map_err(|e| ApiError::internal(format!("{e}")))?;

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
            tracing::info!(
                "created CNAME {hostname} → {cname_target} (CF record {})",
                created.id
            );
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
        .map_err(super::internal)?
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
async fn remove_host_cname(
    pool: &sqlx::PgPool,
    domain_id: Uuid,
    hostname: &str,
    cname_target: &str,
) {
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
                        let _ =
                            sqlx::query("DELETE FROM dns_records WHERE cloudflare_record_id = $1")
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

// ── Item endpoints ────────────────────────────────────────────────────

/// Get a webspace host with its folders, domain bindings (incl. CNAME and CF
/// custom-domain status), and ChangeDetection assignment (requires org read).
#[api_mcp_dioxus_server(server = "get_host")]
pub async fn host_get(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: HostGetInput,
) -> Result<WebspaceHostData, ApiError> {
    let (id, name, kind, org_id, cd_cred_id) =
        sqlx::query_as::<_, (Uuid, String, String, Uuid, Option<Uuid>)>(
            "SELECT id, name, kind, organization_id, changedetection_credential_id \
             FROM webspace_hosts WHERE id = $1",
        )
        .bind(input.id)
        .fetch_optional(pool)
        .await
        .map_err(super::internal)?
        .ok_or_else(|| ApiError::not_found("host not found"))?;

    principal.require_read(&org_id)?;

    let org_name = sqlx::query_scalar::<_, String>("SELECT name FROM organizations WHERE id = $1")
        .bind(org_id)
        .fetch_one(pool)
        .await
        .map_err(super::internal)?;

    let folders =
        sqlx::query_as::<_, (Uuid, String, String, String, Option<String>, Option<String>)>(
            "SELECT id, name, path_prefix, hosting_type, runtime, local_status \
             FROM webspaces WHERE webspace_host_id = $1 ORDER BY path_prefix",
        )
        .bind(input.id)
        .fetch_all(pool)
        .await
        .map_err(super::internal)?
        .into_iter()
        .map(
            |(id, name, path_prefix, hosting_type, runtime, local_status)| FolderRow {
                id,
                name,
                path_prefix,
                hosting_type,
                runtime,
                local_status,
            },
        )
        .collect::<Vec<_>>();

    let binding_rows = sqlx::query_as::<_, (Uuid, Uuid, Option<Uuid>, String, Option<String>)>(
        "SELECT whd.id, whd.domain_id, whd.subdomain_id, d.name, s.name \
         FROM webspace_host_domains whd \
         JOIN domains d ON d.id = whd.domain_id \
         LEFT JOIN subdomains s ON s.id = whd.subdomain_id \
         WHERE whd.webspace_host_id = $1 ORDER BY d.name",
    )
    .bind(input.id)
    .fetch_all(pool)
    .await
    .map_err(super::internal)?;

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
            .fetch_one(pool)
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
            .fetch_one(pool)
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
        if let Ok((project_name, cred_id)) = cloudflare_folder(pool, input.id).await {
            if let Ok((client, account_id)) =
                crate::credentials::cf_client_with_account(pool, cred_id).await
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
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    // The web session exposed org-admin role directly; a Principal doesn't, so
    // look the caller's org role up by subject (email). Token principals have
    // no users row and are org-admin only when globally admin.
    let is_org_admin = principal.admin
        || sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM organization_members om \
             JOIN users u ON u.id = om.user_id \
             WHERE u.email = $1 AND om.organization_id = $2 AND om.role = 'admin')",
        )
        .bind(&principal.subject)
        .bind(org_id)
        .fetch_one(pool)
        .await
        .map_err(super::internal)?;

    Ok(WebspaceHostData {
        id,
        name,
        kind,
        organization_id: org_id,
        organization_name: org_name,
        is_org_admin,
        folders,
        bindings,
        changedetection_credential_id: cd_cred_id,
        changedetection_credential_name: cd_cred_name,
    })
}

/// Rename a webspace host (requires org write). The CF Pages project name
/// lives on the folder; the host name is for display.
#[api_mcp_dioxus_server(server = "update_host_settings")]
pub async fn host_update(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: HostUpdateInput,
) -> Result<(), ApiError> {
    let (org_id, _) = host_org(pool, input.id).await?;
    principal.require_write(&org_id)?;

    sqlx::query("UPDATE webspace_hosts SET name = $1, updated_at = now() WHERE id = $2")
        .bind(&input.name)
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(|e| match e.as_database_error() {
            Some(db) if db.is_unique_violation() => {
                ApiError::conflict("a host with that name already exists in this organization")
            }
            _ => super::internal(e),
        })?;
    Ok(())
}

/// Move a host (and its folders) to another organization (requires write on
/// both orgs).
#[api_mcp_dioxus_server(server = "move_host")]
pub async fn host_move(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: HostMoveInput,
) -> Result<(), ApiError> {
    let (org_id, _) = host_org(pool, input.id).await?;
    principal.require_write(&org_id)?;
    principal.require_write(&input.target_org_id)?;

    if org_id == input.target_org_id {
        return Err(ApiError::bad_request(
            "host is already in that organization",
        ));
    }

    // Host and its child folders both carry organization_id; move them together.
    // Their org-scoped UNIQUE(name) constraints catch collisions and roll back.
    // ponytail: domain bindings keep pointing at source-org domains; rebinding is manual.
    let mut tx = pool.begin().await.map_err(super::internal)?;
    sqlx::query("UPDATE webspace_hosts SET organization_id = $1, updated_at = now() WHERE id = $2")
        .bind(input.target_org_id)
        .bind(input.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| match e.as_database_error() {
            Some(db) if db.is_unique_violation() => ApiError::conflict(
                "a host with the same name already exists in the target organization",
            ),
            _ => super::internal(e),
        })?;
    sqlx::query("UPDATE webspaces SET organization_id = $1 WHERE webspace_host_id = $2")
        .bind(input.target_org_id)
        .bind(input.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| match e.as_database_error() {
            Some(db) if db.is_unique_violation() => ApiError::conflict(
                "a folder with the same name already exists in the target organization",
            ),
            _ => super::internal(e),
        })?;
    tx.commit().await.map_err(super::internal)?;

    crate::api::internal::notify_proxy_reload();
    Ok(())
}

/// Delete a host (requires org write). Cleans up bound CNAMEs and CF custom
/// domains first; folders and bindings cascade on FK delete.
#[api_mcp_dioxus_server(server = "delete_host")]
pub async fn host_delete(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: HostDeleteInput,
) -> Result<(), ApiError> {
    let (org_id, kind) = host_org(pool, input.id).await?;
    principal.require_write(&org_id)?;

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
    .bind(input.id)
    .fetch_all(pool)
    .await
    .map_err(super::internal)?;

    let cf = if kind == "cloudflare" {
        cloudflare_folder(pool, input.id).await.ok()
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
                crate::credentials::cf_client_with_account(pool, *cred_id).await
            {
                let _ = client
                    .remove_pages_custom_domain(&account_id, project_name, &hostname)
                    .await;
            }
            let cname_target = format!("{project_name}.pages.dev");
            remove_host_cname(pool, domain_id, &hostname, &cname_target).await;
        } else if let Some(ad) = agency_domain() {
            remove_host_cname(pool, domain_id, &hostname, &ad).await;
        }
    }

    sqlx::query("DELETE FROM webspace_hosts WHERE id = $1")
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(super::internal)?;

    crate::api::internal::notify_proxy_reload();
    Ok(())
}

/// Bind a domain (or subdomain) to this host and create the appropriate CNAME
/// (requires org write). Proxy hosts also get certs issued in the background.
#[api_mcp_dioxus_server(server = "bind_domain")]
pub async fn host_bind_domain(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: HostBindDomainInput,
) -> Result<(), ApiError> {
    let (org_id, kind) = host_org(pool, input.id).await?;
    principal.require_write(&org_id)?;

    sqlx::query(
        "INSERT INTO webspace_host_domains (webspace_host_id, domain_id, subdomain_id) \
         VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(input.id)
    .bind(input.domain_id)
    .bind(input.subdomain_id)
    .execute(pool)
    .await
    .map_err(super::internal)?;

    if kind == "cloudflare" {
        let (project_name, cred_id) = cloudflare_folder(pool, input.id).await?;
        let (client, account_id) = crate::credentials::cf_client_with_account(pool, cred_id)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))?;
        if let Err(e) = client
            .add_pages_custom_domain(&account_id, &project_name, &input.hostname)
            .await
        {
            tracing::warn!(
                "failed to add custom domain {} to Pages: {e}",
                input.hostname
            );
        }
        let cname_target = client
            .get_pages_project(&account_id, &project_name)
            .await
            .map_err(|e| ApiError::internal(format!("failed to fetch Pages project: {e}")))?
            .subdomain
            .ok_or_else(|| ApiError::internal("Pages project has no subdomain"))?;
        create_host_cname(
            pool,
            input.domain_id,
            input.subdomain_id,
            &input.hostname,
            &cname_target,
            &format!("Pages: {project_name}"),
        )
        .await?;
    } else if let Some(ad) = agency_domain() {
        create_host_cname(
            pool,
            input.domain_id,
            input.subdomain_id,
            &input.hostname,
            &ad,
            "Proxy host",
        )
        .await?;
        // Provision certs for all of the host's bound domains in the background
        // (each FQDN needs its own cert; ACME takes ~a minute per domain).
        let pool = pool.clone();
        let host_id = input.id;
        tokio::spawn(async move {
            if let Err(e) = crate::api::acme::issue_host_certs(&pool, host_id).await {
                tracing::error!("host cert issuance failed: {e}");
            }
            crate::api::internal::notify_proxy_reload();
        });
    }

    crate::api::internal::notify_proxy_reload();
    Ok(())
}

/// Remove a domain binding and its CNAME (requires org write).
#[api_mcp_dioxus_server(server = "unbind_domain")]
pub async fn host_unbind_domain(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: HostUnbindDomainInput,
) -> Result<(), ApiError> {
    let (org_id, kind) = host_org(pool, input.id).await?;
    principal.require_write(&org_id)?;

    let binding = sqlx::query_as::<_, (Uuid, Option<Uuid>)>(
        "SELECT domain_id, subdomain_id FROM webspace_host_domains WHERE id = $1",
    )
    .bind(input.binding_id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?;

    if let Some((domain_id, _)) = binding {
        if kind == "cloudflare" {
            if let Ok((project_name, cred_id)) = cloudflare_folder(pool, input.id).await {
                if let Ok((client, account_id)) =
                    crate::credentials::cf_client_with_account(pool, cred_id).await
                {
                    let _ = client
                        .remove_pages_custom_domain(&account_id, &project_name, &input.hostname)
                        .await;
                }
                let cname_target = format!("{project_name}.pages.dev");
                remove_host_cname(pool, domain_id, &input.hostname, &cname_target).await;
            }
        } else if let Some(ad) = agency_domain() {
            remove_host_cname(pool, domain_id, &input.hostname, &ad).await;
        }
    }

    sqlx::query("DELETE FROM webspace_host_domains WHERE id = $1")
        .bind(input.binding_id)
        .execute(pool)
        .await
        .map_err(super::internal)?;

    crate::api::internal::notify_proxy_reload();
    Ok(())
}

/// Re-create the CNAME for a binding, kind-aware (requires org write).
#[api_mcp_dioxus_server(server = "fix_cname")]
pub async fn host_fix_cname(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: HostFixCnameInput,
) -> Result<(), ApiError> {
    let (org_id, kind) = host_org(pool, input.id).await?;
    principal.require_write(&org_id)?;

    if kind == "cloudflare" {
        let (project_name, cred_id) = cloudflare_folder(pool, input.id).await?;
        let (client, account_id) = crate::credentials::cf_client_with_account(pool, cred_id)
            .await
            .map_err(|e| ApiError::internal(format!("{e}")))?;
        let cname_target = client
            .get_pages_project(&account_id, &project_name)
            .await
            .map_err(|e| ApiError::internal(format!("failed to fetch Pages project: {e}")))?
            .subdomain
            .ok_or_else(|| ApiError::internal("Pages project has no subdomain"))?;
        create_host_cname(
            pool,
            input.domain_id,
            input.subdomain_id,
            &input.hostname,
            &cname_target,
            &format!("Pages: {project_name}"),
        )
        .await
    } else {
        let ad = agency_domain().ok_or_else(|| ApiError::internal("no proxy config"))?;
        create_host_cname(
            pool,
            input.domain_id,
            input.subdomain_id,
            &input.hostname,
            &ad,
            "Proxy host",
        )
        .await
    }
}

/// Trigger a recheck of a custom domain's CF Pages verification status
/// (requires org write); registers the domain if CF doesn't know it yet.
#[api_mcp_dioxus_server(server = "recheck_custom_domain")]
pub async fn host_recheck_custom_domain(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: HostRecheckCustomDomainInput,
) -> Result<String, ApiError> {
    let (org_id, _) = host_org(pool, input.id).await?;
    principal.require_write(&org_id)?;

    let (project_name, cred_id) = cloudflare_folder(pool, input.id).await?;
    let (client, account_id) = crate::credentials::cf_client_with_account(pool, cred_id)
        .await
        .map_err(|e| ApiError::internal(format!("{e}")))?;

    match client
        .retry_pages_custom_domain(&account_id, &project_name, &input.hostname)
        .await
    {
        Ok(dom) => Ok(format!(
            "Validation retried — status: {}",
            dom.status.as_deref().unwrap_or("pending")
        )),
        Err(e) => match client
            .add_pages_custom_domain(&account_id, &project_name, &input.hostname)
            .await
        {
            Ok(dom) => Ok(format!(
                "Domain added — status: {}",
                dom.status.as_deref().unwrap_or("pending")
            )),
            Err(_) => Err(ApiError::internal(format!("retry failed: {e}"))),
        },
    }
}

/// Set or clear the host's ChangeDetection credential (requires org write).
#[api_mcp_dioxus_server(server = "set_host_changedetection")]
pub async fn host_set_changedetection(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: HostSetChangedetectionInput,
) -> Result<(), ApiError> {
    let (org_id, _) = host_org(pool, input.id).await?;
    principal.require_write(&org_id)?;

    sqlx::query(
        "UPDATE webspace_hosts SET changedetection_credential_id = $1, updated_at = now() WHERE id = $2",
    )
    .bind(input.credential_id)
    .bind(input.id)
    .execute(pool)
    .await
    .map_err(super::internal)?;
    Ok(())
}

// ── Bulk (collection) endpoints ───────────────────────────────────────

/// Assign a ChangeDetection credential to many hosts at once (admin only).
#[api_mcp_dioxus_server(server = "bulk_assign_changedetection")]
pub async fn host_bulk_set_changedetection(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: HostBulkSetChangedetectionInput,
) -> Result<BulkOpResult, ApiError> {
    principal.require_admin()?;

    let result = sqlx::query(
        "UPDATE webspace_hosts SET changedetection_credential_id = $1, updated_at = now() \
         WHERE id = ANY($2)",
    )
    .bind(input.credential_id)
    .bind(&input.host_ids)
    .execute(pool)
    .await
    .map_err(super::internal)?;

    Ok(BulkOpResult {
        succeeded: result.rows_affected() as usize,
        failed: 0,
    })
}

/// Clear the ChangeDetection credential on many hosts at once (admin only).
#[api_mcp_dioxus_server(server = "bulk_remove_changedetection")]
pub async fn host_bulk_clear_changedetection(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: HostBulkClearChangedetectionInput,
) -> Result<BulkOpResult, ApiError> {
    principal.require_admin()?;

    let result = sqlx::query(
        "UPDATE webspace_hosts SET changedetection_credential_id = NULL, updated_at = now() \
         WHERE id = ANY($1) AND changedetection_credential_id IS NOT NULL",
    )
    .bind(&input.host_ids)
    .execute(pool)
    .await
    .map_err(super::internal)?;

    Ok(BulkOpResult {
        succeeded: result.rows_affected() as usize,
        failed: 0,
    })
}
