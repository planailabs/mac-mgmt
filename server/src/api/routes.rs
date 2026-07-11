use std::collections::HashMap;

use chrono::{DateTime, Utc};
use rocket::State;
use rocket::http::{ContentType, Header, Status};
use rocket::serde::json::Json;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use base64::Engine;
use sha2::{Digest, Sha256};

use super::auth::{AdminAuth, AuthenticatedToken, SettingAuth, SyncAuth};
use super::push::{self, PushChannels, PushMessage};
use crate::rollout_health::HealthGate;

// ── Common routes (any valid token) ────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub(crate) struct SelfInfo {
    cluster_id: Option<Uuid>,
    cluster_name: Option<String>,
    organization_id: Option<Uuid>,
    token_kind: String,
    /// All cluster IDs this token can access.
    cluster_ids: Vec<Uuid>,
    /// Proxy token scopes. Empty means wildcard (all scopes).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    scopes: Vec<String>,
}

#[utoipa::path(
    get,
    path = "/api/self",
    tag = "Common",
    summary = "Get current token identity",
    description = "Returns cluster info and token kind for the authenticated token.",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Token identity", body = SelfInfo),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[rocket::get("/self")]
pub async fn get_self(
    auth: AuthenticatedToken,
    pool: &State<PgPool>,
) -> Result<Json<SelfInfo>, Status> {
    let name = match auth.cluster_id {
        Some(cid) => {
            let n = sqlx::query_scalar::<_, String>("SELECT name FROM clusters WHERE id = $1")
                .bind(cid)
                .fetch_one(pool.inner())
                .await
                .map_err(|_| Status::InternalServerError)?;
            Some(n)
        }
        None => None,
    };

    // Resolve all cluster IDs this token can access
    let cluster_ids = if let Some(cid) = auth.cluster_id {
        // Single-cluster token
        vec![cid]
    } else if let Some(org_id) = auth.organization_id {
        // Org-scoped token: all clusters in the organization
        sqlx::query_scalar::<_, Uuid>(
            "SELECT cluster_id FROM organization_clusters WHERE organization_id = $1",
        )
        .bind(org_id)
        .fetch_all(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?
    } else {
        // Admin token: all clusters
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM clusters")
            .fetch_all(pool.inner())
            .await
            .map_err(|_| Status::InternalServerError)?
    };

    Ok(Json(SelfInfo {
        cluster_id: auth.cluster_id,
        cluster_name: name,
        organization_id: auth.organization_id,
        token_kind: auth.token_kind,
        cluster_ids,
        scopes: auth.scopes.unwrap_or_default(),
    }))
}

// ── Public server info ────────────────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub(crate) struct ServerInfo {
    /// External web UI URL (e.g. "https://plan.ai").
    web_url: String,
}

#[utoipa::path(
    get,
    path = "/api/server-info",
    tag = "Common",
    summary = "Get public server information",
    description = "Returns the server's external web URL. No authentication required.",
    responses(
        (status = 200, description = "Server info", body = ServerInfo),
    ),
)]
#[rocket::get("/server-info")]
pub async fn get_server_info() -> Json<ServerInfo> {
    let cfg = crate::config::config();
    let web_url = cfg
        .auth
        .as_ref()
        .map(|a| a.external_url.clone())
        .unwrap_or_else(|| cfg.api.external_url.clone());
    Json(ServerInfo { web_url })
}

// ── Existing sync routes ───────────────────────────────────────────────

/// Aggregate skills from remote skill centers for a cluster.
/// Queries cluster_skills and cluster_bundles where skill_center_id IS NOT NULL,
/// groups by skill center, resolves via each skill center's federation API, and merges by priority.
async fn aggregate_remote_skills(
    cluster_id: Uuid,
    pool: &PgPool,
    cache: &crate::skill_center_cache::SkillCenterCache,
    arch: &str,
) -> HashMap<String, String> {
    #[derive(sqlx::FromRow)]
    struct RemoteRef {
        skill_center_id: Option<Uuid>,
        slug: Option<String>,
        channel: Option<String>,
    }

    let direct: Vec<RemoteRef> = sqlx::query_as(
        "SELECT skill_center_id, slug, channel FROM cluster_skills \
         WHERE cluster_id = $1 AND skill_center_id IS NOT NULL",
    )
    .bind(cluster_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    #[derive(sqlx::FromRow)]
    struct RemoteBundleRef {
        skill_center_id: Option<Uuid>,
        remote_id: Option<Uuid>,
    }

    let bundles: Vec<RemoteBundleRef> = sqlx::query_as(
        "SELECT skill_center_id, remote_id FROM cluster_bundles \
         WHERE cluster_id = $1 AND skill_center_id IS NOT NULL",
    )
    .bind(cluster_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut by_sc: HashMap<Uuid, Vec<(String, String)>> = HashMap::new();

    for r in direct {
        if let (Some(sc_id), Some(slug), Some(channel)) = (r.skill_center_id, r.slug, r.channel) {
            by_sc.entry(sc_id).or_default().push((slug, channel));
        }
    }

    let all_caches = cache.get_all().await;
    for b in bundles {
        if let (Some(sc_id), Some(rid)) = (b.skill_center_id, b.remote_id) {
            if let Some(cached) = all_caches.get(&sc_id) {
                if let Some(bundle) = cached.catalog.bundles.iter().find(|fb| fb.id == rid) {
                    for skill in &bundle.skills {
                        by_sc
                            .entry(sc_id)
                            .or_default()
                            .push((skill.skill_slug.clone(), skill.channel.clone()));
                    }
                }
            }
        }
    }

    if by_sc.is_empty() {
        return HashMap::new();
    }

    #[derive(sqlx::FromRow)]
    struct ScInfo {
        id: Uuid,
        url: String,
        federation_token: String,
        priority: i32,
    }

    let centers: Vec<ScInfo> = sqlx::query_as(
        "SELECT id, url, federation_token, priority FROM skill_centers WHERE enabled = true",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let sc_map: HashMap<Uuid, &ScInfo> = centers.iter().map(|s| (s.id, s)).collect();

    let mut ordered: Vec<_> = by_sc.iter().collect();
    ordered.sort_by_key(|(sc_id, _)| sc_map.get(sc_id).map(|s| s.priority).unwrap_or(0));

    let mut result: HashMap<String, String> = HashMap::new();

    for (sc_id, slug_channels) in ordered {
        if crate::builtin_skill_center::is_builtin(sc_id) {
            let resolved = crate::builtin_skill_center::resolve_builtin_skills(slug_channels);
            for (slug, path) in resolved {
                result.insert(slug, path);
            }
            continue;
        }
        if let Some(sc) = sc_map.get(sc_id) {
            let client = crate::skill_center_client::SkillCenterClient::new(
                sc.url.clone(),
                sc.federation_token.clone(),
            );
            match client.resolve_skills(slug_channels, arch).await {
                Ok(resolved) => {
                    for (slug, path) in resolved {
                        result.insert(slug, path);
                    }
                }
                Err(e) => {
                    tracing::error!("skill center resolve failed for {}: {e}", sc.url);
                }
            }
        }
    }

    result
}

/// Aggregate MCP servers from remote skill centers for a cluster.
/// Queries cluster_mcp_servers and cluster_mcp_bundles where skill_center_id IS NOT NULL.
async fn aggregate_remote_mcp_servers(
    cluster_id: Uuid,
    pool: &PgPool,
    cache: &crate::skill_center_cache::SkillCenterCache,
) -> HashMap<String, McpServerEntry> {
    #[derive(sqlx::FromRow)]
    struct RemoteMcpRef {
        skill_center_id: Option<Uuid>,
        slug: Option<String>,
    }

    let direct: Vec<RemoteMcpRef> = sqlx::query_as(
        "SELECT skill_center_id, slug FROM cluster_mcp_servers \
         WHERE cluster_id = $1 AND skill_center_id IS NOT NULL",
    )
    .bind(cluster_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    #[derive(sqlx::FromRow)]
    struct RemoteMcpBundleRef {
        skill_center_id: Option<Uuid>,
        remote_id: Option<Uuid>,
    }

    let bundles: Vec<RemoteMcpBundleRef> = sqlx::query_as(
        "SELECT skill_center_id, remote_id FROM cluster_mcp_bundles \
         WHERE cluster_id = $1 AND skill_center_id IS NOT NULL",
    )
    .bind(cluster_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut by_sc: HashMap<Uuid, Vec<String>> = HashMap::new();

    for r in direct {
        if let (Some(sc_id), Some(slug)) = (r.skill_center_id, r.slug) {
            by_sc.entry(sc_id).or_default().push(slug);
        }
    }

    let all_caches = cache.get_all().await;
    for b in bundles {
        if let (Some(sc_id), Some(rid)) = (b.skill_center_id, b.remote_id) {
            if let Some(cached) = all_caches.get(&sc_id) {
                if let Some(bundle) = cached.catalog.mcp_bundles.iter().find(|fb| fb.id == rid) {
                    for server in &bundle.servers {
                        by_sc.entry(sc_id).or_default().push(server.slug.clone());
                    }
                }
            }
        }
    }

    if by_sc.is_empty() {
        return HashMap::new();
    }

    #[derive(sqlx::FromRow)]
    struct ScInfo {
        id: Uuid,
        url: String,
        federation_token: String,
        priority: i32,
    }

    let centers: Vec<ScInfo> = sqlx::query_as(
        "SELECT id, url, federation_token, priority FROM skill_centers WHERE enabled = true",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let sc_map: HashMap<Uuid, &ScInfo> = centers.iter().map(|s| (s.id, s)).collect();

    let mut ordered: Vec<_> = by_sc.iter().collect();
    ordered.sort_by_key(|(sc_id, _)| sc_map.get(sc_id).map(|s| s.priority).unwrap_or(0));

    let mut result: HashMap<String, McpServerEntry> = HashMap::new();

    for (sc_id, slugs) in ordered {
        if crate::builtin_skill_center::is_builtin(sc_id) {
            let resolved = crate::builtin_skill_center::resolve_builtin_mcp_servers(slugs);
            for (slug, entry) in resolved {
                result.insert(slug, entry);
            }
            continue;
        }
        if let Some(sc) = sc_map.get(sc_id) {
            let client = crate::skill_center_client::SkillCenterClient::new(
                sc.url.clone(),
                sc.federation_token.clone(),
            );
            match client.resolve_mcp_servers(slugs).await {
                Ok(resolved) => {
                    for (slug, entry) in resolved {
                        result.insert(slug, entry);
                    }
                }
                Err(e) => {
                    tracing::error!("skill center MCP resolve failed for {}: {e}", sc.url);
                }
            }
        }
    }

    result
}

/// Resolve transitive MCP server dependencies for remote/builtin skill channels.
///
/// Looks up `mcp_server_slugs` on `FederationSkillChannel` entries in the cached
/// catalogs for each skill center that has skill assignments for this cluster.
async fn resolve_remote_skill_mcp_deps(
    cluster_id: Uuid,
    pool: &PgPool,
    cache: &crate::skill_center_cache::SkillCenterCache,
) -> HashMap<String, McpServerEntry> {
    // Collect winning remote skill slugs per skill center (same logic as aggregate_remote_skills).
    #[derive(sqlx::FromRow)]
    struct RemoteRef {
        skill_center_id: Option<Uuid>,
        slug: Option<String>,
        channel: Option<String>,
    }

    let direct: Vec<RemoteRef> = sqlx::query_as(
        "SELECT skill_center_id, slug, channel FROM cluster_skills \
         WHERE cluster_id = $1 AND skill_center_id IS NOT NULL",
    )
    .bind(cluster_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    #[derive(sqlx::FromRow)]
    struct RemoteBundleRef {
        skill_center_id: Option<Uuid>,
        remote_id: Option<Uuid>,
    }

    let bundles: Vec<RemoteBundleRef> = sqlx::query_as(
        "SELECT skill_center_id, remote_id FROM cluster_bundles \
         WHERE cluster_id = $1 AND skill_center_id IS NOT NULL",
    )
    .bind(cluster_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    // Group (slug, channel) by skill_center_id.
    let mut by_sc: HashMap<Uuid, Vec<(String, String)>> = HashMap::new();
    for r in direct {
        if let (Some(sc_id), Some(slug), Some(channel)) = (r.skill_center_id, r.slug, r.channel) {
            by_sc.entry(sc_id).or_default().push((slug, channel));
        }
    }

    let all_caches = cache.get_all().await;
    for b in bundles {
        if let (Some(sc_id), Some(rid)) = (b.skill_center_id, b.remote_id) {
            if let Some(cached) = all_caches.get(&sc_id) {
                if let Some(bundle) = cached.catalog.bundles.iter().find(|fb| fb.id == rid) {
                    for skill in &bundle.skills {
                        by_sc
                            .entry(sc_id)
                            .or_default()
                            .push((skill.skill_slug.clone(), skill.channel.clone()));
                    }
                }
            }
        }
    }

    if by_sc.is_empty() {
        return HashMap::new();
    }

    // For each skill center, look up mcp_server_slugs from cached catalogs.
    let mut mcp_slugs_by_sc: HashMap<Uuid, Vec<String>> = HashMap::new();
    for (sc_id, slug_channels) in &by_sc {
        if let Some(cached) = all_caches.get(sc_id) {
            for (slug, channel) in slug_channels {
                if let Some(sc) = cached
                    .catalog
                    .skill_channels
                    .iter()
                    .find(|sc| sc.skill_slug == *slug && sc.channel == *channel)
                {
                    for mcp_slug in &sc.mcp_server_slugs {
                        mcp_slugs_by_sc
                            .entry(*sc_id)
                            .or_default()
                            .push(mcp_slug.clone());
                    }
                }
            }
        }
    }

    // Resolve MCP server configs from each skill center.
    let mut result: HashMap<String, McpServerEntry> = HashMap::new();
    for (sc_id, mcp_slugs) in &mcp_slugs_by_sc {
        if mcp_slugs.is_empty() {
            continue;
        }
        if crate::builtin_skill_center::is_builtin(sc_id) {
            let resolved = crate::builtin_skill_center::resolve_builtin_mcp_servers(mcp_slugs);
            for (slug, entry) in resolved {
                result.insert(slug, entry);
            }
            continue;
        }
        // For remote skill centers, resolve via HTTP.
        #[derive(sqlx::FromRow)]
        struct ScInfo {
            url: String,
            federation_token: String,
        }
        if let Ok(sc) = sqlx::query_as::<_, ScInfo>(
            "SELECT url, federation_token FROM skill_centers WHERE id = $1 AND enabled = true",
        )
        .bind(sc_id)
        .fetch_one(pool)
        .await
        {
            let client = crate::skill_center_client::SkillCenterClient::new(
                sc.url.clone(),
                sc.federation_token,
            );
            match client.resolve_mcp_servers(mcp_slugs).await {
                Ok(resolved) => {
                    for (slug, entry) in resolved {
                        result.insert(slug, entry);
                    }
                }
                Err(e) => {
                    tracing::error!("remote skill MCP dep resolve failed for {}: {e}", sc.url,);
                }
            }
        }
    }

    result
}

#[derive(sqlx::FromRow)]
struct McpServerRow {
    slug: String,
    config_json: serde_json::Value,
    nix_packages: Vec<String>,
    is_direct: bool,
}

pub(crate) use mac_mgmt_common::McpServerEntry;

#[utoipa::path(
    get,
    path = "/api/mcp-servers",
    tag = "Sync",
    summary = "List MCP servers for daemon sync",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Map of slug to MCP server entry"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Sync token required"),
    ),
)]
#[rocket::get("/mcp-servers")]
pub async fn get_mcp_servers(
    auth: SyncAuth,
    pool: &State<PgPool>,
    cache: &State<crate::skill_center_cache::SkillCenterCache>,
) -> Result<Json<HashMap<String, McpServerEntry>>, Status> {
    // Precedence: direct (2) > bundle (1) > transitive from skill (0).
    let rows = sqlx::query_as::<_, McpServerRow>(
        "SELECT ms.slug, ms.config_json, ms.nix_packages, true AS is_direct \
         FROM cluster_mcp_servers cms \
         JOIN mcp_servers ms ON ms.id = cms.mcp_server_id \
         WHERE cms.cluster_id = $1 \
         UNION ALL \
         SELECT ms.slug, ms.config_json, ms.nix_packages, false AS is_direct \
         FROM cluster_mcp_bundles cmb \
         JOIN mcp_server_bundle_items msbi ON msbi.bundle_id = cmb.bundle_id \
         JOIN mcp_servers ms ON ms.id = msbi.mcp_server_id \
         WHERE cmb.cluster_id = $1",
    )
    .bind(auth.cluster_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    // For each slug, direct assignment wins over bundle.
    // Track precedence: 2 = direct, 1 = bundle.
    let mut result: HashMap<String, (McpServerEntry, u8)> = HashMap::new();
    for row in &rows {
        let prec: u8 = if row.is_direct { 2 } else { 1 };
        match result.get(&row.slug) {
            Some((_, existing)) if *existing >= prec => {}
            _ => {
                result.insert(
                    row.slug.clone(),
                    (
                        McpServerEntry {
                            config: row.config_json.clone(),
                            nix_packages: row.nix_packages.clone(),
                        },
                        prec,
                    ),
                );
            }
        }
    }

    // Resolve transitive MCP deps from winning skill channels.
    let winning_channels = resolve_winning_skill_channels(auth.cluster_id, pool.inner()).await?;
    let channel_ids: Vec<Uuid> = winning_channels.into_values().collect();

    if !channel_ids.is_empty() {
        let transitive_rows = sqlx::query_as::<_, McpServerRow>(
            "SELECT ms.slug, ms.config_json, ms.nix_packages, false AS is_direct \
             FROM skill_mcp_dependencies smd \
             JOIN mcp_servers ms ON ms.id = smd.mcp_server_id \
             WHERE smd.skill_channel_id = ANY($1)",
        )
        .bind(&channel_ids)
        .fetch_all(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;

        for row in &transitive_rows {
            // Transitive has lowest precedence (0).
            if !result.contains_key(&row.slug) {
                result.insert(
                    row.slug.clone(),
                    (
                        McpServerEntry {
                            config: row.config_json.clone(),
                            nix_packages: row.nix_packages.clone(),
                        },
                        0,
                    ),
                );
            }
        }
    }

    // Resolve transitive MCP deps from remote/builtin skill channels via
    // cached federation catalogs (mcp_server_slugs on FederationSkillChannel).
    let remote_transitive =
        resolve_remote_skill_mcp_deps(auth.cluster_id, pool.inner(), cache.inner()).await;
    for (slug, entry) in remote_transitive {
        // Transitive has lowest precedence (0) — don't override existing.
        if !result.contains_key(&slug) {
            result.insert(slug, (entry, 0));
        }
    }

    let local_result: HashMap<String, McpServerEntry> = result
        .into_iter()
        .map(|(slug, (entry, _))| (slug, entry))
        .collect();

    // Aggregate MCP servers from remote skill centers
    let mut merged =
        aggregate_remote_mcp_servers(auth.cluster_id, pool.inner(), cache.inner()).await;
    // Local overlays remote (local wins on slug collision)
    for (slug, entry) in local_result {
        merged.insert(slug, entry);
    }

    Ok(Json(merged))
}

// ── Unified packages endpoint ─────────────────────────────────────────

use mac_mgmt_common::{PackageSource, PackageSyncResponse};

#[derive(sqlx::FromRow)]
struct PkgRow {
    slug: String,
    nix_packages: Vec<String>,
}

#[utoipa::path(
    get,
    path = "/api/packages",
    tag = "Sync",
    summary = "Unified package list for daemon sync (MCP + skill + manual)",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Package-to-sources map"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Sync token required"),
    ),
)]
#[rocket::get("/packages")]
pub async fn get_packages(
    auth: SyncAuth,
    pool: &State<PgPool>,
    cache: &State<crate::skill_center_cache::SkillCenterCache>,
) -> Result<Json<PackageSyncResponse>, Status> {
    let mut packages: HashMap<String, Vec<PackageSource>> = HashMap::new();

    // ── 1. MCP server packages (direct + bundle + transitive from skills) ──
    let mcp_rows = sqlx::query_as::<_, PkgRow>(
        "SELECT ms.slug, ms.nix_packages \
         FROM cluster_mcp_servers cms \
         JOIN mcp_servers ms ON ms.id = cms.mcp_server_id \
         WHERE cms.cluster_id = $1 AND ms.nix_packages != '{}' \
         UNION ALL \
         SELECT ms.slug, ms.nix_packages \
         FROM cluster_mcp_bundles cmb \
         JOIN mcp_server_bundle_items msbi ON msbi.bundle_id = cmb.bundle_id \
         JOIN mcp_servers ms ON ms.id = msbi.mcp_server_id \
         WHERE cmb.cluster_id = $1 AND ms.nix_packages != '{}'",
    )
    .bind(auth.cluster_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    for row in &mcp_rows {
        for pkg in &row.nix_packages {
            packages
                .entry(pkg.clone())
                .or_default()
                .push(PackageSource::McpServer {
                    slug: row.slug.clone(),
                });
        }
    }

    // Transitive MCP deps from winning skill channels
    let winning = resolve_winning_skill_channels(auth.cluster_id, pool.inner()).await?;
    let channel_ids: Vec<Uuid> = winning.values().copied().collect();

    if !channel_ids.is_empty() {
        let trans_mcp = sqlx::query_as::<_, PkgRow>(
            "SELECT ms.slug, ms.nix_packages \
             FROM skill_mcp_dependencies smd \
             JOIN mcp_servers ms ON ms.id = smd.mcp_server_id \
             WHERE smd.skill_channel_id = ANY($1) AND ms.nix_packages != '{}'",
        )
        .bind(&channel_ids)
        .fetch_all(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;

        for row in &trans_mcp {
            for pkg in &row.nix_packages {
                packages
                    .entry(pkg.clone())
                    .or_default()
                    .push(PackageSource::McpServer {
                        slug: row.slug.clone(),
                    });
            }
        }
    }

    // Remote MCP server packages (from federation)
    let remote_mcp =
        aggregate_remote_mcp_servers(auth.cluster_id, pool.inner(), cache.inner()).await;
    for (slug, entry) in &remote_mcp {
        for pkg in &entry.nix_packages {
            packages
                .entry(pkg.clone())
                .or_default()
                .push(PackageSource::McpServer { slug: slug.clone() });
        }
    }

    // ── 2. Skill channel packages ──
    if !channel_ids.is_empty() {
        let skill_rows = sqlx::query_as::<_, PkgRow>(
            "SELECT s.slug, sc.nix_packages \
             FROM skill_channels sc \
             JOIN skills s ON s.id = sc.skill_id \
             WHERE sc.id = ANY($1) AND sc.nix_packages != '{}'",
        )
        .bind(&channel_ids)
        .fetch_all(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;

        for row in &skill_rows {
            for pkg in &row.nix_packages {
                packages
                    .entry(pkg.clone())
                    .or_default()
                    .push(PackageSource::Skill {
                        slug: row.slug.clone(),
                    });
            }
        }
    }

    // Remote skill channel packages (from federation cache)
    {
        let all_caches = cache.get_all().await;
        // Collect remote skill slugs+channels assigned to this cluster
        #[derive(sqlx::FromRow)]
        struct RemoteSkillRef {
            skill_center_id: Option<Uuid>,
            slug: Option<String>,
        }
        let direct_remote: Vec<RemoteSkillRef> = sqlx::query_as(
            "SELECT skill_center_id, slug FROM cluster_skills \
             WHERE cluster_id = $1 AND skill_center_id IS NOT NULL",
        )
        .bind(auth.cluster_id)
        .fetch_all(pool.inner())
        .await
        .unwrap_or_default();

        for r in &direct_remote {
            if let (Some(sc_id), Some(slug)) = (r.skill_center_id, &r.slug) {
                if let Some(cached) = all_caches.get(&sc_id) {
                    for sc in &cached.catalog.skill_channels {
                        if sc.skill_slug == *slug {
                            for pkg in &sc.nix_packages {
                                packages
                                    .entry(pkg.clone())
                                    .or_default()
                                    .push(PackageSource::Skill { slug: slug.clone() });
                            }
                        }
                    }
                }
            }
        }

        // Remote bundle skill packages
        #[derive(sqlx::FromRow)]
        struct RemoteBundleRef {
            skill_center_id: Option<Uuid>,
            remote_id: Option<Uuid>,
        }
        let bundle_remote: Vec<RemoteBundleRef> = sqlx::query_as(
            "SELECT skill_center_id, remote_id FROM cluster_bundles \
             WHERE cluster_id = $1 AND skill_center_id IS NOT NULL",
        )
        .bind(auth.cluster_id)
        .fetch_all(pool.inner())
        .await
        .unwrap_or_default();

        for b in &bundle_remote {
            if let (Some(sc_id), Some(rid)) = (b.skill_center_id, b.remote_id) {
                if let Some(cached) = all_caches.get(&sc_id) {
                    if let Some(bundle) = cached.catalog.bundles.iter().find(|fb| fb.id == rid) {
                        for skill in &bundle.skills {
                            for sc in &cached.catalog.skill_channels {
                                if sc.skill_slug == skill.skill_slug && sc.channel == skill.channel
                                {
                                    for pkg in &sc.nix_packages {
                                        packages.entry(pkg.clone()).or_default().push(
                                            PackageSource::Skill {
                                                slug: skill.skill_slug.clone(),
                                            },
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // ── 3. Manual cluster packages ──
    let manual: Vec<String> =
        sqlx::query_scalar("SELECT package FROM cluster_packages WHERE cluster_id = $1")
            .bind(auth.cluster_id)
            .fetch_all(pool.inner())
            .await
            .map_err(|_| Status::InternalServerError)?;

    for pkg in manual {
        packages.entry(pkg).or_default().push(PackageSource::Manual);
    }

    // Deduplicate sources per package
    for sources in packages.values_mut() {
        sources.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
        sources.dedup();
    }

    Ok(Json(PackageSyncResponse { packages }))
}

#[utoipa::path(
    get,
    path = "/api/config",
    tag = "Sync",
    summary = "Get cluster config JSON for daemon sync",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Config JSON"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Sync token required"),
        (status = 404, description = "No config saved"),
    ),
)]
#[rocket::get("/config")]
pub async fn get_config(
    auth: SyncAuth,
    pool: &State<PgPool>,
) -> Result<Json<serde_json::Value>, Status> {
    let config = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT config_json FROM cluster_configs \
         WHERE cluster_id = $1 \
         ORDER BY created_at DESC \
         LIMIT 1",
    )
    .bind(auth.cluster_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    match config {
        Some(mut json) => {
            mac_mgmt_common::config_migrate::migrate(&mut json);

            // Inject the cluster's p2p PSK into relay.cluster_psk.
            // Generate one if it doesn't exist yet.
            if let Ok(psk_hex) = ensure_cluster_psk(pool.inner(), auth.cluster_id).await {
                let relay = json.as_object_mut().and_then(|o| {
                    o.entry("relay")
                        .or_insert_with(|| serde_json::json!({}))
                        .as_object_mut()
                });
                if let Some(relay) = relay {
                    relay.insert(
                        "cluster_psk".to_string(),
                        serde_json::Value::String(psk_hex),
                    );
                }
            }

            Ok(Json(json))
        }
        None => Err(Status::NotFound),
    }
}

/// Fetch or generate the cluster's p2p pre-shared key (32 bytes, hex-encoded).
async fn ensure_cluster_psk(pool: &PgPool, cluster_id: uuid::Uuid) -> Result<String, sqlx::Error> {
    // Try to read existing PSK.
    let existing: Option<Vec<u8>> =
        sqlx::query_scalar("SELECT p2p_psk FROM clusters WHERE id = $1")
            .bind(cluster_id)
            .fetch_optional(pool)
            .await?;

    if let Some(Some(psk)) = existing.map(Some) {
        if !psk.is_empty() {
            return Ok(hex::encode(&psk));
        }
    }

    // Generate and store a new 32-byte PSK.
    let psk: [u8; 32] = rand::random();
    sqlx::query("UPDATE clusters SET p2p_psk = $1 WHERE id = $2")
        .bind(&psk[..])
        .bind(cluster_id)
        .execute(pool)
        .await?;

    Ok(hex::encode(psk))
}

// ── Update target (for daemon self-update) ──────────────────────────

pub(crate) use mac_mgmt_common::{NixpkgsPin, UpdateTarget};

/// A rollout whose target currently applies to a cluster.
#[derive(sqlx::FromRow)]
pub(crate) struct ActiveRolloutTarget {
    pub id: Uuid,
    pub target_version: Option<String>,
    pub nixpkgs_commit: Option<String>,
}

/// Resolve the rollout that applies to `cluster_id`, shared by /api/update,
/// /api/nixpkgs and cloud-init. A cluster is targeted while its stage is
/// actively rolling; once the target has been delivered (recorded in
/// rollout_deliveries) the cluster keeps resolving to that rollout for the
/// rollout's whole lifetime — rolling or paused/gated — so a pause never
/// downgrades a cluster that already updated. Rolled-back and completed
/// rollouts stop applying (rollback reverts on purpose; completion persists
/// the pin onto the clusters).
///
/// A stage with ramp_minutes releases gradually: each cluster gets a stable
/// bucket in [0,100) from hashing (rollout id, cluster id), and is eligible
/// once bucket < 100 * elapsed-since-stage-start / ramp_minutes. Daemons
/// poll /api/update periodically, so eligibility growing over time is
/// picked up without extra pushes.
pub(crate) async fn active_rollout_for_cluster(
    pool: &PgPool,
    cluster_id: Uuid,
) -> Result<Option<ActiveRolloutTarget>, sqlx::Error> {
    sqlx::query_as(
        "SELECT r.id, r.target_version, r.nixpkgs_commit FROM rollouts r \
         WHERE (r.status = 'rolling' AND EXISTS (\
                 SELECT 1 FROM rollout_stages rs \
                 WHERE rs.rollout_id = r.id AND rs.status = 'rolling' \
                   AND (rs.group_id = '00000000-0000-0000-0000-000000000000'::uuid \
                        OR rs.group_id IN (SELECT group_id FROM rollout_group_members WHERE cluster_id = $1)) \
                   AND (rs.ramp_minutes IS NULL OR rs.started_at IS NULL \
                        OR mod(mod(hashtextextended(r.id::text || $1::text, 0), 100) + 100, 100) \
                           < 100.0 * EXTRACT(EPOCH FROM (now() - rs.started_at)) / (rs.ramp_minutes * 60)))) \
            OR (r.status IN ('rolling', 'paused') AND EXISTS (\
                 SELECT 1 FROM rollout_deliveries rd \
                 WHERE rd.rollout_id = r.id AND rd.cluster_id = $1)) \
         ORDER BY r.created_at DESC LIMIT 1",
    )
    .bind(cluster_id)
    .fetch_optional(pool)
    .await
}

/// Record that `cluster_id` fetched `rollout_id`'s target. Idempotent;
/// best-effort — a failed insert must not fail the sync request itself.
async fn record_rollout_delivery(pool: &PgPool, rollout_id: Uuid, cluster_id: Uuid) {
    if let Err(e) = sqlx::query(
        "INSERT INTO rollout_deliveries (rollout_id, cluster_id) VALUES ($1, $2) \
         ON CONFLICT (rollout_id, cluster_id) DO NOTHING",
    )
    .bind(rollout_id)
    .bind(cluster_id)
    .execute(pool)
    .await
    {
        tracing::warn!("failed to record rollout delivery: {e}");
    }
}

#[utoipa::path(
    get,
    path = "/api/update",
    tag = "Sync",
    summary = "Get the target version for this daemon",
    description = "Returns the version the daemon should update to. If an active rollout targets this cluster (or already delivered to it — delivered targets stick for the rollout's lifetime, even while paused), returns the rollout's version; otherwise returns the cluster's pinned version. Null means stay on current version.",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Update target"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Sync token required"),
    ),
)]
#[rocket::get("/update?<system>")]
pub async fn get_update_target(
    auth: SyncAuth,
    pool: &State<PgPool>,
    system: Option<String>,
) -> Result<Json<UpdateTarget>, Status> {
    let rollout = active_rollout_for_cluster(pool.inner(), auth.cluster_id)
        .await
        .map_err(|_| Status::InternalServerError)?;
    if let Some(r) = &rollout {
        record_rollout_delivery(pool.inner(), r.id, auth.cluster_id).await;
    }

    // A rollout may carry only nixpkgs_commit and leave target_version unset —
    // fall through to the pinned/latest resolution in that case.
    let chosen: Option<String> = if let Some(ver) = rollout.and_then(|r| r.target_version) {
        Some(ver)
    } else {
        let pinned = sqlx::query_scalar::<_, Option<String>>(
            "SELECT pinned_version FROM clusters WHERE id = $1",
        )
        .bind(auth.cluster_id)
        .fetch_one(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;

        match pinned {
            Some(ver) => Some(ver),
            // No pinned version: fall back to the latest semver from
            // daemon_versions. Non-semver channel versions ("rolling")
            // are excluded — they are opt-in via pin or rollout only,
            // and the int[] cast would error on them.
            None => sqlx::query_scalar::<_, String>(
                "SELECT version FROM daemon_versions \
                 WHERE version ~ '^[0-9]+(\\.[0-9]+)*$' \
                 ORDER BY string_to_array(version, '.')::int[] DESC LIMIT 1",
            )
            .fetch_optional(pool.inner())
            .await
            .map_err(|_| Status::InternalServerError)?,
        }
    };

    // Resolve nix store path live from xzar (mirrors the skills flow):
    // look up the `daemon/{version}/{system}` pin for the daemon's system.
    let store_path = match (&chosen, system.as_deref()) {
        (Some(ver), Some(sys)) => {
            let cfg = crate::config::config();
            match cfg.xzar.as_ref() {
                Some(xzar) => {
                    let pins = crate::xzar::fetch_pins(&xzar.url, &xzar.token)
                        .await
                        .map_err(|e| {
                            tracing::error!("xzar fetch failed: {e}");
                            Status::InternalServerError
                        })?;
                    crate::xzar::store_path_for_pin(&pins, &format!("daemon/{ver}/{sys}"))
                }
                None => None,
            }
        }
        _ => None,
    };

    Ok(Json(UpdateTarget {
        target_version: chosen,
        store_path,
    }))
}

/// Cached rolling nixpkgs commit resolved from the GitLab CI API.
/// Tuple: (commit SHA, resolved at).
static ROLLING_NIXPKGS: std::sync::OnceLock<
    tokio::sync::RwLock<Option<(String, std::time::Instant)>>,
> = std::sync::OnceLock::new();

/// Resolve the latest successful CI pipeline commit for the configured
/// nixpkgs branch. Caches the result for 5 minutes.
async fn resolve_rolling_nixpkgs_commit() -> Option<String> {
    let cache = ROLLING_NIXPKGS.get_or_init(|| tokio::sync::RwLock::new(None));
    let ttl = std::time::Duration::from_secs(300);

    // Check cache under read lock.
    {
        let guard = cache.read().await;
        if let Some((sha, at)) = guard.as_ref() {
            if at.elapsed() < ttl {
                return Some(sha.clone());
            }
        }
    }

    // Cache miss or expired — resolve from GitLab API.
    let cfg = &crate::config::config().git;
    let encoded_project = cfg.nixpkgs_project.replace('/', "%2F");
    let url = format!(
        "{}/api/v4/projects/{}/pipelines?ref={}&status=success&per_page=1",
        cfg.gitlab_url.trim_end_matches('/'),
        encoded_project,
        cfg.nixpkgs_branch,
    );

    let client = reqwest::Client::new();
    let resp = match client.get(&url).send().await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            tracing::warn!("GitLab pipeline API returned {}: {url}", r.status());
            return cache.read().await.as_ref().map(|(s, _)| s.clone());
        }
        Err(e) => {
            tracing::warn!("GitLab pipeline API failed: {e}");
            return cache.read().await.as_ref().map(|(s, _)| s.clone());
        }
    };

    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("GitLab pipeline API parse error: {e}");
            return cache.read().await.as_ref().map(|(s, _)| s.clone());
        }
    };

    let sha = body
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|p| p.get("sha"))
        .and_then(|v| v.as_str())
        .map(String::from);

    if let Some(ref s) = sha {
        let mut guard = cache.write().await;
        *guard = Some((s.clone(), std::time::Instant::now()));
    }

    sha
}

#[utoipa::path(
    get,
    path = "/api/nixpkgs",
    tag = "Sync",
    summary = "Get the target nixpkgs commit for this daemon",
    description = "Returns the commit the daemon should pin nixpkgs to. If an active rollout targeting this cluster (or already delivered to it — delivered targets stick for the rollout's lifetime, even while paused) carries a nixpkgs_commit, that wins; otherwise returns the cluster's persistent pin. If neither is set, resolves the latest successful CI pipeline on the default branch.",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Nixpkgs pin"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Sync token required"),
    ),
)]
#[rocket::get("/nixpkgs")]
pub async fn get_nixpkgs_pin(
    auth: SyncAuth,
    pool: &State<PgPool>,
) -> Result<Json<NixpkgsPin>, Status> {
    // Active rollout for this cluster (mirrors get_update_target).
    let rollout = active_rollout_for_cluster(pool.inner(), auth.cluster_id)
        .await
        .map_err(|_| Status::InternalServerError)?;
    if let Some(r) = &rollout {
        record_rollout_delivery(pool.inner(), r.id, auth.cluster_id).await;
    }

    if let Some(commit) = rollout.and_then(|r| r.nixpkgs_commit) {
        return Ok(Json(NixpkgsPin {
            commit: Some(commit),
        }));
    }

    // Fall back to cluster's persistent pin.
    let pinned = sqlx::query_scalar::<_, Option<String>>(
        "SELECT nixpkgs_commit FROM clusters WHERE id = $1",
    )
    .bind(auth.cluster_id)
    .fetch_one(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    if pinned.is_some() {
        return Ok(Json(NixpkgsPin { commit: pinned }));
    }

    // No explicit pin — resolve rolling commit from latest successful CI pipeline.
    let rolling = resolve_rolling_nixpkgs_commit().await;
    Ok(Json(NixpkgsPin { commit: rolling }))
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct NixCachesResponse {
    caches: Vec<NixCacheEntry>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
struct NixCacheEntry {
    url: String,
    public_key: String,
}

#[utoipa::path(
    get,
    path = "/api/nix-caches",
    tag = "Sync",
    summary = "Get nix binary cache URLs and their signing public keys",
    description = "Returns substituter URLs and trusted public keys the daemon should use for nix operations. Daemon appends these as --extra-substituters / --extra-trusted-public-keys.",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Nix caches list"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Sync token required"),
    ),
)]
#[rocket::get("/nix-caches")]
pub async fn get_nix_caches(_auth: SyncAuth) -> Json<NixCachesResponse> {
    let cfg = crate::config::config();
    let mut caches = Vec::new();
    if let Some(ref xzar) = cfg.xzar {
        caches.push(NixCacheEntry {
            url: xzar.url.clone(),
            public_key: xzar.public_key.clone(),
        });
    }
    Json(NixCachesResponse { caches })
}

#[derive(sqlx::FromRow)]
struct SkillSlugChannel {
    skill_channel_id: Uuid,
    slug: String,
    channel: String,
    is_direct: bool,
}

/// Resolve the winning skill_channel_id per slug for a cluster.
/// Direct assignments beat bundle assignments for the same slug.
async fn resolve_winning_skill_channels(
    cluster_id: Uuid,
    pool: &PgPool,
) -> Result<HashMap<String, Uuid>, Status> {
    let rows = sqlx::query_as::<_, SkillSlugChannel>(
        "SELECT sc.id AS skill_channel_id, s.slug, sc.channel, true AS is_direct \
         FROM cluster_skills cs \
         JOIN skill_channels sc ON sc.id = cs.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE cs.cluster_id = $1 \
         UNION ALL \
         SELECT sc.id AS skill_channel_id, s.slug, sc.channel, false AS is_direct \
         FROM cluster_bundles cb \
         JOIN bundle_items bi ON bi.bundle_id = cb.bundle_id \
         JOIN skill_channels sc ON sc.id = bi.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE cb.cluster_id = $1",
    )
    .bind(cluster_id)
    .fetch_all(pool)
    .await
    .map_err(|_| Status::InternalServerError)?;

    let mut winners: HashMap<String, (Uuid, bool)> = HashMap::new();
    for row in &rows {
        match winners.get(&row.slug) {
            Some((_, true)) => {}
            _ => {
                winners.insert(row.slug.clone(), (row.skill_channel_id, row.is_direct));
            }
        }
    }

    Ok(winners
        .into_iter()
        .map(|(slug, (id, _))| (slug, id))
        .collect())
}

#[utoipa::path(
    get,
    path = "/api/skills",
    tag = "Sync",
    summary = "Get resolved skill store paths for daemon sync",
    security(("bearer" = [])),
    params(
        ("arch" = String, Query, description = "Target architecture (e.g. x86_64-linux)"),
    ),
    responses(
        (status = 200, description = "Map of slug to Nix store path", body = HashMap<String, String>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Sync token required"),
    ),
)]
#[rocket::get("/skills?<arch>")]
pub async fn get_skills(
    auth: SyncAuth,
    pool: &State<PgPool>,
    arch: String,
    cache: &State<crate::skill_center_cache::SkillCenterCache>,
) -> Result<Json<HashMap<String, String>>, Status> {
    let rows = sqlx::query_as::<_, SkillSlugChannel>(
        "SELECT sc.id AS skill_channel_id, s.slug, sc.channel, true AS is_direct \
         FROM cluster_skills cs \
         JOIN skill_channels sc ON sc.id = cs.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE cs.cluster_id = $1 \
         UNION ALL \
         SELECT sc.id AS skill_channel_id, s.slug, sc.channel, false AS is_direct \
         FROM cluster_bundles cb \
         JOIN bundle_items bi ON bi.bundle_id = cb.bundle_id \
         JOIN skill_channels sc ON sc.id = bi.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE cb.cluster_id = $1",
    )
    .bind(auth.cluster_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    if rows.is_empty() {
        return Ok(Json(HashMap::new()));
    }

    let mut slug_channel: HashMap<String, (String, bool)> = HashMap::new();
    for row in &rows {
        match slug_channel.get(&row.slug) {
            Some((_, true)) => {}
            _ => {
                slug_channel.insert(row.slug.clone(), (row.channel.clone(), row.is_direct));
            }
        }
    }

    let cfg = crate::config::config();
    let Some(ref xzar) = cfg.xzar else {
        return Ok(Json(std::collections::HashMap::new()));
    };
    let pins = crate::xzar::fetch_pins(&xzar.url, &xzar.token)
        .await
        .map_err(|e| {
            tracing::error!("xzar fetch failed: {e}");
            Status::InternalServerError
        })?;

    let skills: Vec<(String, String)> = slug_channel
        .into_iter()
        .map(|(slug, (channel, _))| (slug, channel))
        .collect();
    let local_result = crate::xzar::resolve_store_paths(&pins, &skills, &arch);

    // Aggregate skills from remote skill centers
    let remote_skills =
        aggregate_remote_skills(auth.cluster_id, pool.inner(), cache.inner(), &arch).await;
    // Remote items go in first, then local overlays (local wins on slug collision)
    let mut merged = remote_skills;
    for (slug, path) in local_result {
        merged.insert(slug, path);
    }

    Ok(Json(merged))
}

// ── Setting token routes ───────────────────────────────────────────────

// -- Config --

#[utoipa::path(
    get,
    path = "/api/setting/config/schema",
    tag = "Setting — Config",
    summary = "Get JSON Schema for cluster config",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "JSON Schema", content_type = "application/json"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::get("/setting/config/schema")]
pub async fn setting_config_schema(_auth: SettingAuth) -> (rocket::http::ContentType, String) {
    let schema = schemars::schema_for!(mac_mgmt_common::ClusterConfig);
    (
        rocket::http::ContentType::JSON,
        serde_json::to_string_pretty(&schema).unwrap(),
    )
}

#[utoipa::path(
    get,
    path = "/api/setting/config",
    tag = "Setting — Config",
    summary = "Get cluster config as JSON",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Cluster config JSON"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
        (status = 404, description = "No config saved"),
    ),
)]
#[rocket::get("/setting/config")]
pub async fn setting_get_config(
    auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<Json<serde_json::Value>, Status> {
    let config = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT config_json FROM cluster_configs \
         WHERE cluster_id = $1 \
         ORDER BY created_at DESC \
         LIMIT 1",
    )
    .bind(auth.cluster_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?
    .ok_or(Status::NotFound)?;

    Ok(Json(config))
}

#[derive(Deserialize, ToSchema)]
pub struct SetConfigBody {
    #[serde(flatten)]
    config: serde_json::Value,
}

#[utoipa::path(
    put,
    path = "/api/setting/config",
    tag = "Setting — Config",
    summary = "Set cluster config (JSON body)",
    security(("bearer" = [])),
    request_body = SetConfigBody,
    responses(
        (status = 201, description = "Config saved"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
        (status = 422, description = "Invalid config"),
    ),
)]
#[rocket::put("/setting/config", data = "<body>")]
pub async fn setting_set_config(
    auth: SettingAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    body: Json<SetConfigBody>,
) -> Result<Status, Status> {
    // Apply config migrations before validation
    let mut config = body.config.clone();
    mac_mgmt_common::config_migrate::migrate(&mut config);

    // Validate by deserializing into ClusterConfig
    let _: mac_mgmt_common::ClusterConfig =
        serde_json::from_value(config.clone()).map_err(|e| {
            tracing::error!(
                "setting_set_config: rejecting cluster={} config: {e}; body={}",
                auth.cluster_id,
                serde_json::to_string(&config).unwrap_or_default()
            );
            Status::UnprocessableEntity
        })?;

    sqlx::query("INSERT INTO cluster_configs (cluster_id, config_json) VALUES ($1, $2)")
        .bind(auth.cluster_id)
        .bind(&config)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;

    push::notify(channels, auth.cluster_id, PushMessage::SyncConfig).await;
    Ok(Status::Created)
}

// -- Config partial update --

#[derive(Deserialize, ToSchema)]
pub struct PatchConfigBody {
    section: String,
    key: String,
    value: serde_json::Value,
}

#[utoipa::path(
    patch,
    path = "/api/setting/config",
    tag = "Setting — Config",
    summary = "Partially update cluster config",
    description = "Updates a single key within a config section. Reads the current config, merges the change, validates, and saves as a new version.",
    security(("bearer" = [])),
    request_body = PatchConfigBody,
    responses(
        (status = 200, description = "Config updated"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
        (status = 422, description = "Invalid config after merge"),
    ),
)]
#[rocket::patch("/setting/config", data = "<body>")]
pub async fn setting_patch_config(
    auth: SettingAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    body: Json<PatchConfigBody>,
) -> Result<Status, Status> {
    // Load current config
    let current: serde_json::Value = sqlx::query_scalar(
        "SELECT config_json FROM cluster_configs \
         WHERE cluster_id = $1 \
         ORDER BY created_at DESC \
         LIMIT 1",
    )
    .bind(auth.cluster_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?
    .unwrap_or(serde_json::json!({}));

    let mut config = current;

    // Apply config migrations to the base before merging
    mac_mgmt_common::config_migrate::migrate(&mut config);

    // Merge: config[section][key] = value
    let section_obj = config
        .as_object_mut()
        .ok_or(Status::InternalServerError)?
        .entry(&body.section)
        .or_insert(serde_json::json!({}));
    let section_map = section_obj
        .as_object_mut()
        .ok_or(Status::UnprocessableEntity)?;
    section_map.insert(body.key.clone(), body.value.clone());

    // Validate
    let _: mac_mgmt_common::ClusterConfig =
        serde_json::from_value(config.clone()).map_err(|_| Status::UnprocessableEntity)?;

    sqlx::query("INSERT INTO cluster_configs (cluster_id, config_json) VALUES ($1, $2)")
        .bind(auth.cluster_id)
        .bind(&config)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;

    push::notify(channels, auth.cluster_id, PushMessage::SyncConfig).await;
    Ok(Status::Ok)
}

// -- Skills --

#[utoipa::path(
    get,
    path = "/api/setting/skills",
    tag = "Setting — Skills",
    summary = "List cluster skill assignments",
    description = "Returns installed skill channels with `installed_bundle` indicating whether the skill comes from a bundle. `cluster_skill_id` is present only for direct assignments.",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Cluster skills", body = Vec<SkillChannelRow>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::get("/setting/skills")]
pub async fn setting_list_skills(
    auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<SkillChannelRow>>, Status> {
    let rows = build_skill_channel_rows(auth.cluster_id, pool.inner())
        .await?
        .into_iter()
        .filter(|r| r.installed)
        .collect();
    Ok(Json(rows))
}

#[derive(Deserialize, ToSchema)]
pub struct AddSkillBody {
    skill_channel_id: Uuid,
}

#[utoipa::path(
    post,
    path = "/api/setting/skills",
    tag = "Setting — Skills",
    summary = "Add a direct skill assignment",
    security(("bearer" = [])),
    request_body = AddSkillBody,
    responses(
        (status = 201, description = "Skill added"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::post("/setting/skills", data = "<body>")]
pub async fn setting_add_skill(
    auth: SettingAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    body: Json<AddSkillBody>,
) -> Result<Status, Status> {
    sqlx::query("INSERT INTO cluster_skills (cluster_id, skill_channel_id) VALUES ($1, $2)")
        .bind(auth.cluster_id)
        .bind(body.skill_channel_id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    push::notify(channels, auth.cluster_id, PushMessage::SyncSkills).await;
    Ok(Status::Created)
}

#[utoipa::path(
    delete,
    path = "/api/setting/skills/{id}",
    tag = "Setting — Skills",
    summary = "Remove a direct skill assignment",
    security(("bearer" = [])),
    params(("id" = Uuid, Path, description = "Cluster skill assignment ID")),
    responses(
        (status = 204, description = "Skill removed"),
        (status = 400, description = "Invalid UUID"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::delete("/setting/skills/<id>")]
pub async fn setting_remove_skill(
    auth: SettingAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    id: &str,
) -> Result<Status, Status> {
    let uuid: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    sqlx::query("DELETE FROM cluster_skills WHERE id = $1 AND cluster_id = $2")
        .bind(uuid)
        .bind(auth.cluster_id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    push::notify(channels, auth.cluster_id, PushMessage::SyncSkills).await;
    Ok(Status::NoContent)
}

// -- Skills batch --

#[derive(Deserialize, ToSchema)]
pub struct BatchSkillsBody {
    #[serde(default)]
    add: Vec<Uuid>,
    #[serde(default)]
    remove: Vec<Uuid>,
}

#[utoipa::path(
    patch,
    path = "/api/setting/skills/batch",
    tag = "Setting — Skills",
    summary = "Batch add/remove direct skill assignments",
    description = "Add and remove skill channels in a single request. `add` contains skill_channel_ids to assign (duplicates skipped). `remove` contains cluster_skill_ids to delete.",
    security(("bearer" = [])),
    request_body = BatchSkillsBody,
    responses(
        (status = 200, description = "Skills updated"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::patch("/setting/skills/batch", data = "<body>")]
pub async fn setting_batch_skills(
    auth: SettingAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    body: Json<BatchSkillsBody>,
) -> Result<Status, Status> {
    let mut tx = pool
        .inner()
        .begin()
        .await
        .map_err(|_| Status::InternalServerError)?;
    for id in &body.remove {
        sqlx::query("DELETE FROM cluster_skills WHERE id = $1 AND cluster_id = $2")
            .bind(id)
            .bind(auth.cluster_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| Status::InternalServerError)?;
    }
    for scid in &body.add {
        sqlx::query("INSERT INTO cluster_skills (cluster_id, skill_channel_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(auth.cluster_id)
            .bind(scid)
            .execute(&mut *tx)
            .await
            .map_err(|_| Status::InternalServerError)?;
    }
    tx.commit().await.map_err(|_| Status::InternalServerError)?;
    push::notify(channels, auth.cluster_id, PushMessage::SyncSkills).await;
    Ok(Status::Ok)
}

// -- Bundles --

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub(crate) struct ClusterBundleRow {
    cluster_bundle_id: Uuid,
    bundle_slug: String,
    bundle_name: String,
    bundle_description: String,
}

#[utoipa::path(
    get,
    path = "/api/setting/bundles",
    tag = "Setting — Bundles",
    summary = "List cluster bundle assignments",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Cluster bundles", body = Vec<ClusterBundleRow>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::get("/setting/bundles")]
pub async fn setting_list_bundles(
    auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<ClusterBundleRow>>, Status> {
    let rows = sqlx::query_as::<_, ClusterBundleRow>(
        "SELECT cb.id as cluster_bundle_id, b.slug as bundle_slug, b.name as bundle_name, b.description as bundle_description \
         FROM cluster_bundles cb \
         JOIN bundles b ON b.id = cb.bundle_id \
         WHERE cb.cluster_id = $1 \
         ORDER BY b.slug",
    )
    .bind(auth.cluster_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(rows))
}

#[derive(Deserialize, ToSchema)]
pub struct AddBundleBody {
    bundle_id: Uuid,
}

#[utoipa::path(
    post,
    path = "/api/setting/bundles",
    tag = "Setting — Bundles",
    summary = "Add a bundle assignment",
    security(("bearer" = [])),
    request_body = AddBundleBody,
    responses(
        (status = 201, description = "Bundle added"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::post("/setting/bundles", data = "<body>")]
pub async fn setting_add_bundle(
    auth: SettingAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    body: Json<AddBundleBody>,
) -> Result<Status, Status> {
    sqlx::query("INSERT INTO cluster_bundles (cluster_id, bundle_id) VALUES ($1, $2)")
        .bind(auth.cluster_id)
        .bind(body.bundle_id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    push::notify(channels, auth.cluster_id, PushMessage::SyncSkills).await;
    Ok(Status::Created)
}

#[utoipa::path(
    delete,
    path = "/api/setting/bundles/{id}",
    tag = "Setting — Bundles",
    summary = "Remove a bundle assignment",
    security(("bearer" = [])),
    params(("id" = Uuid, Path, description = "Cluster bundle assignment ID")),
    responses(
        (status = 204, description = "Bundle removed"),
        (status = 400, description = "Invalid UUID"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::delete("/setting/bundles/<id>")]
pub async fn setting_remove_bundle(
    auth: SettingAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    id: &str,
) -> Result<Status, Status> {
    let uuid: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    sqlx::query("DELETE FROM cluster_bundles WHERE id = $1 AND cluster_id = $2")
        .bind(uuid)
        .bind(auth.cluster_id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    push::notify(channels, auth.cluster_id, PushMessage::SyncSkills).await;
    Ok(Status::NoContent)
}

// -- Bundles batch --

#[derive(Deserialize, ToSchema)]
pub struct BatchBundlesBody {
    #[serde(default)]
    add: Vec<Uuid>,
    #[serde(default)]
    remove: Vec<Uuid>,
}

#[utoipa::path(
    patch,
    path = "/api/setting/bundles/batch",
    tag = "Setting — Bundles",
    summary = "Batch add/remove bundle assignments",
    description = "`add` contains bundle_ids to assign (duplicates skipped). `remove` contains cluster_bundle_ids to delete.",
    security(("bearer" = [])),
    request_body = BatchBundlesBody,
    responses(
        (status = 200, description = "Bundles updated"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::patch("/setting/bundles/batch", data = "<body>")]
pub async fn setting_batch_bundles(
    auth: SettingAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    body: Json<BatchBundlesBody>,
) -> Result<Status, Status> {
    let mut tx = pool
        .inner()
        .begin()
        .await
        .map_err(|_| Status::InternalServerError)?;
    for id in &body.remove {
        sqlx::query("DELETE FROM cluster_bundles WHERE id = $1 AND cluster_id = $2")
            .bind(id)
            .bind(auth.cluster_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| Status::InternalServerError)?;
    }
    for bid in &body.add {
        sqlx::query("INSERT INTO cluster_bundles (cluster_id, bundle_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(auth.cluster_id)
            .bind(bid)
            .execute(&mut *tx)
            .await
            .map_err(|_| Status::InternalServerError)?;
    }
    tx.commit().await.map_err(|_| Status::InternalServerError)?;
    push::notify(channels, auth.cluster_id, PushMessage::SyncSkills).await;
    Ok(Status::Ok)
}

// -- MCP Servers --

#[utoipa::path(
    get,
    path = "/api/setting/mcp-servers",
    tag = "Setting — MCP Servers",
    summary = "List cluster MCP server assignments",
    description = "Returns installed MCP servers with `installed_bundle` and `installed_transitive` flags. `cluster_mcp_server_id` is present only for direct assignments.",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Cluster MCP servers", body = Vec<McpServerOptionRow>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::get("/setting/mcp-servers")]
pub async fn setting_list_mcp_servers(
    auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<McpServerOptionRow>>, Status> {
    let rows = build_mcp_server_options(auth.cluster_id, pool.inner())
        .await?
        .into_iter()
        .filter(|r| r.installed)
        .collect();
    Ok(Json(rows))
}

#[derive(Deserialize, ToSchema)]
pub struct AddMcpServerBody {
    mcp_server_id: Uuid,
}

#[utoipa::path(
    post,
    path = "/api/setting/mcp-servers",
    tag = "Setting — MCP Servers",
    summary = "Add a direct MCP server assignment",
    security(("bearer" = [])),
    request_body = AddMcpServerBody,
    responses(
        (status = 201, description = "MCP server added"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::post("/setting/mcp-servers", data = "<body>")]
pub async fn setting_add_mcp_server(
    auth: SettingAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    body: Json<AddMcpServerBody>,
) -> Result<Status, Status> {
    sqlx::query("INSERT INTO cluster_mcp_servers (cluster_id, mcp_server_id) VALUES ($1, $2)")
        .bind(auth.cluster_id)
        .bind(body.mcp_server_id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    push::notify(channels, auth.cluster_id, PushMessage::SyncMcpServers).await;
    Ok(Status::Created)
}

#[utoipa::path(
    delete,
    path = "/api/setting/mcp-servers/{id}",
    tag = "Setting — MCP Servers",
    summary = "Remove a direct MCP server assignment",
    security(("bearer" = [])),
    params(("id" = Uuid, Path, description = "Cluster MCP server assignment ID")),
    responses(
        (status = 204, description = "MCP server removed"),
        (status = 400, description = "Invalid UUID"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::delete("/setting/mcp-servers/<id>")]
pub async fn setting_remove_mcp_server(
    auth: SettingAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    id: &str,
) -> Result<Status, Status> {
    let uuid: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    sqlx::query("DELETE FROM cluster_mcp_servers WHERE id = $1 AND cluster_id = $2")
        .bind(uuid)
        .bind(auth.cluster_id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    push::notify(channels, auth.cluster_id, PushMessage::SyncMcpServers).await;
    Ok(Status::NoContent)
}

// -- MCP Servers batch --

#[derive(Deserialize, ToSchema)]
pub struct BatchMcpServersBody {
    #[serde(default)]
    add: Vec<Uuid>,
    #[serde(default)]
    remove: Vec<Uuid>,
}

#[utoipa::path(
    patch,
    path = "/api/setting/mcp-servers/batch",
    tag = "Setting — MCP Servers",
    summary = "Batch add/remove direct MCP server assignments",
    description = "`add` contains mcp_server_ids to assign (duplicates skipped). `remove` contains cluster_mcp_server_ids to delete.",
    security(("bearer" = [])),
    request_body = BatchMcpServersBody,
    responses(
        (status = 200, description = "MCP servers updated"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::patch("/setting/mcp-servers/batch", data = "<body>")]
pub async fn setting_batch_mcp_servers(
    auth: SettingAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    body: Json<BatchMcpServersBody>,
) -> Result<Status, Status> {
    let mut tx = pool
        .inner()
        .begin()
        .await
        .map_err(|_| Status::InternalServerError)?;
    for id in &body.remove {
        sqlx::query("DELETE FROM cluster_mcp_servers WHERE id = $1 AND cluster_id = $2")
            .bind(id)
            .bind(auth.cluster_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| Status::InternalServerError)?;
    }
    for msid in &body.add {
        sqlx::query("INSERT INTO cluster_mcp_servers (cluster_id, mcp_server_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(auth.cluster_id)
            .bind(msid)
            .execute(&mut *tx)
            .await
            .map_err(|_| Status::InternalServerError)?;
    }
    tx.commit().await.map_err(|_| Status::InternalServerError)?;
    push::notify(channels, auth.cluster_id, PushMessage::SyncMcpServers).await;
    Ok(Status::Ok)
}

// -- MCP Bundles --

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub(crate) struct ClusterMcpBundleRow {
    cluster_mcp_bundle_id: Uuid,
    bundle_slug: String,
    bundle_name: String,
    bundle_description: String,
}

#[utoipa::path(
    get,
    path = "/api/setting/mcp-bundles",
    tag = "Setting — MCP Bundles",
    summary = "List cluster MCP bundle assignments",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Cluster MCP bundles", body = Vec<ClusterMcpBundleRow>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::get("/setting/mcp-bundles")]
pub async fn setting_list_mcp_bundles(
    auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<ClusterMcpBundleRow>>, Status> {
    let rows = sqlx::query_as::<_, ClusterMcpBundleRow>(
        "SELECT cmb.id as cluster_mcp_bundle_id, msb.slug as bundle_slug, msb.name as bundle_name, msb.description as bundle_description \
         FROM cluster_mcp_bundles cmb \
         JOIN mcp_server_bundles msb ON msb.id = cmb.bundle_id \
         WHERE cmb.cluster_id = $1 \
         ORDER BY msb.slug",
    )
    .bind(auth.cluster_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(rows))
}

#[derive(Deserialize, ToSchema)]
pub struct AddMcpBundleBody {
    bundle_id: Uuid,
}

#[utoipa::path(
    post,
    path = "/api/setting/mcp-bundles",
    tag = "Setting — MCP Bundles",
    summary = "Add an MCP bundle assignment",
    security(("bearer" = [])),
    request_body = AddMcpBundleBody,
    responses(
        (status = 201, description = "MCP bundle added"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::post("/setting/mcp-bundles", data = "<body>")]
pub async fn setting_add_mcp_bundle(
    auth: SettingAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    body: Json<AddMcpBundleBody>,
) -> Result<Status, Status> {
    sqlx::query("INSERT INTO cluster_mcp_bundles (cluster_id, bundle_id) VALUES ($1, $2)")
        .bind(auth.cluster_id)
        .bind(body.bundle_id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    push::notify(channels, auth.cluster_id, PushMessage::SyncMcpServers).await;
    Ok(Status::Created)
}

#[utoipa::path(
    delete,
    path = "/api/setting/mcp-bundles/{id}",
    tag = "Setting — MCP Bundles",
    summary = "Remove an MCP bundle assignment",
    security(("bearer" = [])),
    params(("id" = Uuid, Path, description = "Cluster MCP bundle assignment ID")),
    responses(
        (status = 204, description = "MCP bundle removed"),
        (status = 400, description = "Invalid UUID"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::delete("/setting/mcp-bundles/<id>")]
pub async fn setting_remove_mcp_bundle(
    auth: SettingAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    id: &str,
) -> Result<Status, Status> {
    let uuid: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    sqlx::query("DELETE FROM cluster_mcp_bundles WHERE id = $1 AND cluster_id = $2")
        .bind(uuid)
        .bind(auth.cluster_id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    push::notify(channels, auth.cluster_id, PushMessage::SyncMcpServers).await;
    Ok(Status::NoContent)
}

// -- MCP Bundles batch --

#[derive(Deserialize, ToSchema)]
pub struct BatchMcpBundlesBody {
    #[serde(default)]
    add: Vec<Uuid>,
    #[serde(default)]
    remove: Vec<Uuid>,
}

#[utoipa::path(
    patch,
    path = "/api/setting/mcp-bundles/batch",
    tag = "Setting — MCP Bundles",
    summary = "Batch add/remove MCP bundle assignments",
    description = "`add` contains bundle_ids to assign (duplicates skipped). `remove` contains cluster_mcp_bundle_ids to delete.",
    security(("bearer" = [])),
    request_body = BatchMcpBundlesBody,
    responses(
        (status = 200, description = "MCP bundles updated"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::patch("/setting/mcp-bundles/batch", data = "<body>")]
pub async fn setting_batch_mcp_bundles(
    auth: SettingAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    body: Json<BatchMcpBundlesBody>,
) -> Result<Status, Status> {
    let mut tx = pool
        .inner()
        .begin()
        .await
        .map_err(|_| Status::InternalServerError)?;
    for id in &body.remove {
        sqlx::query("DELETE FROM cluster_mcp_bundles WHERE id = $1 AND cluster_id = $2")
            .bind(id)
            .bind(auth.cluster_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| Status::InternalServerError)?;
    }
    for bid in &body.add {
        sqlx::query("INSERT INTO cluster_mcp_bundles (cluster_id, bundle_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(auth.cluster_id)
            .bind(bid)
            .execute(&mut *tx)
            .await
            .map_err(|_| Status::InternalServerError)?;
    }
    tx.commit().await.map_err(|_| Status::InternalServerError)?;
    push::notify(channels, auth.cluster_id, PushMessage::SyncMcpServers).await;
    Ok(Status::Ok)
}

// ── Setting — manual cluster packages ──────────────────────────────────

#[derive(Clone, Serialize, ToSchema, sqlx::FromRow)]
pub(crate) struct ClusterPackageRow {
    id: Uuid,
    package: String,
    created_at: DateTime<Utc>,
}

#[derive(Deserialize, ToSchema)]
pub(crate) struct AddPackageBody {
    package: String,
}

#[derive(Deserialize, ToSchema)]
pub(crate) struct BatchPackagesBody {
    #[serde(default)]
    add: Vec<String>,
    #[serde(default)]
    remove: Vec<Uuid>,
}

#[utoipa::path(
    get,
    path = "/api/setting/packages",
    tag = "Setting — Packages",
    summary = "List manual packages for cluster",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Manual packages", body = Vec<ClusterPackageRow>),
    ),
)]
#[rocket::get("/setting/packages")]
pub async fn setting_list_packages(
    auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<ClusterPackageRow>>, Status> {
    let rows: Vec<ClusterPackageRow> = sqlx::query_as(
        "SELECT id, package, created_at FROM cluster_packages \
         WHERE cluster_id = $1 ORDER BY package",
    )
    .bind(auth.cluster_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    Ok(Json(rows))
}

#[utoipa::path(
    post,
    path = "/api/setting/packages",
    tag = "Setting — Packages",
    summary = "Add a manual package to the cluster",
    security(("bearer" = [])),
    request_body = AddPackageBody,
    responses(
        (status = 201, description = "Package added"),
        (status = 409, description = "Package already exists"),
    ),
)]
#[rocket::post("/setting/packages", data = "<body>")]
pub async fn setting_add_package(
    auth: SettingAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    body: Json<AddPackageBody>,
) -> Result<Status, Status> {
    let pkg = body.package.trim().to_string();
    if pkg.is_empty() {
        return Err(Status::BadRequest);
    }
    sqlx::query(
        "INSERT INTO cluster_packages (cluster_id, package) VALUES ($1, $2) \
         ON CONFLICT (cluster_id, package) DO NOTHING",
    )
    .bind(auth.cluster_id)
    .bind(&pkg)
    .execute(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    push::notify(channels, auth.cluster_id, PushMessage::SyncPackages).await;
    Ok(Status::Created)
}

#[utoipa::path(
    delete,
    path = "/api/setting/packages/{id}",
    tag = "Setting — Packages",
    summary = "Remove a manual package from the cluster",
    security(("bearer" = [])),
    params(("id" = Uuid, Path, description = "cluster_packages row ID")),
    responses(
        (status = 204, description = "Removed"),
        (status = 404, description = "Not found"),
    ),
)]
#[rocket::delete("/setting/packages/<id>")]
pub async fn setting_remove_package(
    auth: SettingAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    id: &str,
) -> Result<Status, Status> {
    let id: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    let deleted = sqlx::query("DELETE FROM cluster_packages WHERE id = $1 AND cluster_id = $2")
        .bind(id)
        .bind(auth.cluster_id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?
        .rows_affected();

    if deleted == 0 {
        return Err(Status::NotFound);
    }

    push::notify(channels, auth.cluster_id, PushMessage::SyncPackages).await;
    Ok(Status::NoContent)
}

#[utoipa::path(
    patch,
    path = "/api/setting/packages/batch",
    tag = "Setting — Packages",
    summary = "Batch add/remove manual packages",
    security(("bearer" = [])),
    request_body = BatchPackagesBody,
    responses(
        (status = 200, description = "Batch applied"),
    ),
)]
#[rocket::patch("/setting/packages/batch", data = "<body>")]
pub async fn setting_batch_packages(
    auth: SettingAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    body: Json<BatchPackagesBody>,
) -> Result<Status, Status> {
    let mut tx = pool
        .inner()
        .begin()
        .await
        .map_err(|_| Status::InternalServerError)?;
    for id in &body.remove {
        sqlx::query("DELETE FROM cluster_packages WHERE id = $1 AND cluster_id = $2")
            .bind(id)
            .bind(auth.cluster_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| Status::InternalServerError)?;
    }
    for pkg in &body.add {
        let pkg = pkg.trim();
        if !pkg.is_empty() {
            sqlx::query(
                "INSERT INTO cluster_packages (cluster_id, package) VALUES ($1, $2) \
                 ON CONFLICT DO NOTHING",
            )
            .bind(auth.cluster_id)
            .bind(pkg)
            .execute(&mut *tx)
            .await
            .map_err(|_| Status::InternalServerError)?;
        }
    }
    tx.commit().await.map_err(|_| Status::InternalServerError)?;
    push::notify(channels, auth.cluster_id, PushMessage::SyncPackages).await;
    Ok(Status::Ok)
}

// -- Available resources (for dropdowns) --

#[derive(Clone, Serialize, ToSchema, sqlx::FromRow)]
pub(crate) struct SkillChannelRow {
    id: Uuid,
    skill_slug: String,
    skill_name: String,
    skill_description: String,
    channel: String,
    installed: bool,
    installed_bundle: bool,
    /// Present only for direct (non-bundle) assignments; use with DELETE /setting/skills/{id}.
    cluster_skill_id: Option<Uuid>,
}

async fn build_skill_channel_rows(
    cluster_id: Uuid,
    pool: &PgPool,
) -> Result<Vec<SkillChannelRow>, Status> {
    sqlx::query_as::<_, SkillChannelRow>(
        "SELECT sc.id, s.slug as skill_slug, s.name as skill_name, s.description as skill_description, sc.channel, \
                (cs.id IS NOT NULL OR bi.id IS NOT NULL) as installed, \
                (bi.id IS NOT NULL) as installed_bundle, \
                cs.id as cluster_skill_id \
         FROM skill_channels sc \
         JOIN skills s ON s.id = sc.skill_id \
         LEFT JOIN cluster_skills cs ON cs.skill_channel_id = sc.id AND cs.cluster_id = $1 \
         LEFT JOIN bundle_items bi ON bi.skill_channel_id = sc.id \
              AND bi.bundle_id IN (SELECT bundle_id FROM cluster_bundles WHERE cluster_id = $1) \
         WHERE NOT s.hide_from_public_catalog \
         ORDER BY s.slug, sc.channel",
    )
    .bind(cluster_id)
    .fetch_all(pool)
    .await
    .map_err(|_| Status::InternalServerError)
}

#[utoipa::path(
    get,
    path = "/api/setting/available/skill-channels",
    tag = "Setting — Available",
    summary = "List all skill channels",
    description = "Returns all skill channels with an `installed` flag indicating whether the cluster has this skill channel assigned (directly or via a bundle).",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "All skill channels", body = Vec<SkillChannelRow>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::get("/setting/available/skill-channels")]
pub async fn setting_available_skill_channels(
    auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<SkillChannelRow>>, Status> {
    Ok(Json(
        build_skill_channel_rows(auth.cluster_id, pool.inner()).await?,
    ))
}

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub(crate) struct OptionRow {
    id: Uuid,
    slug: String,
    name: String,
    description: String,
    installed: bool,
}

#[derive(Clone, Serialize, ToSchema)]
pub(crate) struct McpServerOptionRow {
    id: Uuid,
    slug: String,
    name: String,
    description: String,
    installed: bool,
    installed_bundle: bool,
    installed_transitive: bool,
    /// Present only for direct (non-bundle, non-transitive) assignments; use with DELETE /setting/mcp-servers/{id}.
    cluster_mcp_server_id: Option<Uuid>,
}

async fn build_bundle_rows(cluster_id: Uuid, pool: &PgPool) -> Result<Vec<OptionRow>, Status> {
    sqlx::query_as::<_, OptionRow>(
        "SELECT b.id, b.slug, b.name, b.description, \
                (cb.id IS NOT NULL) as installed \
         FROM bundles b \
         LEFT JOIN cluster_bundles cb ON cb.bundle_id = b.id AND cb.cluster_id = $1 \
         WHERE NOT b.hide_from_public_catalog \
         ORDER BY b.slug",
    )
    .bind(cluster_id)
    .fetch_all(pool)
    .await
    .map_err(|_| Status::InternalServerError)
}

#[utoipa::path(
    get,
    path = "/api/setting/available/bundles",
    tag = "Setting — Available",
    summary = "List all skill bundles",
    description = "Returns all skill bundles with an `installed` flag indicating whether the cluster has this bundle assigned.",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "All bundles", body = Vec<OptionRow>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::get("/setting/available/bundles")]
pub async fn setting_available_bundles(
    auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<OptionRow>>, Status> {
    Ok(Json(
        build_bundle_rows(auth.cluster_id, pool.inner()).await?,
    ))
}

/// Fetch the set of MCP server IDs transitively required by a cluster's winning skill channels.
async fn transitive_mcp_server_ids(
    cluster_id: Uuid,
    pool: &PgPool,
) -> Result<std::collections::HashSet<Uuid>, Status> {
    let winning = resolve_winning_skill_channels(cluster_id, pool).await?;
    let channel_ids: Vec<Uuid> = winning.into_values().collect();
    if channel_ids.is_empty() {
        return Ok(std::collections::HashSet::new());
    }
    let rows = sqlx::query_scalar::<_, Uuid>(
        "SELECT DISTINCT mcp_server_id FROM skill_mcp_dependencies WHERE skill_channel_id = ANY($1)",
    )
    .bind(&channel_ids)
    .fetch_all(pool)
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(rows.into_iter().collect())
}

#[derive(sqlx::FromRow)]
struct McpServerBaseRow {
    id: Uuid,
    slug: String,
    name: String,
    description: String,
    installed_direct: bool,
    installed_bundle: bool,
    cluster_mcp_server_id: Option<Uuid>,
}

/// Build the full MCP server option list with all install flags.
async fn build_mcp_server_options(
    cluster_id: Uuid,
    pool: &PgPool,
) -> Result<Vec<McpServerOptionRow>, Status> {
    let base_rows = sqlx::query_as::<_, McpServerBaseRow>(
        "SELECT ms.id, ms.slug, ms.name, ms.description, \
                (cms.id IS NOT NULL) as installed_direct, \
                (msbi.id IS NOT NULL) as installed_bundle, \
                cms.id as cluster_mcp_server_id \
         FROM mcp_servers ms \
         LEFT JOIN cluster_mcp_servers cms ON cms.mcp_server_id = ms.id AND cms.cluster_id = $1 \
         LEFT JOIN mcp_server_bundle_items msbi ON msbi.mcp_server_id = ms.id \
              AND msbi.bundle_id IN (SELECT bundle_id FROM cluster_mcp_bundles WHERE cluster_id = $1) \
         WHERE NOT ms.hide_from_public_catalog \
         ORDER BY ms.slug",
    )
    .bind(cluster_id)
    .fetch_all(pool)
    .await
    .map_err(|_| Status::InternalServerError)?;

    let transitive_ids = transitive_mcp_server_ids(cluster_id, pool).await?;

    Ok(base_rows
        .into_iter()
        .map(|r| {
            let installed_transitive = transitive_ids.contains(&r.id);
            McpServerOptionRow {
                id: r.id,
                slug: r.slug,
                name: r.name,
                description: r.description,
                installed: r.installed_direct || r.installed_bundle || installed_transitive,
                installed_bundle: r.installed_bundle,
                installed_transitive,
                cluster_mcp_server_id: r.cluster_mcp_server_id,
            }
        })
        .collect())
}

#[utoipa::path(
    get,
    path = "/api/setting/available/mcp-servers",
    tag = "Setting — Available",
    summary = "List all MCP servers",
    description = "Returns all MCP servers with install flags: installed (any source), installed_bundle, installed_transitive (via skill dependency).",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "All MCP servers", body = Vec<McpServerOptionRow>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::get("/setting/available/mcp-servers")]
pub async fn setting_available_mcp_servers(
    auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<McpServerOptionRow>>, Status> {
    let rows = build_mcp_server_options(auth.cluster_id, pool.inner()).await?;
    Ok(Json(rows))
}

async fn build_mcp_bundle_rows(cluster_id: Uuid, pool: &PgPool) -> Result<Vec<OptionRow>, Status> {
    sqlx::query_as::<_, OptionRow>(
        "SELECT msb.id, msb.slug, msb.name, msb.description, \
                (cmb.id IS NOT NULL) as installed \
         FROM mcp_server_bundles msb \
         LEFT JOIN cluster_mcp_bundles cmb ON cmb.bundle_id = msb.id AND cmb.cluster_id = $1 \
         WHERE NOT msb.hide_from_public_catalog \
         ORDER BY msb.slug",
    )
    .bind(cluster_id)
    .fetch_all(pool)
    .await
    .map_err(|_| Status::InternalServerError)
}

#[utoipa::path(
    get,
    path = "/api/setting/available/mcp-bundles",
    tag = "Setting — Available",
    summary = "List all MCP bundles",
    description = "Returns all MCP bundles with an `installed` flag indicating whether the cluster has this bundle assigned.",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "All MCP bundles", body = Vec<OptionRow>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::get("/setting/available/mcp-bundles")]
pub async fn setting_available_mcp_bundles(
    auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<OptionRow>>, Status> {
    Ok(Json(
        build_mcp_bundle_rows(auth.cluster_id, pool.inner()).await?,
    ))
}

// -- Bundle contents --

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub(crate) struct BundleSkillChannelRow {
    id: Uuid,
    skill_slug: String,
    skill_name: String,
    skill_description: String,
    channel: String,
}

#[utoipa::path(
    get,
    path = "/api/setting/available/bundles/{id}/skills",
    tag = "Setting — Available",
    summary = "List skill channels in a bundle",
    description = "Returns the skill channels contained in the given skill bundle.",
    security(("bearer" = [])),
    params(("id" = Uuid, Path, description = "Bundle ID")),
    responses(
        (status = 200, description = "Skill channels in the bundle", body = Vec<BundleSkillChannelRow>),
        (status = 400, description = "Invalid UUID"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::get("/setting/available/bundles/<id>/skills")]
pub async fn setting_bundle_skills(
    _auth: SettingAuth,
    pool: &State<PgPool>,
    id: &str,
) -> Result<Json<Vec<BundleSkillChannelRow>>, Status> {
    let uuid: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    let rows = sqlx::query_as::<_, BundleSkillChannelRow>(
        "SELECT sc.id, s.slug as skill_slug, s.name as skill_name, s.description as skill_description, sc.channel \
         FROM bundle_items bi \
         JOIN skill_channels sc ON sc.id = bi.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE bi.bundle_id = $1 \
         ORDER BY s.slug, sc.channel",
    )
    .bind(uuid)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(rows))
}

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub(crate) struct BundleMcpServerRow {
    id: Uuid,
    slug: String,
    name: String,
    description: String,
}

#[utoipa::path(
    get,
    path = "/api/setting/available/mcp-bundles/{id}/mcp-servers",
    tag = "Setting — Available",
    summary = "List MCP servers in an MCP bundle",
    description = "Returns the MCP servers contained in the given MCP bundle.",
    security(("bearer" = [])),
    params(("id" = Uuid, Path, description = "MCP bundle ID")),
    responses(
        (status = 200, description = "MCP servers in the bundle", body = Vec<BundleMcpServerRow>),
        (status = 400, description = "Invalid UUID"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::get("/setting/available/mcp-bundles/<id>/mcp-servers")]
pub async fn setting_mcp_bundle_servers(
    _auth: SettingAuth,
    pool: &State<PgPool>,
    id: &str,
) -> Result<Json<Vec<BundleMcpServerRow>>, Status> {
    let uuid: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    let rows = sqlx::query_as::<_, BundleMcpServerRow>(
        "SELECT ms.id, ms.slug, ms.name, ms.description \
         FROM mcp_server_bundle_items msbi \
         JOIN mcp_servers ms ON ms.id = msbi.mcp_server_id \
         WHERE msbi.bundle_id = $1 \
         ORDER BY ms.slug",
    )
    .bind(uuid)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(rows))
}

// -- Catalog (combined view) --

#[derive(Serialize, ToSchema)]
pub(crate) struct CatalogBundle {
    id: Uuid,
    slug: String,
    name: String,
    description: String,
    installed: bool,
    skills: Vec<SkillChannelRow>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct CatalogMcpBundle {
    id: Uuid,
    slug: String,
    name: String,
    description: String,
    installed: bool,
    mcp_servers: Vec<McpServerOptionRow>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct Catalog {
    skill_channels: Vec<SkillChannelRow>,
    bundles: Vec<CatalogBundle>,
    mcp_servers: Vec<McpServerOptionRow>,
    mcp_bundles: Vec<CatalogMcpBundle>,
}

#[derive(sqlx::FromRow)]
struct BundleItemLink {
    bundle_id: Uuid,
    skill_channel_id: Uuid,
}

#[derive(sqlx::FromRow)]
struct McpBundleItemLink {
    bundle_id: Uuid,
    mcp_server_id: Uuid,
}

#[utoipa::path(
    get,
    path = "/api/setting/catalog",
    tag = "Setting — Available",
    summary = "List all available resources with install status",
    description = "Returns all skill channels, bundles, MCP servers and MCP bundles in a single response. Bundles include their contained skills/MCP servers with full install flags.",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Full catalog", body = Catalog),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::get("/setting/catalog")]
pub async fn setting_catalog(
    auth: SettingAuth,
    pool: &State<PgPool>,
    _cache: &State<crate::skill_center_cache::SkillCenterCache>,
) -> Result<Json<Catalog>, Status> {
    let cid = auth.cluster_id;
    let p = pool.inner();

    let skill_channels = build_skill_channel_rows(cid, p).await?;
    let bundle_rows = build_bundle_rows(cid, p).await?;
    let mcp_servers = build_mcp_server_options(cid, p).await?;
    let mcp_bundle_rows = build_mcp_bundle_rows(cid, p).await?;

    // Fetch bundle membership links
    let skill_links =
        sqlx::query_as::<_, BundleItemLink>("SELECT bundle_id, skill_channel_id FROM bundle_items")
            .fetch_all(p)
            .await
            .map_err(|_| Status::InternalServerError)?;

    let mcp_links = sqlx::query_as::<_, McpBundleItemLink>(
        "SELECT bundle_id, mcp_server_id FROM mcp_server_bundle_items",
    )
    .fetch_all(p)
    .await
    .map_err(|_| Status::InternalServerError)?;

    // Index skill channels and MCP servers by id for lookup
    let sc_by_id: std::collections::HashMap<Uuid, &SkillChannelRow> =
        skill_channels.iter().map(|r| (r.id, r)).collect();
    let ms_by_id: std::collections::HashMap<Uuid, &McpServerOptionRow> =
        mcp_servers.iter().map(|r| (r.id, r)).collect();

    // Group skill links by bundle_id
    let mut skill_links_by_bundle: std::collections::HashMap<Uuid, Vec<Uuid>> =
        std::collections::HashMap::new();
    for link in &skill_links {
        skill_links_by_bundle
            .entry(link.bundle_id)
            .or_default()
            .push(link.skill_channel_id);
    }

    // Group MCP links by bundle_id
    let mut mcp_links_by_bundle: std::collections::HashMap<Uuid, Vec<Uuid>> =
        std::collections::HashMap::new();
    for link in &mcp_links {
        mcp_links_by_bundle
            .entry(link.bundle_id)
            .or_default()
            .push(link.mcp_server_id);
    }

    let bundles = bundle_rows
        .into_iter()
        .map(|b| {
            let skills = skill_links_by_bundle
                .get(&b.id)
                .map(|ids| {
                    ids.iter()
                        .filter_map(|id| sc_by_id.get(id).map(|r| (*r).clone()))
                        .collect()
                })
                .unwrap_or_default();
            CatalogBundle {
                id: b.id,
                slug: b.slug,
                name: b.name,
                description: b.description,
                installed: b.installed,
                skills,
            }
        })
        .collect();

    let mcp_bundles = mcp_bundle_rows
        .into_iter()
        .map(|b| {
            let servers = mcp_links_by_bundle
                .get(&b.id)
                .map(|ids| {
                    ids.iter()
                        .filter_map(|id| ms_by_id.get(id).map(|r| (*r).clone()))
                        .collect()
                })
                .unwrap_or_default();
            CatalogMcpBundle {
                id: b.id,
                slug: b.slug,
                name: b.name,
                description: b.description,
                installed: b.installed,
                mcp_servers: servers,
            }
        })
        .collect();

    Ok(Json(Catalog {
        skill_channels,
        bundles,
        mcp_servers,
        mcp_bundles,
    }))
}

// ── Admin routes ────────────────────────────────────────────────────

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub(crate) struct AdminClusterRow {
    id: Uuid,
    name: String,
    created_at: DateTime<Utc>,
}

#[utoipa::path(
    get,
    path = "/api/admin/clusters",
    tag = "Admin",
    summary = "List all clusters",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "All clusters", body = Vec<AdminClusterRow>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin token required"),
    ),
)]
#[rocket::get("/admin/clusters")]
pub async fn admin_list_clusters(
    _auth: AdminAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<AdminClusterRow>>, Status> {
    let rows = sqlx::query_as::<_, AdminClusterRow>(
        "SELECT id, name, created_at FROM clusters ORDER BY name",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(rows))
}

#[derive(Deserialize, ToSchema)]
pub struct CreateTokenForClusterBody {
    label: String,
    kind: String,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct CreatedToken {
    token: String,
}

#[utoipa::path(
    post,
    path = "/api/admin/clusters/{cluster_id}/tokens",
    tag = "Admin",
    summary = "Create a sync or setting token for a cluster",
    security(("bearer" = [])),
    params(("cluster_id" = Uuid, Path, description = "Cluster ID")),
    request_body = CreateTokenForClusterBody,
    responses(
        (status = 201, description = "Token created", body = CreatedToken),
        (status = 400, description = "Invalid kind (must be sync or setting)"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin token required"),
    ),
)]
#[rocket::post("/admin/clusters/<cluster_id>/tokens", data = "<body>")]
pub async fn admin_create_token(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    cluster_id: &str,
    body: Json<CreateTokenForClusterBody>,
) -> Result<(Status, Json<CreatedToken>), Status> {
    use rand::Rng;
    use sha2::{Digest, Sha256};

    let cid: Uuid = cluster_id.parse().map_err(|_| Status::BadRequest)?;

    if body.kind != "sync" && body.kind != "setting" {
        return Err(Status::BadRequest);
    }

    let label = body.label.trim();
    if label.is_empty() {
        return Err(Status::BadRequest);
    }

    let raw_token: String = hex::encode(rand::rng().random::<[u8; 32]>());
    let hash = hex::encode(Sha256::digest(raw_token.as_bytes()));

    // Setting tokens expire after 6 hours; sync tokens don't expire.
    let expires_at = if body.kind == "setting" {
        Some(chrono::Utc::now() + chrono::Duration::hours(6))
    } else {
        None
    };

    sqlx::query(
        "INSERT INTO tokens (cluster_id, token_hash, label, kind, expires_at) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(cid)
    .bind(&hash)
    .bind(label)
    .bind(&body.kind)
    .bind(expires_at)
    .execute(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    Ok((Status::Created, Json(CreatedToken { token: raw_token })))
}

// ── Admin — Organization CRUD ───────────────────────────────────────

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub(crate) struct AdminOrganizationRow {
    id: Uuid,
    name: String,
    created_at: DateTime<Utc>,
}

#[utoipa::path(
    get,
    path = "/api/admin/organizations",
    tag = "Admin",
    summary = "List all organizations",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "All organizations", body = Vec<AdminOrganizationRow>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin token required"),
    ),
)]
#[rocket::get("/admin/organizations")]
pub async fn admin_list_organizations(
    _auth: AdminAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<AdminOrganizationRow>>, Status> {
    let rows = sqlx::query_as::<_, AdminOrganizationRow>(
        "SELECT id, name, created_at FROM organizations ORDER BY name",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(rows))
}

#[derive(Deserialize, ToSchema)]
pub struct CreateOrganizationBody {
    pub name: String,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct CreatedOrganization {
    id: Uuid,
    name: String,
}

#[utoipa::path(
    post,
    path = "/api/admin/organizations",
    tag = "Admin",
    summary = "Create an organization",
    security(("bearer" = [])),
    request_body = CreateOrganizationBody,
    responses(
        (status = 201, description = "Organization created", body = CreatedOrganization),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin token required"),
        (status = 409, description = "Organization name already exists"),
    ),
)]
#[rocket::post("/admin/organizations", data = "<body>")]
pub async fn admin_create_organization(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    body: Json<CreateOrganizationBody>,
) -> Result<(Status, Json<CreatedOrganization>), Status> {
    let name = body.name.trim();
    if name.is_empty() {
        return Err(Status::BadRequest);
    }

    #[derive(sqlx::FromRow)]
    struct InsertedOrg {
        id: Uuid,
        name: String,
    }
    let row = sqlx::query_as::<_, InsertedOrg>(
        "INSERT INTO organizations (name) VALUES ($1) RETURNING id, name",
    )
    .bind(name)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| {
        if let Some(db_err) = e.as_database_error() {
            if db_err.is_unique_violation() {
                return Status::Conflict;
            }
        }
        tracing::error!("admin_create_organization: insert {name}: {e}");
        Status::InternalServerError
    })?;

    Ok((
        Status::Created,
        Json(CreatedOrganization {
            id: row.id,
            name: row.name,
        }),
    ))
}

// ── Admin — Organization token creation ─────────────────────────────

#[derive(Deserialize, ToSchema)]
pub struct CreateOrgTokenBody {
    label: String,
}

#[utoipa::path(
    post,
    path = "/api/admin/organizations/{org_id}/tokens",
    tag = "Admin",
    summary = "Create a setting token scoped to an organization",
    description = "Creates a setting token that grants access to all clusters in the organization.",
    security(("bearer" = [])),
    params(("org_id" = Uuid, Path, description = "Organization ID")),
    request_body = CreateOrgTokenBody,
    responses(
        (status = 201, description = "Token created", body = CreatedToken),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin token required"),
    ),
)]
#[rocket::post("/admin/organizations/<org_id>/tokens", data = "<body>")]
pub async fn admin_create_org_token(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    org_id: &str,
    body: Json<CreateOrgTokenBody>,
) -> Result<(Status, Json<CreatedToken>), Status> {
    use rand::Rng;
    use sha2::{Digest, Sha256};

    let oid: Uuid = org_id.parse().map_err(|_| Status::BadRequest)?;

    let label = body.label.trim();
    if label.is_empty() {
        return Err(Status::BadRequest);
    }

    // Verify organization exists
    let exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM organizations WHERE id = $1)")
            .bind(oid)
            .fetch_one(pool.inner())
            .await
            .map_err(|_| Status::InternalServerError)?;

    if !exists {
        return Err(Status::NotFound);
    }

    let raw_token: String = hex::encode(rand::rng().random::<[u8; 32]>());
    let hash = hex::encode(Sha256::digest(raw_token.as_bytes()));
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(6);

    sqlx::query("INSERT INTO tokens (organization_id, token_hash, label, kind, expires_at) VALUES ($1, $2, $3, 'setting', $4)")
        .bind(oid)
        .bind(&hash)
        .bind(label)
        .bind(expires_at)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;

    Ok((Status::Created, Json(CreatedToken { token: raw_token })))
}

// ── Admin — Cluster CRUD ────────────────────────────────────────────

#[derive(Deserialize, ToSchema)]
pub struct CreateClusterForOrgBody {
    pub name: String,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct CreatedCluster {
    id: Uuid,
    name: String,
}

#[utoipa::path(
    post,
    path = "/api/admin/organizations/{org_id}/clusters",
    tag = "Admin",
    summary = "Create a cluster inside an organization",
    security(("bearer" = [])),
    params(("org_id" = Uuid, Path, description = "Organization ID")),
    request_body = CreateClusterForOrgBody,
    responses(
        (status = 201, description = "Cluster created", body = CreatedCluster),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Organization not found"),
    ),
)]
#[rocket::post("/admin/organizations/<org_id>/clusters", data = "<body>")]
pub async fn admin_create_cluster(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    org_id: &str,
    body: Json<CreateClusterForOrgBody>,
) -> Result<(Status, Json<CreatedCluster>), Status> {
    let oid: Uuid = org_id.parse().map_err(|_| Status::BadRequest)?;
    let name = body.name.trim();
    if name.is_empty() {
        return Err(Status::BadRequest);
    }

    let org_exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM organizations WHERE id = $1)")
            .bind(oid)
            .fetch_one(pool.inner())
            .await
            .map_err(|e| {
                tracing::error!("admin_create_cluster: checking organization {oid}: {e}");
                Status::InternalServerError
            })?;
    if !org_exists {
        return Err(Status::NotFound);
    }

    let mut tx = pool.inner().begin().await.map_err(|e| {
        tracing::error!("admin_create_cluster: begin tx: {e}");
        Status::InternalServerError
    })?;
    #[derive(sqlx::FromRow)]
    struct InsertedCluster {
        id: Uuid,
        name: String,
    }
    let row = sqlx::query_as::<_, InsertedCluster>(
        "INSERT INTO clusters (name) VALUES ($1) RETURNING id, name",
    )
    .bind(name)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("admin_create_cluster: insert cluster {name}: {e}");
        Status::InternalServerError
    })?;
    sqlx::query("INSERT INTO organization_clusters (organization_id, cluster_id) VALUES ($1, $2)")
        .bind(oid)
        .bind(row.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!(
                "admin_create_cluster: link org {oid} to cluster {}: {e}",
                row.id
            );
            Status::InternalServerError
        })?;
    tx.commit().await.map_err(|e| {
        tracing::error!("admin_create_cluster: commit: {e}");
        Status::InternalServerError
    })?;

    Ok((
        Status::Created,
        Json(CreatedCluster {
            id: row.id,
            name: row.name,
        }),
    ))
}

#[utoipa::path(
    delete,
    path = "/api/admin/clusters/{cluster_id}",
    tag = "Admin",
    summary = "Delete a cluster (cascades tokens, configs, heartbeats)",
    security(("bearer" = [])),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "Cluster not found"),
    ),
)]
#[rocket::delete("/admin/clusters/<cluster_id>")]
pub async fn admin_delete_cluster(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    cluster_id: &str,
) -> Result<Status, Status> {
    let cid: Uuid = cluster_id.parse().map_err(|_| Status::BadRequest)?;
    let res = sqlx::query("DELETE FROM clusters WHERE id = $1")
        .bind(cid)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    if res.rows_affected() == 0 {
        return Err(Status::NotFound);
    }
    Ok(Status::NoContent)
}

#[derive(Serialize, ToSchema)]
pub(crate) struct AdminMachineRow {
    instance_id: String,
    hostname: Option<String>,
    version: String,
    reported_at: DateTime<Utc>,
    services_extended: Option<serde_json::Value>,
    /// True for disposable chaos-test nodes; excluded from rollout/fleet health.
    #[serde(default)]
    chaos: bool,
}

#[utoipa::path(
    get,
    path = "/api/admin/clusters/{cluster_id}/machines",
    tag = "Admin",
    summary = "List heartbeat-reporting machines for a cluster",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Machines", body = Vec<AdminMachineRow>),
    ),
)]
#[rocket::get("/admin/clusters/<cluster_id>/machines")]
pub async fn admin_list_cluster_machines(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    cluster_id: &str,
) -> Result<Json<Vec<AdminMachineRow>>, Status> {
    let cid: Uuid = cluster_id.parse().map_err(|_| Status::BadRequest)?;
    let rows = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            String,
            DateTime<Utc>,
            Option<serde_json::Value>,
            bool,
        ),
    >(
        "SELECT instance_id, hostname, version, reported_at, services_extended, chaos \
         FROM daemon_heartbeats WHERE cluster_id = $1 ORDER BY reported_at DESC",
    )
    .bind(cid)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(
        rows.into_iter()
            .map(
                |(instance_id, hostname, version, reported_at, services_extended, chaos)| {
                    AdminMachineRow {
                        instance_id,
                        hostname,
                        version,
                        reported_at,
                        services_extended,
                        chaos,
                    }
                },
            )
            .collect(),
    ))
}

// ── Setting — Cloud-init bootstrap ──────────────────────────────────

#[derive(Deserialize, ToSchema, Default)]
pub struct CloudInitBody {
    /// Nix system identifier (default: x86_64-linux).
    #[serde(default)]
    pub system: Option<String>,
    /// Public URL the daemon should dial; defaults to the server's configured api.external_url.
    #[serde(default)]
    pub server_url: Option<String>,
    /// Explicit daemon version; omit to resolve from rollout/pinned.
    #[serde(default)]
    pub daemon_version: Option<String>,
    /// Sync token label.
    #[serde(default)]
    pub label: Option<String>,
    /// Optional pregenerated Ed25519 host key in OpenSSH PEM format.
    /// When provided, it is written to the daemon's host-key path so the
    /// caller-computed instance_id matches what the daemon will report.
    /// When absent, the daemon generates its own on first boot.
    #[serde(default)]
    pub host_key_pem: Option<String>,
    /// Optional caller-computed instance_id (hex SHA-256 of the SSH-wire
    /// public key). Stored as a token label suffix so it can be correlated
    /// with heartbeats; the daemon itself derives instance_id from its host key.
    #[serde(default)]
    pub instance_id: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct CloudInitResponse {
    /// Rendered cloud-init YAML.
    pub cloud_init: String,
    /// The sync token embedded in `cloud_init` (for convenience).
    pub sync_token: String,
    /// Echoes the caller-supplied instance_id, or null if none was supplied.
    pub instance_id: Option<String>,
}

#[utoipa::path(
    post,
    path = "/api/setting/cloud-init",
    tag = "Setting — Config",
    summary = "Generate a cloud-init bootstrap for this cluster",
    description = "Mints a fresh sync token for the cluster and returns a ready-to-use cloud-init YAML that installs the daemon, writes the config pointing at this server, and starts the service. Optionally embeds a caller-provided Ed25519 host key so the daemon's instance_id is predictable.",
    security(("bearer" = [])),
    request_body = CloudInitBody,
    responses(
        (status = 200, description = "Cloud-init YAML + metadata", body = CloudInitResponse),
        (status = 422, description = "Could not resolve daemon version or server URL"),
    ),
)]
#[rocket::post("/setting/cloud-init", data = "<body>")]
pub async fn setting_cloud_init(
    auth: SettingAuth,
    pool: &State<PgPool>,
    body: Json<CloudInitBody>,
) -> Result<Json<CloudInitResponse>, Status> {
    use rand::Rng;

    let body = body.into_inner();
    let system = body.system.unwrap_or_else(|| "x86_64-linux".to_string());
    let label = body.label.unwrap_or_else(|| {
        let ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
        match &body.instance_id {
            Some(iid) => format!("cloud-init-{ts}-{}", &iid[..iid.len().min(12)]),
            None => format!("cloud-init-{ts}"),
        }
    });

    let version = if let Some(v) = body.daemon_version {
        v
    } else {
        // Same resolution as /api/update, including delivered-rollout
        // stickiness; no delivery is recorded here — the booted daemon
        // records one on its first /api/update fetch.
        let rollout_version: Option<String> =
            active_rollout_for_cluster(pool.inner(), auth.cluster_id)
                .await
                .map_err(|_| Status::InternalServerError)?
                .and_then(|r| r.target_version);
        let pinned: Option<String> =
            sqlx::query_scalar("SELECT pinned_version FROM clusters WHERE id = $1")
                .bind(auth.cluster_id)
                .fetch_optional(pool.inner())
                .await
                .map_err(|_| Status::InternalServerError)?
                .flatten();
        // Final fallback: highest semver in daemon_versions (mirrors
        // get_update_target — channel versions like "rolling" are opt-in
        // only). Operators sync this table from xzar via the admin UI.
        let latest: Option<String> = sqlx::query_scalar(
            "SELECT version FROM daemon_versions \
             WHERE version ~ '^[0-9]+(\\.[0-9]+)*$' \
             ORDER BY string_to_array(version, '.')::int[] DESC LIMIT 1",
        )
        .fetch_optional(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
        rollout_version
            .or(pinned)
            .or(latest)
            .ok_or(Status::UnprocessableEntity)?
    };

    let server_url = body
        .server_url
        .unwrap_or_else(|| crate::config::config().api.external_url.clone());
    let server_url = server_url.trim_end_matches('/').to_string();

    let raw_token: String = hex::encode(rand::rng().random::<[u8; 32]>());
    let hash = hex::encode(Sha256::digest(raw_token.as_bytes()));
    sqlx::query(
        "INSERT INTO tokens (cluster_id, token_hash, label, kind) VALUES ($1, $2, $3, 'sync')",
    )
    .bind(auth.cluster_id)
    .bind(&hash)
    .bind(&label)
    .execute(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    let yaml = render_cloud_init(
        &server_url,
        &raw_token,
        &version,
        &system,
        body.host_key_pem.as_deref(),
    );
    Ok(Json(CloudInitResponse {
        cloud_init: yaml,
        sync_token: raw_token,
        instance_id: body.instance_id,
    }))
}

pub(crate) fn render_cloud_init(
    server_url: &str,
    token: &str,
    version: &str,
    system: &str,
    host_key_pem: Option<&str>,
) -> String {
    let download_url = format!("{server_url}/api/daemon-download/{version}/{system}");
    let host_key_block = match host_key_pem {
        Some(pem) => {
            let indented = pem
                .lines()
                .map(|l| format!("      {l}"))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "  - path: /root/.config/mac-mgmt/host_ed25519_key
    owner: root:root
    permissions: '0600'
    content: |
{indented}
"
            )
        }
        None => String::new(),
    };
    format!(
        "#cloud-config
packages:
  - curl
  - ca-certificates
  - xz-utils
  - wget

write_files:
  - path: /root/.config/mac-mgmt/config.toml
    owner: root:root
    permissions: '0600'
    content: |
      [server]
      url = \"{server_url}\"
      token = \"{token}\"
{host_key_block}
runcmd:
  - [ curl, -fsSL, -o, /usr/local/bin/mac-mgmt, \"{download_url}\" ]
  - [ chmod, \"0755\", /usr/local/bin/mac-mgmt ]
  - [ env, HOME=/root, /usr/local/bin/mac-mgmt, setup ]
  - [ systemctl, daemon-reload ]
  - [ systemctl, enable, --now, mac-mgmt.service ]
"
    )
}

// ── Proxy token creation ─────────────────────────────────────────────

/// Valid proxy token scopes.
const VALID_SCOPES: &[&str] = &[
    "files:read",
    "files:write",
    "shell:exec",
    "logs:read",
    "tcp:*",
];

/// Check if a scope string is valid (exact match from VALID_SCOPES, or tcp:{name}).
fn is_valid_scope(s: &str) -> bool {
    VALID_SCOPES.contains(&s) || (s.starts_with("tcp:") && s.len() > 4 && s != "tcp:*")
}

#[derive(Deserialize, ToSchema)]
pub struct CreateProxyTokenBody {
    /// Scopes for the proxy token. If empty or omitted, defaults to all scopes.
    /// Valid scopes: files:read, files:write, shell:exec, logs:read, tcp:*, tcp:{name}
    #[serde(default)]
    pub scopes: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub struct ProxyTokenResponse {
    pub proxy_token: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    /// The scopes granted to this token. Empty means all scopes (wildcard).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
}

/// Create a short-lived proxy token for browser-based tunnel access.
/// Accepts admin or setting tokens. The proxy token inherits the cluster
/// scope of the creating token. Optionally accepts a list of scopes to
/// restrict the token's capabilities.
#[utoipa::path(
    post,
    path = "/api/proxy-token",
    tag = "Common",
    summary = "Create a temporary proxy token",
    description = "Creates a short-lived token (6 hours) for accessing tunnels through the relay proxy. \
                   Optionally specify scopes to restrict access (e.g. files:read, tcp:ollama).",
    security(("bearer" = [])),
    request_body(content = CreateProxyTokenBody, description = "Optional scopes"),
    responses(
        (status = 201, description = "Proxy token created", body = ProxyTokenResponse),
        (status = 400, description = "Invalid scope"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
)]
#[rocket::post("/proxy-token", data = "<body>")]
pub async fn create_proxy_token(
    auth: AuthenticatedToken,
    pool: &State<PgPool>,
    body: Json<CreateProxyTokenBody>,
) -> Result<(Status, Json<ProxyTokenResponse>), (Status, &'static str)> {
    use rand::Rng;
    use sha2::{Digest, Sha256};

    if auth.token_kind != "admin" && auth.token_kind != "setting" {
        return Err((Status::Forbidden, "admin or setting token required"));
    }

    // Validate requested scopes.
    for scope in &body.scopes {
        if !is_valid_scope(scope) {
            return Err((Status::BadRequest, "invalid scope"));
        }
    }

    // If the creating token is itself a proxy token with scopes, the new token's
    // scopes must be a subset. (Admin/setting tokens can grant any scope.)
    if auth.token_kind == "proxy" {
        if let Some(ref parent_scopes) = auth.scopes {
            if !parent_scopes.is_empty() {
                for scope in &body.scopes {
                    let allowed = parent_scopes.iter().any(|ps| {
                        ps == "*" || ps == scope || (scope.starts_with("tcp:") && ps == "tcp:*")
                    });
                    if !allowed {
                        return Err((Status::Forbidden, "scope exceeds parent token"));
                    }
                }
            }
        }
    }

    let raw_token: String = hex::encode(rand::rng().random::<[u8; 32]>());
    let hash = hex::encode(Sha256::digest(raw_token.as_bytes()));
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(24);
    let scopes_json: Option<serde_json::Value> = if body.scopes.is_empty() {
        None
    } else {
        Some(serde_json::json!(&body.scopes))
    };

    sqlx::query(
        "INSERT INTO tokens (cluster_id, organization_id, token_hash, label, kind, expires_at, scopes) \
         VALUES ($1, $2, $3, 'proxy', 'proxy', $4, $5)",
    )
    .bind(auth.cluster_id)
    .bind(auth.organization_id)
    .bind(&hash)
    .bind(expires_at)
    .bind(&scopes_json)
    .execute(pool.inner())
    .await
    .map_err(|_| (Status::InternalServerError, "database error"))?;

    Ok((
        Status::Created,
        Json(ProxyTokenResponse {
            proxy_token: raw_token,
            expires_at,
            scopes: body.into_inner().scopes,
        }),
    ))
}

// ── Relay URLs (portal picker) ───────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub struct RelayUrlRow {
    pub cluster_id: Uuid,
    pub instance_id: String,
    pub hostname: Option<String>,
    /// Tunnel (service) name the URL points at, e.g. `openclaw`.
    pub tunnel: String,
    /// Browser-reachable relay portal URL for this tunnel.
    pub url: String,
    /// The instance's relay proxy base URL (no tunnel subdomain).
    pub relay_proxy_url: String,
    pub reported_at: DateTime<Utc>,
}

/// Build the relay portal URL for one tunnel: `{scheme}{iid12}-{tunnel}.{proxy-host}`
/// — the same subdomain scheme the relay's `parse_subdomain` expects.
fn relay_tunnel_url(proxy_url: &str, instance_id: &str, tunnel: &str) -> String {
    let scheme = if proxy_url.starts_with("https://") {
        "https://"
    } else {
        "http://"
    };
    let host = proxy_url
        .strip_prefix(scheme)
        .unwrap_or(proxy_url)
        .trim_end_matches('/');
    let prefix: String = instance_id.chars().take(12).collect();
    format!("{scheme}{prefix}-{tunnel}.{host}")
}

/// List every relay portal URL the calling token can reach. Scope comes from
/// the token itself: cluster tokens see their cluster, org tokens their
/// organization's clusters, admin tokens everything. Proxy tokens with
/// explicit scopes only see tunnels their `tcp:*`/`tcp:{name}` scopes cover.
#[utoipa::path(
    get,
    path = "/api/relay-urls",
    tag = "Common",
    summary = "List relay URLs reachable with the calling token",
    description = "Lists relay portal URLs (one per instance tunnel) for every cluster the bearer \
                   token can access. Intended as a picker/easy-access API for building portals.",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Relay URLs", body = Vec<RelayUrlRow>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Token has no cluster scope"),
    ),
)]
#[rocket::get("/relay-urls")]
pub async fn list_relay_urls(
    auth: AuthenticatedToken,
    pool: &State<PgPool>,
) -> Result<Json<Vec<RelayUrlRow>>, (Status, &'static str)> {
    #[derive(sqlx::FromRow)]
    struct Row {
        cluster_id: Uuid,
        instance_id: String,
        hostname: Option<String>,
        relay_proxy_url: String,
        tunnels: Option<serde_json::Value>,
        reported_at: DateTime<Utc>,
    }

    const BASE: &str = "SELECT cluster_id, instance_id, hostname, relay_proxy_url, tunnels, reported_at \
         FROM daemon_heartbeats WHERE relay_proxy_url IS NOT NULL AND relay_proxy_url <> ''";

    let rows: Vec<Row> = if let Some(cid) = auth.cluster_id {
        sqlx::query_as(&format!(
            "{BASE} AND cluster_id = $1 ORDER BY reported_at DESC"
        ))
        .bind(cid)
        .fetch_all(pool.inner())
        .await
    } else if let Some(org_id) = auth.organization_id {
        sqlx::query_as(&format!(
            "{BASE} AND cluster_id IN \
             (SELECT cluster_id FROM organization_clusters WHERE organization_id = $1) \
             ORDER BY reported_at DESC"
        ))
        .bind(org_id)
        .fetch_all(pool.inner())
        .await
    } else if auth.token_kind == "admin" {
        sqlx::query_as(&format!("{BASE} ORDER BY reported_at DESC"))
            .fetch_all(pool.inner())
            .await
    } else {
        return Err((Status::Forbidden, "token has no cluster scope"));
    }
    .map_err(|_| (Status::InternalServerError, "database error"))?;

    let scope_allows = |tunnel: &str| match &auth.scopes {
        Some(scopes) if !scopes.is_empty() => scopes
            .iter()
            .any(|s| s == "*" || s == "tcp:*" || s == &format!("tcp:{tunnel}")),
        _ => true,
    };

    let mut out = Vec::new();
    for r in rows {
        let names: Vec<String> = r
            .tunnels
            .as_ref()
            .and_then(|t| t.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|t| t.get("name").and_then(|v| v.as_str()).map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        for name in names {
            if !scope_allows(&name) {
                continue;
            }
            out.push(RelayUrlRow {
                url: relay_tunnel_url(&r.relay_proxy_url, &r.instance_id, &name),
                cluster_id: r.cluster_id,
                instance_id: r.instance_id.clone(),
                hostname: r.hostname.clone(),
                tunnel: name,
                relay_proxy_url: r.relay_proxy_url.clone(),
                reported_at: r.reported_at,
            });
        }
    }
    Ok(Json(out))
}

// ── Admin — Skill MCP dependencies ───────────────────────────────────

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub(crate) struct SkillMcpDepRow {
    id: Uuid,
    skill_channel_id: Uuid,
    mcp_server_id: Uuid,
    mcp_server_slug: String,
    mcp_server_name: String,
    mcp_server_description: String,
}

#[utoipa::path(
    get,
    path = "/api/admin/skill-channels/{skill_channel_id}/mcp-dependencies",
    tag = "Admin",
    summary = "List MCP server dependencies for a skill channel",
    security(("bearer" = [])),
    params(("skill_channel_id" = Uuid, Path, description = "Skill channel ID")),
    responses(
        (status = 200, description = "List of MCP dependencies", body = Vec<SkillMcpDepRow>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin token required"),
    ),
)]
#[rocket::get("/admin/skill-channels/<skill_channel_id>/mcp-dependencies")]
pub async fn admin_list_skill_mcp_deps(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    skill_channel_id: &str,
) -> Result<Json<Vec<SkillMcpDepRow>>, Status> {
    let sc_id: Uuid = skill_channel_id.parse().map_err(|_| Status::BadRequest)?;
    let rows = sqlx::query_as::<_, SkillMcpDepRow>(
        "SELECT smd.id, smd.skill_channel_id, smd.mcp_server_id, ms.slug AS mcp_server_slug, ms.name AS mcp_server_name, ms.description AS mcp_server_description \
         FROM skill_mcp_dependencies smd \
         JOIN mcp_servers ms ON ms.id = smd.mcp_server_id \
         WHERE smd.skill_channel_id = $1 \
         ORDER BY ms.slug",
    )
    .bind(sc_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(rows))
}

#[derive(Deserialize, ToSchema)]
pub struct AddSkillMcpDepBody {
    mcp_server_id: Uuid,
}

#[utoipa::path(
    post,
    path = "/api/admin/skill-channels/{skill_channel_id}/mcp-dependencies",
    tag = "Admin",
    summary = "Add MCP server dependency to a skill channel",
    security(("bearer" = [])),
    params(("skill_channel_id" = Uuid, Path, description = "Skill channel ID")),
    request_body = AddSkillMcpDepBody,
    responses(
        (status = 201, description = "Dependency added"),
        (status = 400, description = "Invalid ID"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin token required"),
        (status = 409, description = "Dependency already exists"),
    ),
)]
#[rocket::post(
    "/admin/skill-channels/<skill_channel_id>/mcp-dependencies",
    data = "<body>"
)]
pub async fn admin_add_skill_mcp_dep(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    federation_push: &State<crate::api::push::FederationPushChannel>,
    skill_channel_id: &str,
    body: Json<AddSkillMcpDepBody>,
) -> Result<Status, Status> {
    let sc_id: Uuid = skill_channel_id.parse().map_err(|_| Status::BadRequest)?;
    let res = sqlx::query(
        "INSERT INTO skill_mcp_dependencies (skill_channel_id, mcp_server_id) VALUES ($1, $2)",
    )
    .bind(sc_id)
    .bind(body.mcp_server_id)
    .execute(pool.inner())
    .await;

    match res {
        Ok(_) => {
            crate::api::push::notify_federation(federation_push.inner());
            Ok(Status::Created)
        }
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Err(Status::Conflict),
        Err(sqlx::Error::Database(e)) if e.is_foreign_key_violation() => Err(Status::BadRequest),
        Err(_) => Err(Status::InternalServerError),
    }
}

#[utoipa::path(
    delete,
    path = "/api/admin/skill-channels/{skill_channel_id}/mcp-dependencies/{dep_id}",
    tag = "Admin",
    summary = "Remove MCP server dependency from a skill channel",
    security(("bearer" = [])),
    params(
        ("skill_channel_id" = Uuid, Path, description = "Skill channel ID"),
        ("dep_id" = Uuid, Path, description = "Dependency row ID"),
    ),
    responses(
        (status = 200, description = "Dependency removed"),
        (status = 400, description = "Invalid ID"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin token required"),
        (status = 404, description = "Not found"),
    ),
)]
#[rocket::delete("/admin/skill-channels/<skill_channel_id>/mcp-dependencies/<dep_id>")]
pub async fn admin_remove_skill_mcp_dep(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    federation_push: &State<crate::api::push::FederationPushChannel>,
    skill_channel_id: &str,
    dep_id: &str,
) -> Result<Status, Status> {
    let sc_id: Uuid = skill_channel_id.parse().map_err(|_| Status::BadRequest)?;
    let d_id: Uuid = dep_id.parse().map_err(|_| Status::BadRequest)?;
    let res =
        sqlx::query("DELETE FROM skill_mcp_dependencies WHERE id = $1 AND skill_channel_id = $2")
            .bind(d_id)
            .bind(sc_id)
            .execute(pool.inner())
            .await
            .map_err(|_| Status::InternalServerError)?;

    if res.rows_affected() == 0 {
        Err(Status::NotFound)
    } else {
        crate::api::push::notify_federation(federation_push.inner());
        Ok(Status::Ok)
    }
}

// ── Admin — skill channel nix packages ─────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/admin/skill-channels/{skill_channel_id}/nix-packages",
    tag = "Admin — Skills",
    summary = "Get nix packages for a skill channel",
    security(("bearer" = [])),
    params(("skill_channel_id" = Uuid, Path)),
    responses(
        (status = 200, description = "Nix packages list", body = Vec<String>),
    ),
)]
#[rocket::get("/admin/skill-channels/<skill_channel_id>/nix-packages")]
pub async fn admin_get_skill_nix_packages(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    skill_channel_id: &str,
) -> Result<Json<Vec<String>>, Status> {
    let sc_id: Uuid = skill_channel_id.parse().map_err(|_| Status::BadRequest)?;
    let pkgs: Vec<String> =
        sqlx::query_scalar("SELECT unnest(nix_packages) FROM skill_channels WHERE id = $1")
            .bind(sc_id)
            .fetch_all(pool.inner())
            .await
            .map_err(|_| Status::InternalServerError)?;

    Ok(Json(pkgs))
}

#[derive(Deserialize)]
pub(crate) struct SetNixPackagesBody {
    packages: Vec<String>,
}

#[utoipa::path(
    put,
    path = "/api/admin/skill-channels/{skill_channel_id}/nix-packages",
    tag = "Admin — Skills",
    summary = "Set nix packages for a skill channel",
    security(("bearer" = [])),
    params(("skill_channel_id" = Uuid, Path)),
    responses(
        (status = 200, description = "Updated"),
        (status = 404, description = "Skill channel not found"),
    ),
)]
#[rocket::put(
    "/admin/skill-channels/<skill_channel_id>/nix-packages",
    data = "<body>"
)]
pub async fn admin_set_skill_nix_packages(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    federation_push: &State<crate::api::push::FederationPushChannel>,
    skill_channel_id: &str,
    body: Json<SetNixPackagesBody>,
) -> Result<Status, Status> {
    let sc_id: Uuid = skill_channel_id.parse().map_err(|_| Status::BadRequest)?;
    let res = sqlx::query("UPDATE skill_channels SET nix_packages = $1 WHERE id = $2")
        .bind(&body.packages)
        .bind(sc_id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;

    if res.rows_affected() == 0 {
        return Err(Status::NotFound);
    }

    crate::api::push::notify_federation(federation_push.inner());
    push::notify_skill_channel_clusters(channels, pool.inner(), &[sc_id]).await;
    Ok(Status::Ok)
}

// ── SSH key routes ─────────────────────────────────────────────────────

pub(crate) use mac_mgmt_common::SshKeySyncEntry;

#[utoipa::path(
    get,
    path = "/api/ssh-keys",
    tag = "Sync",
    summary = "List SSH public keys for daemon sync",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "SSH public keys"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Sync token required"),
    ),
)]
#[rocket::get("/ssh-keys")]
pub async fn get_ssh_keys(
    auth: SyncAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<SshKeySyncEntry>>, Status> {
    let keys = sqlx::query_scalar::<_, String>(
        "SELECT public_key FROM cluster_ssh_keys WHERE cluster_id = $1",
    )
    .bind(auth.cluster_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    Ok(Json(
        keys.into_iter()
            .map(|k| SshKeySyncEntry { public_key: k })
            .collect(),
    ))
}

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub(crate) struct SshKeyRow {
    id: Uuid,
    fingerprint: String,
    comment: String,
    created_at: DateTime<Utc>,
}

#[utoipa::path(
    get,
    path = "/api/setting/ssh-keys",
    tag = "Setting — SSH Keys",
    summary = "List cluster SSH keys",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "SSH keys", body = Vec<SshKeyRow>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::get("/setting/ssh-keys")]
pub async fn setting_list_ssh_keys(
    auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<SshKeyRow>>, Status> {
    let rows = sqlx::query_as::<_, SshKeyRow>(
        "SELECT id, fingerprint, comment, created_at \
         FROM cluster_ssh_keys WHERE cluster_id = $1 ORDER BY created_at",
    )
    .bind(auth.cluster_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(rows))
}

#[derive(Deserialize, ToSchema)]
pub struct AddSshKeyBody {
    public_key: String,
}

fn parse_ssh_public_key(raw: &str) -> Result<(String, String), Status> {
    let parts: Vec<&str> = raw.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(Status::UnprocessableEntity);
    }
    let b64_data = base64::engine::general_purpose::STANDARD
        .decode(parts[1])
        .map_err(|_| Status::UnprocessableEntity)?;
    let fingerprint = format!(
        "SHA256:{}",
        base64::engine::general_purpose::STANDARD.encode(Sha256::digest(&b64_data))
    );
    let comment = if parts.len() > 2 {
        parts[2..].join(" ")
    } else {
        String::new()
    };
    Ok((fingerprint, comment))
}

#[utoipa::path(
    post,
    path = "/api/setting/ssh-keys",
    tag = "Setting — SSH Keys",
    summary = "Add an SSH public key",
    security(("bearer" = [])),
    request_body = AddSshKeyBody,
    responses(
        (status = 201, description = "SSH key added"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
        (status = 409, description = "Key already exists"),
        (status = 422, description = "Invalid SSH public key"),
    ),
)]
#[rocket::post("/setting/ssh-keys", data = "<body>")]
pub async fn setting_add_ssh_key(
    auth: SettingAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    body: Json<AddSshKeyBody>,
) -> Result<Status, Status> {
    let trimmed = body.public_key.trim();
    let (fingerprint, comment) = parse_ssh_public_key(trimmed)?;

    sqlx::query(
        "INSERT INTO cluster_ssh_keys (cluster_id, public_key, comment, fingerprint) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(auth.cluster_id)
    .bind(trimmed)
    .bind(&comment)
    .bind(&fingerprint)
    .execute(pool.inner())
    .await
    .map_err(|e| {
        if e.to_string().contains("unique constraint") || e.to_string().contains("duplicate key") {
            Status::Conflict
        } else {
            Status::InternalServerError
        }
    })?;

    push::notify(channels, auth.cluster_id, PushMessage::SyncSshKeys).await;
    Ok(Status::Created)
}

#[utoipa::path(
    delete,
    path = "/api/setting/ssh-keys/{id}",
    tag = "Setting — SSH Keys",
    summary = "Remove an SSH public key",
    security(("bearer" = [])),
    params(("id" = Uuid, Path, description = "SSH key ID")),
    responses(
        (status = 204, description = "SSH key removed"),
        (status = 400, description = "Invalid UUID"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::delete("/setting/ssh-keys/<id>")]
pub async fn setting_remove_ssh_key(
    auth: SettingAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    id: &str,
) -> Result<Status, Status> {
    let uuid: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    sqlx::query("DELETE FROM cluster_ssh_keys WHERE id = $1 AND cluster_id = $2")
        .bind(uuid)
        .bind(auth.cluster_id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    push::notify(channels, auth.cluster_id, PushMessage::SyncSshKeys).await;
    Ok(Status::NoContent)
}

// ── Client Certificates (setting + admin + org + relay validation) ──
//
// All certificates/CAs live in the unified `client_certificates` table with
// columns: scope ('admin'|'organization'|'cluster'), scope_id, is_ca, fingerprint,
// certificate_pem, label.

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct ClientCertRow {
    pub id: Uuid,
    pub fingerprint: String,
    pub label: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize, ToSchema)]
pub struct AddClientCertBody {
    #[serde(default)]
    pub fingerprint: Option<String>,
    #[serde(default)]
    pub certificate_pem: Option<String>,
    #[serde(default)]
    pub label: String,
}

#[derive(Deserialize, ToSchema)]
pub struct AddClientCaBody {
    pub certificate_pem: String,
    #[serde(default)]
    pub label: String,
}

/// Compute SHA-256 fingerprint from a PEM-encoded certificate.
#[cfg(any(feature = "server", feature = "server-api-only"))]
fn fingerprint_from_pem(pem: &str) -> Result<String, Status> {
    fingerprint_from_pem_str(pem).map_err(|_| Status::BadRequest)
}

/// Compute SHA-256 fingerprint from PEM (public, for use in web server functions).
#[cfg(any(feature = "server", feature = "server-api-only"))]
pub fn fingerprint_from_pem_str(pem: &str) -> Result<String, &'static str> {
    let der = pem_to_der(pem).map_err(|_| "invalid PEM")?;
    use sha2::Digest;
    let hash = sha2::Sha256::digest(&der);
    Ok(format!("sha256:{}", hex::encode(hash)))
}

/// Resolve fingerprint from an `AddClientCertBody`. Either the fingerprint is
/// provided directly, or it is computed from the PEM.
#[cfg(any(feature = "server", feature = "server-api-only"))]
fn resolve_fingerprint(body: &AddClientCertBody) -> Result<(String, Option<String>), Status> {
    match (&body.fingerprint, &body.certificate_pem) {
        (Some(fp), pem) => Ok((fp.trim().to_lowercase(), pem.clone())),
        (None, Some(pem)) => {
            let fp = fingerprint_from_pem(pem)?;
            Ok((fp, Some(pem.clone())))
        }
        (None, None) => Err(Status::BadRequest),
    }
}

// ── Setting (cluster) Client Certificates ──────────────────────────

#[utoipa::path(
    get,
    path = "/api/setting/client-certs",
    tag = "Setting — Client Certificates",
    summary = "List cluster client certificates",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Client certs", body = Vec<ClientCertRow>),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[rocket::get("/setting/client-certs")]
pub async fn setting_list_client_certs(
    auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<ClientCertRow>>, Status> {
    let rows = sqlx::query_as::<_, ClientCertRow>(
        "SELECT id, fingerprint, label, created_at \
         FROM client_certificates \
         WHERE scope = 'cluster' AND scope_id = $1 AND is_ca = false \
         ORDER BY created_at",
    )
    .bind(auth.cluster_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(rows))
}

#[utoipa::path(
    post,
    path = "/api/setting/client-certs",
    tag = "Setting — Client Certificates",
    summary = "Add a client certificate",
    security(("bearer" = [])),
    request_body = AddClientCertBody,
    responses(
        (status = 201, description = "Client cert added"),
        (status = 401, description = "Unauthorized"),
        (status = 409, description = "Certificate already exists"),
    ),
)]
#[rocket::post("/setting/client-certs", data = "<body>")]
pub async fn setting_add_client_cert(
    auth: SettingAuth,
    pool: &State<PgPool>,
    body: Json<AddClientCertBody>,
) -> Result<Status, Status> {
    let (fingerprint, pem) = resolve_fingerprint(&body)?;
    let label = body.label.trim().to_string();

    sqlx::query(
        "INSERT INTO client_certificates (scope, scope_id, is_ca, fingerprint, certificate_pem, label) \
         VALUES ('cluster', $1, false, $2, $3, $4)",
    )
    .bind(auth.cluster_id)
    .bind(&fingerprint)
    .bind(&pem)
    .bind(&label)
    .execute(pool.inner())
    .await
    .map_err(|e| {
        if e.to_string().contains("unique constraint") || e.to_string().contains("duplicate key") {
            Status::Conflict
        } else {
            Status::InternalServerError
        }
    })?;

    Ok(Status::Created)
}

#[utoipa::path(
    delete,
    path = "/api/setting/client-certs/{id}",
    tag = "Setting — Client Certificates",
    summary = "Remove a client certificate",
    security(("bearer" = [])),
    params(("id" = Uuid, Path, description = "Client cert ID")),
    responses(
        (status = 204, description = "Client cert removed"),
        (status = 400, description = "Invalid UUID"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[rocket::delete("/setting/client-certs/<id>")]
pub async fn setting_remove_client_cert(
    auth: SettingAuth,
    pool: &State<PgPool>,
    id: &str,
) -> Result<Status, Status> {
    let uuid: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    sqlx::query(
        "DELETE FROM client_certificates WHERE id = $1 AND scope = 'cluster' AND scope_id = $2",
    )
    .bind(uuid)
    .bind(auth.cluster_id)
    .execute(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Status::NoContent)
}

// ── Setting (cluster) Client CAs ───────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/setting/client-cas",
    tag = "Setting — Client CAs",
    summary = "List cluster client CAs",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Client CAs", body = Vec<ClientCertRow>),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[rocket::get("/setting/client-cas")]
pub async fn setting_list_client_cas(
    auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<ClientCertRow>>, Status> {
    let rows = sqlx::query_as::<_, ClientCertRow>(
        "SELECT id, fingerprint, label, created_at \
         FROM client_certificates \
         WHERE scope = 'cluster' AND scope_id = $1 AND is_ca = true \
         ORDER BY created_at",
    )
    .bind(auth.cluster_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(rows))
}

#[utoipa::path(
    post,
    path = "/api/setting/client-cas",
    tag = "Setting — Client CAs",
    summary = "Add a cluster client CA",
    security(("bearer" = [])),
    request_body = AddClientCaBody,
    responses(
        (status = 201, description = "Client CA added"),
        (status = 401, description = "Unauthorized"),
        (status = 409, description = "CA already exists"),
    ),
)]
#[rocket::post("/setting/client-cas", data = "<body>")]
pub async fn setting_add_client_ca(
    auth: SettingAuth,
    pool: &State<PgPool>,
    body: Json<AddClientCaBody>,
) -> Result<Status, Status> {
    let fingerprint = fingerprint_from_pem(&body.certificate_pem)?;
    let label = body.label.trim().to_string();

    sqlx::query(
        "INSERT INTO client_certificates (scope, scope_id, is_ca, fingerprint, certificate_pem, label) \
         VALUES ('cluster', $1, true, $2, $3, $4)",
    )
    .bind(auth.cluster_id)
    .bind(&fingerprint)
    .bind(&body.certificate_pem)
    .bind(&label)
    .execute(pool.inner())
    .await
    .map_err(|e| {
        if e.to_string().contains("unique constraint") || e.to_string().contains("duplicate key") {
            Status::Conflict
        } else {
            Status::InternalServerError
        }
    })?;

    Ok(Status::Created)
}

#[utoipa::path(
    delete,
    path = "/api/setting/client-cas/{id}",
    tag = "Setting — Client CAs",
    summary = "Remove a cluster client CA",
    security(("bearer" = [])),
    params(("id" = Uuid, Path, description = "Client CA ID")),
    responses(
        (status = 204, description = "Client CA removed"),
        (status = 400, description = "Invalid UUID"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[rocket::delete("/setting/client-cas/<id>")]
pub async fn setting_remove_client_ca(
    auth: SettingAuth,
    pool: &State<PgPool>,
    id: &str,
) -> Result<Status, Status> {
    let uuid: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    sqlx::query(
        "DELETE FROM client_certificates WHERE id = $1 AND scope = 'cluster' AND scope_id = $2 AND is_ca = true",
    )
    .bind(uuid)
    .bind(auth.cluster_id)
    .execute(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Status::NoContent)
}

// ── Admin Client Certificates ───────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/admin/client-certs",
    tag = "Admin — Client Certificates",
    summary = "List admin client certificates",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Admin client certs", body = Vec<ClientCertRow>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin token required"),
    ),
)]
#[rocket::get("/admin/client-certs")]
pub async fn admin_list_client_certs(
    _auth: AdminAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<ClientCertRow>>, Status> {
    let rows = sqlx::query_as::<_, ClientCertRow>(
        "SELECT id, fingerprint, label, created_at \
         FROM client_certificates \
         WHERE scope = 'admin' AND is_ca = false \
         ORDER BY created_at",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(rows))
}

#[utoipa::path(
    post,
    path = "/api/admin/client-certs",
    tag = "Admin — Client Certificates",
    summary = "Add an admin client certificate",
    security(("bearer" = [])),
    request_body = AddClientCertBody,
    responses(
        (status = 201, description = "Admin client cert added"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin token required"),
        (status = 409, description = "Certificate already exists"),
    ),
)]
#[rocket::post("/admin/client-certs", data = "<body>")]
pub async fn admin_add_client_cert(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    body: Json<AddClientCertBody>,
) -> Result<Status, Status> {
    let (fingerprint, pem) = resolve_fingerprint(&body)?;
    let label = body.label.trim().to_string();

    sqlx::query(
        "INSERT INTO client_certificates (scope, scope_id, is_ca, fingerprint, certificate_pem, label) \
         VALUES ('admin', NULL, false, $1, $2, $3)",
    )
    .bind(&fingerprint)
    .bind(&pem)
    .bind(&label)
    .execute(pool.inner())
    .await
    .map_err(|e| {
        if e.to_string().contains("unique constraint") || e.to_string().contains("duplicate key") {
            Status::Conflict
        } else {
            Status::InternalServerError
        }
    })?;

    Ok(Status::Created)
}

#[utoipa::path(
    delete,
    path = "/api/admin/client-certs/{id}",
    tag = "Admin — Client Certificates",
    summary = "Remove an admin client certificate",
    security(("bearer" = [])),
    params(("id" = Uuid, Path, description = "Admin client cert ID")),
    responses(
        (status = 204, description = "Admin client cert removed"),
        (status = 400, description = "Invalid UUID"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin token required"),
    ),
)]
#[rocket::delete("/admin/client-certs/<id>")]
pub async fn admin_remove_client_cert(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    id: &str,
) -> Result<Status, Status> {
    let uuid: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    sqlx::query(
        "DELETE FROM client_certificates WHERE id = $1 AND scope = 'admin' AND is_ca = false",
    )
    .bind(uuid)
    .execute(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Status::NoContent)
}

// ── Admin Client CAs ────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/admin/client-cas",
    tag = "Admin — Client CAs",
    summary = "List admin client CAs",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Admin client CAs", body = Vec<ClientCertRow>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin token required"),
    ),
)]
#[rocket::get("/admin/client-cas")]
pub async fn admin_list_client_cas(
    _auth: AdminAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<ClientCertRow>>, Status> {
    let rows = sqlx::query_as::<_, ClientCertRow>(
        "SELECT id, fingerprint, label, created_at \
         FROM client_certificates \
         WHERE scope = 'admin' AND is_ca = true \
         ORDER BY created_at",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(rows))
}

#[utoipa::path(
    post,
    path = "/api/admin/client-cas",
    tag = "Admin — Client CAs",
    summary = "Add an admin client CA",
    security(("bearer" = [])),
    request_body = AddClientCaBody,
    responses(
        (status = 201, description = "Admin client CA added"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin token required"),
        (status = 409, description = "CA already exists"),
    ),
)]
#[rocket::post("/admin/client-cas", data = "<body>")]
pub async fn admin_add_client_ca(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    body: Json<AddClientCaBody>,
) -> Result<Status, Status> {
    let fingerprint = fingerprint_from_pem(&body.certificate_pem)?;
    let label = body.label.trim().to_string();

    sqlx::query(
        "INSERT INTO client_certificates (scope, scope_id, is_ca, fingerprint, certificate_pem, label) \
         VALUES ('admin', NULL, true, $1, $2, $3)",
    )
    .bind(&fingerprint)
    .bind(&body.certificate_pem)
    .bind(&label)
    .execute(pool.inner())
    .await
    .map_err(|e| {
        if e.to_string().contains("unique constraint") || e.to_string().contains("duplicate key") {
            Status::Conflict
        } else {
            Status::InternalServerError
        }
    })?;

    Ok(Status::Created)
}

#[utoipa::path(
    delete,
    path = "/api/admin/client-cas/{id}",
    tag = "Admin — Client CAs",
    summary = "Remove an admin client CA",
    security(("bearer" = [])),
    params(("id" = Uuid, Path, description = "Admin client CA ID")),
    responses(
        (status = 204, description = "Admin client CA removed"),
        (status = 400, description = "Invalid UUID"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin token required"),
    ),
)]
#[rocket::delete("/admin/client-cas/<id>")]
pub async fn admin_remove_client_ca(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    id: &str,
) -> Result<Status, Status> {
    let uuid: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    sqlx::query(
        "DELETE FROM client_certificates WHERE id = $1 AND scope = 'admin' AND is_ca = true",
    )
    .bind(uuid)
    .execute(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Status::NoContent)
}

// ── Organization Client Certificates ────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/admin/organizations/{org_id}/client-certs",
    tag = "Organization — Client Certificates",
    summary = "List organization client certificates",
    security(("bearer" = [])),
    params(("org_id" = Uuid, Path, description = "Organization ID")),
    responses(
        (status = 200, description = "Org client certs", body = Vec<ClientCertRow>),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[rocket::get("/admin/organizations/<org_id>/client-certs")]
pub async fn org_list_client_certs(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    org_id: &str,
) -> Result<Json<Vec<ClientCertRow>>, Status> {
    let oid: Uuid = org_id.parse().map_err(|_| Status::BadRequest)?;
    let rows = sqlx::query_as::<_, ClientCertRow>(
        "SELECT id, fingerprint, label, created_at \
         FROM client_certificates \
         WHERE scope = 'organization' AND scope_id = $1 AND is_ca = false \
         ORDER BY created_at",
    )
    .bind(oid)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(rows))
}

#[utoipa::path(
    post,
    path = "/api/admin/organizations/{org_id}/client-certs",
    tag = "Organization — Client Certificates",
    summary = "Add an organization client certificate",
    security(("bearer" = [])),
    params(("org_id" = Uuid, Path, description = "Organization ID")),
    request_body = AddClientCertBody,
    responses(
        (status = 201, description = "Org client cert added"),
        (status = 401, description = "Unauthorized"),
        (status = 409, description = "Certificate already exists"),
    ),
)]
#[rocket::post("/admin/organizations/<org_id>/client-certs", data = "<body>")]
pub async fn org_add_client_cert(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    org_id: &str,
    body: Json<AddClientCertBody>,
) -> Result<Status, Status> {
    let oid: Uuid = org_id.parse().map_err(|_| Status::BadRequest)?;
    let (fingerprint, pem) = resolve_fingerprint(&body)?;
    let label = body.label.trim().to_string();

    sqlx::query(
        "INSERT INTO client_certificates (scope, scope_id, is_ca, fingerprint, certificate_pem, label) \
         VALUES ('organization', $1, false, $2, $3, $4)",
    )
    .bind(oid)
    .bind(&fingerprint)
    .bind(&pem)
    .bind(&label)
    .execute(pool.inner())
    .await
    .map_err(|e| {
        if e.to_string().contains("unique constraint") || e.to_string().contains("duplicate key") {
            Status::Conflict
        } else {
            Status::InternalServerError
        }
    })?;

    Ok(Status::Created)
}

#[utoipa::path(
    delete,
    path = "/api/admin/organizations/{org_id}/client-certs/{id}",
    tag = "Organization — Client Certificates",
    summary = "Remove an organization client certificate",
    security(("bearer" = [])),
    params(
        ("org_id" = Uuid, Path, description = "Organization ID"),
        ("id" = Uuid, Path, description = "Client cert ID"),
    ),
    responses(
        (status = 204, description = "Org client cert removed"),
        (status = 400, description = "Invalid UUID"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[rocket::delete("/admin/organizations/<org_id>/client-certs/<id>")]
pub async fn org_remove_client_cert(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    org_id: &str,
    id: &str,
) -> Result<Status, Status> {
    let oid: Uuid = org_id.parse().map_err(|_| Status::BadRequest)?;
    let uuid: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    sqlx::query(
        "DELETE FROM client_certificates WHERE id = $1 AND scope = 'organization' AND scope_id = $2 AND is_ca = false",
    )
    .bind(uuid)
    .bind(oid)
    .execute(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Status::NoContent)
}

// ── Organization Client CAs ────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/admin/organizations/{org_id}/client-cas",
    tag = "Organization — Client CAs",
    summary = "List organization client CAs",
    security(("bearer" = [])),
    params(("org_id" = Uuid, Path, description = "Organization ID")),
    responses(
        (status = 200, description = "Org client CAs", body = Vec<ClientCertRow>),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[rocket::get("/admin/organizations/<org_id>/client-cas")]
pub async fn org_list_client_cas(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    org_id: &str,
) -> Result<Json<Vec<ClientCertRow>>, Status> {
    let oid: Uuid = org_id.parse().map_err(|_| Status::BadRequest)?;
    let rows = sqlx::query_as::<_, ClientCertRow>(
        "SELECT id, fingerprint, label, created_at \
         FROM client_certificates \
         WHERE scope = 'organization' AND scope_id = $1 AND is_ca = true \
         ORDER BY created_at",
    )
    .bind(oid)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(rows))
}

#[utoipa::path(
    post,
    path = "/api/admin/organizations/{org_id}/client-cas",
    tag = "Organization — Client CAs",
    summary = "Add an organization client CA",
    security(("bearer" = [])),
    params(("org_id" = Uuid, Path, description = "Organization ID")),
    request_body = AddClientCaBody,
    responses(
        (status = 201, description = "Org client CA added"),
        (status = 401, description = "Unauthorized"),
        (status = 409, description = "CA already exists"),
    ),
)]
#[rocket::post("/admin/organizations/<org_id>/client-cas", data = "<body>")]
pub async fn org_add_client_ca(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    org_id: &str,
    body: Json<AddClientCaBody>,
) -> Result<Status, Status> {
    let oid: Uuid = org_id.parse().map_err(|_| Status::BadRequest)?;
    let fingerprint = fingerprint_from_pem(&body.certificate_pem)?;
    let label = body.label.trim().to_string();

    sqlx::query(
        "INSERT INTO client_certificates (scope, scope_id, is_ca, fingerprint, certificate_pem, label) \
         VALUES ('organization', $1, true, $2, $3, $4)",
    )
    .bind(oid)
    .bind(&fingerprint)
    .bind(&body.certificate_pem)
    .bind(&label)
    .execute(pool.inner())
    .await
    .map_err(|e| {
        if e.to_string().contains("unique constraint") || e.to_string().contains("duplicate key") {
            Status::Conflict
        } else {
            Status::InternalServerError
        }
    })?;

    Ok(Status::Created)
}

#[utoipa::path(
    delete,
    path = "/api/admin/organizations/{org_id}/client-cas/{id}",
    tag = "Organization — Client CAs",
    summary = "Remove an organization client CA",
    security(("bearer" = [])),
    params(
        ("org_id" = Uuid, Path, description = "Organization ID"),
        ("id" = Uuid, Path, description = "Client CA ID"),
    ),
    responses(
        (status = 204, description = "Org client CA removed"),
        (status = 400, description = "Invalid UUID"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[rocket::delete("/admin/organizations/<org_id>/client-cas/<id>")]
pub async fn org_remove_client_ca(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    org_id: &str,
    id: &str,
) -> Result<Status, Status> {
    let oid: Uuid = org_id.parse().map_err(|_| Status::BadRequest)?;
    let uuid: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    sqlx::query(
        "DELETE FROM client_certificates WHERE id = $1 AND scope = 'organization' AND scope_id = $2 AND is_ca = true",
    )
    .bind(uuid)
    .bind(oid)
    .execute(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Status::NoContent)
}

// ── Relay certificate auth (fingerprint lookup + CA validation) ─────

#[derive(Deserialize, ToSchema)]
pub struct CertAuthBody {
    certificate_pem: String,
}

#[derive(Serialize, ToSchema)]
pub struct CertAuthResponse {
    pub cluster_ids: Vec<Uuid>,
    pub organization_ids: Vec<Uuid>,
    pub token_kind: String,
}

/// GET /api/cert-auth — backward-compatible fingerprint-only validation.
#[utoipa::path(
    get,
    path = "/api/cert-auth",
    tag = "Relay — Certificate Auth",
    summary = "Validate a client certificate fingerprint (legacy)",
    params(("fingerprint" = String, Query, description = "SHA-256 fingerprint")),
    responses(
        (status = 200, description = "Certificate found", body = CertAuthResponse),
        (status = 404, description = "Certificate not found"),
    ),
)]
#[rocket::get("/cert-auth?<fingerprint>")]
pub async fn cert_auth(
    pool: &State<PgPool>,
    fingerprint: &str,
) -> Result<Json<CertAuthResponse>, Status> {
    let fp = fingerprint.trim().to_lowercase();
    cert_auth_lookup(pool.inner(), &fp, None).await
}

/// POST /api/cert-auth — full PEM certificate validation with CA support.
#[utoipa::path(
    post,
    path = "/api/cert-auth",
    tag = "Relay — Certificate Auth",
    summary = "Validate a client certificate (PEM)",
    request_body = CertAuthBody,
    responses(
        (status = 200, description = "Certificate authorized", body = CertAuthResponse),
        (status = 404, description = "Certificate not authorized"),
    ),
)]
#[rocket::post("/cert-auth", data = "<body>")]
pub async fn cert_auth_post(
    pool: &State<PgPool>,
    body: Json<CertAuthBody>,
) -> Result<Json<CertAuthResponse>, Status> {
    let fp = fingerprint_from_pem(&body.certificate_pem)?;
    cert_auth_lookup(pool.inner(), &fp, Some(&body.certificate_pem)).await
}

/// A scope match from the client_certificates table.
#[cfg(any(feature = "server", feature = "server-api-only"))]
#[derive(sqlx::FromRow)]
struct CertMatch {
    scope: String,
    scope_id: Option<Uuid>,
}

/// Core cert-auth logic shared by GET (legacy) and POST (new).
#[cfg(any(feature = "server", feature = "server-api-only"))]
async fn cert_auth_lookup(
    pool: &PgPool,
    fingerprint: &str,
    certificate_pem: Option<&str>,
) -> Result<Json<CertAuthResponse>, Status> {
    // 1. Direct fingerprint lookup (non-CA entries).
    let matches = sqlx::query_as::<_, CertMatch>(
        "SELECT scope, scope_id FROM client_certificates \
         WHERE fingerprint = $1 AND is_ca = false",
    )
    .bind(fingerprint)
    .fetch_all(pool)
    .await
    .map_err(|_| Status::InternalServerError)?;

    // Backfill PEM if we matched and PEM was provided.
    if !matches.is_empty() {
        if let Some(pem) = certificate_pem {
            let _ = sqlx::query(
                "UPDATE client_certificates SET certificate_pem = $1 \
                 WHERE fingerprint = $2 AND is_ca = false AND certificate_pem IS NULL",
            )
            .bind(pem)
            .bind(fingerprint)
            .execute(pool)
            .await;
        }
    }

    if !matches.is_empty() {
        return build_cert_auth_response(pool, &matches).await;
    }

    // 2. CA validation (if PEM provided and no fingerprint match).
    if let Some(pem) = certificate_pem {
        let ca_matches = validate_against_cas(pool, pem).await?;
        if !ca_matches.is_empty() {
            return build_cert_auth_response(pool, &ca_matches).await;
        }
    }

    Err(Status::NotFound)
}

/// Build a CertAuthResponse from a set of scope matches.
#[cfg(any(feature = "server", feature = "server-api-only"))]
async fn build_cert_auth_response(
    pool: &PgPool,
    matches: &[CertMatch],
) -> Result<Json<CertAuthResponse>, Status> {
    // Check for admin scope first.
    if matches.iter().any(|m| m.scope == "admin") {
        let all_ids = sqlx::query_scalar::<_, Uuid>("SELECT id FROM clusters")
            .fetch_all(pool)
            .await
            .map_err(|_| Status::InternalServerError)?;
        return Ok(Json(CertAuthResponse {
            cluster_ids: all_ids,
            organization_ids: vec![],
            token_kind: "cert_admin".to_string(),
        }));
    }

    let mut cluster_ids = Vec::new();
    let mut organization_ids = Vec::new();

    // Collect organization matches and resolve their clusters.
    for m in matches.iter().filter(|m| m.scope == "organization") {
        if let Some(oid) = m.scope_id {
            organization_ids.push(oid);
            let org_clusters = sqlx::query_scalar::<_, Uuid>(
                "SELECT cluster_id FROM organization_clusters WHERE organization_id = $1",
            )
            .bind(oid)
            .fetch_all(pool)
            .await
            .map_err(|_| Status::InternalServerError)?;
            cluster_ids.extend(org_clusters);
        }
    }

    // Collect direct cluster matches.
    for m in matches.iter().filter(|m| m.scope == "cluster") {
        if let Some(cid) = m.scope_id {
            cluster_ids.push(cid);
        }
    }

    cluster_ids.sort();
    cluster_ids.dedup();
    organization_ids.sort();
    organization_ids.dedup();

    let token_kind = if !organization_ids.is_empty() {
        "cert_organization"
    } else {
        "cert_cluster"
    };

    Ok(Json(CertAuthResponse {
        cluster_ids,
        organization_ids,
        token_kind: token_kind.to_string(),
    }))
}

/// Validate a client certificate PEM against stored CA certificates.
/// Returns matching scopes (like a fingerprint match would).
#[cfg(any(feature = "server", feature = "server-api-only"))]
async fn validate_against_cas(pool: &PgPool, client_pem: &str) -> Result<Vec<CertMatch>, Status> {
    // Parse client certificate.
    let client_der = self::pem_to_der(client_pem).map_err(|_| Status::BadRequest)?;
    let (_, client_cert) =
        x509_parser::parse_x509_certificate(&client_der).map_err(|_| Status::BadRequest)?;

    // Load all CA certificates.
    #[derive(sqlx::FromRow)]
    struct CaRow {
        scope: String,
        scope_id: Option<Uuid>,
        certificate_pem: String,
    }
    let cas = sqlx::query_as::<_, CaRow>(
        "SELECT scope, scope_id, certificate_pem FROM client_certificates WHERE is_ca = true",
    )
    .fetch_all(pool)
    .await
    .map_err(|_| Status::InternalServerError)?;

    let mut matches = Vec::new();
    for ca in &cas {
        let Ok(ca_der) = self::pem_to_der(&ca.certificate_pem) else {
            continue;
        };
        let Ok((_, ca_cert)) = x509_parser::parse_x509_certificate(&ca_der) else {
            continue;
        };
        // Check issuer matches CA subject, then verify signature.
        if client_cert.issuer() == ca_cert.subject() {
            if client_cert
                .verify_signature(Some(ca_cert.public_key()))
                .is_ok()
            {
                matches.push(CertMatch {
                    scope: ca.scope.clone(),
                    scope_id: ca.scope_id,
                });
            }
        }
    }

    Ok(matches)
}

/// Decode PEM to DER bytes.
#[cfg(any(feature = "server", feature = "server-api-only"))]
fn pem_to_der(pem: &str) -> Result<Vec<u8>, ()> {
    let b64: String = pem
        .lines()
        .filter(|l| !l.starts_with("-----"))
        .collect::<Vec<_>>()
        .join("");
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &b64).map_err(|_| ())
}

// ── Rollout Groups (admin) ──────────────────────────────────────────

#[derive(Deserialize, ToSchema)]
pub(crate) struct CreateRolloutGroupBody {
    name: String,
    #[serde(default)]
    description: String,
}

// ── Heartbeat (sync token) ──────────────────────────────────────────

pub(crate) use mac_mgmt_common::HeartbeatBody;

#[utoipa::path(
    post,
    path = "/api/heartbeat",
    tag = "Sync",
    summary = "Report daemon heartbeat",
    description = "Upserts a heartbeat record for the authenticated daemon instance.",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Heartbeat recorded"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Sync token required"),
    ),
)]
#[rocket::post("/heartbeat", data = "<body>")]
pub async fn post_heartbeat(
    auth: SyncAuth,
    pool: &State<PgPool>,
    body: Json<HeartbeatBody>,
) -> Result<Status, Status> {
    // Verify the daemon's cryptographic identity proof.
    verify_heartbeat_signature(&body).map_err(|e| {
        tracing::warn!("heartbeat signature verification failed: {e}");
        Status::Forbidden
    })?;

    let sample_json = body
        .sample
        .as_ref()
        .and_then(|s| serde_json::to_value(s).ok());
    let svc_ext_json = if body.services_extended.is_empty() {
        None
    } else {
        serde_json::to_value(&body.services_extended).ok()
    };
    let svc_samples_json = if body.service_samples.is_empty() {
        None
    } else {
        serde_json::to_value(&body.service_samples).ok()
    };
    let failure_signals_json = if body.failure_signals.is_empty() {
        None
    } else {
        serde_json::to_value(&body.failure_signals).ok()
    };

    sqlx::query(
        "INSERT INTO daemon_heartbeats (cluster_id, instance_id, version, hostname, environment, services, tunnels, relay_proxy_hostname, nixpkgs_commit, sample, services_extended, git_sha, file_tunnels, relay_proxy_url, shell_tunnels, service_samples, failure_signals, chaos) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, EXISTS(SELECT 1 FROM chaos_nodes c WHERE c.cluster_id = $1 AND c.instance_id = $2)) \
         ON CONFLICT (cluster_id, instance_id) \
         DO UPDATE SET version = $3, hostname = $4, environment = $5, services = $6, tunnels = $7, relay_proxy_hostname = $8, nixpkgs_commit = $9, sample = $10, services_extended = $11, git_sha = $12, file_tunnels = $13, relay_proxy_url = $14, shell_tunnels = $15, service_samples = $16, failure_signals = $17, chaos = EXCLUDED.chaos, reported_at = now()",
    )
    .bind(auth.cluster_id)
    .bind(&body.instance_id)
    .bind(&body.version)
    .bind(&body.hostname)
    .bind(&body.environment)
    .bind(&body.services)
    .bind(&body.tunnels)
    .bind(&body.relay_proxy_hostname)
    .bind(&body.nixpkgs_commit)
    .bind(&sample_json)
    .bind(&svc_ext_json)
    .bind(&body.git_sha)
    .bind(&body.file_tunnels)
    .bind(&body.relay_proxy_url)
    .bind(&body.shell_tunnels)
    .bind(&svc_samples_json)
    .bind(&failure_signals_json)
    .execute(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    // Clean up assessment_probes for services no longer reported by this instance.
    // This prevents stale probe data from disabled services showing up in the UI.
    if !body.services_extended.is_empty() {
        let active_services: Vec<String> = body
            .services_extended
            .iter()
            .map(|s| s.name.clone())
            .collect();
        if let Err(e) = sqlx::query(
            "DELETE FROM assessment_probes \
             WHERE instance_id = $1 AND service != ALL($2)",
        )
        .bind(&body.instance_id)
        .bind(&active_services)
        .execute(pool.inner())
        .await
        {
            tracing::debug!("failed to clean up stale probes: {e}");
        }
    }

    Ok(Status::Ok)
}

/// Maximum allowed clock skew for heartbeat signatures (seconds).
const HEARTBEAT_MAX_AGE_SECS: i64 = 120;

/// Verify the ed25519 signature on a heartbeat and check that the public
/// key fingerprint matches the claimed instance_id.
fn verify_heartbeat_signature(body: &HeartbeatBody) -> Result<(), &'static str> {
    use base64::Engine;
    use ed25519_dalek::{Signature, VerifyingKey};
    use sha2::{Digest, Sha256};

    if body.public_key.is_empty() || body.signature.is_empty() {
        return Err("missing public_key or signature");
    }

    // Check timestamp freshness
    let now = chrono::Utc::now().timestamp();
    if (now - body.signed_at).abs() > HEARTBEAT_MAX_AGE_SECS {
        return Err("signed_at timestamp too old or too far in the future");
    }

    // Decode the public key (SSH wire format: 4-byte type length + type + 4-byte key length + key)
    let pk_bytes = base64::engine::general_purpose::STANDARD
        .decode(&body.public_key)
        .map_err(|_| "invalid base64 in public_key")?;

    // Verify fingerprint matches instance_id
    let fingerprint = hex::encode(Sha256::digest(&pk_bytes));
    if fingerprint != body.instance_id {
        return Err("public key fingerprint does not match instance_id");
    }

    // Extract the raw 32-byte ed25519 public key from the SSH wire format.
    // Format: [4-byte len]["ssh-ed25519"][4-byte len][32-byte key]
    let raw_pk =
        extract_ed25519_pubkey(&pk_bytes).ok_or("invalid SSH ed25519 public key format")?;

    let verifying_key =
        VerifyingKey::from_bytes(raw_pk).map_err(|_| "invalid ed25519 public key")?;

    // Decode signature
    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(&body.signature)
        .map_err(|_| "invalid base64 in signature")?;
    let signature =
        Signature::from_slice(&sig_bytes).map_err(|_| "invalid ed25519 signature format")?;

    // Verify signature over "{instance_id}:{signed_at}"
    let message = format!("{}:{}", body.instance_id, body.signed_at);
    use ed25519_dalek::Verifier;
    verifying_key
        .verify(message.as_bytes(), &signature)
        .map_err(|_| "signature verification failed")?;

    Ok(())
}

/// Maximum allowed clock skew for assessment signatures (seconds).
const ASSESSMENT_MAX_AGE_SECS: i64 = 300;

/// Verify an ed25519 signature over "{instance_id}:{signed_at}" for an assessment-
/// style payload (inventory snapshot or probe report). Same format as the heartbeat
/// signature, but with a slightly looser clock-skew budget (5 min) because probe
/// runs can themselves take 30s+ before the body is built.
fn verify_assessment_signature(
    instance_id: &str,
    signed_at: i64,
    public_key: &str,
    signature: &str,
) -> Result<(), &'static str> {
    use ed25519_dalek::{Signature, VerifyingKey};

    if public_key.is_empty() || signature.is_empty() {
        return Err("missing public_key or signature");
    }

    let now = chrono::Utc::now().timestamp();
    if (now - signed_at).abs() > ASSESSMENT_MAX_AGE_SECS {
        return Err("signed_at timestamp too old or too far in the future");
    }

    let pk_bytes = base64::engine::general_purpose::STANDARD
        .decode(public_key)
        .map_err(|_| "invalid base64 in public_key")?;

    let fingerprint = hex::encode(Sha256::digest(&pk_bytes));
    if fingerprint != instance_id {
        return Err("public key fingerprint does not match instance_id");
    }

    let raw_pk =
        extract_ed25519_pubkey(&pk_bytes).ok_or("invalid SSH ed25519 public key format")?;
    let verifying_key =
        VerifyingKey::from_bytes(raw_pk).map_err(|_| "invalid ed25519 public key")?;

    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature)
        .map_err(|_| "invalid base64 in signature")?;
    let signature =
        Signature::from_slice(&sig_bytes).map_err(|_| "invalid ed25519 signature format")?;

    let message = format!("{instance_id}:{signed_at}");
    use ed25519_dalek::Verifier;
    verifying_key
        .verify(message.as_bytes(), &signature)
        .map_err(|_| "signature verification failed")?;

    Ok(())
}

// ── System assessment (sync token) ──────────────────────────────────

pub(crate) use mac_mgmt_common::{Assessment, ProbeReport};

#[utoipa::path(
    post,
    path = "/api/assessment",
    tag = "Sync",
    summary = "Submit system assessment snapshot",
    description = "Stores a full inventory + security-posture snapshot for the authenticated daemon instance.",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Assessment recorded"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Sync token required / signature invalid"),
    ),
)]
#[rocket::post("/assessment", data = "<body>")]
pub async fn post_assessment(
    auth: SyncAuth,
    pool: &State<PgPool>,
    body: Json<Assessment>,
) -> Result<Status, Status> {
    verify_assessment_signature(
        &body.instance_id,
        body.collected_at,
        &body.public_key,
        &body.signature,
    )
    .map_err(|e| {
        tracing::warn!("assessment signature verification failed: {e}");
        Status::Forbidden
    })?;

    let inventory_json = serde_json::to_value(&body.inventory).map_err(|_| Status::BadRequest)?;
    let security_json = serde_json::to_value(&body.security).map_err(|_| Status::BadRequest)?;
    let svc_inv_json = if body.service_inventories.is_empty() {
        None
    } else {
        serde_json::to_value(&body.service_inventories).ok()
    };
    let svc_sec_json = if body.service_security.is_empty() {
        None
    } else {
        serde_json::to_value(&body.service_security).ok()
    };
    let collected_at =
        DateTime::<Utc>::from_timestamp(body.collected_at, 0).ok_or(Status::BadRequest)?;

    sqlx::query(
        "INSERT INTO assessments (cluster_id, instance_id, collected_at, inventory, security, service_inventories, service_security) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(auth.cluster_id)
    .bind(&body.instance_id)
    .bind(collected_at)
    .bind(&inventory_json)
    .bind(&security_json)
    .bind(&svc_inv_json)
    .bind(&svc_sec_json)
    .execute(pool.inner())
    .await
    .map_err(|e| {
        tracing::warn!("assessment insert failed: {e}");
        Status::InternalServerError
    })?;

    Ok(Status::Ok)
}

#[utoipa::path(
    post,
    path = "/api/assessment/probe",
    tag = "Sync",
    summary = "Submit a single probe result",
    description = "Stores one functional-probe result (e.g. full-prompt round-trip against an LLM backend) for the authenticated daemon instance. Used as a rollout gate datasource.",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Probe result recorded"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Sync token required / signature invalid"),
    ),
)]
#[rocket::post("/assessment/probe", data = "<body>")]
pub async fn post_assessment_probe(
    auth: SyncAuth,
    pool: &State<PgPool>,
    body: Json<ProbeReport>,
) -> Result<Status, Status> {
    verify_assessment_signature(
        &body.instance_id,
        body.collected_at,
        &body.public_key,
        &body.signature,
    )
    .map_err(|e| {
        tracing::warn!("probe signature verification failed: {e}");
        Status::Forbidden
    })?;

    let collected_at =
        DateTime::<Utc>::from_timestamp(body.collected_at, 0).ok_or(Status::BadRequest)?;

    sqlx::query(
        "INSERT INTO assessment_probes \
            (cluster_id, instance_id, service, kind, ok, duration_ms, \
             tokens_in, tokens_out, first_token_ms, model, canary_digest, \
             error_class, error_detail, collected_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
    )
    .bind(auth.cluster_id)
    .bind(&body.instance_id)
    .bind(&body.service)
    .bind(&body.kind)
    .bind(body.ok)
    .bind(body.duration_ms as i64)
    .bind(body.tokens_in.map(|v| v as i32))
    .bind(body.tokens_out.map(|v| v as i32))
    .bind(body.first_token_ms.map(|v| v as i64))
    .bind(&body.model)
    .bind(&body.canary_digest)
    .bind(&body.error_class)
    .bind(&body.error_detail)
    .bind(collected_at)
    .execute(pool.inner())
    .await
    .map_err(|e| {
        tracing::warn!("probe insert failed: {e}");
        Status::InternalServerError
    })?;

    Ok(Status::Ok)
}

/// Extract the raw 32-byte ed25519 public key from SSH wire format.
fn extract_ed25519_pubkey(data: &[u8]) -> Option<&[u8; 32]> {
    // SSH wire format: u32 type_len, type_bytes, u32 key_len, key_bytes
    if data.len() < 4 {
        return None;
    }
    let type_len = u32::from_be_bytes(data[..4].try_into().ok()?) as usize;
    let key_start = 4 + type_len;
    if data.len() < key_start + 4 {
        return None;
    }
    let key_len = u32::from_be_bytes(data[key_start..key_start + 4].try_into().ok()?) as usize;
    if key_len != 32 || data.len() < key_start + 4 + 32 {
        return None;
    }
    let key_bytes = &data[key_start + 4..key_start + 4 + 32];
    key_bytes.try_into().ok()
}

// ── Rollout Groups (admin) ──────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/api/admin/rollout-groups",
    tag = "Admin — Rollouts",
    summary = "Create a rollout group",
    security(("bearer" = [])),
    request_body = CreateRolloutGroupBody,
    responses(
        (status = 201, description = "Group created"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[rocket::post("/admin/rollout-groups", data = "<body>")]
pub async fn admin_create_rollout_group(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    body: Json<CreateRolloutGroupBody>,
) -> Result<Status, Status> {
    sqlx::query("INSERT INTO rollout_groups (name, description) VALUES ($1, $2)")
        .bind(&body.name)
        .bind(&body.description)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    Ok(Status::Created)
}

#[derive(Serialize, ToSchema)]
pub(crate) struct RolloutGroupRow {
    id: Uuid,
    name: String,
    description: String,
    member_count: i64,
}

#[utoipa::path(
    get,
    path = "/api/admin/rollout-groups",
    tag = "Admin — Rollouts",
    summary = "List rollout groups",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Rollout groups", body = Vec<RolloutGroupRow>),
    ),
)]
#[rocket::get("/admin/rollout-groups")]
pub async fn admin_list_rollout_groups(
    _auth: AdminAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<RolloutGroupRow>>, Status> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        name: String,
        description: String,
        member_count: i64,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT rg.id, rg.name, rg.description, COUNT(rgm.id) AS member_count \
         FROM rollout_groups rg \
         LEFT JOIN rollout_group_members rgm ON rgm.group_id = rg.id \
         WHERE rg.id != '00000000-0000-0000-0000-000000000000'::uuid \
         GROUP BY rg.id ORDER BY rg.name",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    Ok(Json(
        rows.into_iter()
            .map(|r| RolloutGroupRow {
                id: r.id,
                name: r.name,
                description: r.description,
                member_count: r.member_count,
            })
            .collect(),
    ))
}

#[derive(Deserialize, ToSchema)]
pub(crate) struct AddGroupMemberBody {
    cluster_id: Uuid,
}

#[utoipa::path(
    post,
    path = "/api/admin/rollout-groups/{group_id}/members",
    tag = "Admin — Rollouts",
    summary = "Add cluster to rollout group",
    security(("bearer" = [])),
    responses(
        (status = 201, description = "Member added"),
        (status = 409, description = "Already a member"),
    ),
)]
#[rocket::post("/admin/rollout-groups/<group_id>/members", data = "<body>")]
pub async fn admin_add_group_member(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    group_id: &str,
    body: Json<AddGroupMemberBody>,
) -> Result<Status, Status> {
    let gid: Uuid = group_id.parse().map_err(|_| Status::BadRequest)?;
    if gid == Uuid::nil() {
        // The "All Clusters" sentinel group is implicit — every cluster is
        // a member by virtue of existing. Adding rows would be meaningless and
        // confuses the resolver, which special-cases the nil UUID.
        return Err(Status::Forbidden);
    }
    sqlx::query("INSERT INTO rollout_group_members (group_id, cluster_id) VALUES ($1, $2)")
        .bind(gid)
        .bind(body.cluster_id)
        .execute(pool.inner())
        .await
        .map_err(|e| {
            if e.to_string().contains("duplicate") || e.to_string().contains("unique") {
                Status::Conflict
            } else {
                Status::InternalServerError
            }
        })?;
    Ok(Status::Created)
}

#[utoipa::path(
    delete,
    path = "/api/admin/rollout-groups/{group_id}/members/{cluster_id}",
    tag = "Admin — Rollouts",
    summary = "Remove cluster from rollout group",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Member removed"),
    ),
)]
#[rocket::delete("/admin/rollout-groups/<group_id>/members/<cluster_id>")]
pub async fn admin_remove_group_member(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    group_id: &str,
    cluster_id: &str,
) -> Result<Status, Status> {
    let gid: Uuid = group_id.parse().map_err(|_| Status::BadRequest)?;
    let cid: Uuid = cluster_id.parse().map_err(|_| Status::BadRequest)?;
    sqlx::query("DELETE FROM rollout_group_members WHERE group_id = $1 AND cluster_id = $2")
        .bind(gid)
        .bind(cid)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    Ok(Status::Ok)
}

// ── Rollouts (admin) ────────────────────────────────────────────────

#[derive(Deserialize, ToSchema)]
pub(crate) struct CreateRolloutBody {
    /// Optional human-friendly name for this rollout.
    #[serde(default)]
    name: Option<String>,
    /// Optional target version to roll out (semver, e.g., "0.1.6"). If null,
    /// the rollout only changes the nixpkgs pin.
    #[serde(default)]
    target_version: Option<String>,
    /// Ordered list of group IDs for the rollout stages
    group_ids: Vec<Uuid>,
    /// Optional nixpkgs commit SHA to pin alongside the version. Null leaves
    /// each cluster's existing nixpkgs pin untouched.
    #[serde(default)]
    nixpkgs_commit: Option<String>,
    /// Optional health gate applied to every rollout stage. Null preserves
    /// legacy ungated rollout behavior.
    #[serde(default)]
    health_gate: Option<HealthGate>,
    /// Optional gradual release: over this many minutes from each stage's
    /// start, a growing hash-bucketed fraction of the group's clusters
    /// becomes eligible for the target. Null releases each stage at once.
    #[serde(default)]
    ramp_minutes: Option<i32>,
}

#[utoipa::path(
    post,
    path = "/api/admin/rollouts",
    tag = "Admin — Rollouts",
    summary = "Create a new version rollout",
    description = "Creates a staged rollout to move groups of clusters to a target daemon version. Validates the version is semver and prevents downgrades.",
    security(("bearer" = [])),
    request_body = CreateRolloutBody,
    responses(
        (status = 201, description = "Rollout created", body = String),
        (status = 400, description = "Bad request"),
        (status = 409, description = "Would downgrade some clusters"),
    ),
)]
#[rocket::post("/admin/rollouts", data = "<body>")]
pub async fn admin_create_rollout(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    body: Json<CreateRolloutBody>,
) -> Result<Json<serde_json::Value>, Status> {
    // Normalize target_version to None if empty/whitespace.
    let target_version: Option<String> = body
        .target_version
        .as_ref()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());

    // Validate the target version: either semver, or a non-semver channel
    // version ("rolling") that exists in daemon_versions.
    if let Some(ver) = &target_version {
        let target_parts: Vec<u64> = ver.split('.').filter_map(|p| p.parse().ok()).collect();
        if target_parts.len() < 3 {
            let known: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM daemon_versions WHERE version = $1)",
            )
            .bind(ver)
            .fetch_one(pool.inner())
            .await
            .map_err(|_| Status::InternalServerError)?;
            if !known {
                return Err(Status::BadRequest);
            }
        }
    }

    // Normalize + validate nixpkgs_commit if present: 7-40 hex chars.
    let nixpkgs_commit: Option<String> = body
        .nixpkgs_commit
        .as_ref()
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty());
    if let Some(commit) = &nixpkgs_commit {
        let valid =
            (7..=40).contains(&commit.len()) && commit.chars().all(|c| c.is_ascii_hexdigit());
        if !valid {
            return Err(Status::BadRequest);
        }
    }

    // Require at least one of target_version or nixpkgs_commit.
    if target_version.is_none() && nixpkgs_commit.is_none() {
        return Err(Status::BadRequest);
    }

    if matches!(body.ramp_minutes, Some(m) if m <= 0) {
        return Err(Status::BadRequest);
    }

    // Check which clusters would be skipped (already at higher version).
    // Only meaningful when a target_version is given.
    let skipped_names: Vec<String> = if let Some(ver) = &target_version {
        #[derive(sqlx::FromRow)]
        struct SkippedCluster {
            name: String,
            pinned_version: String,
        }

        // Downgrade detection only makes sense between two semver
        // versions — channel versions ("rolling") have no ordering, so
        // clusters pinned to one (or a rollout targeting one) are never
        // reported as skipped. Numeric array comparison, not string
        // comparison, so "0.10.0" > "0.9.0".
        let skipped = sqlx::query_as::<_, SkippedCluster>(
            "SELECT c.name, c.pinned_version FROM clusters c \
             WHERE c.pinned_version IS NOT NULL \
               AND $1 ~ '^[0-9]+(\\.[0-9]+)*$' \
               AND c.pinned_version ~ '^[0-9]+(\\.[0-9]+)*$' \
               AND string_to_array(c.pinned_version, '.')::int[] > string_to_array($1, '.')::int[] \
               AND ('00000000-0000-0000-0000-000000000000'::uuid = ANY($2) \
                    OR c.id IN (\
                      SELECT rgm.cluster_id FROM rollout_group_members rgm \
                      WHERE rgm.group_id = ANY($2)))",
        )
        .bind(ver)
        .bind(&body.group_ids)
        .fetch_all(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;

        skipped
            .iter()
            .map(|s| format!("{} (v{})", s.name, s.pinned_version))
            .collect()
    } else {
        Vec::new()
    };

    let rollout_id = Uuid::new_v4();

    let mut tx = pool
        .inner()
        .begin()
        .await
        .map_err(|_| Status::InternalServerError)?;

    let name = body
        .name
        .as_ref()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty());
    sqlx::query(
        "INSERT INTO rollouts (id, name, target_version, nixpkgs_commit) VALUES ($1, $2, $3, $4)",
    )
    .bind(rollout_id)
    .bind(&name)
    .bind(&target_version)
    .bind(&nixpkgs_commit)
    .execute(&mut *tx)
    .await
    .map_err(|_| Status::InternalServerError)?;

    if body.group_ids.is_empty() {
        return Err(Status::UnprocessableEntity);
    }

    let health_gate = body
        .health_gate
        .as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(|_| Status::BadRequest)?;

    for (i, group_id) in body.group_ids.iter().enumerate() {
        sqlx::query(
            "INSERT INTO rollout_stages (rollout_id, group_id, stage_order, health_gate, ramp_minutes) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(rollout_id)
        .bind(group_id)
        .bind(i as i32)
        .bind(&health_gate)
        .bind(body.ramp_minutes)
        .execute(&mut *tx)
        .await
        .map_err(|_| Status::InternalServerError)?;
    }

    tx.commit().await.map_err(|_| Status::InternalServerError)?;

    let mut result = serde_json::json!({ "id": rollout_id });
    if !skipped_names.is_empty() {
        result["skipped_clusters"] = serde_json::json!(skipped_names);
    }
    Ok(Json(result))
}

#[derive(Serialize, ToSchema)]
pub(crate) struct RolloutRow {
    id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    stage_count: i64,
}

#[utoipa::path(
    get,
    path = "/api/admin/rollouts",
    tag = "Admin — Rollouts",
    summary = "List rollouts",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Rollout list", body = Vec<RolloutRow>),
    ),
)]
#[rocket::get("/admin/rollouts")]
pub async fn admin_list_rollouts(
    _auth: AdminAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<RolloutRow>>, Status> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        name: Option<String>,
        status: String,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        stage_count: i64,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT r.id, r.name, r.status, r.created_at, r.updated_at, COUNT(rs.id) AS stage_count \
         FROM rollouts r \
         LEFT JOIN rollout_stages rs ON rs.rollout_id = r.id \
         GROUP BY r.id ORDER BY r.created_at DESC",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    Ok(Json(
        rows.into_iter()
            .map(|r| RolloutRow {
                id: r.id,
                name: r.name,
                status: r.status,
                created_at: r.created_at,
                updated_at: r.updated_at,
                stage_count: r.stage_count,
            })
            .collect(),
    ))
}

#[derive(Serialize, ToSchema)]
pub(crate) struct RolloutDetail {
    id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    target_version: Option<String>,
    nixpkgs_commit: Option<String>,
    status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    stages: Vec<StageDetail>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct StageDetail {
    id: Uuid,
    group_name: String,
    stage_order: i32,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    health_gate: Option<HealthGate>,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    /// Gradual-release window in minutes; null = instant release.
    #[serde(skip_serializing_if = "Option::is_none")]
    ramp_minutes: Option<i32>,
}

#[utoipa::path(
    get,
    path = "/api/admin/rollouts/{rollout_id}",
    tag = "Admin — Rollouts",
    summary = "Get rollout detail with stages",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Rollout detail", body = RolloutDetail),
        (status = 404, description = "Not found"),
    ),
)]
#[rocket::get("/admin/rollouts/<rollout_id>")]
pub async fn admin_get_rollout(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    rollout_id: &str,
) -> Result<Json<RolloutDetail>, Status> {
    let rid: Uuid = rollout_id.parse().map_err(|_| Status::BadRequest)?;

    #[derive(sqlx::FromRow)]
    struct RolloutRow2 {
        id: Uuid,
        name: Option<String>,
        target_version: Option<String>,
        nixpkgs_commit: Option<String>,
        status: String,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    }

    let rollout = sqlx::query_as::<_, RolloutRow2>("SELECT id, name, target_version, nixpkgs_commit, status, created_at, updated_at FROM rollouts WHERE id = $1")
        .bind(rid)
        .fetch_optional(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?
        .ok_or(Status::NotFound)?;

    #[derive(sqlx::FromRow)]
    struct StageRow {
        id: Uuid,
        group_name: String,
        stage_order: i32,
        status: String,
        health_gate: Option<serde_json::Value>,
        started_at: Option<DateTime<Utc>>,
        completed_at: Option<DateTime<Utc>>,
        ramp_minutes: Option<i32>,
    }

    let stages = sqlx::query_as::<_, StageRow>(
        "SELECT rs.id, rg.name AS group_name, rs.stage_order, rs.status, rs.health_gate, rs.started_at, rs.completed_at, rs.ramp_minutes \
         FROM rollout_stages rs JOIN rollout_groups rg ON rg.id = rs.group_id \
         WHERE rs.rollout_id = $1 ORDER BY rs.stage_order"
    )
    .bind(rid)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    Ok(Json(RolloutDetail {
        id: rollout.id,
        name: rollout.name,
        target_version: rollout.target_version,
        nixpkgs_commit: rollout.nixpkgs_commit,
        status: rollout.status,
        created_at: rollout.created_at,
        updated_at: rollout.updated_at,
        stages: stages
            .into_iter()
            .map(|s| StageDetail {
                id: s.id,
                group_name: s.group_name,
                stage_order: s.stage_order,
                status: s.status,
                health_gate: s
                    .health_gate
                    .and_then(|gate| serde_json::from_value(gate).ok()),
                started_at: s.started_at,
                completed_at: s.completed_at,
                ramp_minutes: s.ramp_minutes,
            })
            .collect(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/admin/rollouts/{rollout_id}/start",
    tag = "Admin — Rollouts",
    summary = "Start a rollout (set first stage to rolling)",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Rollout started"),
        (status = 404, description = "Not found"),
        (status = 409, description = "Not in pending state"),
    ),
)]
#[rocket::post("/admin/rollouts/<rollout_id>/start")]
pub async fn admin_start_rollout(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    channels: &State<super::push::PushChannels>,
    rollout_id: &str,
) -> Result<Status, Status> {
    let rid: Uuid = rollout_id.parse().map_err(|_| Status::BadRequest)?;

    let status: String = sqlx::query_scalar("SELECT status FROM rollouts WHERE id = $1")
        .bind(rid)
        .fetch_optional(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?
        .ok_or(Status::NotFound)?;

    if status != "pending" {
        return Err(Status::Conflict);
    }

    let mut tx = pool
        .inner()
        .begin()
        .await
        .map_err(|_| Status::InternalServerError)?;

    sqlx::query("UPDATE rollouts SET status = 'rolling', updated_at = now() WHERE id = $1")
        .bind(rid)
        .execute(&mut *tx)
        .await
        .map_err(|_| Status::InternalServerError)?;

    sqlx::query("UPDATE rollout_stages SET status = 'rolling', started_at = now() WHERE rollout_id = $1 AND stage_order = 0")
        .bind(rid)
        .execute(&mut *tx)
        .await
        .map_err(|_| Status::InternalServerError)?;

    tx.commit().await.map_err(|_| Status::InternalServerError)?;
    super::push::notify_rollout_clusters(
        channels.inner(),
        pool.inner(),
        rid,
        super::push::PushMessage::SelfUpdate,
    )
    .await;
    super::push::notify_rollout_clusters(
        channels.inner(),
        pool.inner(),
        rid,
        super::push::PushMessage::SyncNixpkgs,
    )
    .await;
    Ok(Status::Ok)
}

#[utoipa::path(
    post,
    path = "/api/admin/rollouts/{rollout_id}/advance",
    tag = "Admin — Rollouts",
    summary = "Advance to next stage",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Advanced to next stage"),
        (status = 404, description = "Not found"),
        (status = 409, description = "Cannot advance"),
    ),
)]
#[rocket::post("/admin/rollouts/<rollout_id>/advance")]
pub async fn admin_advance_rollout(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    channels: &State<super::push::PushChannels>,
    rollout_id: &str,
) -> Result<Status, Status> {
    let rid: Uuid = rollout_id.parse().map_err(|_| Status::BadRequest)?;

    // Find current rolling stage
    #[derive(sqlx::FromRow)]
    struct StageInfo {
        stage_order: i32,
    }

    let current = sqlx::query_as::<_, StageInfo>(
        "SELECT stage_order FROM rollout_stages WHERE rollout_id = $1 AND status = 'rolling' LIMIT 1"
    )
    .bind(rid)
    .fetch_optional(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?
    .ok_or(Status::Conflict)?;

    let mut tx = pool
        .inner()
        .begin()
        .await
        .map_err(|_| Status::InternalServerError)?;

    // Complete current stage
    sqlx::query("UPDATE rollout_stages SET status = 'completed', completed_at = now() WHERE rollout_id = $1 AND stage_order = $2")
        .bind(rid)
        .bind(current.stage_order)
        .execute(&mut *tx)
        .await
        .map_err(|_| Status::InternalServerError)?;

    // Start next stage
    let next_order = current.stage_order + 1;
    let updated = sqlx::query("UPDATE rollout_stages SET status = 'rolling', started_at = now() WHERE rollout_id = $1 AND stage_order = $2")
        .bind(rid)
        .bind(next_order)
        .execute(&mut *tx)
        .await
        .map_err(|_| Status::InternalServerError)?;

    if updated.rows_affected() == 0 {
        // No more stages — rollout is complete
        sqlx::query("UPDATE rollouts SET status = 'completed', updated_at = now() WHERE id = $1")
            .bind(rid)
            .execute(&mut *tx)
            .await
            .map_err(|_| Status::InternalServerError)?;
    }

    sqlx::query("UPDATE rollouts SET updated_at = now() WHERE id = $1")
        .bind(rid)
        .execute(&mut *tx)
        .await
        .map_err(|_| Status::InternalServerError)?;

    tx.commit().await.map_err(|_| Status::InternalServerError)?;
    super::push::notify_rollout_clusters(
        channels.inner(),
        pool.inner(),
        rid,
        super::push::PushMessage::SelfUpdate,
    )
    .await;
    super::push::notify_rollout_clusters(
        channels.inner(),
        pool.inner(),
        rid,
        super::push::PushMessage::SyncNixpkgs,
    )
    .await;
    Ok(Status::Ok)
}

#[utoipa::path(
    post,
    path = "/api/admin/rollouts/{rollout_id}/pause",
    tag = "Admin — Rollouts",
    summary = "Pause a rollout",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Rollout paused"),
        (status = 409, description = "No rolling stage"),
    ),
)]
#[rocket::post("/admin/rollouts/<rollout_id>/pause")]
pub async fn admin_pause_rollout(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    rollout_id: &str,
) -> Result<Status, Status> {
    let rid: Uuid = rollout_id.parse().map_err(|_| Status::BadRequest)?;

    let mut tx = pool
        .inner()
        .begin()
        .await
        .map_err(|_| Status::InternalServerError)?;

    let updated = sqlx::query(
        "UPDATE rollout_stages SET status = 'paused' WHERE rollout_id = $1 AND status = 'rolling'",
    )
    .bind(rid)
    .execute(&mut *tx)
    .await
    .map_err(|_| Status::InternalServerError)?;

    if updated.rows_affected() == 0 {
        return Err(Status::Conflict);
    }

    sqlx::query("UPDATE rollouts SET status = 'paused', updated_at = now() WHERE id = $1")
        .bind(rid)
        .execute(&mut *tx)
        .await
        .map_err(|_| Status::InternalServerError)?;

    tx.commit().await.map_err(|_| Status::InternalServerError)?;
    Ok(Status::Ok)
}

#[utoipa::path(
    post,
    path = "/api/admin/rollouts/{rollout_id}/complete",
    tag = "Admin — Rollouts",
    summary = "Complete a rollout and persist config to all targeted clusters",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Rollout completed"),
        (status = 404, description = "Not found"),
    ),
)]
#[rocket::post("/admin/rollouts/<rollout_id>/complete")]
pub async fn admin_complete_rollout(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    channels: &State<super::push::PushChannels>,
    rollout_id: &str,
) -> Result<Status, Status> {
    let rid: Uuid = rollout_id.parse().map_err(|_| Status::BadRequest)?;

    // Get rollout target
    let (target_version, nixpkgs_commit): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT target_version, nixpkgs_commit FROM rollouts WHERE id = $1")
            .bind(rid)
            .fetch_optional(pool.inner())
            .await
            .map_err(|_| Status::InternalServerError)?
            .ok_or(Status::NotFound)?;

    let mut tx = pool
        .inner()
        .begin()
        .await
        .map_err(|_| Status::InternalServerError)?;

    // Mark all stages completed
    sqlx::query("UPDATE rollout_stages SET status = 'completed', completed_at = COALESCE(completed_at, now()) WHERE rollout_id = $1")
        .bind(rid)
        .execute(&mut *tx)
        .await
        .map_err(|_| Status::InternalServerError)?;

    // Mark rollout completed
    sqlx::query("UPDATE rollouts SET status = 'completed', updated_at = now() WHERE id = $1")
        .bind(rid)
        .execute(&mut *tx)
        .await
        .map_err(|_| Status::InternalServerError)?;

    // Pin version for all targeted clusters (only if the rollout carried one).
    if let Some(version) = &target_version {
        sqlx::query(
            "UPDATE clusters SET pinned_version = $1 WHERE id IN (\
             SELECT DISTINCT rgm.cluster_id FROM rollout_stages rs \
             JOIN LATERAL ( \
               SELECT cluster_id FROM rollout_group_members WHERE group_id = rs.group_id \
               UNION ALL \
               SELECT id FROM clusters WHERE rs.group_id = '00000000-0000-0000-0000-000000000000'::uuid \
             ) rgm ON true \
             WHERE rs.rollout_id = $2)",
        )
        .bind(version)
        .bind(rid)
        .execute(&mut *tx)
        .await
        .map_err(|_| Status::InternalServerError)?;
    }

    // Persist nixpkgs commit (if the rollout carried one) to the same clusters.
    if let Some(commit) = &nixpkgs_commit {
        sqlx::query(
            "UPDATE clusters SET nixpkgs_commit = $1 WHERE id IN (\
             SELECT DISTINCT rgm.cluster_id FROM rollout_stages rs \
             JOIN LATERAL ( \
               SELECT cluster_id FROM rollout_group_members WHERE group_id = rs.group_id \
               UNION ALL \
               SELECT id AS cluster_id FROM clusters WHERE rs.group_id = '00000000-0000-0000-0000-000000000000'::uuid \
             ) rgm ON true \
             WHERE rs.rollout_id = $2)",
        )
        .bind(commit)
        .bind(rid)
        .execute(&mut *tx)
        .await
        .map_err(|_| Status::InternalServerError)?;
    }

    tx.commit().await.map_err(|_| Status::InternalServerError)?;
    super::push::notify_all_rollout_clusters(
        channels.inner(),
        pool.inner(),
        rid,
        super::push::PushMessage::SelfUpdate,
    )
    .await;
    if nixpkgs_commit.is_some() {
        super::push::notify_all_rollout_clusters(
            channels.inner(),
            pool.inner(),
            rid,
            super::push::PushMessage::SyncNixpkgs,
        )
        .await;
    }
    Ok(Status::Ok)
}

// ── Resume (unpause) ────────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/api/admin/rollouts/{rollout_id}/resume",
    tag = "Admin — Rollouts",
    summary = "Resume a paused rollout",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Rollout resumed"),
        (status = 409, description = "Not paused"),
    ),
)]
#[rocket::post("/admin/rollouts/<rollout_id>/resume")]
pub async fn admin_resume_rollout(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    channels: &State<super::push::PushChannels>,
    rollout_id: &str,
) -> Result<Status, Status> {
    let rid: Uuid = rollout_id.parse().map_err(|_| Status::BadRequest)?;

    let mut tx = pool
        .inner()
        .begin()
        .await
        .map_err(|_| Status::InternalServerError)?;

    let updated = sqlx::query(
        "UPDATE rollout_stages SET status = 'rolling' WHERE rollout_id = $1 AND status = 'paused'",
    )
    .bind(rid)
    .execute(&mut *tx)
    .await
    .map_err(|_| Status::InternalServerError)?;

    if updated.rows_affected() == 0 {
        return Err(Status::Conflict);
    }

    sqlx::query("UPDATE rollouts SET status = 'rolling', updated_at = now() WHERE id = $1")
        .bind(rid)
        .execute(&mut *tx)
        .await
        .map_err(|_| Status::InternalServerError)?;

    tx.commit().await.map_err(|_| Status::InternalServerError)?;
    super::push::notify_rollout_clusters(
        channels.inner(),
        pool.inner(),
        rid,
        super::push::PushMessage::SelfUpdate,
    )
    .await;
    Ok(Status::Ok)
}

// ── Delete rollout ──────────────────────────────────────────────────

#[utoipa::path(
    delete,
    path = "/api/admin/rollouts/{rollout_id}",
    tag = "Admin — Rollouts",
    summary = "Delete a rollout and its stages",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Rollout deleted"),
        (status = 404, description = "Not found"),
    ),
)]
#[rocket::delete("/admin/rollouts/<rollout_id>")]
pub async fn admin_delete_rollout(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    rollout_id: &str,
) -> Result<Status, Status> {
    let rid: Uuid = rollout_id.parse().map_err(|_| Status::BadRequest)?;
    let result = sqlx::query("DELETE FROM rollouts WHERE id = $1")
        .bind(rid)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;

    if result.rows_affected() == 0 {
        Err(Status::NotFound)
    } else {
        Ok(Status::Ok)
    }
}

// ── Delete rollout group ────────────────────────────────────────────

#[utoipa::path(
    delete,
    path = "/api/admin/rollout-groups/{group_id}",
    tag = "Admin — Rollouts",
    summary = "Delete a rollout group",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Group deleted"),
        (status = 404, description = "Not found"),
    ),
)]
#[rocket::delete("/admin/rollout-groups/<group_id>")]
pub async fn admin_delete_rollout_group(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    group_id: &str,
) -> Result<Status, Status> {
    let gid: Uuid = group_id.parse().map_err(|_| Status::BadRequest)?;
    if gid.is_nil() {
        return Err(Status::Forbidden);
    }
    let result = sqlx::query("DELETE FROM rollout_groups WHERE id = $1")
        .bind(gid)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;

    if result.rows_affected() == 0 {
        Err(Status::NotFound)
    } else {
        Ok(Status::Ok)
    }
}

// ── Get rollout group detail with members ───────────────────────────

#[derive(Serialize, ToSchema)]
pub(crate) struct GroupDetail {
    id: Uuid,
    name: String,
    description: String,
    members: Vec<GroupMemberRow>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct GroupMemberRow {
    member_id: Uuid,
    cluster_id: Uuid,
    cluster_name: String,
}

#[utoipa::path(
    get,
    path = "/api/admin/rollout-groups/{group_id}",
    tag = "Admin — Rollouts",
    summary = "Get rollout group detail with members",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Group detail", body = GroupDetail),
        (status = 404, description = "Not found"),
    ),
)]
#[rocket::get("/admin/rollout-groups/<group_id>")]
pub async fn admin_get_rollout_group(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    group_id: &str,
) -> Result<Json<GroupDetail>, Status> {
    let gid: Uuid = group_id.parse().map_err(|_| Status::BadRequest)?;

    #[derive(sqlx::FromRow)]
    struct GRow {
        id: Uuid,
        name: String,
        description: String,
    }

    let group =
        sqlx::query_as::<_, GRow>("SELECT id, name, description FROM rollout_groups WHERE id = $1")
            .bind(gid)
            .fetch_optional(pool.inner())
            .await
            .map_err(|_| Status::InternalServerError)?
            .ok_or(Status::NotFound)?;

    #[derive(sqlx::FromRow)]
    struct MRow {
        member_id: Uuid,
        cluster_id: Uuid,
        cluster_name: String,
    }

    let members = sqlx::query_as::<_, MRow>(
        "SELECT rgm.id AS member_id, rgm.cluster_id, c.name AS cluster_name \
         FROM rollout_group_members rgm \
         JOIN clusters c ON c.id = rgm.cluster_id \
         WHERE rgm.group_id = $1 ORDER BY c.name",
    )
    .bind(gid)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    Ok(Json(GroupDetail {
        id: group.id,
        name: group.name,
        description: group.description,
        members: members
            .into_iter()
            .map(|m| GroupMemberRow {
                member_id: m.member_id,
                cluster_id: m.cluster_id,
                cluster_name: m.cluster_name,
            })
            .collect(),
    }))
}

// ── Admin — Daemon binary download ────────────────────────────────────

/// Response wrapper that attaches Content-Disposition and
/// Content-Type headers to a raw file body.
pub struct BinaryDownload {
    body: Vec<u8>,
    filename: String,
}

impl<'r> rocket::response::Responder<'r, 'static> for BinaryDownload {
    fn respond_to(self, _req: &'r rocket::Request<'_>) -> rocket::response::Result<'static> {
        rocket::Response::build()
            .header(ContentType::Binary)
            .header(Header::new(
                "Content-Disposition",
                format!("attachment; filename=\"{}\"", self.filename),
            ))
            .sized_body(self.body.len(), std::io::Cursor::new(self.body))
            .ok()
    }
}

#[utoipa::path(
    get,
    path = "/api/daemon-download/{version}/{system}",
    tag = "Download",
    summary = "Download daemon binary for a version and system",
    description = "Resolves the nix store path from xzar for `daemon/{version}/{system}`, realises it, and returns the `bin/mac-mgmt` binary. No authentication required.",
    params(
        ("version" = String, Path, description = "Daemon version (e.g., 0.1.6)"),
        ("system" = String, Path, description = "Nix system (e.g., aarch64-darwin)"),
    ),
    responses(
        (status = 200, description = "Binary file"),
        (status = 404, description = "No store path found for this version/system"),
        (status = 500, description = "Realisation or read failed"),
    ),
)]
#[rocket::get("/daemon-download/<version>/<system>")]
pub async fn download_daemon(version: &str, system: &str) -> Result<BinaryDownload, Status> {
    let cfg = crate::config::config();
    let xzar = cfg.xzar.as_ref().ok_or_else(|| {
        tracing::error!("xzar not configured");
        Status::InternalServerError
    })?;

    let pins = crate::xzar::fetch_pins(&xzar.url, &xzar.token)
        .await
        .map_err(|e| {
            tracing::error!("xzar fetch failed: {e}");
            Status::InternalServerError
        })?;

    let pin_name = format!("daemon/{version}/{system}");
    let store_path = crate::xzar::store_path_for_pin(&pins, &pin_name).ok_or_else(|| {
        tracing::warn!("no xzar pin found for {pin_name}");
        Status::NotFound
    })?;

    // Realise the store path (downloads from binary cache if needed).
    let output = tokio::process::Command::new("nix-store")
        .args(["--realise", &store_path])
        .output()
        .await
        .map_err(|e| {
            tracing::error!("failed to run nix-store --realise: {e}");
            Status::InternalServerError
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!("nix-store --realise failed: {stderr}");
        return Err(Status::InternalServerError);
    }

    let bin_path = std::path::Path::new(&store_path)
        .join("bin")
        .join("mac-mgmt");
    let body = tokio::fs::read(&bin_path).await.map_err(|e| {
        tracing::error!("failed to read {}: {e}", bin_path.display());
        Status::InternalServerError
    })?;

    let filename = format!("mac-mgmt-{version}-{system}");
    Ok(BinaryDownload { body, filename })
}

// ── Nixpkgs archive (public) ───────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/nixpkgs-archive/{commit}",
    tag = "Download",
    summary = "Download nixpkgs source archive for a commit",
    description = "Generates a tar.xz archive of the nixpkgs source tree at the given commit from the server's mirror clone. Results are cached in /tmp with file locking so concurrent requests coalesce. No authentication required.",
    params(
        ("commit" = String, Path, description = "Full 40-character hex commit SHA"),
    ),
    responses(
        (status = 200, description = "tar.xz archive"),
        (status = 400, description = "Invalid commit SHA format"),
        (status = 404, description = "Commit not found in nixpkgs mirror"),
        (status = 500, description = "Archive generation failed"),
    ),
)]
#[rocket::get("/nixpkgs-archive/<commit>")]
pub async fn get_nixpkgs_archive(commit: &str) -> Result<BinaryDownload, Status> {
    // Validate commit SHA format.
    if commit.len() != 40 || !commit.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Status::BadRequest);
    }

    let cache_path = format!("/tmp/mac-mgmt-nixpkgs-{commit}.tar.xz");
    let lock_path = format!("{cache_path}.lock");
    let partial_path = format!("{cache_path}.partial");

    let cfg = &crate::config::config().git;
    let repo_path = std::path::PathBuf::from(&cfg.state_dir).join("repos/nixpkgs.git");
    if !repo_path.exists() {
        tracing::error!("nixpkgs mirror clone not found at {}", repo_path.display());
        return Err(Status::InternalServerError);
    }

    // File locking: try exclusive lock to become the generator, or wait
    // for the current generator to finish.
    let lock_file = std::fs::File::create(&lock_path).map_err(|e| {
        tracing::error!("failed to create lock file {lock_path}: {e}");
        Status::InternalServerError
    })?;

    use fs2::FileExt;
    let generated_by_us;
    match lock_file.try_lock_exclusive() {
        Ok(()) => {
            // We hold the exclusive lock. Check if cache already exists
            // (another process may have generated it before we locked).
            if std::path::Path::new(&cache_path).exists() {
                generated_by_us = false;
            } else {
                // Generate the tarball.
                generated_by_us = true;
                if let Err(e) =
                    generate_nixpkgs_archive(commit, &repo_path, &partial_path, &cache_path).await
                {
                    let _ = lock_file.unlock();
                    return Err(e);
                }
            }
            let _ = lock_file.unlock();
        }
        Err(_) => {
            // Another request is generating. Wait on a blocking thread.
            let lf = lock_file;
            tokio::task::spawn_blocking(move || {
                let _ = lf.lock_exclusive();
                let _ = lf.unlock();
            })
            .await
            .map_err(|e| {
                tracing::error!("lock wait failed: {e}");
                Status::InternalServerError
            })?;
            generated_by_us = false;
        }
    }

    let _ = generated_by_us; // suppress unused warning

    // Read the cached file and serve it.
    let body = tokio::fs::read(&cache_path).await.map_err(|e| {
        tracing::error!("failed to read cached archive {cache_path}: {e}");
        Status::InternalServerError
    })?;

    Ok(BinaryDownload {
        body,
        filename: format!("nixpkgs-{commit}.tar.xz"),
    })
}

/// Run `git archive | xz` to generate a compressed nixpkgs tarball.
/// Writes to `partial_path` first, then renames to `cache_path` atomically.
async fn generate_nixpkgs_archive(
    commit: &str,
    repo_path: &std::path::Path,
    partial_path: &str,
    cache_path: &str,
) -> Result<(), Status> {
    // Check if the commit exists in the mirror.
    let exists = commit_exists_in_repo(commit, repo_path).await;
    if !exists {
        // Fetch and retry once.
        tracing::info!("commit {commit} not in nixpkgs mirror, fetching...");
        let _ = tokio::process::Command::new("git")
            .args(["remote", "update", "--prune"])
            .current_dir(repo_path)
            .output()
            .await;
        if !commit_exists_in_repo(commit, repo_path).await {
            tracing::warn!("commit {commit} not found after fetch");
            return Err(Status::NotFound);
        }
    }

    tracing::info!("generating nixpkgs archive for {commit}");

    use std::process::Stdio;

    // git archive --format=tar --prefix=nixpkgs-{commit}/ {commit}
    let mut git = tokio::process::Command::new("git")
        .args([
            "archive",
            "--format=tar",
            &format!("--prefix=nixpkgs-{commit}/"),
            commit,
        ])
        .current_dir(repo_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            tracing::error!("failed to spawn git archive: {e}");
            Status::InternalServerError
        })?;

    let git_stdout = git.stdout.take().unwrap().into_owned_fd().map_err(|e| {
        tracing::error!("failed to get git stdout fd: {e}");
        Status::InternalServerError
    })?;
    let output_file = std::fs::File::create(partial_path).map_err(|e| {
        tracing::error!("failed to create {partial_path}: {e}");
        Status::InternalServerError
    })?;

    // xz -1 (fast compression)
    let xz = tokio::process::Command::new("xz")
        .args(["-1"])
        .stdin(git_stdout)
        .stdout(output_file)
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            tracing::error!("failed to spawn xz: {e}");
            Status::InternalServerError
        })?;

    let (git_out, xz_out) = tokio::join!(git.wait_with_output(), xz.wait_with_output());

    let git_out = git_out.map_err(|e| {
        tracing::error!("git archive wait failed: {e}");
        Status::InternalServerError
    })?;
    if !git_out.status.success() {
        let stderr = String::from_utf8_lossy(&git_out.stderr);
        tracing::error!("git archive failed: {stderr}");
        let _ = std::fs::remove_file(partial_path);
        return Err(Status::InternalServerError);
    }

    let xz_out = xz_out.map_err(|e| {
        tracing::error!("xz wait failed: {e}");
        Status::InternalServerError
    })?;
    if !xz_out.status.success() {
        let stderr = String::from_utf8_lossy(&xz_out.stderr);
        tracing::error!("xz failed: {stderr}");
        let _ = std::fs::remove_file(partial_path);
        return Err(Status::InternalServerError);
    }

    // Atomic rename.
    std::fs::rename(partial_path, cache_path).map_err(|e| {
        tracing::error!("failed to rename {partial_path} -> {cache_path}: {e}");
        Status::InternalServerError
    })?;

    tracing::info!("nixpkgs archive for {commit} cached at {cache_path}");
    Ok(())
}

/// Check if a commit exists in a git repo.
async fn commit_exists_in_repo(commit: &str, repo_path: &std::path::Path) -> bool {
    tokio::process::Command::new("git")
        .args(["cat-file", "-t", commit])
        .current_dir(repo_path)
        .output()
        .await
        .is_ok_and(|o| o.status.success())
}

// ── Rollout rollback (admin) ────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/api/admin/rollouts/{rollout_id}/rollback",
    tag = "Admin — Rollouts",
    summary = "Roll back a rollout to its captured baseline",
    description = "Marks every non-pending stage as rolled_back and rewinds each cohort cluster's pinned_version + nixpkgs_commit to the baseline captured when the rollout first started rolling. Pushes SelfUpdate + SyncNixpkgs so daemons that took the bad version downgrade on the next tick.",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Rollout rolled back"),
        (status = 404, description = "Not found"),
    ),
)]
#[rocket::post("/admin/rollouts/<rollout_id>/rollback")]
pub async fn admin_rollback_rollout(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    rollout_id: &str,
) -> Result<Status, Status> {
    let rid: Uuid = rollout_id.parse().map_err(|_| Status::BadRequest)?;

    #[derive(sqlx::FromRow)]
    struct BaselineRow {
        baseline_version: Option<String>,
        baseline_nixpkgs_commit: Option<String>,
    }
    let baseline: BaselineRow = sqlx::query_as(
        "SELECT baseline_version, baseline_nixpkgs_commit FROM rollouts WHERE id = $1",
    )
    .bind(rid)
    .fetch_optional(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?
    .ok_or(Status::NotFound)?;

    let cohort: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT rgm.cluster_id FROM rollout_stages rs \
         JOIN LATERAL ( \
           SELECT cluster_id FROM rollout_group_members WHERE group_id = rs.group_id \
           UNION ALL \
           SELECT id FROM clusters WHERE rs.group_id = '00000000-0000-0000-0000-000000000000'::uuid \
         ) rgm ON true \
         WHERE rs.rollout_id = $1",
    )
    .bind(rid)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    let mut tx = pool
        .inner()
        .begin()
        .await
        .map_err(|_| Status::InternalServerError)?;
    sqlx::query(
        "UPDATE rollout_stages SET status = 'rolled_back' \
         WHERE rollout_id = $1 AND status IN ('rolling', 'paused', 'completed')",
    )
    .bind(rid)
    .execute(&mut *tx)
    .await
    .map_err(|_| Status::InternalServerError)?;
    sqlx::query("UPDATE rollouts SET status = 'rolled_back', updated_at = now() WHERE id = $1")
        .bind(rid)
        .execute(&mut *tx)
        .await
        .map_err(|_| Status::InternalServerError)?;
    sqlx::query("UPDATE clusters SET pinned_version = $1, nixpkgs_commit = $2 WHERE id = ANY($3)")
        .bind(&baseline.baseline_version)
        .bind(&baseline.baseline_nixpkgs_commit)
        .bind(&cohort)
        .execute(&mut *tx)
        .await
        .map_err(|_| Status::InternalServerError)?;
    tx.commit().await.map_err(|_| Status::InternalServerError)?;

    super::push::notify_all_rollout_clusters(
        channels.inner(),
        pool.inner(),
        rid,
        PushMessage::SelfUpdate,
    )
    .await;
    super::push::notify_all_rollout_clusters(
        channels.inner(),
        pool.inner(),
        rid,
        PushMessage::SyncNixpkgs,
    )
    .await;
    Ok(Status::Ok)
}

// ── Rollout health gates (admin) ────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub(crate) struct StageHealthReport {
    pub stage_id: Uuid,
    pub has_gate: bool,
    pub evaluation: Option<serde_json::Value>,
    pub last_evaluated_at: Option<DateTime<Utc>>,
}

#[utoipa::path(
    get,
    path = "/api/admin/rollouts/{rollout_id}/stages/{stage_id}/health",
    tag = "Admin — Rollouts",
    summary = "Get latest health-gate evaluation for a stage",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Stage health report", body = StageHealthReport),
        (status = 404, description = "Not found"),
    ),
)]
#[rocket::get("/admin/rollouts/<rollout_id>/stages/<stage_id>/health")]
pub async fn admin_stage_health(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    rollout_id: &str,
    stage_id: &str,
) -> Result<Json<StageHealthReport>, Status> {
    let rid: Uuid = rollout_id.parse().map_err(|_| Status::BadRequest)?;
    let sid: Uuid = stage_id.parse().map_err(|_| Status::BadRequest)?;

    #[derive(sqlx::FromRow)]
    struct StageRow {
        has_gate: bool,
    }
    let stage: StageRow = sqlx::query_as(
        "SELECT (health_gate IS NOT NULL) AS has_gate \
         FROM rollout_stages WHERE id = $1 AND rollout_id = $2",
    )
    .bind(sid)
    .bind(rid)
    .fetch_optional(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?
    .ok_or(Status::NotFound)?;

    // Also evaluate now so callers see live state, not just the last tick.
    let live = crate::rollout_health::evaluate_stage(pool.inner(), sid)
        .await
        .map_err(|_| Status::InternalServerError)?;

    #[derive(sqlx::FromRow)]
    struct LastRow {
        report: serde_json::Value,
        evaluated_at: DateTime<Utc>,
    }
    let last: Option<LastRow> = sqlx::query_as(
        "SELECT report, evaluated_at FROM rollout_stage_health_evaluations \
         WHERE stage_id = $1 ORDER BY evaluated_at DESC LIMIT 1",
    )
    .bind(sid)
    .fetch_optional(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    let (evaluation, last_evaluated_at) = match (live, last) {
        (Some(l), _) => (
            Some(serde_json::to_value(&l).unwrap_or_default()),
            Some(Utc::now()),
        ),
        (None, Some(l)) => (Some(l.report), Some(l.evaluated_at)),
        (None, None) => (None, None),
    };

    Ok(Json(StageHealthReport {
        stage_id: sid,
        has_gate: stage.has_gate,
        evaluation,
        last_evaluated_at,
    }))
}

#[utoipa::path(
    post,
    path = "/api/admin/rollouts/{rollout_id}/stages/{stage_id}/request-assessment",
    tag = "Admin — Rollouts",
    summary = "Request fresh assessment from every instance in a stage's cohort",
    description = "Sends PushCommand::RequestAssessment via SSE to every cluster in the stage's group. Use to force a health re-evaluation before advancing.",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Push dispatched"),
        (status = 404, description = "Not found"),
    ),
)]
#[rocket::post("/admin/rollouts/<rollout_id>/stages/<stage_id>/request-assessment")]
pub async fn admin_request_stage_assessment(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    rollout_id: &str,
    stage_id: &str,
) -> Result<Status, Status> {
    let rid: Uuid = rollout_id.parse().map_err(|_| Status::BadRequest)?;
    let sid: Uuid = stage_id.parse().map_err(|_| Status::BadRequest)?;

    #[derive(sqlx::FromRow)]
    struct GroupRow {
        group_id: Uuid,
    }
    let stage: GroupRow =
        sqlx::query_as("SELECT group_id FROM rollout_stages WHERE id = $1 AND rollout_id = $2")
            .bind(sid)
            .bind(rid)
            .fetch_optional(pool.inner())
            .await
            .map_err(|_| Status::InternalServerError)?
            .ok_or(Status::NotFound)?;

    let cohort: Vec<Uuid> = sqlx::query_scalar(
        "SELECT cluster_id FROM rollout_group_members WHERE group_id = $1 \
         UNION ALL \
         SELECT id FROM clusters WHERE $1 = '00000000-0000-0000-0000-000000000000'::uuid",
    )
    .bind(stage.group_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    let map = channels.inner().read().await;
    for cid in cohort {
        if let Some(tx) = map.get(&cid) {
            let _ = tx.send(PushMessage::RequestAssessment);
        }
    }
    Ok(Status::Ok)
}

// ── Admin — Skill Center Registration (mgmt feature) ───────────────

#[derive(Serialize, sqlx::FromRow)]
pub(crate) struct SkillCenterRow {
    id: Uuid,
    name: String,
    url: String,
    priority: i32,
    enabled: bool,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[rocket::get("/admin/skill-centers")]
pub async fn admin_list_skill_centers(
    _auth: AdminAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<SkillCenterRow>>, Status> {
    let rows = sqlx::query_as::<_, SkillCenterRow>(
        "SELECT id, name, url, priority, enabled, created_at, updated_at \
         FROM skill_centers ORDER BY priority DESC, name",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct CreateSkillCenterBody {
    name: String,
    url: String,
    federation_token: String,
    #[serde(default)]
    priority: i32,
    #[serde(default = "default_true")]
    enabled: bool,
}

fn default_true() -> bool {
    true
}

#[rocket::post("/admin/skill-centers", data = "<body>")]
pub async fn admin_create_skill_center(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    body: Json<CreateSkillCenterBody>,
) -> Result<Json<SkillCenterRow>, Status> {
    let result = sqlx::query_as::<_, SkillCenterRow>(
        "INSERT INTO skill_centers (name, url, federation_token, priority, enabled) \
         VALUES ($1, $2, $3, $4, $5) \
         RETURNING id, name, url, priority, enabled, created_at, updated_at",
    )
    .bind(&body.name)
    .bind(&body.url)
    .bind(&body.federation_token)
    .bind(body.priority)
    .bind(body.enabled)
    .fetch_one(pool.inner())
    .await;

    match result {
        Ok(r) => Ok(Json(r)),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Err(Status::Conflict),
        Err(_) => Err(Status::InternalServerError),
    }
}

#[derive(Deserialize)]
pub struct UpdateSkillCenterBody {
    name: Option<String>,
    url: Option<String>,
    federation_token: Option<String>,
    priority: Option<i32>,
    enabled: Option<bool>,
}

#[rocket::put("/admin/skill-centers/<id>", data = "<body>")]
pub async fn admin_update_skill_center(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    id: &str,
    body: Json<UpdateSkillCenterBody>,
) -> Result<Json<SkillCenterRow>, Status> {
    let uuid: Uuid = id.parse().map_err(|_| Status::BadRequest)?;

    // Fetch current values
    let current = sqlx::query_as::<_, SkillCenterRow>(
        "SELECT id, name, url, priority, enabled, created_at, updated_at \
         FROM skill_centers WHERE id = $1",
    )
    .bind(uuid)
    .fetch_optional(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?
    .ok_or(Status::NotFound)?;

    let name = body.name.as_deref().unwrap_or(&current.name);
    let url = body.url.as_deref().unwrap_or(&current.url);
    let priority = body.priority.unwrap_or(current.priority);
    let enabled = body.enabled.unwrap_or(current.enabled);

    // For federation_token, only update if provided
    if let Some(ref token) = body.federation_token {
        sqlx::query(
            "UPDATE skill_centers SET name = $1, url = $2, federation_token = $3, \
             priority = $4, enabled = $5, updated_at = now() WHERE id = $6",
        )
        .bind(name)
        .bind(url)
        .bind(token)
        .bind(priority)
        .bind(enabled)
        .bind(uuid)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    } else {
        sqlx::query(
            "UPDATE skill_centers SET name = $1, url = $2, \
             priority = $3, enabled = $4, updated_at = now() WHERE id = $5",
        )
        .bind(name)
        .bind(url)
        .bind(priority)
        .bind(enabled)
        .bind(uuid)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    }

    let updated = sqlx::query_as::<_, SkillCenterRow>(
        "SELECT id, name, url, priority, enabled, created_at, updated_at \
         FROM skill_centers WHERE id = $1",
    )
    .bind(uuid)
    .fetch_one(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    Ok(Json(updated))
}

#[rocket::delete("/admin/skill-centers/<id>")]
pub async fn admin_delete_skill_center(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    id: &str,
) -> Result<Status, Status> {
    let uuid: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    let res = sqlx::query("DELETE FROM skill_centers WHERE id = $1")
        .bind(uuid)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    if res.rows_affected() == 0 {
        Err(Status::NotFound)
    } else {
        Ok(Status::NoContent)
    }
}

// ── Admin — Federation Tokens ──────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateFederationTokenBody {
    label: String,
}

#[rocket::post("/admin/federation-tokens", data = "<body>")]
pub async fn admin_create_federation_token(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    body: Json<CreateFederationTokenBody>,
) -> Result<Json<CreatedToken>, Status> {
    let raw_token = format!("fed_{}", hex::encode(rand::random::<[u8; 32]>()));
    let hash = hex::encode(sha2::Sha256::digest(raw_token.as_bytes()));

    sqlx::query("INSERT INTO tokens (token_hash, label, kind) VALUES ($1, $2, 'federation')")
        .bind(&hash)
        .bind(&body.label)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;

    Ok(Json(CreatedToken { token: raw_token }))
}
