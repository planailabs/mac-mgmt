use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Badge, BadgeVariant, Button, ButtonVariant, Card, FormField, PageHeader, SectionHeading, Td, TdMuted, Th};

// ── Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WebspaceData {
    id: Uuid,
    name: String,
    hosting_type: String,
    cloudflare_pages_project: Option<String>,
    cloudflare_credential_id: Option<Uuid>,
    runtime: Option<String>,
    local_status: Option<String>,
    relay_url: Option<String>,
    organization_id: Uuid,
    organization_name: String,
    /// Whether the current user can manage tokens for this webspace's org.
    is_org_admin: bool,
    auth_mode: String,
    auth_basic_list_name: Option<String>,
    bindings: Vec<DomainBinding>,
    // Live CF Pages info
    pages_subdomain: Option<String>,
    production_branch: Option<String>,
    git_source: Option<GitRepoInfo>,
    build_config: Option<BuildConfigInfo>,
    changedetection_credential_id: Option<Uuid>,
    changedetection_credential_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeploymentRow {
    id: Uuid,
    status: String,
    error_message: Option<String>,
    tarball_size: Option<i64>,
    created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenCreateResult {
    token: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct GitRepoInfo {
    provider: String, // "github" or "gitlab"
    owner: String,
    repo: String,
    production_branch: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct BuildConfigInfo {
    build_command: Option<String>,
    destination_dir: Option<String>,
    root_dir: Option<String>,
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
    /// CF Pages custom domain verification status (active, pending, verifying, etc.)
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PagesDeployResult {
    project_name: String,
    subdomain: Option<String>,
}

// ── Server functions ──────────────────────────────────────────────────

#[server]
async fn get_webspace(webspace_id: Uuid) -> Result<WebspaceData, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let row = sqlx::query_as::<_, (Uuid, String, String, Option<String>, Option<Uuid>, Option<String>, Option<String>, Uuid, Option<String>, String, Option<Uuid>, Option<Uuid>)>(
        "SELECT w.id, w.name, w.hosting_type, w.cloudflare_pages_project, w.cloudflare_credential_id, \
         w.runtime, w.local_status, w.organization_id, w.relay_url, w.auth_mode, w.auth_basic_list_id, \
         w.changedetection_credential_id \
         FROM webspaces w WHERE w.id = $1",
    )
    .bind(webspace_id).fetch_optional(&pool).await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("webspace not found"))?;

    let (id, name, hosting_type, cf_project, cf_cred_id, runtime, local_status, org_id, relay_url, auth_mode, auth_basic_list_id, cd_cred_id) = row;

    // Fetch basic auth list name if set
    let auth_basic_list_name = if let Some(list_id) = auth_basic_list_id {
        sqlx::query_scalar::<_, String>("SELECT name FROM basic_auth_lists WHERE id = $1")
            .bind(list_id).fetch_optional(&pool).await.ok().flatten()
    } else {
        None
    };

    use crate::web::user::WebUserExt;
    user.require_org_read(&org_id)?;

    let org_name = sqlx::query_scalar::<_, String>("SELECT name FROM organizations WHERE id = $1")
        .bind(org_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let binding_rows = sqlx::query_as::<_, (Uuid, Uuid, Option<Uuid>, String, Option<String>)>(
        "SELECT wd.id, wd.domain_id, wd.subdomain_id, d.name, s.name \
         FROM webspace_domains wd \
         JOIN domains d ON d.id = wd.domain_id \
         LEFT JOIN subdomains s ON s.id = wd.subdomain_id \
         WHERE wd.webspace_id = $1 ORDER BY d.name",
    )
    .bind(webspace_id).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    // For relay/tunnel webspaces the CNAME target is the agency domain
    let agency_domain = if hosting_type == "relay" || hosting_type == "tunnel" {
        crate::config::config().proxy.as_ref().map(|p| p.agency_domain.clone())
    } else {
        None
    };

    let mut bindings = Vec::new();
    for (binding_id, domain_id, subdomain_id, domain_name, subdomain_name) in binding_rows {
        let hostname = match &subdomain_name {
            Some(sub) if sub != "@" => format!("{sub}.{domain_name}"),
            _ => domain_name.clone(),
        };
        let sub_name = subdomain_name.as_deref().unwrap_or("@");
        // Check CNAME: *.pages.dev for pages, agency_domain for relay/tunnel
        let cname_ok = if let Some(ref ad) = agency_domain {
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM dns_records WHERE domain_id = $1 AND name = $2 \
                 AND record_type = 'CNAME' AND record_value = $3)",
            )
            .bind(domain_id).bind(sub_name).bind(ad)
            .fetch_one(&pool).await.unwrap_or(false)
        } else {
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM dns_records WHERE domain_id = $1 AND name = $2 \
                 AND record_type = 'CNAME' AND record_value LIKE '%.pages.dev')",
            )
            .bind(domain_id).bind(sub_name)
            .fetch_one(&pool).await.unwrap_or(false)
        };

        bindings.push(DomainBinding { binding_id, domain_id, subdomain_id, domain_name, subdomain_name, hostname, cname_ok, cf_domain_status: None });
    }

    // Fetch live CF Pages info
    let mut pages_subdomain = None;
    let mut production_branch = None;
    let mut git_source = None;
    let mut build_config_info = None;

    if let (Some(project_name), Some(cred_id)) = (&cf_project, cf_cred_id) {
        if let Ok((client, account_id)) = build_cf_pages_client(&pool, cred_id).await {
            if let Ok(project) = client.get_pages_project(&account_id, project_name).await {
                pages_subdomain = project.subdomain;
                production_branch = project.production_branch;
                if let Some(src) = &project.source {
                    let cfg = src.get("config");
                    if cfg.is_some() {
                        git_source = Some(GitRepoInfo {
                            provider: src.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            owner: cfg.and_then(|c| c.get("owner")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            repo: cfg.and_then(|c| c.get("repo_name")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            production_branch: cfg.and_then(|c| c.get("production_branch")).and_then(|v| v.as_str()).unwrap_or("main").to_string(),
                        });
                    }
                }
                if let Some(bc) = &project.build_config {
                    build_config_info = Some(BuildConfigInfo {
                        build_command: bc.get("build_command").and_then(|v| v.as_str()).map(String::from),
                        destination_dir: bc.get("destination_dir").and_then(|v| v.as_str()).map(String::from),
                        root_dir: bc.get("root_dir").and_then(|v| v.as_str()).map(String::from),
                    });
                }
            }

            // Fetch custom domain verification statuses
            match client.list_pages_custom_domains(&account_id, project_name).await {
                Ok(cf_domains) => {
                    for binding in &mut bindings {
                        let hostname_lower = binding.hostname.to_lowercase();
                        if let Some(cf_dom) = cf_domains.iter().find(|d| d.name.to_lowercase() == hostname_lower) {
                            binding.cf_domain_status = cf_dom.status.clone();
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(project = %project_name, error = %e, "failed to fetch Pages custom domains");
                }
            }
        }
    }

    // Fetch changedetection credential name
    let cd_cred_name = if let Some(cid) = cd_cred_id {
        sqlx::query_scalar::<_, String>("SELECT name FROM credentials WHERE id = $1")
            .bind(cid).fetch_optional(&pool).await.ok().flatten()
    } else {
        None
    };

    Ok(WebspaceData {
        id, name, hosting_type, cloudflare_pages_project: cf_project,
        cloudflare_credential_id: cf_cred_id, runtime, local_status, relay_url,
        organization_id: org_id, organization_name: org_name,
        is_org_admin: user.is_org_admin(&org_id),
        auth_mode, auth_basic_list_name,
        bindings,
        pages_subdomain, production_branch, git_source, build_config: build_config_info,
        changedetection_credential_id: cd_cred_id,
        changedetection_credential_name: cd_cred_name,
    })
}

#[server]
async fn list_cd_creds() -> Result<Vec<CredOption>, ServerFnError> {
    let _user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let rows = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, name FROM credentials WHERE credential_type = 'changedetection' ORDER BY name",
    )
    .fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rows.into_iter().map(|(id, name)| CredOption { id, name }).collect())
}

#[server]
async fn set_webspace_changedetection(webspace_id: Uuid, credential_id: Option<Uuid>) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM webspaces WHERE id = $1",
    ).bind(webspace_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    sqlx::query("UPDATE webspaces SET changedetection_credential_id = $1, updated_at = now() WHERE id = $2")
        .bind(credential_id).bind(webspace_id)
        .execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server]
async fn list_cf_creds_for_pages() -> Result<Vec<CredOption>, ServerFnError> {
    let _user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let rows = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, name FROM credentials WHERE credential_type = 'cloudflare' ORDER BY name",
    )
    .fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rows.into_iter().map(|(id, name)| CredOption { id, name }).collect())
}

#[server]
async fn list_domains_for_binding(webspace_id: Uuid) -> Result<Vec<DomainOption>, ServerFnError> {
    let _user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id).fetch_one(&pool).await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let rows = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
        "SELECT id, name, cloudflare_zone_id FROM domains WHERE organization_id = $1 ORDER BY name",
    )
    .bind(org_id).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut domains = Vec::new();
    for (id, name, cloudflare_zone_id) in rows {
        let subs = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, name FROM subdomains WHERE domain_id = $1 ORDER BY name",
        ).bind(id).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

        domains.push(DomainOption {
            id, name, cloudflare_zone_id,
            subdomains: subs.into_iter().map(|(sid, sname)| SubdomainOption { id: sid, name: sname }).collect(),
        });
    }

    Ok(domains)
}

/// Deploy a CF Pages project for this webspace.
#[server]
async fn deploy_pages_project(webspace_id: Uuid, credential_id: Uuid) -> Result<PagesDeployResult, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let row = sqlx::query_as::<_, (String, Uuid)>(
        "SELECT name, organization_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id).fetch_optional(&pool).await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("webspace not found"))?;

    let (ws_name, org_id) = row;
    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    let (client, account_id) = build_cf_pages_client(&pool, credential_id).await?;

    tracing::info!("deploying Pages project {ws_name} to account {account_id}");

    // Check if project already exists
    let project = match client.get_pages_project(&account_id, &ws_name).await {
        Ok(p) => {
            tracing::info!("Pages project {ws_name} already exists");
            p
        }
        Err(_) => {
            let p = client.create_pages_project(&account_id, &ws_name, "main").await
                .map_err(|e| ServerFnError::new(format!(
                    "failed to create Pages project: {e}. \
                     Ensure the API token has 'Cloudflare Pages:Edit' permission for account {account_id}"
                )))?;
            tracing::info!("created Pages project {ws_name}");
            p
        }
    };

    sqlx::query(
        "UPDATE webspaces SET cloudflare_pages_project = $1, cloudflare_credential_id = $2, updated_at = now() WHERE id = $3",
    )
    .bind(&ws_name).bind(credential_id).bind(webspace_id)
    .execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(PagesDeployResult {
        project_name: ws_name,
        subdomain: project.subdomain,
    })
}

/// Connect a git repo to the CF Pages project.
#[server]
async fn connect_git_repo(
    webspace_id: Uuid,
    provider: String,
    owner: String,
    repo_name: String,
    production_branch: String,
    build_command: String,
    destination_dir: String,
    root_dir: String,
) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let row = sqlx::query_as::<_, (Option<String>, Option<Uuid>, Uuid)>(
        "SELECT cloudflare_pages_project, cloudflare_credential_id, organization_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id).fetch_optional(&pool).await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("webspace not found"))?;

    let (project_name, cred_id, org_id) = row;
    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    let project_name = project_name.ok_or_else(|| ServerFnError::new("Pages project not deployed yet"))?;
    let cred_id = cred_id.ok_or_else(|| ServerFnError::new("no Cloudflare credential"))?;

    let (client, account_id) = build_cf_pages_client(&pool, cred_id).await?;

    let update = cloudflare_api::compat::UpdatePagesProject {
        production_branch: Some(production_branch.clone()),
        source: Some(cloudflare_api::compat::PagesSource {
            source_type: Some(provider.clone()),
            config: Some(cloudflare_api::compat::PagesSourceConfig {
                owner: Some(owner),
                repo_name: Some(repo_name),
                production_branch: Some(production_branch),
                pr_comments_enabled: Some(true),
                production_deployments_enabled: Some(true),
                preview_deployment_setting: Some("all".into()),
                preview_branch_includes: None,
                preview_branch_excludes: None,
            }),
        }),
        build_config: Some(cloudflare_api::compat::PagesBuildConfig {
            build_command: if build_command.is_empty() { None } else { Some(build_command) },
            destination_dir: if destination_dir.is_empty() { None } else { Some(destination_dir) },
            root_dir: if root_dir.is_empty() { None } else { Some(root_dir) },
            build_caching: Some(true),
        }),
    };

    client.update_pages_project(&account_id, &project_name, &update).await
        .map_err(|e| ServerFnError::new(format!(
            "failed to connect git repo: {e}. \
             Ensure the GitHub/GitLab integration is authorized in your Cloudflare dashboard"
        )))?;

    tracing::info!("connected git repo to Pages project {project_name}");
    Ok(())
}

/// Update the production branch for a direct-upload Pages project.
#[server]
async fn update_production_branch(webspace_id: Uuid, branch: String) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let row = sqlx::query_as::<_, (Option<String>, Option<Uuid>, Uuid)>(
        "SELECT cloudflare_pages_project, cloudflare_credential_id, organization_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id).fetch_optional(&pool).await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("webspace not found"))?;

    let (project_name, cred_id, org_id) = row;
    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    let project_name = project_name.ok_or_else(|| ServerFnError::new("no Pages project"))?;
    let cred_id = cred_id.ok_or_else(|| ServerFnError::new("no CF credential"))?;

    let (client, account_id) = build_cf_pages_client(&pool, cred_id).await?;

    let update = cloudflare_api::compat::UpdatePagesProject {
        production_branch: Some(branch.clone()),
        source: None,
        build_config: None,
    };

    client.update_pages_project(&account_id, &project_name, &update).await
        .map_err(|e| ServerFnError::new(format!("failed to update production branch: {e}")))?;

    tracing::info!("updated production branch for {project_name} to {branch}");
    Ok(())
}

/// Bind a domain (or subdomain) to this webspace.
/// For CF Pages: adds custom domain + creates CNAME record pointing to {project}.pages.dev.
#[server]
async fn bind_domain(webspace_id: Uuid, domain_id: Uuid, subdomain_id: Option<Uuid>, hostname: String) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    sqlx::query(
        "INSERT INTO webspace_domains (webspace_id, domain_id, subdomain_id) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(webspace_id).bind(domain_id).bind(subdomain_id)
    .execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    // Determine webspace type and create appropriate CNAME
    let ws = sqlx::query_as::<_, (String, Option<String>, Option<Uuid>)>(
        "SELECT hosting_type, cloudflare_pages_project, cloudflare_credential_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let (ws_hosting_type, ws_project, ws_cred_id) = ws;

    // For relay/tunnel: create CNAME pointing to agency_domain
    if ws_hosting_type == "relay" || ws_hosting_type == "tunnel" {
        if let Some(agency_domain) = crate::config::config().proxy.as_ref().map(|p| &p.agency_domain) {
            create_tunnel_cname(&pool, domain_id, subdomain_id, &hostname, agency_domain).await?;
        }
    }

    // For CF Pages webspaces: add custom domain + create CNAME record
    if let (Some(project_name), Some(cred_id)) = (ws_project, ws_cred_id) {
        let (client, account_id) = build_cf_pages_client(&pool, cred_id).await?;

        // 1. Add custom domain to Pages project
        if let Err(e) = client.add_pages_custom_domain(&account_id, &project_name, &hostname).await {
            tracing::warn!("failed to add custom domain {hostname} to Pages: {e}");
        } else {
            tracing::info!("added custom domain {hostname} to Pages project {project_name}");
        }

        // 2. Create CNAME record on the domain's Cloudflare zone
        // Use the subdomain from the API (the preview URL) as the CNAME target
        let cname_target = match client.get_pages_project(&account_id, &project_name).await {
            Ok(project) => match project.subdomain {
                Some(sub) => sub,
                None => return Err(ServerFnError::new("Pages project has no subdomain")),
            },
            Err(e) => return Err(ServerFnError::new(format!("failed to fetch Pages project: {e}"))),
        };
        let domain_cf = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
            "SELECT cloudflare_zone_id, cloudflare_credential_id FROM domains WHERE id = $1",
        ).bind(domain_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

        if let (Some(zone_id), Some(domain_cred_id)) = domain_cf {
            let domain_client = build_domain_cf_client(&pool, domain_cred_id).await?;
            let record = cloudflare_api::compat::CreateDnsRecord {
                record_type: "CNAME".into(),
                name: hostname.clone(),
                content: Some(cname_target.clone()),
                data: None,
                ttl: Some(1),
                proxied: Some(true),
                comment: Some(format!("Pages: {project_name}")),
                priority: None,
            };
            match domain_client.create_dns_record(&zone_id, &record).await {
                Ok(created) => {
                    tracing::info!("created CNAME {hostname} → {cname_target} (CF record {})", created.id);
                    // Also store in dns_records if we have a subdomain
                    if let Some(sub_id) = subdomain_id {
                        let sub_name = sqlx::query_scalar::<_, String>(
                            "SELECT name FROM subdomains WHERE id = $1",
                        ).bind(sub_id).fetch_optional(&pool).await.ok().flatten();

                        if let Some(sub_name) = sub_name {
                            let _ = sqlx::query(
                                "INSERT INTO dns_records (subdomain_id, domain_id, name, record_type, record_value, proxied, cloudflare_record_id) \
                                 VALUES ($1, $2, $3, 'CNAME', $4, true, $5) ON CONFLICT DO NOTHING",
                            )
                            .bind(sub_id).bind(domain_id).bind(&sub_name).bind(&cname_target).bind(&created.id)
                            .execute(&pool).await;
                        }
                    } else {
                        // Root domain binding — ensure @ subdomain entity exists
                        let sub_id = sqlx::query_scalar::<_, Uuid>(
                            "INSERT INTO subdomains (domain_id, name) VALUES ($1, '@') \
                             ON CONFLICT (domain_id, name) DO UPDATE SET updated_at = now() RETURNING id",
                        ).bind(domain_id).fetch_one(&pool).await.ok();

                        if let Some(sub_id) = sub_id {
                            let _ = sqlx::query(
                                "INSERT INTO dns_records (subdomain_id, domain_id, name, record_type, record_value, proxied, cloudflare_record_id) \
                                 VALUES ($1, $2, '@', 'CNAME', $3, true, $4) ON CONFLICT DO NOTHING",
                            )
                            .bind(sub_id).bind(domain_id).bind(&cname_target).bind(&created.id)
                            .execute(&pool).await;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("failed to create CNAME for {hostname}: {e}");
                }
            }
        }
    }

    crate::api::internal::notify_proxy_reload();
    Ok(())
}

/// Trigger a recheck of a custom domain's verification status on CF Pages.
/// Uses the PATCH endpoint which retries validation per the CF API spec.
#[server]
async fn recheck_custom_domain(webspace_id: Uuid, hostname: String) -> Result<String, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM webspaces WHERE id = $1",
    ).bind(webspace_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    let ws = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
        "SELECT cloudflare_pages_project, cloudflare_credential_id FROM webspaces WHERE id = $1",
    ).bind(webspace_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let (project_name, cred_id) = match ws {
        (Some(p), Some(c)) => (p, c),
        _ => return Err(ServerFnError::new("no Pages project")),
    };

    let (client, account_id) = build_cf_pages_client(&pool, cred_id).await?;

    match client.retry_pages_custom_domain(&account_id, &project_name, &hostname).await {
        Ok(dom) => {
            let status = dom.status.as_deref().unwrap_or("pending").to_string();
            Ok(format!("Validation retried — status: {status}"))
        }
        Err(e) => {
            // Domain might not exist on CF yet — try adding it
            match client.add_pages_custom_domain(&account_id, &project_name, &hostname).await {
                Ok(dom) => {
                    let status = dom.status.as_deref().unwrap_or("pending").to_string();
                    Ok(format!("Domain added — status: {status}"))
                }
                Err(_) => Err(ServerFnError::new(format!("retry failed: {e}"))),
            }
        }
    }
}

/// Re-create the CNAME record for a Pages domain binding.
#[server]
async fn fix_cname(webspace_id: Uuid, domain_id: Uuid, subdomain_id: Option<Uuid>, hostname: String) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM webspaces WHERE id = $1",
    ).bind(webspace_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    let ws = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
        "SELECT cloudflare_pages_project, cloudflare_credential_id FROM webspaces WHERE id = $1",
    ).bind(webspace_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let (project_name, cred_id) = match ws {
        (Some(p), Some(c)) => (p, c),
        _ => return Err(ServerFnError::new("webspace has no Pages project")),
    };

    // Use the subdomain from the API (the preview URL) as the CNAME target
    let (pages_client, account_id) = build_cf_pages_client(&pool, cred_id).await?;
    let cname_target = match pages_client.get_pages_project(&account_id, &project_name).await {
        Ok(project) => match project.subdomain {
            Some(sub) => sub,
            None => return Err(ServerFnError::new("Pages project has no subdomain")),
        },
        Err(e) => return Err(ServerFnError::new(format!("failed to fetch Pages project: {e}"))),
    };

    let domain_cf = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
        "SELECT cloudflare_zone_id, cloudflare_credential_id FROM domains WHERE id = $1",
    ).bind(domain_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let (zone_id, domain_cred_id) = match domain_cf {
        (Some(z), Some(c)) => (z, c),
        _ => return Err(ServerFnError::new("domain not deployed to Cloudflare")),
    };

    let client = build_domain_cf_client(&pool, domain_cred_id).await?;

    let record = cloudflare_api::compat::CreateDnsRecord {
        record_type: "CNAME".into(),
        name: hostname.clone(),
        content: Some(cname_target.clone()),
        data: None,
        ttl: Some(1),
        proxied: Some(true),
        comment: Some(format!("Pages: {project_name}")),
        priority: None,
    };

    let created = client.create_dns_record(&zone_id, &record).await
        .map_err(|e| ServerFnError::new(format!("failed to create CNAME: {e}")))?;

    // Store in dns_records
    let sub_name = if let Some(sid) = subdomain_id {
        sqlx::query_scalar::<_, String>("SELECT name FROM subdomains WHERE id = $1")
            .bind(sid).fetch_optional(&pool).await.ok().flatten().unwrap_or_else(|| "@".into())
    } else {
        "@".into()
    };

    // Ensure subdomain entity exists
    let sub_id = if let Some(sid) = subdomain_id {
        sid
    } else {
        sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO subdomains (domain_id, name) VALUES ($1, '@') \
             ON CONFLICT (domain_id, name) DO UPDATE SET updated_at = now() RETURNING id",
        ).bind(domain_id).fetch_one(&pool).await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };

    let _ = sqlx::query(
        "INSERT INTO dns_records (subdomain_id, domain_id, name, record_type, record_value, proxied, cloudflare_record_id) \
         VALUES ($1, $2, $3, 'CNAME', $4, true, $5) ON CONFLICT DO NOTHING",
    )
    .bind(sub_id).bind(domain_id).bind(&sub_name).bind(&cname_target).bind(&created.id)
    .execute(&pool).await;

    tracing::info!("fixed CNAME {hostname} → {cname_target} (CF record {})", created.id);
    Ok(())
}

/// Remove a domain binding. For CF Pages: removes custom domain + CNAME record.
#[server]
async fn unbind_domain(webspace_id: Uuid, binding_id: Uuid, hostname: String) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    // Get binding details before deleting
    let binding = sqlx::query_as::<_, (Uuid, Option<Uuid>)>(
        "SELECT domain_id, subdomain_id FROM webspace_domains WHERE id = $1",
    ).bind(binding_id).fetch_optional(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    // Remove CNAME record (relay/tunnel or Pages)
    let ws = sqlx::query_as::<_, (String, Option<String>, Option<Uuid>)>(
        "SELECT hosting_type, cloudflare_pages_project, cloudflare_credential_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let (ws_hosting_type, ws_project, ws_cred_id) = ws;

    // For relay/tunnel: remove the agency-domain CNAME
    if (ws_hosting_type == "relay" || ws_hosting_type == "tunnel") && binding.is_some() {
        if let Some(agency_domain) = crate::config::config().proxy.as_ref().map(|p| &p.agency_domain) {
            if let Some((domain_id, _)) = binding {
                remove_tunnel_cname(&pool, domain_id, &hostname, agency_domain).await;
            }
        }
    }

    if let (Some(project_name), Some(cred_id)) = (&ws_project, &ws_cred_id) {
        if let Ok((client, account_id)) = build_cf_pages_client(&pool, *cred_id).await {
            // Remove custom domain from Pages
            let _ = client.remove_pages_custom_domain(&account_id, project_name, &hostname).await;

            // Remove the CNAME record from the domain's CF zone
            if let Some((domain_id, _)) = binding {
                let cname_target = format!("{project_name}.pages.dev");
                let domain_cf = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
                    "SELECT cloudflare_zone_id, cloudflare_credential_id FROM domains WHERE id = $1",
                ).bind(domain_id).fetch_optional(&pool).await.ok().flatten();

                if let Some((Some(zone_id), Some(domain_cred_id))) = domain_cf {
                    if let Ok(domain_client) = build_domain_cf_client(&pool, domain_cred_id).await {
                        // Find and delete the CNAME record
                        if let Ok(records) = domain_client.list_dns_records(&zone_id).await {
                            for rec in records {
                                if rec.record_type == "CNAME"
                                    && rec.content.as_deref() == Some(&cname_target)
                                    && (rec.name == hostname || rec.name.ends_with(&format!(".{hostname}")))
                                {
                                    let _ = domain_client.delete_dns_record(&zone_id, &rec.id).await;
                                    tracing::info!("removed CNAME {} → {cname_target}", rec.name);

                                    // Also remove from dns_records table
                                    let _ = sqlx::query(
                                        "DELETE FROM dns_records WHERE cloudflare_record_id = $1",
                                    ).bind(&rec.id).execute(&pool).await;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    sqlx::query("DELETE FROM webspace_domains WHERE id = $1")
        .bind(binding_id).execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    crate::api::internal::notify_proxy_reload();
    Ok(())
}

/// Create a CNAME record pointing to the agency domain (for relay/tunnel webspaces).
#[cfg(feature = "server")]
async fn create_tunnel_cname(
    pool: &sqlx::PgPool,
    domain_id: Uuid,
    subdomain_id: Option<Uuid>,
    hostname: &str,
    cname_target: &str,
) -> Result<(), ServerFnError> {
    let domain_cf = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
        "SELECT cloudflare_zone_id, cloudflare_credential_id FROM domains WHERE id = $1",
    ).bind(domain_id).fetch_one(pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let (zone_id, domain_cred_id) = match domain_cf {
        (Some(z), Some(c)) => (z, c),
        _ => return Err(ServerFnError::new("domain not deployed to Cloudflare")),
    };

    let client = build_domain_cf_client(pool, domain_cred_id).await?;
    let record = cloudflare_api::compat::CreateDnsRecord {
        record_type: "CNAME".into(),
        name: hostname.to_string(),
        content: Some(cname_target.to_string()),
        data: None,
        ttl: Some(1),
        proxied: Some(true),
        comment: Some("Tunnel proxy".into()),
        priority: None,
    };

    match client.create_dns_record(&zone_id, &record).await {
        Ok(created) => {
            tracing::info!("created CNAME {hostname} → {cname_target} (CF record {})", created.id);
            // Ensure subdomain entity exists
            let sub_id = if let Some(sid) = subdomain_id {
                sid
            } else {
                sqlx::query_scalar::<_, Uuid>(
                    "INSERT INTO subdomains (domain_id, name) VALUES ($1, '@') \
                     ON CONFLICT (domain_id, name) DO UPDATE SET updated_at = now() RETURNING id",
                ).bind(domain_id).fetch_one(pool).await
                .map_err(|e| ServerFnError::new(e.to_string()))?
            };

            let sub_name = if let Some(sid) = subdomain_id {
                sqlx::query_scalar::<_, String>("SELECT name FROM subdomains WHERE id = $1")
                    .bind(sid).fetch_optional(pool).await.ok().flatten().unwrap_or_else(|| "@".into())
            } else {
                "@".into()
            };

            let _ = sqlx::query(
                "INSERT INTO dns_records (subdomain_id, domain_id, name, record_type, record_value, proxied, cloudflare_record_id) \
                 VALUES ($1, $2, $3, 'CNAME', $4, true, $5) ON CONFLICT DO NOTHING",
            )
            .bind(sub_id).bind(domain_id).bind(&sub_name).bind(cname_target).bind(&created.id)
            .execute(pool).await;
        }
        Err(e) => {
            tracing::warn!("failed to create CNAME for {hostname}: {e}");
        }
    }

    Ok(())
}

/// Remove a tunnel CNAME record pointing to the agency domain.
#[cfg(feature = "server")]
async fn remove_tunnel_cname(
    pool: &sqlx::PgPool,
    domain_id: Uuid,
    hostname: &str,
    agency_domain: &str,
) {
    let domain_cf = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
        "SELECT cloudflare_zone_id, cloudflare_credential_id FROM domains WHERE id = $1",
    ).bind(domain_id).fetch_optional(pool).await.ok().flatten();

    if let Some((Some(zone_id), Some(domain_cred_id))) = domain_cf {
        if let Ok(client) = build_domain_cf_client(pool, domain_cred_id).await {
            if let Ok(records) = client.list_dns_records(&zone_id).await {
                for rec in records {
                    if rec.record_type == "CNAME"
                        && rec.content.as_deref() == Some(agency_domain)
                        && (rec.name == hostname || rec.name.ends_with(&format!(".{hostname}")))
                    {
                        let _ = client.delete_dns_record(&zone_id, &rec.id).await;
                        tracing::info!("removed CNAME {} → {agency_domain}", rec.name);
                        let _ = sqlx::query("DELETE FROM dns_records WHERE cloudflare_record_id = $1")
                            .bind(&rec.id).execute(pool).await;
                        break;
                    }
                }
            }
        }
    }
}

/// Re-create the CNAME record for a relay/tunnel domain binding.
#[server]
async fn fix_cname_tunnel(webspace_id: Uuid, domain_id: Uuid, subdomain_id: Option<Uuid>, hostname: String) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM webspaces WHERE id = $1",
    ).bind(webspace_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    let agency_domain = crate::config::config().proxy.as_ref()
        .map(|p| p.agency_domain.clone())
        .ok_or_else(|| ServerFnError::new("no proxy config with agency_domain"))?;

    create_tunnel_cname(&pool, domain_id, subdomain_id, &hostname, &agency_domain).await
}

#[cfg(feature = "server")]
async fn build_cf_pages_client(pool: &sqlx::PgPool, cred_id: Uuid) -> Result<(cloudflare_api::compat::SimpleClient, String), ServerFnError> {
    crate::credentials::cf_client_with_account(pool, cred_id).await
        .map_err(|e| ServerFnError::new(format!("{e}")))
}

#[cfg(feature = "server")]
async fn build_domain_cf_client(pool: &sqlx::PgPool, cred_id: Uuid) -> Result<cloudflare_api::compat::SimpleClient, ServerFnError> {
    crate::credentials::cf_client(pool, cred_id).await
        .map_err(|e| ServerFnError::new(format!("{e}")))
}

// ── Deployments & tokens ─────────────────────────────────────────────

#[server]
async fn list_deployments(webspace_id: Uuid) -> Result<Vec<DeploymentRow>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("webspace not found"))?;

    use crate::web::user::WebUserExt;
    user.require_org_read(&org_id)?;

    let rows = sqlx::query_as::<_, (Uuid, String, Option<String>, Option<i64>, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, status, error_message, tarball_size, created_at \
         FROM deployments WHERE webspace_id = $1 ORDER BY created_at DESC LIMIT 20",
    )
    .bind(webspace_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|(id, status, error_message, tarball_size, created_at)| DeploymentRow {
            id,
            status,
            error_message,
            tarball_size,
            created_at: created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        })
        .collect())
}

#[server]
async fn create_deploy_token(
    webspace_id: Uuid,
    org_id: Uuid,
    label: String,
) -> Result<TokenCreateResult, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    // Require org admin (not just write) for token creation.
    use crate::web::user::WebUserExt;
    user.require_org_admin(&org_id)?;

    // Verify the webspace belongs to this org.
    let ws_org = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("webspace not found"))?;

    if ws_org != org_id {
        return Err(ServerFnError::new("webspace does not belong to this organization"));
    }

    use rand::Rng;
    let token_bytes: [u8; 32] = rand::rng().random();
    let token = hex::encode(token_bytes);

    use sha2::{Digest, Sha256};
    let hash = hex::encode(Sha256::digest(token.as_bytes()));

    let scopes = serde_json::json!({ "webspace_id": webspace_id.to_string() });

    sqlx::query(
        "INSERT INTO tokens (organization_id, token_hash, label, kind, scopes) VALUES ($1, $2, $3, 'deploy', $4)",
    )
    .bind(org_id)
    .bind(&hash)
    .bind(&label)
    .bind(&scopes)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(TokenCreateResult { token })
}

// ── Component ─────────────────────────────────────────────────────────

#[component]
pub fn WebspaceDetail(id: String) -> Element {
    let webspace_id = Uuid::parse_str(&id).ok();
    let refresh = use_context_provider(|| Signal::new(0u32));
    let webspace = use_server_future(move || {
        let wid = webspace_id;
        let _ = *refresh.read(); // reactive dependency — bumping refresh re-runs this future
        async move {
            match wid {
                Some(id) => get_webspace(id).await,
                None => Err(ServerFnError::new("invalid webspace ID")),
            }
        }
    })?;

    let data = match &*webspace.read() {
        Some(Ok(d)) => d.clone(),
        Some(Err(e)) => return rsx! { div { class: "text-danger", "Error: {e}" } },
        None => return rsx! { div { class: "text-fg-muted", "Loading..." } },
    };

    let is_pages = data.hosting_type == "cloudflare_pages";
    let has_project = data.cloudflare_pages_project.is_some();

    rsx! {
        PageHeader { "{data.name}" }

        // Info card
        Card {
            div { class: "p-6 grid grid-cols-2 md:grid-cols-4 gap-4",
                div {
                    div { class: "text-sm text-fg-muted", "Hosting Type" }
                    match data.hosting_type.as_str() {
                        "cloudflare_pages" => rsx! { Badge { variant: BadgeVariant::Info, "Cloudflare Pages" } },
                        "local" => rsx! { Badge { "Local" } },
                        "relay" => rsx! { Badge { variant: BadgeVariant::Accent, "Relay Tunnel" } },
                        "tunnel" => rsx! { Badge { variant: BadgeVariant::Accent, "Tunnel" } },
                        _ => rsx! { span { "{data.hosting_type}" } },
                    }
                }
                if is_pages {
                    div {
                        div { class: "text-sm text-fg-muted", "Pages Project" }
                        if let Some(ref proj) = data.cloudflare_pages_project {
                            span { class: "font-mono text-sm", "{proj}" }
                        } else {
                            span { class: "text-fg-muted", "Not deployed" }
                        }
                    }
                } else if data.hosting_type == "relay" || data.hosting_type == "tunnel" {
                    div {
                        div { class: "text-sm text-fg-muted", "Upstream URL" }
                        if let Some(ref url) = data.relay_url {
                            span { class: "font-mono text-sm break-all", "{url}" }
                        } else {
                            span { class: "text-fg-muted", "-" }
                        }
                    }
                } else {
                    div {
                        div { class: "text-sm text-fg-muted", "Runtime" }
                        div { {data.runtime.as_deref().unwrap_or("-")} }
                    }
                    div {
                        div { class: "text-sm text-fg-muted", "Status" }
                        match data.local_status.as_deref() {
                            Some("running") => rsx! { Badge { variant: BadgeVariant::Success, "Running" } },
                            Some("error") => rsx! { Badge { variant: BadgeVariant::Danger, "Error" } },
                            Some(s) => rsx! { Badge { "{s}" } },
                            None => rsx! { span { class: "text-fg-muted", "-" } },
                        }
                    }
                }
                div {
                    div { class: "text-sm text-fg-muted", "Organization" }
                    div { "{data.organization_name}" }
                }
                if !is_pages {
                    div {
                        div { class: "text-sm text-fg-muted", "Auth" }
                        match data.auth_mode.as_str() {
                            "oidc" => rsx! { Badge { variant: BadgeVariant::Info, "OIDC" } },
                            "basic" => rsx! {
                                div { class: "flex items-center gap-1",
                                    Badge { variant: BadgeVariant::Accent, "Basic" }
                                    if let Some(ref list_name) = data.auth_basic_list_name {
                                        span { class: "text-sm text-fg-muted", "({list_name})" }
                                    }
                                }
                            },
                            _ => rsx! { span { class: "text-fg-muted", "None" } },
                        }
                    }
                }
            }
        }

        // Settings (editable name + upstream URL)
        SectionHeading { class: "mt-6", "Settings" }
        WebspaceSettingsSection {
            webspace_id: data.id,
            current_name: data.name.clone(),
            hosting_type: data.hosting_type.clone(),
            current_relay_url: data.relay_url.clone(),
        }

        // CF Pages deployment
        if is_pages && !has_project {
            SectionHeading { class: "mt-6", "Deploy Pages Project" }
            PagesDeploySection { webspace_id: data.id }
        }

        // Source info (only for deployed CF Pages projects)
        if is_pages && has_project {
            SectionHeading { class: "mt-6", "Deployment Source" }
            if data.git_source.is_some() {
                // Git-connected project
                GitSourceDisplay {
                    git_source: data.git_source.clone().unwrap(),
                    build_config: data.build_config.clone(),
                    pages_subdomain: data.pages_subdomain.clone(),
                }
            } else {
                // Direct upload project
                DirectUploadDisplay {
                    webspace_id: data.id,
                    project_name: data.cloudflare_pages_project.clone().unwrap_or_default(),
                    production_branch: data.production_branch.clone().unwrap_or_else(|| "main".into()),
                    pages_subdomain: data.pages_subdomain.clone(),
                }
            }
        }

        // Deployments (for direct-upload Pages projects)
        if is_pages && has_project && data.git_source.is_none() {
            SectionHeading { class: "mt-6", "Deployments" }
            DeploymentsSection { webspace_id: data.id }
        }

        // Deploy token (org admins can create inline)
        if is_pages && has_project && data.git_source.is_none() && data.is_org_admin {
            SectionHeading { class: "mt-6", "Deploy Token" }
            DeployTokenSection { webspace_id: data.id, organization_id: data.organization_id }
        }

        // Auth settings (not for Pages)
        if !is_pages {
            SectionHeading { class: "mt-6", "Auth" }
            AuthSettingsSection {
                webspace_id: data.id,
                organization_id: data.organization_id,
                current_mode: data.auth_mode.clone(),
                current_list_name: data.auth_basic_list_name.clone(),
            }
        }

        // Change Detection
        SectionHeading { class: "mt-6", "Change Detection" }
        Card {
            div { class: "p-6 flex items-center gap-3",
                if let Some(ref name) = data.changedetection_credential_name {
                    Badge { variant: BadgeVariant::Info, "{name}" }
                } else {
                    span { class: "text-fg-muted text-sm", "Not configured" }
                }
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
            webspace_id: data.id,
            bindings: data.bindings.clone(),
            is_pages,
            hosting_type: data.hosting_type.clone(),
        }

        // Danger zone
        SectionHeading { class: "mt-6", "Danger Zone" }
        MoveWebspaceSection { webspace_id: data.id, current_org_id: data.organization_id }
        DeleteWebspaceSection { webspace_id: data.id }
    }
}

/// Git repo connection / display section.
#[component]
fn GitRepoSection(
    webspace_id: Uuid,
    git_source: Option<GitRepoInfo>,
    build_config: Option<BuildConfigInfo>,
    pages_subdomain: Option<String>,
) -> Element {
    if let Some(ref git) = git_source {
        // Already connected — show info
        let repo_url = match git.provider.as_str() {
            "github" => format!("https://github.com/{}/{}", git.owner, git.repo),
            "gitlab" => format!("https://gitlab.com/{}/{}", git.owner, git.repo),
            _ => format!("{}/{}", git.owner, git.repo),
        };
        return rsx! {
            Card {
                div { class: "p-6 space-y-3",
                    div { class: "flex items-center gap-2",
                        Badge { variant: BadgeVariant::Success, "Connected" }
                        span { class: "font-mono text-sm", "{git.provider}" }
                    }
                    div { class: "flex items-center gap-2",
                        span { class: "text-sm text-fg-muted", "Repository:" }
                        a {
                            href: "{repo_url}",
                            target: "_blank",
                            class: "font-mono text-sm text-brand underline",
                            "{git.owner}/{git.repo}"
                        }
                    }
                    div { class: "flex items-center gap-2",
                        span { class: "text-sm text-fg-muted", "Branch:" }
                        span { class: "font-mono text-sm", "{git.production_branch}" }
                    }
                    if let Some(ref bc) = build_config {
                        if bc.build_command.is_some() || bc.destination_dir.is_some() {
                            div { class: "border-t border-line-soft pt-3 mt-3",
                                div { class: "text-sm text-fg-muted mb-1", "Build Settings" }
                                if let Some(ref cmd) = bc.build_command {
                                    div { class: "flex items-center gap-2",
                                        span { class: "text-sm text-fg-muted", "Command:" }
                                        span { class: "font-mono text-sm bg-surface-2 px-2 py-0.5 rounded", "{cmd}" }
                                    }
                                }
                                if let Some(ref dir) = bc.destination_dir {
                                    div { class: "flex items-center gap-2",
                                        span { class: "text-sm text-fg-muted", "Output:" }
                                        span { class: "font-mono text-sm", "{dir}" }
                                    }
                                }
                                if let Some(ref root) = bc.root_dir {
                                    div { class: "flex items-center gap-2",
                                        span { class: "text-sm text-fg-muted", "Root:" }
                                        span { class: "font-mono text-sm", "{root}" }
                                    }
                                }
                            }
                        }
                    }
                    if let Some(ref sub) = pages_subdomain {
                        div { class: "flex items-center gap-2",
                            span { class: "text-sm text-fg-muted", "Preview:" }
                            a {
                                href: "https://{sub}",
                                target: "_blank",
                                class: "font-mono text-sm text-brand underline",
                                "https://{sub}"
                            }
                        }
                    }
                }
            }
        };
    }

    // Not connected — show form
    let mut refresh: Signal<u32> = use_context();
    let mut provider = use_signal(|| "github".to_string());
    let mut owner = use_signal(String::new);
    let mut repo_name = use_signal(String::new);
    let mut branch = use_signal(|| "main".to_string());
    let mut build_cmd = use_signal(String::new);
    let mut dest_dir = use_signal(String::new);
    let mut root_dir = use_signal(String::new);
    let mut connecting = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    rsx! {
        Card {
            div { class: "p-6 space-y-4",
                p { class: "text-fg-muted text-sm mb-2",
                    "Connect a GitHub or GitLab repository. The Cloudflare GitHub/GitLab integration must be "
                    a { href: "https://dash.cloudflare.com/?to=/:account/pages", target: "_blank", class: "text-brand underline", "authorized in your Cloudflare dashboard" }
                    " first."
                }

                div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                    FormField { label: "Provider",
                        select {
                            class: "input",
                            value: "{provider}",
                            oninput: move |evt| provider.set(evt.value()),
                            option { value: "github", "GitHub" }
                            option { value: "gitlab", "GitLab" }
                        }
                    }
                    FormField { label: "Production Branch",
                        input { class: "input", r#type: "text", value: "{branch}",
                            placeholder: "main",
                            oninput: move |evt| branch.set(evt.value()) }
                    }
                }

                div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                    FormField { label: "Owner (user or org)",
                        input { class: "input", r#type: "text", required: true,
                            placeholder: "my-github-org",
                            value: "{owner}", oninput: move |evt| owner.set(evt.value()) }
                    }
                    FormField { label: "Repository Name",
                        input { class: "input", r#type: "text", required: true,
                            placeholder: "my-website",
                            value: "{repo_name}", oninput: move |evt| repo_name.set(evt.value()) }
                    }
                }

                SectionHeading { "Build Settings (optional)" }
                div { class: "grid grid-cols-1 md:grid-cols-3 gap-4",
                    FormField { label: "Build Command",
                        input { class: "input font-mono text-sm", r#type: "text",
                            placeholder: "npm run build",
                            value: "{build_cmd}", oninput: move |evt| build_cmd.set(evt.value()) }
                    }
                    FormField { label: "Output Directory",
                        input { class: "input font-mono text-sm", r#type: "text",
                            placeholder: "dist",
                            value: "{dest_dir}", oninput: move |evt| dest_dir.set(evt.value()) }
                    }
                    FormField { label: "Root Directory",
                        help: "Subdirectory where the project lives (leave empty for repo root)",
                        input { class: "input font-mono text-sm", r#type: "text",
                            placeholder: "/",
                            value: "{root_dir}", oninput: move |evt| root_dir.set(evt.value()) }
                    }
                }

                if let Some(err) = &*error.read() {
                    div { class: "text-danger text-sm", "{err}" }
                }

                Button {
                    variant: ButtonVariant::Primary,
                    disabled: owner.read().is_empty() || repo_name.read().is_empty() || *connecting.read(),
                    onclick: {
                        let wid = webspace_id;
                        move |_| {
                            let p = provider.read().clone();
                            let o = owner.read().clone();
                            let r = repo_name.read().clone();
                            let b = branch.read().clone();
                            let bc = build_cmd.read().clone();
                            let dd = dest_dir.read().clone();
                            let rd = root_dir.read().clone();
                            connecting.set(true);
                            error.set(None);
                            spawn(async move {
                                match connect_git_repo(wid, p, o, r, b, bc, dd, rd).await {
                                    Ok(()) => {
                                        refresh += 1;
                                    }
                                    Err(e) => {
                                        error.set(Some(format!("{e}")));
                                        connecting.set(false);
                                    }
                                }
                            });
                        }
                    },
                    if *connecting.read() { "Connecting..." } else { "Connect Repository" }
                }
            }
        }
    }
}

/// Deploy a CF Pages project — user chooses Git or Direct Upload (cannot be changed later).
#[component]
fn PagesDeploySection(webspace_id: Uuid) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let creds = use_server_future(list_cf_creds_for_pages)?;
    let cred_list = match &*creds.read() { Some(Ok(c)) => c.clone(), _ => vec![] };

    let mut cred_id = use_signal(|| cred_list.first().map(|c| c.id.to_string()).unwrap_or_default());
    let mut mode = use_signal(|| "direct".to_string()); // "direct" or "git"
    // Git fields
    let mut git_provider = use_signal(|| "github".to_string());
    let mut git_owner = use_signal(String::new);
    let mut git_repo = use_signal(String::new);
    let mut git_branch = use_signal(|| "main".to_string());
    let mut build_cmd = use_signal(String::new);
    let mut dest_dir = use_signal(String::new);
    let mut root_dir = use_signal(String::new);

    let mut deploying = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    if cred_list.is_empty() {
        return rsx! {
            Card { div { class: "p-6 text-fg-muted",
                "No Cloudflare credentials. "
                Link { to: crate::web::app::Route::CredentialForm {}, class: "text-brand underline", "Add one" }
                " first."
            }}
        };
    }

    let is_git = *mode.read() == "git";

    rsx! {
        Card { div { class: "p-6 space-y-4",
            p { class: "text-sm text-fg-muted",
                "Choose the deployment source. " span { class: "font-semibold text-fg", "This cannot be changed after creation." }
            }

            div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                FormField { label: "Cloudflare Credential",
                    select { class: "input", value: "{cred_id}", oninput: move |evt| cred_id.set(evt.value()),
                        for c in &cred_list { option { value: "{c.id}", "{c.name}" } }
                    }
                }
                FormField { label: "Deployment Source",
                    select { class: "input", value: "{mode}", oninput: move |evt| mode.set(evt.value()),
                        option { value: "direct", "Direct Upload (Wrangler CLI)" }
                        option { value: "git", "Git Repository (GitHub / GitLab)" }
                    }
                }
            }

            if is_git {
                p { class: "text-sm text-fg-muted",
                    "The GitHub/GitLab integration must be "
                    a { href: "https://dash.cloudflare.com/?to=/:account/pages", target: "_blank", class: "text-brand underline", "authorized in Cloudflare" }
                    " first."
                }
                div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                    FormField { label: "Provider",
                        select { class: "input", value: "{git_provider}", oninput: move |evt| git_provider.set(evt.value()),
                            option { value: "github", "GitHub" } option { value: "gitlab", "GitLab" }
                        }
                    }
                    FormField { label: "Branch",
                        input { class: "input", r#type: "text", value: "{git_branch}", placeholder: "main",
                            oninput: move |evt| git_branch.set(evt.value()) }
                    }
                }
                div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                    FormField { label: "Owner",
                        input { class: "input", r#type: "text", required: true, placeholder: "my-org",
                            value: "{git_owner}", oninput: move |evt| git_owner.set(evt.value()) }
                    }
                    FormField { label: "Repository",
                        input { class: "input", r#type: "text", required: true, placeholder: "my-site",
                            value: "{git_repo}", oninput: move |evt| git_repo.set(evt.value()) }
                    }
                }
                SectionHeading { "Build Settings (optional)" }
                div { class: "grid grid-cols-1 md:grid-cols-3 gap-4",
                    FormField { label: "Build Command",
                        input { class: "input font-mono text-sm", r#type: "text", placeholder: "npm run build",
                            value: "{build_cmd}", oninput: move |evt| build_cmd.set(evt.value()) }
                    }
                    FormField { label: "Output Directory",
                        input { class: "input font-mono text-sm", r#type: "text", placeholder: "dist",
                            value: "{dest_dir}", oninput: move |evt| dest_dir.set(evt.value()) }
                    }
                    FormField { label: "Root Directory",
                        input { class: "input font-mono text-sm", r#type: "text", placeholder: "/",
                            value: "{root_dir}", oninput: move |evt| root_dir.set(evt.value()) }
                    }
                }
            }

            if let Some(err) = &*error.read() {
                div { class: "text-danger text-sm", "{err}" }
            }

            Button { variant: ButtonVariant::Primary,
                disabled: *deploying.read() || (is_git && (git_owner.read().is_empty() || git_repo.read().is_empty())),
                onclick: {
                    let wid = webspace_id;
                    let cid_str = cred_id.read().clone();
                    let is_git = is_git;
                    let gp = git_provider.read().clone();
                    let go = git_owner.read().clone();
                    let gr = git_repo.read().clone();
                    let gb = git_branch.read().clone();
                    let bc = build_cmd.read().clone();
                    let dd = dest_dir.read().clone();
                    let rd = root_dir.read().clone();
                    move |_| {
                        let cid_str = cid_str.clone();
                        let gp = gp.clone(); let go = go.clone(); let gr = gr.clone();
                        let gb = gb.clone(); let bc = bc.clone(); let dd = dd.clone(); let rd = rd.clone();
                        deploying.set(true); error.set(None);
                        spawn(async move {
                            if let Ok(cid) = uuid::Uuid::parse_str(&cid_str) {
                                // Step 1: create project
                                match deploy_pages_project(wid, cid).await {
                                    Ok(_) => {
                                        // Step 2: if git, connect repo
                                        if is_git {
                                            if let Err(e) = connect_git_repo(wid, gp, go, gr, gb, bc, dd, rd).await {
                                                error.set(Some(format!("Project created but git connection failed: {e}")));
                                                deploying.set(false);
                                                refresh += 1;
                                                return;
                                            }
                                        }
                                        refresh += 1;
                                    }
                                    Err(e) => error.set(Some(format!("{e}"))),
                                }
                            }
                            deploying.set(false);
                        });
                    }
                },
                if *deploying.read() { "Creating..." } else if is_git { "Create with Git" } else { "Create with Direct Upload" }
            }
        }}
    }
}

/// Display for a git-connected Pages project.
#[component]
fn GitSourceDisplay(git_source: GitRepoInfo, build_config: Option<BuildConfigInfo>, pages_subdomain: Option<String>) -> Element {
    let repo_url = match git_source.provider.as_str() {
        "github" => format!("https://github.com/{}/{}", git_source.owner, git_source.repo),
        "gitlab" => format!("https://gitlab.com/{}/{}", git_source.owner, git_source.repo),
        _ => format!("{}/{}", git_source.owner, git_source.repo),
    };

    rsx! {
        Card { div { class: "p-6 space-y-3",
            div { class: "flex items-center gap-2",
                Badge { variant: BadgeVariant::Info, "Git" }
                span { class: "font-mono text-sm", "{git_source.provider}" }
            }
            div { class: "flex items-center gap-2",
                span { class: "text-sm text-fg-muted", "Repository:" }
                a { href: "{repo_url}", target: "_blank", class: "font-mono text-sm text-brand underline", "{git_source.owner}/{git_source.repo}" }
            }
            div { class: "flex items-center gap-2",
                span { class: "text-sm text-fg-muted", "Branch:" }
                span { class: "font-mono text-sm", "{git_source.production_branch}" }
            }
            if let Some(bc) = &build_config {
                if bc.build_command.is_some() || bc.destination_dir.is_some() {
                    div { class: "border-t border-line-soft pt-3 mt-3",
                        div { class: "text-sm text-fg-muted mb-1", "Build Settings" }
                        if let Some(ref cmd) = bc.build_command {
                            div { span { class: "text-sm text-fg-muted", "Command: " } span { class: "font-mono text-sm bg-surface-2 px-2 py-0.5 rounded", "{cmd}" } }
                        }
                        if let Some(ref dir) = bc.destination_dir {
                            div { span { class: "text-sm text-fg-muted", "Output: " } span { class: "font-mono text-sm", "{dir}" } }
                        }
                        if let Some(ref root) = bc.root_dir {
                            div { span { class: "text-sm text-fg-muted", "Root: " } span { class: "font-mono text-sm", "{root}" } }
                        }
                    }
                }
            }
            if let Some(ref sub) = pages_subdomain {
                div { span { class: "text-sm text-fg-muted", "Preview: " }
                    a { href: "https://{sub}", target: "_blank", class: "font-mono text-sm text-brand underline", "https://{sub}" } }
            }
        }}
    }
}

/// Display for a direct-upload Pages project with deploy API instructions.
#[component]
fn DirectUploadDisplay(webspace_id: Uuid, project_name: String, production_branch: String, pages_subdomain: Option<String>) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let ws_id = webspace_id.to_string();

    let mut branch_input = use_signal(move || production_branch.clone());
    let mut saving_branch = use_signal(|| false);
    let mut branch_msg = use_signal(|| None::<String>);

    rsx! {
        Card { div { class: "p-6 space-y-4",
            div { class: "flex items-center gap-2",
                Badge { variant: BadgeVariant::Accent, "Direct Upload" }
                span { class: "font-mono text-sm text-fg-muted", "{project_name}" }
            }

            // Production branch setting
            div {
                div { class: "text-sm font-medium mb-1", "Production Branch" }
                div { class: "text-sm text-fg-muted mb-2",
                    "Deployments to this branch go live. Other branches create preview deployments."
                }
                div { class: "flex items-end gap-3",
                    FormField { label: "Branch",
                        input { class: "input w-48 font-mono text-sm", r#type: "text",
                            value: "{branch_input}",
                            oninput: move |evt| branch_input.set(evt.value()),
                        }
                    }
                    Button {
                        variant: ButtonVariant::Secondary,
                        disabled: *saving_branch.read(),
                        onclick: {
                            let wid = webspace_id;
                            move |_| {
                                let b = branch_input.read().clone();
                                saving_branch.set(true);
                                branch_msg.set(None);
                                spawn(async move {
                                    match update_production_branch(wid, b).await {
                                        Ok(()) => {
                                            refresh += 1;
                                        }
                                        Err(e) => {
                                            branch_msg.set(Some(format!("Error: {e}")));
                                            saving_branch.set(false);
                                        }
                                    }
                                });
                            }
                        },
                        if *saving_branch.read() { "Saving..." } else { "Update Branch" }
                    }
                    if let Some(msg) = &*branch_msg.read() {
                        span { class: "text-sm text-fg-muted", "{msg}" }
                    }
                }
            }

            // Deploy instructions
            div { class: "border-t border-line-soft pt-4",
                div { class: "text-sm font-medium mb-1", "Deploy" }
                div { class: "text-sm text-fg-muted mb-2",
                    "Create a deploy token at " span { class: "font-semibold", "Tokens → Create Token → Kind: Deploy" }
                    ", then use the CLI to upload your build output."
                }

                div { class: "text-sm text-fg-muted mb-1", "Install:" }
                div { class: "font-mono text-sm bg-surface-2 px-4 py-2 rounded select-all mb-3",
                    "cargo install --git https://git.plan.ai/plan-ai/mac-mgmt web-agency-upload"
                }

                div { class: "text-sm text-fg-muted mb-1", "Deploy:" }
                div { class: "font-mono text-sm bg-surface-2 px-4 py-2 rounded select-all mb-3 whitespace-pre",
                    "export WEB_AGENCY_TOKEN=your-deploy-token\nexport WEB_AGENCY_URL=https://your-server.example.com\nweb-agency-upload ./dist"
                }

                div { class: "text-sm text-fg-muted mb-1", "Or with explicit arguments:" }
                div { class: "font-mono text-sm bg-surface-2 px-4 py-2 rounded select-all mb-3 whitespace-pre",
                    "web-agency-upload ./dist \\\n  --token YOUR_TOKEN \\\n  --url https://your-server.example.com \\\n  --webspace-id {ws_id}"
                }

                div { class: "text-sm text-fg-muted mb-1", "Deploy to a preview branch:" }
                div { class: "font-mono text-sm bg-surface-2 px-4 py-2 rounded select-all",
                    "web-agency-upload ./dist --branch preview"
                }

                div { class: "mt-3",
                    Link {
                        to: crate::web::app::Route::DocPage { slug: "upload-cli".into() },
                        class: "text-sm text-brand underline",
                        "Full CLI documentation →"
                    }
                }
            }

            if let Some(ref sub) = pages_subdomain {
                div { class: "border-t border-line-soft pt-4",
                    span { class: "text-sm text-fg-muted", "Preview: " }
                    a { href: "https://{sub}", target: "_blank", class: "font-mono text-sm text-brand underline", "https://{sub}" }
                }
            }
        }}
    }
}

/// Recent deployments table.
#[component]
fn DeploymentsSection(webspace_id: Uuid) -> Element {
    let deploys = use_server_future(move || {
        let wid = webspace_id;
        async move { list_deployments(wid).await }
    })?;

    let rows = match &*deploys.read() {
        Some(Ok(r)) => r.clone(),
        Some(Err(e)) => return rsx! { Card { div { class: "p-4 text-danger text-sm", "Error: {e}" } } },
        None => return rsx! { Card { div { class: "p-4 text-fg-muted text-sm", "Loading..." } } },
    };

    rsx! {
        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead { tr { Th { "Status" } Th { "Size" } Th { "Created" } Th { "Error" } } }
                    tbody {
                        if rows.is_empty() {
                            tr { td { class: "td text-fg-muted text-center", colspan: "4", "No deployments yet" } }
                        }
                        for row in &rows {
                            tr {
                                Td {
                                    match row.status.as_str() {
                                        "success" => rsx! { Badge { variant: BadgeVariant::Success, "Success" } },
                                        "failed" => rsx! { Badge { variant: BadgeVariant::Danger, "Failed" } },
                                        "deploying" | "uploading" => rsx! { Badge { variant: BadgeVariant::Warn, "{row.status}" } },
                                        "pending" => rsx! { Badge { "Pending" } },
                                        _ => rsx! { Badge { "{row.status}" } },
                                    }
                                }
                                TdMuted {
                                    {row.tarball_size.map(|s| {
                                        if s > 1_048_576 { format!("{:.1} MB", s as f64 / 1_048_576.0) }
                                        else if s > 1024 { format!("{:.0} KB", s as f64 / 1024.0) }
                                        else { format!("{s} B") }
                                    }).unwrap_or_else(|| "-".into())}
                                }
                                TdMuted { "{row.created_at}" }
                                Td {
                                    if let Some(err) = &row.error_message {
                                        span { class: "text-sm text-danger", "{err}" }
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

/// Inline deploy-token creation (org admin only).
#[component]
fn DeployTokenSection(webspace_id: Uuid, organization_id: Uuid) -> Element {
    let mut label = use_signal(String::new);
    let mut creating = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut result = use_signal(|| None::<TokenCreateResult>);

    if let Some(res) = &*result.read() {
        return rsx! {
            Card { div { class: "p-6",
                div { class: "text-sm text-fg-muted mb-2", "Copy this token now — it won't be shown again." }
                div { class: "font-mono text-sm bg-surface-2 p-3 rounded break-all select-all", "{res.token}" }
            }}
        };
    }

    rsx! {
        Card { div { class: "p-4",
            div { class: "flex items-end gap-3",
                FormField { label: "Label",
                    input {
                        class: "input w-64",
                        r#type: "text",
                        placeholder: "e.g. CI deploy",
                        required: true,
                        value: "{label}",
                        oninput: move |evt| label.set(evt.value()),
                    }
                }
                Button {
                    variant: ButtonVariant::Primary,
                    disabled: label.read().is_empty() || *creating.read(),
                    onclick: {
                        let wid = webspace_id;
                        let oid = organization_id;
                        move |_| {
                            let l = label.read().clone();
                            creating.set(true);
                            error.set(None);
                            spawn(async move {
                                match create_deploy_token(wid, oid, l).await {
                                    Ok(r) => result.set(Some(r)),
                                    Err(e) => error.set(Some(format!("{e}"))),
                                }
                                creating.set(false);
                            });
                        }
                    },
                    if *creating.read() { "Creating..." } else { "Create Deploy Token" }
                }
            }
            if let Some(err) = &*error.read() {
                div { class: "mt-2 text-danger text-sm", "{err}" }
            }
        }}
    }
}

// ── Move webspace ──────────────────────────────────────────────────

#[server]
async fn list_move_target_orgs() -> Result<Vec<crate::web::user::OrgOption>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    crate::web::user::list_user_write_orgs(&user, &pool).await
}

#[server]
async fn move_webspace(webspace_id: Uuid, target_org_id: Uuid) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM webspaces WHERE id = $1",
    ).bind(webspace_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;
    user.require_org_write(&target_org_id)?;

    if org_id == target_org_id {
        return Err(ServerFnError::new("webspace is already in that organization"));
    }

    // Check for name conflicts in target org
    let conflict = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM webspaces WHERE organization_id = $1 AND name = (SELECT name FROM webspaces WHERE id = $2))",
    ).bind(target_org_id).bind(webspace_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    if conflict {
        return Err(ServerFnError::new("a webspace with the same name already exists in the target organization"));
    }

    sqlx::query("UPDATE webspaces SET organization_id = $1 WHERE id = $2")
        .bind(target_org_id).bind(webspace_id).execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

// ── Delete webspace ─────────────────────────────────────────────────

#[server]
async fn delete_webspace(webspace_id: Uuid) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM webspaces WHERE id = $1",
    ).bind(webspace_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    // Clean up changedetection tags: delete all sub-URL tags and the webspace tag.
    if let Ok(Some(cred_id)) = sqlx::query_scalar::<_, Uuid>(
        "SELECT changedetection_credential_id FROM webspaces WHERE id = $1 \
         AND changedetection_credential_id IS NOT NULL",
    )
    .bind(webspace_id)
    .fetch_optional(&pool)
    .await
    {
        match crate::credentials::changedetection_client(&pool, cred_id).await {
            Ok((client, group_name)) => {
                // Delete all sub-URL tags.
                let suburl_tags = sqlx::query_scalar::<_, Uuid>(
                    "SELECT tag_id FROM changedetection_suburls \
                     WHERE webspace_id = $1 AND tag_id IS NOT NULL",
                )
                .bind(webspace_id)
                .fetch_all(&pool)
                .await
                .unwrap_or_default();
                for tag_id in &suburl_tags {
                    if let Err(e) = client.delete_tag(tag_id).await {
                        tracing::warn!(%webspace_id, %tag_id, "failed to delete suburl tag: {e}");
                    }
                }
                // Delete the webspace-level tag ("group:ws_name").
                let ws_name = sqlx::query_scalar::<_, String>(
                    "SELECT name FROM webspaces WHERE id = $1",
                )
                .bind(webspace_id)
                .fetch_optional(&pool)
                .await
                .ok()
                .flatten();
                if let Some(ws_name) = ws_name {
                    let ws_tag_title = format!("{group_name}:{ws_name}");
                    if let Ok(tags) = client.list_tags().await {
                        for (uuid_str, tag) in tags.into_inner().iter() {
                            if tag.title.as_deref().map(|t| t.as_str()) == Some(&ws_tag_title) {
                                if let Ok(uuid) = uuid_str.parse::<Uuid>() {
                                    if let Err(e) = client.delete_tag(&uuid).await {
                                        tracing::warn!(%webspace_id, title = ws_tag_title, "failed to delete webspace tag: {e}");
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(%webspace_id, "failed to get changedetection client for tag cleanup: {e}");
            }
        }
    }

    // Remove domain bindings
    sqlx::query("DELETE FROM webspace_domains WHERE webspace_id = $1")
        .bind(webspace_id).execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    // Remove deployments
    sqlx::query("DELETE FROM deployments WHERE webspace_id = $1")
        .bind(webspace_id).execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    // Delete the webspace
    sqlx::query("DELETE FROM webspaces WHERE id = $1")
        .bind(webspace_id).execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    crate::api::internal::notify_proxy_reload();
    Ok(())
}

#[component]
fn MoveWebspaceSection(webspace_id: Uuid, current_org_id: Uuid) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let orgs = use_server_future(list_move_target_orgs)?;
    let mut selected_org = use_signal(String::new);
    let mut moving = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    let org_list = match &*orgs.read() {
        Some(Ok(list)) => list.clone(),
        _ => vec![],
    };

    let targets: Vec<_> = org_list.into_iter().filter(|o| o.id != current_org_id).collect();
    if targets.is_empty() {
        return rsx! {};
    }

    rsx! {
        Card {
            div { class: "p-4 flex items-center justify-between gap-4",
                div {
                    div { class: "font-medium text-danger", "Move to another organization" }
                    div { class: "text-sm text-fg-muted", "Transfers this webspace to a different organization." }
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
                        onclick: {
                            let wid = webspace_id;
                            move |_| {
                                let target = selected_org.read().clone();
                                if let Ok(tid) = Uuid::parse_str(&target) {
                                    moving.set(true);
                                    error.set(None);
                                    spawn(async move {
                                        match move_webspace(wid, tid).await {
                                            Ok(()) => {
                                                refresh += 1;
                                            }
                                            Err(e) => {
                                                error.set(Some(format!("{e}")));
                                                moving.set(false);
                                            }
                                        }
                                    });
                                }
                            }
                        },
                        if *moving.read() { "Moving..." } else { "Move Webspace" }
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
fn DeleteWebspaceSection(webspace_id: Uuid) -> Element {
    let mut deleting = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    rsx! {
        Card {
            div { class: "p-4 flex items-center justify-between",
                div {
                    div { class: "text-sm font-medium", "Delete this webspace" }
                    div { class: "text-sm text-fg-muted", "All domain bindings and deployments will be removed." }
                }
                Button {
                    variant: ButtonVariant::Danger,
                    disabled: *deleting.read(),
                    onclick: {
                        let wid = webspace_id;
                        move |_| {
                            deleting.set(true);
                            error.set(None);
                            spawn(async move {
                                match delete_webspace(wid).await {
                                    Ok(()) => { navigator().push(crate::web::app::Route::WebspaceList {}); }
                                    Err(e) => {
                                        error.set(Some(format!("{e}")));
                                        deleting.set(false);
                                    }
                                }
                            });
                        }
                    },
                    if *deleting.read() { "Deleting..." } else { "Delete Webspace" }
                }
            }
            if let Some(err) = &*error.read() {
                div { class: "px-4 pb-4 text-danger text-sm", "{err}" }
            }
        }
    }
}

// ── Webspace settings (name, upstream URL) ──────────────────────────

#[server]
async fn update_webspace_settings(webspace_id: Uuid, name: String, relay_url: Option<String>) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let row = sqlx::query_as::<_, (Uuid, String, Option<String>, Option<Uuid>)>(
        "SELECT organization_id, hosting_type, cloudflare_pages_project, cloudflare_credential_id FROM webspaces WHERE id = $1",
    ).bind(webspace_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let (org_id, hosting_type, cf_project, cf_cred_id) = row;

    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    // For CF Pages: rename the Pages project if the name changed
    if hosting_type == "cloudflare_pages" {
        if let (Some(old_project), Some(cred_id)) = (&cf_project, cf_cred_id) {
            if old_project != &name {
                let (client, account_id) = crate::credentials::cf_client_with_account(&pool, cred_id).await
                    .map_err(|e| ServerFnError::new(format!("{e}")))?;

                // CF Pages doesn't support rename — create new project, but that's disruptive.
                // Instead just update local name; the Pages project name stays the same.
                // The name field in our DB is for display purposes.
                tracing::info!(
                    old = old_project, new = &name,
                    "renaming webspace (Pages project name unchanged: {old_project})"
                );
                let _ = (client, account_id); // suppress unused warning
            }
        }
    }

    sqlx::query(
        "UPDATE webspaces SET name = $1, relay_url = $2, updated_at = now() WHERE id = $3",
    )
    .bind(&name).bind(&relay_url).bind(webspace_id)
    .execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    crate::api::internal::notify_proxy_reload();
    Ok(())
}

#[component]
fn WebspaceSettingsSection(
    webspace_id: Uuid,
    current_name: String,
    hosting_type: String,
    current_relay_url: Option<String>,
) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let mut name = use_signal(move || current_name.clone());
    let mut relay_url = use_signal(move || current_relay_url.clone().unwrap_or_default());
    let mut saving = use_signal(|| false);
    let mut message = use_signal(|| None::<String>);

    let is_relay_or_tunnel = hosting_type == "relay" || hosting_type == "tunnel";

    rsx! {
        Card {
            div { class: "p-4",
                div { class: "flex items-end gap-3 flex-wrap",
                    FormField { label: "Name",
                        input {
                            class: "input w-64",
                            r#type: "text",
                            value: "{name}",
                            oninput: move |evt| name.set(evt.value()),
                        }
                    }

                    if is_relay_or_tunnel {
                        FormField { label: "Upstream URL",
                            input {
                                class: "input w-80 font-mono",
                                r#type: "url",
                                placeholder: "https://backend.example.com",
                                value: "{relay_url}",
                                oninput: move |evt| relay_url.set(evt.value()),
                            }
                        }
                    }

                    Button {
                        variant: ButtonVariant::Primary,
                        disabled: *saving.read(),
                        onclick: {
                            let wid = webspace_id;
                            move |_| {
                                let n = name.read().clone();
                                let url = if is_relay_or_tunnel {
                                    let u = relay_url.read().clone();
                                    if u.is_empty() { None } else { Some(u) }
                                } else {
                                    None
                                };
                                saving.set(true);
                                message.set(None);
                                spawn(async move {
                                    match update_webspace_settings(wid, n, url).await {
                                        Ok(()) => {
                                            message.set(Some("Saved".into()));
                                            refresh += 1;
                                        }
                                        Err(e) => message.set(Some(format!("Error: {e}"))),
                                    }
                                    saving.set(false);
                                });
                            }
                        },
                        if *saving.read() { "Saving..." } else { "Save" }
                    }

                    if let Some(msg) = &*message.read() {
                        span { class: "text-sm text-fg-muted", "{msg}" }
                    }
                }
            }
        }
    }
}

// ── Auth settings ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BasicAuthListOption {
    id: Uuid,
    name: String,
}

#[server]
async fn load_basic_auth_lists(org_id: Uuid) -> Result<Vec<BasicAuthListOption>, ServerFnError> {
    let _user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let rows = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, name FROM basic_auth_lists WHERE organization_id = $1 ORDER BY name",
    )
    .bind(org_id).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rows.into_iter().map(|(id, name)| BasicAuthListOption { id, name }).collect())
}

#[server]
async fn update_webspace_auth(webspace_id: Uuid, auth_mode: String, auth_basic_list_id: Option<Uuid>) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM webspaces WHERE id = $1",
    ).bind(webspace_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    sqlx::query(
        "UPDATE webspaces SET auth_mode = $1, auth_basic_list_id = $2, updated_at = now() WHERE id = $3",
    )
    .bind(&auth_mode).bind(auth_basic_list_id).bind(webspace_id)
    .execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    crate::api::internal::notify_proxy_reload();
    Ok(())
}

#[component]
fn ChangeDetectionSection(webspace_id: Uuid, current_credential_id: Option<Uuid>, current_credential_name: Option<String>) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let cd_creds = use_server_future(list_cd_creds)?;
    let cred_list: Vec<CredOption> = match &*cd_creds.read() { Some(Ok(c)) => c.clone(), _ => vec![] };

    let mut selected_cred = use_signal(move || current_credential_id.map(|id| id.to_string()).unwrap_or_default());
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
                            onclick: {
                                let wid = webspace_id;
                                move |_| {
                                    let cred_str = selected_cred.read().clone();
                                    saving.set(true);
                                    result_msg.set(None);
                                    spawn(async move {
                                        let cid = uuid::Uuid::parse_str(&cred_str).ok();
                                        match set_webspace_changedetection(wid, cid).await {
                                            Ok(()) => {
                                                result_msg.set(Some("Saved".into()));
                                                *refresh.write() += 1;
                                            }
                                            Err(e) => result_msg.set(Some(format!("Error: {e}"))),
                                        }
                                        saving.set(false);
                                    });
                                }
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
fn AuthSettingsSection(webspace_id: Uuid, organization_id: Uuid, current_mode: String, current_list_name: Option<String>) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let lists = use_server_future(move || {
        let oid = organization_id;
        async move { load_basic_auth_lists(oid).await }
    })?;
    let basic_auth_lists = match &*lists.read() {
        Some(Ok(l)) => l.clone(),
        _ => vec![],
    };

    let mut auth_mode = use_signal(move || current_mode.clone());
    let mut basic_list_id = use_signal(|| basic_auth_lists.first().map(|b| b.id.to_string()).unwrap_or_default());
    let mut saving = use_signal(|| false);
    let mut message = use_signal(|| None::<String>);

    rsx! {
        Card {
            div { class: "p-4 space-y-4",
                div { class: "flex items-end gap-3 flex-wrap",
                    FormField { label: "Auth Mode",
                        select {
                            class: "input w-48",
                            value: "{auth_mode}",
                            oninput: move |evt| auth_mode.set(evt.value()),
                            option { value: "none", "None" }
                            option { value: "oidc", "OIDC (org members)" }
                            option { value: "basic", "HTTP Basic" }
                        }
                    }

                    if *auth_mode.read() == "basic" {
                        FormField { label: "Basic Auth List",
                            select {
                                class: "input w-48",
                                value: "{basic_list_id}",
                                oninput: move |evt| basic_list_id.set(evt.value()),
                                if basic_auth_lists.is_empty() {
                                    option { value: "", "No lists — create one first" }
                                }
                                for b in &basic_auth_lists {
                                    option { value: "{b.id}", "{b.name}" }
                                }
                            }
                        }
                    }

                    Button {
                        variant: ButtonVariant::Primary,
                        disabled: *saving.read(),
                        onclick: {
                            let wid = webspace_id;
                            move |_| {
                                let mode = auth_mode.read().clone();
                                let list_id = if mode == "basic" {
                                    uuid::Uuid::parse_str(&basic_list_id.read()).ok()
                                } else {
                                    None
                                };
                                saving.set(true);
                                message.set(None);
                                spawn(async move {
                                    match update_webspace_auth(wid, mode, list_id).await {
                                        Ok(()) => {
                                            message.set(Some("Saved".into()));
                                            refresh += 1;
                                        }
                                        Err(e) => message.set(Some(format!("Error: {e}"))),
                                    }
                                    saving.set(false);
                                });
                            }
                        },
                        if *saving.read() { "Saving..." } else { "Save" }
                    }

                    if let Some(msg) = &*message.read() {
                        span { class: "text-sm text-fg-muted", "{msg}" }
                    }
                }

                if *auth_mode.read() == "oidc" {
                    div { class: "text-sm text-fg-muted",
                        "Members of this webspace's organization will have access after logging in via the agency."
                    }
                }
            }
        }
    }
}

/// Domain bindings list + add form.
#[component]
fn DomainBindingsSection(webspace_id: Uuid, bindings: Vec<DomainBinding>, is_pages: bool, #[props(default = String::new())] hosting_type: String) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let is_tunnel = hosting_type == "relay" || hosting_type == "tunnel";
    let show_cname = is_pages || is_tunnel;
    let domains = use_server_future(move || {
        let wid = webspace_id;
        async move { list_domains_for_binding(wid).await }
    })?;
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
    let mut recheck_msg: Signal<Option<String>> = use_signal(|| None);

    rsx! {
        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead {
                        tr {
                            Th { "Hostname" }
                            Th { "Domain" }
                            if show_cname {
                                Th { "CNAME" }
                            }
                            if is_pages {
                                Th { "Verification" }
                            }
                            Th { "" }
                        }
                    }
                    tbody {
                        if bindings.is_empty() {
                            tr {
                                td {
                                    class: "td text-fg-muted text-center",
                                    colspan: if is_pages { "5" } else if is_tunnel { "4" } else { "3" },
                                    "No domains bound"
                                }
                            }
                        }
                        for b in &bindings {
                            {
                                let bid = b.binding_id;
                                let did = b.domain_id;
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
                                        if show_cname {
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
                                                                let wid = webspace_id;
                                                                let hostname = hostname.clone();
                                                                let sub_id = b.subdomain_id;
                                                                move |_| {
                                                                    let hostname = hostname.clone();
                                                                    fixing.set(Some(bid));
                                                                    spawn(async move {
                                                                        if is_tunnel {
                                                                            let _ = fix_cname_tunnel(wid, did, sub_id, hostname).await;
                                                                        } else {
                                                                            let _ = fix_cname(wid, did, sub_id, hostname).await;
                                                                        }
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
                                        }
                                        if is_pages {
                                            // Verification status column
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
                                                                    let wid = webspace_id;
                                                                    let hostname = hostname.clone();
                                                                    move |_| {
                                                                        let hostname = hostname.clone();
                                                                        rechecking.set(Some(bid));
                                                                        recheck_msg.set(None);
                                                                        spawn(async move {
                                                                            match recheck_custom_domain(wid, hostname).await {
                                                                                Ok(_) => {
                                                                                    rechecking.set(None);
                                                                                    refresh += 1;
                                                                                }
                                                                                Err(e) => {
                                                                                    recheck_msg.set(Some(format!("Error: {e}")));
                                                                                    rechecking.set(None);
                                                                                }
                                                                            }
                                                                        });
                                                                    }
                                                                },
                                                                if is_rechecking { "..." } else { "Recheck" }
                                                            }
                                                        }
                                                    },
                                                    Some("initializing") | Some("deactivated") | Some("blocked") | Some("error") => rsx! {
                                                        Badge { variant: BadgeVariant::Danger, {cf_status.as_deref().unwrap_or("error")} }
                                                    },
                                                    Some(s) => rsx! { Badge { "{s}" } },
                                                    None => rsx! {
                                                        div { class: "flex items-center gap-2",
                                                            Badge { variant: BadgeVariant::Danger, "Not on CF" }
                                                            Button {
                                                                variant: ButtonVariant::Secondary,
                                                                disabled: is_rechecking,
                                                                onclick: {
                                                                    let wid = webspace_id;
                                                                    let hostname = hostname.clone();
                                                                    move |_| {
                                                                        let hostname = hostname.clone();
                                                                        rechecking.set(Some(bid));
                                                                        spawn(async move {
                                                                            let _ = recheck_custom_domain(wid, hostname).await;
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
                                                    let wid = webspace_id;
                                                    let hostname = hostname.clone();
                                                    move |_| {
                                                        let hostname = hostname.clone();
                                                        removing.set(Some(bid));
                                                        spawn(async move {
                                                            let _ = unbind_domain(wid, bid, hostname).await;
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

            // Add binding form — select encodes "domain_id|subdomain_id_or_none|hostname"
            div { class: "p-4 border-t border-line-soft",
                div { class: "flex items-end gap-3",
                    FormField { label: "Bind Domain / Subdomain",
                        select {
                            class: "input",
                            value: "{selected_domain}",
                            oninput: move |evt| selected_domain.set(evt.value()),
                            option { value: "", "Select..." }
                            for d in &domain_list {
                                // Root domain option
                                option { value: "{d.id}||{d.name}", "{d.name} (root)" }
                                // Subdomain options
                                for s in &d.subdomains {
                                    {
                                        let hostname = if s.name == "@" {
                                            d.name.clone()
                                        } else {
                                            format!("{}.{}", s.name, d.name)
                                        };
                                        rsx! {
                                            option { value: "{d.id}|{s.id}|{hostname}", "  {hostname}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Button {
                        variant: ButtonVariant::Primary,
                        disabled: selected_domain.read().is_empty() || *adding.read(),
                        onclick: {
                            let wid = webspace_id;
                            let sel = selected_domain.read().clone();
                            move |_| {
                                let sel = sel.clone();
                                adding.set(true);
                                error.set(None);
                                spawn(async move {
                                    // Parse "domain_id|subdomain_id_or_empty|hostname"
                                    let parts: Vec<&str> = sel.splitn(3, '|').collect();
                                    if parts.len() == 3 {
                                        let did = uuid::Uuid::parse_str(parts[0]).ok();
                                        let sid = if parts[1].is_empty() { None } else { uuid::Uuid::parse_str(parts[1]).ok() };
                                        let hostname = parts[2].to_string();
                                        if let Some(did) = did {
                                            match bind_domain(wid, did, sid, hostname).await {
                                                Ok(()) => { refresh += 1; }
                                                Err(e) => error.set(Some(format!("{e}"))),
                                            }
                                        }
                                    }
                                    adding.set(false);
                                });
                            }
                        },
                        if *adding.read() { "Adding..." } else { "Bind" }
                    }
                }
                if let Some(err) = &*error.read() {
                    div { class: "mt-2 text-danger text-sm", "{err}" }
                }
            }
        }
    }
}
