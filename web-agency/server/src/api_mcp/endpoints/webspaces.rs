//! Webspace (folder) endpoints.

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
pub struct WebspaceRow {
    pub id: Uuid,
    pub name: String,
    pub host_id: Uuid,
    pub host_name: String,
    pub host_kind: String,
    pub path_prefix: String,
    pub hosting_type: String,
    pub runtime: Option<String>,
    pub local_status: Option<String>,
    pub auth_mode: String,
    pub organization_name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebspaceListInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebspaceDeleteInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GitRepoInfo {
    /// "github" or "gitlab".
    pub provider: String,
    pub owner: String,
    pub repo: String,
    pub production_branch: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BuildConfigInfo {
    pub build_command: Option<String>,
    pub destination_dir: Option<String>,
    pub root_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebspaceData {
    pub id: Uuid,
    pub name: String,
    pub hosting_type: String,
    pub cloudflare_pages_project: Option<String>,
    pub cloudflare_credential_id: Option<Uuid>,
    pub runtime: Option<String>,
    pub local_status: Option<String>,
    pub relay_url: Option<String>,
    pub organization_id: Uuid,
    pub organization_name: String,
    /// Whether the caller can manage tokens for this webspace's org.
    pub is_org_admin: bool,
    pub auth_mode: String,
    pub auth_basic_list_name: Option<String>,
    // Parent host (the hostname/domain/CNAME/CD owner) and this folder's mount path.
    pub webspace_host_id: Uuid,
    pub host_name: String,
    pub host_kind: String,
    pub path_prefix: String,
    // Live CF Pages info
    pub pages_subdomain: Option<String>,
    pub production_branch: Option<String>,
    pub git_source: Option<GitRepoInfo>,
    pub build_config: Option<BuildConfigInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebspaceGetInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebspaceCreateInput {
    /// Parent proxy host the folder is mounted on.
    pub host_id: Uuid,
    pub name: String,
    /// Mount path under the host's hostname, e.g. "/api".
    pub path_prefix: String,
    /// "local", "relay" or "tunnel".
    pub hosting_type: String,
    #[serde(default)]
    pub runtime: Option<String>,
    #[serde(default)]
    pub relay_url: Option<String>,
    #[serde(default)]
    pub relay_credential_id: Option<Uuid>,
    /// "none", "oidc" or "basic".
    pub auth_mode: String,
    #[serde(default)]
    pub auth_basic_list_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebspaceUpdateInput {
    pub id: Uuid,
    /// New display name (Pages project name stays unchanged for CF folders).
    #[serde(default)]
    pub name: Option<String>,
    /// New mount path (normalized: leading slash, no doubles/trailing).
    #[serde(default)]
    pub path_prefix: Option<String>,
    /// New upstream URL for relay/tunnel folders.
    #[serde(default)]
    pub relay_url: Option<String>,
    /// Set true to clear the upstream URL (when `relay_url` is absent).
    #[serde(default)]
    pub clear_relay_url: Option<bool>,
    /// New auth mode: "none", "oidc" or "basic".
    #[serde(default)]
    pub auth_mode: Option<String>,
    /// Basic-auth list to use (for `auth_mode == "basic"`).
    #[serde(default)]
    pub auth_basic_list_id: Option<Uuid>,
    /// Set true to clear the basic-auth list (when `auth_basic_list_id` is absent).
    #[serde(default)]
    pub clear_auth_basic_list: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebspaceMoveInput {
    pub id: Uuid,
    pub target_org_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebspaceDeployPagesInput {
    pub id: Uuid,
    /// Cloudflare credential to create/link the Pages project with.
    pub credential_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebspaceConnectGitInput {
    pub id: Uuid,
    /// "github" or "gitlab".
    pub provider: String,
    pub owner: String,
    pub repo_name: String,
    pub production_branch: String,
    /// Empty string leaves the build command unset.
    #[serde(default)]
    pub build_command: String,
    /// Empty string leaves the output directory unset.
    #[serde(default)]
    pub destination_dir: String,
    /// Empty string leaves the root directory unset.
    #[serde(default)]
    pub root_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebspaceSetProductionBranchInput {
    pub id: Uuid,
    pub branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebspaceDeploymentsInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeploymentRow {
    pub id: Uuid,
    pub status: String,
    pub error_message: Option<String>,
    pub tarball_size: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebspaceDiscoverPagesInput {
    pub credential_id: Uuid,
    pub org_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DiscoveredProject {
    pub name: String,
    pub id: Option<String>,
    pub subdomain: Option<String>,
    pub already_imported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebspaceImportPagesInput {
    pub credential_id: Uuid,
    pub org_id: Uuid,
    pub project_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ImportResult {
    pub imported: u32,
    pub skipped: u32,
    pub errors: Vec<String>,
}

// ── Server helpers ────────────────────────────────────────────────────

/// Cloudflare Pages client + account id for a credential.
#[cfg(feature = "server")]
async fn cf_pages_client(
    pool: &sqlx::PgPool,
    cred_id: Uuid,
) -> Result<(cloudflare_api::compat::SimpleClient, String), ApiError> {
    crate::credentials::cf_client_with_account(pool, cred_id)
        .await
        .map_err(|e| ApiError::internal(format!("{e}")))
}

/// Normalize a mount path: leading slash, no trailing slash, no doubles.
#[cfg(feature = "server")]
fn normalize_path_prefix(raw: &str) -> Result<String, ApiError> {
    let mut path = raw.trim().to_string();
    if !path.starts_with('/') {
        return Err(ApiError::bad_request("path must start with /"));
    }
    while path.contains("//") {
        path = path.replace("//", "/");
    }
    while path.len() > 1 && path.ends_with('/') {
        path.pop();
    }
    Ok(path)
}

/// List webspace folders the caller may see (admins: all; else their orgs').
#[api_mcp_dioxus_server(server = "list_webspaces")]
pub async fn webspace_list(
    pool: &sqlx::PgPool,
    principal: &Principal,
    _input: WebspaceListInput,
) -> Result<Vec<WebspaceRow>, ApiError> {
    type Row = (
        Uuid,
        String,
        Uuid,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        String,
        String,
    );
    let query = "SELECT w.id, w.name, h.id, h.name, h.kind, w.path_prefix, w.hosting_type, w.runtime, w.local_status, w.auth_mode, o.name \
                 FROM webspaces w \
                 JOIN webspace_hosts h ON h.id = w.webspace_host_id \
                 JOIN organizations o ON o.id = w.organization_id";

    let rows = match principal.read_filter() {
        None => {
            sqlx::query_as::<_, Row>(&format!("{query} ORDER BY h.name, w.path_prefix"))
                .fetch_all(pool)
                .await
        }
        Some(org_ids) => {
            sqlx::query_as::<_, Row>(&format!(
                "{query} WHERE w.organization_id = ANY($1) ORDER BY h.name, w.path_prefix"
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
                host_id,
                host_name,
                host_kind,
                path_prefix,
                hosting_type,
                runtime,
                local_status,
                auth_mode,
                organization_name,
            )| WebspaceRow {
                id,
                name,
                host_id,
                host_name,
                host_kind,
                path_prefix,
                hosting_type,
                runtime,
                local_status,
                auth_mode,
                organization_name,
            },
        )
        .collect())
}

/// Delete a webspace folder (requires org write). Removes its deployments and
/// notifies the proxy.
#[api_mcp_dioxus_server(server = "delete_webspace")]
pub async fn webspace_delete(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: WebspaceDeleteInput,
) -> Result<(), ApiError> {
    let org_id = super::owning_org(pool, "webspaces", input.id, "webspace").await?;
    principal.require_write(&org_id)?;

    sqlx::query("DELETE FROM deployments WHERE webspace_id = $1")
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(super::internal)?;

    sqlx::query("DELETE FROM webspaces WHERE id = $1")
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(super::internal)?;

    crate::api::internal::notify_proxy_reload();
    Ok(())
}

/// Get a webspace folder (requires org read): host info, auth settings and,
/// for CF Pages folders, live project details (subdomain, branch, git source,
/// build config).
#[api_mcp_dioxus_server(server = "get_webspace")]
pub async fn webspace_get(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: WebspaceGetInput,
) -> Result<WebspaceData, ApiError> {
    let row = sqlx::query_as::<_, (Uuid, String, String, Option<String>, Option<Uuid>, Option<String>, Option<String>, Uuid, Option<String>, String, Option<Uuid>, Uuid, String, String, String)>(
        "SELECT w.id, w.name, w.hosting_type, w.cloudflare_pages_project, w.cloudflare_credential_id, \
         w.runtime, w.local_status, w.organization_id, w.relay_url, w.auth_mode, w.auth_basic_list_id, \
         w.webspace_host_id, h.name, h.kind, w.path_prefix \
         FROM webspaces w JOIN webspace_hosts h ON h.id = w.webspace_host_id WHERE w.id = $1",
    )
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?
    .ok_or_else(|| ApiError::not_found("webspace not found"))?;

    let (
        id,
        name,
        hosting_type,
        cf_project,
        cf_cred_id,
        runtime,
        local_status,
        org_id,
        relay_url,
        auth_mode,
        auth_basic_list_id,
        webspace_host_id,
        host_name,
        host_kind,
        path_prefix,
    ) = row;

    principal.require_read(&org_id)?;

    // Fetch basic auth list name if set
    let auth_basic_list_name = if let Some(list_id) = auth_basic_list_id {
        sqlx::query_scalar::<_, String>("SELECT name FROM basic_auth_lists WHERE id = $1")
            .bind(list_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    let org_name = sqlx::query_scalar::<_, String>("SELECT name FROM organizations WHERE id = $1")
        .bind(org_id)
        .fetch_one(pool)
        .await
        .map_err(super::internal)?;

    // Fetch live CF Pages info
    let mut pages_subdomain = None;
    let mut production_branch = None;
    let mut git_source = None;
    let mut build_config_info = None;

    if let (Some(project_name), Some(cred_id)) = (&cf_project, cf_cred_id) {
        if let Ok((client, account_id)) = cf_pages_client(pool, cred_id).await {
            if let Ok(project) = client.get_pages_project(&account_id, project_name).await {
                pages_subdomain = project.subdomain;
                production_branch = project.production_branch;
                if let Some(src) = &project.source {
                    let cfg = src.get("config");
                    if cfg.is_some() {
                        git_source = Some(GitRepoInfo {
                            provider: src
                                .get("type")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            owner: cfg
                                .and_then(|c| c.get("owner"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            repo: cfg
                                .and_then(|c| c.get("repo_name"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            production_branch: cfg
                                .and_then(|c| c.get("production_branch"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("main")
                                .to_string(),
                        });
                    }
                }
                if let Some(bc) = &project.build_config {
                    build_config_info = Some(BuildConfigInfo {
                        build_command: bc
                            .get("build_command")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        destination_dir: bc
                            .get("destination_dir")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        root_dir: bc
                            .get("root_dir")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                    });
                }
            }
        }
    }

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

    Ok(WebspaceData {
        id,
        name,
        hosting_type,
        cloudflare_pages_project: cf_project,
        cloudflare_credential_id: cf_cred_id,
        runtime,
        local_status,
        relay_url,
        organization_id: org_id,
        organization_name: org_name,
        is_org_admin,
        auth_mode,
        auth_basic_list_name,
        webspace_host_id,
        host_name,
        host_kind,
        path_prefix,
        pages_subdomain,
        production_branch,
        git_source,
        build_config: build_config_info,
    })
}

/// Create a webspace folder on a proxy host (requires org write). Returns the
/// new folder's id and notifies the proxy.
#[api_mcp_dioxus_server(server = "create_folder")]
pub async fn webspace_create(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: WebspaceCreateInput,
) -> Result<Uuid, ApiError> {
    let (org_id, kind) = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT organization_id, kind FROM webspace_hosts WHERE id = $1",
    )
    .bind(input.host_id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?
    .ok_or_else(|| ApiError::not_found("host not found"))?;

    principal.require_write(&org_id)?;

    if kind != "proxy" {
        return Err(ApiError::bad_request(
            "folders can only be added to proxy hosts",
        ));
    }
    if !matches!(input.hosting_type.as_str(), "local" | "relay" | "tunnel") {
        return Err(ApiError::bad_request("invalid folder hosting type"));
    }
    let path = normalize_path_prefix(&input.path_prefix)?;

    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO webspaces (organization_id, webspace_host_id, name, path_prefix, hosting_type, runtime, relay_url, relay_credential_id, auth_mode, auth_basic_list_id, local_status) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'stopped') RETURNING id",
    )
    .bind(org_id)
    .bind(input.host_id)
    .bind(&input.name)
    .bind(&path)
    .bind(&input.hosting_type)
    .bind(&input.runtime)
    .bind(&input.relay_url)
    .bind(input.relay_credential_id)
    .bind(&input.auth_mode)
    .bind(input.auth_basic_list_id)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::internal(format!("failed to create folder: {e}")))?;

    crate::api::internal::notify_proxy_reload();
    Ok(id)
}

/// Update a webspace folder's settings (requires org write). Only provided
/// fields change: display name, mount path, upstream URL (relay/tunnel) and
/// auth mode/basic-auth list. Notifies the proxy.
#[api_mcp_dioxus_server(server = "update_webspace_settings")]
pub async fn webspace_update(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: WebspaceUpdateInput,
) -> Result<(), ApiError> {
    let row = sqlx::query_as::<_, (Uuid, String, Option<String>, Option<Uuid>, String, String, Option<String>, String, Option<Uuid>)>(
        "SELECT organization_id, hosting_type, cloudflare_pages_project, cloudflare_credential_id, \
         name, path_prefix, relay_url, auth_mode, auth_basic_list_id FROM webspaces WHERE id = $1",
    )
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?
    .ok_or_else(|| ApiError::not_found("webspace not found"))?;

    let (
        org_id,
        hosting_type,
        cf_project,
        cf_cred_id,
        cur_name,
        cur_path,
        cur_relay_url,
        cur_auth_mode,
        cur_auth_list,
    ) = row;

    principal.require_write(&org_id)?;

    // For CF Pages: renaming only changes the local display name; the Pages
    // project name stays the same (CF Pages doesn't support rename).
    if let Some(new_name) = &input.name {
        if hosting_type == "cloudflare_pages" {
            if let (Some(old_project), Some(cred_id)) = (&cf_project, cf_cred_id) {
                if old_project != new_name {
                    let (client, account_id) = cf_pages_client(pool, cred_id).await?;
                    tracing::info!(
                        old = old_project,
                        new = new_name,
                        "renaming webspace (Pages project name unchanged: {old_project})"
                    );
                    let _ = (client, account_id); // suppress unused warning
                }
            }
        }
    }

    let name = input.name.unwrap_or(cur_name);
    let path_prefix = match &input.path_prefix {
        Some(raw) => normalize_path_prefix(raw)?,
        None => cur_path,
    };
    let relay_url = if input.relay_url.is_some() {
        input.relay_url
    } else if input.clear_relay_url == Some(true) {
        None
    } else {
        cur_relay_url
    };
    let auth_mode = input.auth_mode.unwrap_or(cur_auth_mode);
    let auth_basic_list_id = if input.auth_basic_list_id.is_some() {
        input.auth_basic_list_id
    } else if input.clear_auth_basic_list == Some(true) {
        None
    } else {
        cur_auth_list
    };

    sqlx::query(
        "UPDATE webspaces SET name = $1, path_prefix = $2, relay_url = $3, auth_mode = $4, \
         auth_basic_list_id = $5, updated_at = now() WHERE id = $6",
    )
    .bind(&name)
    .bind(&path_prefix)
    .bind(&relay_url)
    .bind(&auth_mode)
    .bind(auth_basic_list_id)
    .bind(input.id)
    .execute(pool)
    .await
    .map_err(super::internal)?;

    crate::api::internal::notify_proxy_reload();
    Ok(())
}

/// Move a webspace folder to another organization (requires write on both
/// orgs).
#[api_mcp_dioxus_server(server = "move_webspace")]
pub async fn webspace_move(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: WebspaceMoveInput,
) -> Result<(), ApiError> {
    let org_id = super::owning_org(pool, "webspaces", input.id, "webspace").await?;
    principal.require_write(&org_id)?;
    principal.require_write(&input.target_org_id)?;

    if org_id == input.target_org_id {
        return Err(ApiError::bad_request(
            "webspace is already in that organization",
        ));
    }

    // Check for name conflicts in target org
    let conflict = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM webspaces WHERE organization_id = $1 AND name = (SELECT name FROM webspaces WHERE id = $2))",
    )
    .bind(input.target_org_id)
    .bind(input.id)
    .fetch_one(pool)
    .await
    .map_err(super::internal)?;
    if conflict {
        return Err(ApiError::conflict(
            "a webspace with the same name already exists in the target organization",
        ));
    }

    sqlx::query("UPDATE webspaces SET organization_id = $1 WHERE id = $2")
        .bind(input.target_org_id)
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(super::internal)?;

    Ok(())
}

/// Create (or link an existing) Cloudflare Pages project for this webspace
/// (requires org write). Returns a status string.
#[api_mcp_dioxus_server(server = "deploy_pages_project")]
pub async fn webspace_deploy_pages(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: WebspaceDeployPagesInput,
) -> Result<String, ApiError> {
    let (ws_name, org_id) = sqlx::query_as::<_, (String, Uuid)>(
        "SELECT name, organization_id FROM webspaces WHERE id = $1",
    )
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?
    .ok_or_else(|| ApiError::not_found("webspace not found"))?;

    principal.require_write(&org_id)?;

    let (client, account_id) = cf_pages_client(pool, input.credential_id).await?;

    tracing::info!("deploying Pages project {ws_name} to account {account_id}");

    // Check if project already exists
    let (project, created) = match client.get_pages_project(&account_id, &ws_name).await {
        Ok(p) => {
            tracing::info!("Pages project {ws_name} already exists");
            (p, false)
        }
        Err(_) => {
            let p = client
                .create_pages_project(&account_id, &ws_name, "main")
                .await
                .map_err(|e| {
                    ApiError::internal(format!(
                        "failed to create Pages project: {e}. \
                         Ensure the API token has 'Cloudflare Pages:Edit' permission for account {account_id}"
                    ))
                })?;
            tracing::info!("created Pages project {ws_name}");
            (p, true)
        }
    };

    sqlx::query(
        "UPDATE webspaces SET cloudflare_pages_project = $1, cloudflare_credential_id = $2, updated_at = now() WHERE id = $3",
    )
    .bind(&ws_name)
    .bind(input.credential_id)
    .bind(input.id)
    .execute(pool)
    .await
    .map_err(super::internal)?;

    let verb = if created { "created" } else { "linked existing" };
    Ok(match project.subdomain {
        Some(sub) => format!("{verb} Pages project '{ws_name}' (subdomain: {sub})"),
        None => format!("{verb} Pages project '{ws_name}'"),
    })
}

/// Connect a git repo to the webspace's CF Pages project (requires org write).
#[api_mcp_dioxus_server(server = "connect_git_repo")]
pub async fn webspace_connect_git(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: WebspaceConnectGitInput,
) -> Result<(), ApiError> {
    let (project_name, cred_id, org_id) = sqlx::query_as::<_, (Option<String>, Option<Uuid>, Uuid)>(
        "SELECT cloudflare_pages_project, cloudflare_credential_id, organization_id FROM webspaces WHERE id = $1",
    )
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?
    .ok_or_else(|| ApiError::not_found("webspace not found"))?;

    principal.require_write(&org_id)?;

    let project_name =
        project_name.ok_or_else(|| ApiError::bad_request("Pages project not deployed yet"))?;
    let cred_id = cred_id.ok_or_else(|| ApiError::bad_request("no Cloudflare credential"))?;

    let (client, account_id) = cf_pages_client(pool, cred_id).await?;

    let update = cloudflare_api::compat::UpdatePagesProject {
        production_branch: Some(input.production_branch.clone()),
        source: Some(cloudflare_api::compat::PagesSource {
            source_type: Some(input.provider.clone()),
            config: Some(cloudflare_api::compat::PagesSourceConfig {
                owner: Some(input.owner),
                repo_name: Some(input.repo_name),
                production_branch: Some(input.production_branch),
                pr_comments_enabled: Some(true),
                production_deployments_enabled: Some(true),
                preview_deployment_setting: Some("all".into()),
                preview_branch_includes: None,
                preview_branch_excludes: None,
            }),
        }),
        build_config: Some(cloudflare_api::compat::PagesBuildConfig {
            build_command: if input.build_command.is_empty() {
                None
            } else {
                Some(input.build_command)
            },
            destination_dir: if input.destination_dir.is_empty() {
                None
            } else {
                Some(input.destination_dir)
            },
            root_dir: if input.root_dir.is_empty() {
                None
            } else {
                Some(input.root_dir)
            },
            build_caching: Some(true),
        }),
    };

    client
        .update_pages_project(&account_id, &project_name, &update)
        .await
        .map_err(|e| {
            ApiError::internal(format!(
                "failed to connect git repo: {e}. \
             Ensure the GitHub/GitLab integration is authorized in your Cloudflare dashboard"
            ))
        })?;

    tracing::info!("connected git repo to Pages project {project_name}");
    Ok(())
}

/// Update the production branch of the webspace's direct-upload CF Pages
/// project (requires org write).
#[api_mcp_dioxus_server(server = "update_production_branch")]
pub async fn webspace_set_production_branch(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: WebspaceSetProductionBranchInput,
) -> Result<(), ApiError> {
    let (project_name, cred_id, org_id) = sqlx::query_as::<_, (Option<String>, Option<Uuid>, Uuid)>(
        "SELECT cloudflare_pages_project, cloudflare_credential_id, organization_id FROM webspaces WHERE id = $1",
    )
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?
    .ok_or_else(|| ApiError::not_found("webspace not found"))?;

    principal.require_write(&org_id)?;

    let project_name = project_name.ok_or_else(|| ApiError::bad_request("no Pages project"))?;
    let cred_id = cred_id.ok_or_else(|| ApiError::bad_request("no CF credential"))?;

    let (client, account_id) = cf_pages_client(pool, cred_id).await?;

    let update = cloudflare_api::compat::UpdatePagesProject {
        production_branch: Some(input.branch.clone()),
        source: None,
        build_config: None,
    };

    client
        .update_pages_project(&account_id, &project_name, &update)
        .await
        .map_err(|e| ApiError::internal(format!("failed to update production branch: {e}")))?;

    tracing::info!(
        "updated production branch for {project_name} to {}",
        input.branch
    );
    Ok(())
}

/// List the webspace's 20 most recent deployments (requires org read).
#[api_mcp_dioxus_server(server = "list_deployments")]
pub async fn webspace_deployments(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: WebspaceDeploymentsInput,
) -> Result<Vec<DeploymentRow>, ApiError> {
    let org_id = super::owning_org(pool, "webspaces", input.id, "webspace").await?;
    principal.require_read(&org_id)?;

    let rows = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            Option<String>,
            Option<i64>,
            chrono::DateTime<chrono::Utc>,
        ),
    >(
        "SELECT id, status, error_message, tarball_size, created_at \
         FROM deployments WHERE webspace_id = $1 ORDER BY created_at DESC LIMIT 20",
    )
    .bind(input.id)
    .fetch_all(pool)
    .await
    .map_err(super::internal)?;

    Ok(rows
        .into_iter()
        .map(
            |(id, status, error_message, tarball_size, created_at)| DeploymentRow {
                id,
                status,
                error_message,
                tarball_size,
                created_at: created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            },
        )
        .collect())
}

/// Discover CF Pages projects on a credential's account and flag those already
/// imported into the organization (requires org read).
#[api_mcp_dioxus_server(server = "discover_pages_projects")]
pub async fn webspace_discover_pages(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: WebspaceDiscoverPagesInput,
) -> Result<Vec<DiscoveredProject>, ApiError> {
    principal.require_read(&input.org_id)?;

    let (client, account_id) = cf_pages_client(pool, input.credential_id).await?;

    let projects = client
        .list_pages_projects(&account_id)
        .await
        .map_err(|e| ApiError::internal(format!("CF Pages API: {e}")))?;

    // Check which project IDs are already imported
    let existing_ids: Vec<String> = sqlx::query_scalar(
        "SELECT cloudflare_pages_project_id FROM webspaces WHERE organization_id = $1 AND cloudflare_pages_project_id IS NOT NULL",
    )
    .bind(input.org_id)
    .fetch_all(pool)
    .await
    .map_err(super::internal)?;

    let existing_names: Vec<String> = sqlx::query_scalar(
        "SELECT cloudflare_pages_project FROM webspaces WHERE organization_id = $1 AND cloudflare_pages_project IS NOT NULL",
    )
    .bind(input.org_id)
    .fetch_all(pool)
    .await
    .map_err(super::internal)?;

    Ok(projects
        .into_iter()
        .map(|p| {
            let already = p.id.as_ref().map_or(false, |id| existing_ids.contains(id))
                || existing_names.contains(&p.name);
            DiscoveredProject {
                name: p.name,
                id: p.id,
                subdomain: p.subdomain,
                already_imported: already,
            }
        })
        .collect())
}

/// Import CF Pages projects as cloudflare hosts (each with one main folder)
/// into an organization (requires org write). Collects per-project errors and
/// notifies the proxy when anything was imported.
#[api_mcp_dioxus_server(server = "import_pages_projects")]
pub async fn webspace_import_pages(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: WebspaceImportPagesInput,
) -> Result<ImportResult, ApiError> {
    principal.require_write(&input.org_id)?;

    let (client, account_id) = cf_pages_client(pool, input.credential_id).await?;

    let mut imported = 0u32;
    let mut skipped = 0u32;
    let mut errors = Vec::new();

    for name in &input.project_names {
        // Skip if already exists
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM webspaces WHERE organization_id = $1 AND cloudflare_pages_project = $2)",
        )
        .bind(input.org_id)
        .bind(name)
        .fetch_one(pool)
        .await
        .map_err(super::internal)?;

        if exists {
            skipped += 1;
            continue;
        }

        // Fetch project to get ID
        match client.get_pages_project(&account_id, name).await {
            Ok(project) => {
                // Each imported project becomes a cloudflare host with one main-folder.
                let result = async {
                    let host_id = sqlx::query_scalar::<_, Uuid>(
                        "INSERT INTO webspace_hosts (organization_id, name, kind) VALUES ($1, $2, 'cloudflare') RETURNING id",
                    )
                    .bind(input.org_id)
                    .bind(name)
                    .fetch_one(pool)
                    .await?;
                    sqlx::query(
                        "INSERT INTO webspaces (organization_id, webspace_host_id, name, path_prefix, hosting_type, cloudflare_pages_project, cloudflare_pages_project_id, cloudflare_credential_id) \
                         VALUES ($1, $2, $3, '/', 'cloudflare_pages', $4, $5, $6)",
                    )
                    .bind(input.org_id)
                    .bind(host_id)
                    .bind(name)
                    .bind(name)
                    .bind(&project.id)
                    .bind(input.credential_id)
                    .execute(pool)
                    .await?;
                    Ok::<_, sqlx::Error>(())
                }
                .await;

                match result {
                    Ok(_) => imported += 1,
                    Err(e) => errors.push(format!("{name}: {e}")),
                }
            }
            Err(e) => errors.push(format!("{name}: {e}")),
        }
    }

    if imported > 0 {
        crate::api::internal::notify_proxy_reload();
    }
    Ok(ImportResult {
        imported,
        skipped,
        errors,
    })
}
