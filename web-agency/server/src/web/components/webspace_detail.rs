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
    organization_name: String,
    bindings: Vec<DomainBinding>,
    // Live CF Pages info
    pages_subdomain: Option<String>,
    production_branch: Option<String>,
    git_source: Option<GitRepoInfo>,
    build_config: Option<BuildConfigInfo>,
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
    domain_name: String,
    subdomain_name: Option<String>,
    /// Full hostname: subdomain.domain or just domain
    hostname: String,
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

    let row = sqlx::query_as::<_, (Uuid, String, String, Option<String>, Option<Uuid>, Option<String>, Option<String>, Uuid)>(
        "SELECT w.id, w.name, w.hosting_type, w.cloudflare_pages_project, w.cloudflare_credential_id, \
         w.runtime, w.local_status, w.organization_id \
         FROM webspaces w WHERE w.id = $1",
    )
    .bind(webspace_id).fetch_optional(&pool).await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("webspace not found"))?;

    let (id, name, hosting_type, cf_project, cf_cred_id, runtime, local_status, org_id) = row;

    if !user.is_admin && !user.org_ids().contains(&org_id) {
        return Err(ServerFnError::new("access denied"));
    }

    let org_name = sqlx::query_scalar::<_, String>("SELECT name FROM organizations WHERE id = $1")
        .bind(org_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let bindings = sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>)>(
        "SELECT wd.id, wd.domain_id, d.name, s.name \
         FROM webspace_domains wd \
         JOIN domains d ON d.id = wd.domain_id \
         LEFT JOIN subdomains s ON s.id = wd.subdomain_id \
         WHERE wd.webspace_id = $1 ORDER BY d.name",
    )
    .bind(webspace_id).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    .into_iter()
    .map(|(binding_id, domain_id, domain_name, subdomain_name)| {
        let hostname = match &subdomain_name {
            Some(sub) if sub != "@" => format!("{sub}.{domain_name}"),
            _ => domain_name.clone(),
        };
        DomainBinding { binding_id, domain_id, domain_name, subdomain_name, hostname }
    })
    .collect();

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
                    if let Some(cfg) = &src.config {
                        git_source = Some(GitRepoInfo {
                            provider: src.source_type.clone().unwrap_or_default(),
                            owner: cfg.owner.clone().unwrap_or_default(),
                            repo: cfg.repo_name.clone().unwrap_or_default(),
                            production_branch: cfg.production_branch.clone().unwrap_or_else(|| "main".into()),
                        });
                    }
                }
                if let Some(bc) = &project.build_config {
                    build_config_info = Some(BuildConfigInfo {
                        build_command: bc.build_command.clone(),
                        destination_dir: bc.destination_dir.clone(),
                        root_dir: bc.root_dir.clone(),
                    });
                }
            }
        }
    }

    Ok(WebspaceData {
        id, name, hosting_type, cloudflare_pages_project: cf_project,
        cloudflare_credential_id: cf_cred_id, runtime, local_status,
        organization_name: org_name, bindings,
        pages_subdomain, production_branch, git_source, build_config: build_config_info,
    })
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
    if !user.is_admin && !user.write_org_ids().contains(&org_id) {
        return Err(ServerFnError::new("write access required"));
    }

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
    if !user.is_admin && !user.write_org_ids().contains(&org_id) {
        return Err(ServerFnError::new("write access required"));
    }

    let project_name = project_name.ok_or_else(|| ServerFnError::new("Pages project not deployed yet"))?;
    let cred_id = cred_id.ok_or_else(|| ServerFnError::new("no Cloudflare credential"))?;

    let (client, account_id) = build_cf_pages_client(&pool, cred_id).await?;

    let update = cloudflare_api::UpdatePagesProject {
        production_branch: Some(production_branch.clone()),
        source: Some(cloudflare_api::PagesSource {
            source_type: Some(provider.clone()),
            config: Some(cloudflare_api::PagesSourceConfig {
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
        build_config: Some(cloudflare_api::PagesBuildConfig {
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
    if !user.is_admin && !user.write_org_ids().contains(&org_id) {
        return Err(ServerFnError::new("write access required"));
    }

    let project_name = project_name.ok_or_else(|| ServerFnError::new("no Pages project"))?;
    let cred_id = cred_id.ok_or_else(|| ServerFnError::new("no CF credential"))?;

    let (client, account_id) = build_cf_pages_client(&pool, cred_id).await?;

    let update = cloudflare_api::UpdatePagesProject {
        production_branch: Some(branch.clone()),
        source: None,
        build_config: None,
    };

    client.update_pages_project(&account_id, &project_name, &update).await
        .map_err(|e| ServerFnError::new(format!("failed to update production branch: {e}")))?;

    tracing::info!("updated production branch for {project_name} to {branch}");
    Ok(())
}

/// Bind a domain (or subdomain) to this webspace. For CF Pages, adds the custom domain.
#[server]
async fn bind_domain(webspace_id: Uuid, domain_id: Uuid, subdomain_id: Option<Uuid>, hostname: String) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    if !user.is_admin && !user.write_org_ids().contains(&org_id) {
        return Err(ServerFnError::new("write access required"));
    }

    sqlx::query(
        "INSERT INTO webspace_domains (webspace_id, domain_id, subdomain_id) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(webspace_id).bind(domain_id).bind(subdomain_id)
    .execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    // For CF Pages webspaces, add as custom domain
    let ws = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
        "SELECT cloudflare_pages_project, cloudflare_credential_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    if let (Some(project_name), Some(cred_id)) = ws {
        let (client, account_id) = build_cf_pages_client(&pool, cred_id).await?;
        if let Err(e) = client.add_pages_custom_domain(&account_id, &project_name, &hostname).await {
            tracing::warn!("failed to add custom domain {hostname} to Pages: {e}");
        } else {
            tracing::info!("added custom domain {hostname} to Pages project {project_name}");
        }
    }

    Ok(())
}

/// Remove a domain binding. For CF Pages, removes the custom domain.
#[server]
async fn unbind_domain(webspace_id: Uuid, binding_id: Uuid, hostname: String) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    if !user.is_admin && !user.write_org_ids().contains(&org_id) {
        return Err(ServerFnError::new("write access required"));
    }

    // Remove CF Pages custom domain
    let ws = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
        "SELECT cloudflare_pages_project, cloudflare_credential_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    if let (Some(project_name), Some(cred_id)) = ws {
        let (client, account_id) = build_cf_pages_client(&pool, cred_id).await?;
        let _ = client.remove_pages_custom_domain(&account_id, &project_name, &hostname).await;
    }

    sqlx::query("DELETE FROM webspace_domains WHERE id = $1")
        .bind(binding_id).execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[cfg(feature = "server")]
async fn build_cf_pages_client(pool: &sqlx::PgPool, cred_id: Uuid) -> Result<(cloudflare_api::Client, String), ServerFnError> {
    let encrypted = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT encrypted_data FROM credentials WHERE id = $1 AND credential_type = 'cloudflare'",
    )
    .bind(cred_id).fetch_optional(pool).await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("Cloudflare credential not found"))?;

    let decrypted = crate::crypto::decrypt(&encrypted)
        .map_err(|e| ServerFnError::new(format!("decryption failed: {e}")))?;
    let data: serde_json::Value = serde_json::from_slice(&decrypted)
        .map_err(|e| ServerFnError::new(format!("invalid credential: {e}")))?;

    let token = data["api_token"].as_str()
        .ok_or_else(|| ServerFnError::new("missing api_token"))?;
    let account_id = data["account_id"].as_str().unwrap_or("").to_string();

    if account_id.is_empty() {
        return Err(ServerFnError::new("account_id required in credential for Pages"));
    }

    Ok((cloudflare_api::Client::new(token), account_id))
}

// ── Component ─────────────────────────────────────────────────────────

#[component]
pub fn WebspaceDetail(id: String) -> Element {
    let webspace_id = Uuid::parse_str(&id).ok();
    let webspace = use_server_future(move || {
        let wid = webspace_id;
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
            }
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

        // Domain bindings
        SectionHeading { class: "mt-6", "Domain Bindings" }
        DomainBindingsSection {
            webspace_id: data.id,
            bindings: data.bindings.clone(),
            is_pages,
        }
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
    let mut provider = use_signal(|| "github".to_string());
    let mut owner = use_signal(String::new);
    let mut repo_name = use_signal(String::new);
    let mut branch = use_signal(|| "main".to_string());
    let mut build_cmd = use_signal(String::new);
    let mut dest_dir = use_signal(String::new);
    let mut root_dir = use_signal(String::new);
    let mut connecting = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut success = use_signal(|| false);

    if *success.read() {
        return rsx! {
            Card {
                div { class: "p-6",
                    Badge { variant: BadgeVariant::Success, "Git repository connected" }
                    div { class: "mt-2 text-sm text-fg-muted", "Reload the page to see deployment details." }
                }
            }
        };
    }

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
                                    Ok(()) => success.set(true),
                                    Err(e) => error.set(Some(format!("{e}"))),
                                }
                                connecting.set(false);
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
    let mut result = use_signal(|| None::<PagesDeployResult>);

    if let Some(res) = &*result.read() {
        return rsx! {
            Card { div { class: "p-6",
                div { class: "flex items-center gap-2 mb-2",
                    Badge { variant: BadgeVariant::Success, "Deployed" }
                    span { class: "font-mono text-sm", "{res.project_name}" }
                }
                if let Some(ref sub) = res.subdomain {
                    div { class: "text-sm text-fg-muted", "Preview: " span { class: "font-mono", "https://{sub}" } }
                }
                div { class: "mt-2 text-sm text-fg-muted", "Reload the page to see full project details." }
            }}
        };
    }

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
                                    Ok(r) => {
                                        // Step 2: if git, connect repo
                                        if is_git {
                                            if let Err(e) = connect_git_repo(wid, gp, go, gr, gb, bc, dd, rd).await {
                                                error.set(Some(format!("Project created but git connection failed: {e}")));
                                                result.set(Some(r));
                                                deploying.set(false);
                                                return;
                                            }
                                        }
                                        result.set(Some(r));
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
                                        Ok(()) => branch_msg.set(Some("Production branch updated".into())),
                                        Err(e) => branch_msg.set(Some(format!("Error: {e}"))),
                                    }
                                    saving_branch.set(false);
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
                    "cargo install --path web-agency/upload-cli"
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

/// Domain bindings list + add form.
#[component]
fn DomainBindingsSection(webspace_id: Uuid, bindings: Vec<DomainBinding>, is_pages: bool) -> Element {
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

    rsx! {
        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead {
                        tr {
                            Th { "Hostname" }
                            Th { "Domain" }
                            if is_pages {
                                Th { "CF Pages" }
                            }
                            Th { "" }
                        }
                    }
                    tbody {
                        if bindings.is_empty() {
                            tr {
                                td {
                                    class: "td text-fg-muted text-center",
                                    colspan: if is_pages { "4" } else { "3" },
                                    "No domains bound"
                                }
                            }
                        }
                        for b in &bindings {
                            {
                                let bid = b.binding_id;
                                let hostname = b.hostname.clone();
                                let is_removing = *removing.read() == Some(bid);
                                rsx! {
                                    tr {
                                        Td { class: "font-mono", "{b.hostname}" }
                                        TdMuted { "{b.domain_name}" }
                                        if is_pages {
                                            Td { Badge { variant: BadgeVariant::Info, "Custom Domain" } }
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
                                                            // Trigger re-fetch by navigating to same page
                                                            navigator().replace(crate::web::app::Route::WebspaceDetail { id: wid.to_string() });
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
                                                Ok(()) => { navigator().replace(crate::web::app::Route::WebspaceDetail { id: wid.to_string() }); }
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
