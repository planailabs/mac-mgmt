//! MCP server + MCP bundle endpoints: the admin-only catalog CRUD (servers,
//! bundles, bundle items, nix package deps), cluster attach/detach for both,
//! the derived per-cluster server lists (from bundles / transitive via
//! skills), and the remote skill-center catalog options.

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

// ── Shared DTOs ─────────────────────────────────────────────────────────

/// A catalog listing entry: either a local item or a remote item from an
/// enabled skill center (then `skill_center_name` is set).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpCatalogEntry {
    /// Local items: the real row id. Remote items: the remote catalog id.
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    /// Only set for local items.
    pub created_at: Option<DateTime<Utc>>,
    pub hide_from_public_catalog: bool,
    /// Set for remote items from a skill center.
    pub skill_center_name: Option<String>,
    /// Route discriminant ("mcp_server" or "mcp_bundle").
    pub route_kind: String,
}

/// An MCP server from the local catalog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpServerEntry {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    /// MCP server launch config (command/args/env), schema-validated.
    pub config_json: serde_json::Value,
    /// Nix package attribute names this server needs on the host.
    pub nix_packages: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub hide_from_public_catalog: bool,
}

#[cfg(feature = "server")]
impl From<crate::models::McpServer> for McpServerEntry {
    fn from(s: crate::models::McpServer) -> Self {
        McpServerEntry {
            id: s.id,
            slug: s.slug,
            name: s.name,
            description: s.description,
            config_json: s.config_json,
            nix_packages: s.nix_packages,
            created_at: s.created_at,
            hide_from_public_catalog: s.hide_from_public_catalog,
        }
    }
}

/// An MCP server bundle from the local catalog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpBundleEntry {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub hide_from_public_catalog: bool,
}

#[cfg(feature = "server")]
impl From<crate::models::McpServerBundle> for McpBundleEntry {
    fn from(b: crate::models::McpServerBundle) -> Self {
        McpBundleEntry {
            id: b.id,
            slug: b.slug,
            name: b.name,
            description: b.description,
            created_at: b.created_at,
            hide_from_public_catalog: b.hide_from_public_catalog,
        }
    }
}

/// A local MCP server option (id/slug/name) for selection dropdowns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct McpServerOptionEntry {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
}

/// A local MCP bundle option (id/slug/name) for selection dropdowns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct McpBundleOptionEntry {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
}

// ── MCP server catalog ──────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpServersListInput {}

/// was: list_mcp_servers() in web/components/mcp_server_list.rs
#[api_mcp_dioxus_server(server = "list_mcp_servers")]
pub async fn mcp_server_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: McpServersListInput,
) -> Result<Vec<McpCatalogEntry>, ApiError> {
    p.require_admin()?;

    let local =
        sqlx::query_as::<_, crate::models::McpServer>("SELECT * FROM mcp_servers ORDER BY slug")
            .fetch_all(pool)
            .await
            .map_err(internal)?;

    let mut entries: Vec<McpCatalogEntry> = local
        .into_iter()
        .map(|s| McpCatalogEntry {
            id: s.id,
            slug: s.slug,
            name: s.name,
            description: s.description,
            created_at: Some(s.created_at),
            hide_from_public_catalog: s.hide_from_public_catalog,
            skill_center_name: None,
            route_kind: "mcp_server".to_string(),
        })
        .collect();

    if let Some(cache) = crate::skill_center_cache::SkillCenterCache::global() {
        let all = cache.get_all().await;
        let sc_names = enabled_skill_center_names(pool).await.unwrap_or_default();

        for (sc_id, cached) in &all {
            let sc_name = sc_names.get(sc_id).cloned().unwrap_or_default();
            if sc_name.is_empty() {
                continue;
            }
            for srv in &cached.catalog.mcp_servers {
                entries.push(McpCatalogEntry {
                    id: srv.id,
                    slug: srv.slug.clone(),
                    name: srv.name.clone(),
                    description: srv.description.clone(),
                    created_at: None,
                    hide_from_public_catalog: srv.hidden,
                    skill_center_name: Some(sc_name.clone()),
                    route_kind: "mcp_server".to_string(),
                });
            }
        }
    }

    entries.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(entries)
}

/// Names of enabled skill centers, keyed by id.
#[cfg(feature = "server")]
async fn enabled_skill_center_names(
    pool: &sqlx::PgPool,
) -> Result<std::collections::HashMap<Uuid, String>, sqlx::Error> {
    Ok(sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, name FROM skill_centers WHERE enabled = true",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpServerGetInput {
    pub id: Uuid,
}

/// was: get_mcp_server() in web/components/mcp_server_detail.rs
#[api_mcp_dioxus_server(server = "get_mcp_server")]
pub async fn mcp_server_get(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: McpServerGetInput,
) -> Result<McpServerEntry, ApiError> {
    p.require_admin()?;
    let server =
        sqlx::query_as::<_, crate::models::McpServer>("SELECT * FROM mcp_servers WHERE id = $1")
            .bind(input.id)
            .fetch_optional(pool)
            .await
            .map_err(internal)?
            .ok_or_else(|| ApiError::not_found("mcp server not found"))?;
    Ok(server.into())
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpServerUpsertInput {
    /// Existing server id to update; omit to create a new server.
    pub id: Option<Uuid>,
    /// Slug, only used on create (immutable afterwards).
    pub slug: String,
    pub name: String,
    pub description: String,
    /// MCP server config as a JSON string; schema-validated.
    pub config_json: String,
    pub hide_from_public_catalog: bool,
}

/// was: upsert_mcp_server() in web/components/mcp_server_detail.rs
#[api_mcp_dioxus_server(server = "upsert_mcp_server")]
pub async fn mcp_server_upsert(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: McpServerUpsertInput,
) -> Result<McpServerEntry, ApiError> {
    p.require_admin()?;
    let parsed: serde_json::Value = serde_json::from_str(&input.config_json)
        .map_err(|e| ApiError::bad_request(format!("invalid JSON: {e}")))?;
    crate::mcp_schema::validate_mcp_server_config(&parsed)
        .map_err(|e| ApiError::bad_request(format!("schema validation failed: {e}")))?;

    let result = if let Some(id) = input.id {
        sqlx::query_as::<_, crate::models::McpServer>(
            "UPDATE mcp_servers SET name = $1, description = $2, config_json = $3, hide_from_public_catalog = $4 WHERE id = $5 RETURNING *",
        )
        .bind(&input.name)
        .bind(&input.description)
        .bind(&parsed)
        .bind(input.hide_from_public_catalog)
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(internal)?
    } else {
        sqlx::query_as::<_, crate::models::McpServer>(
            "INSERT INTO mcp_servers (slug, name, description, config_json, hide_from_public_catalog) VALUES ($1, $2, $3, $4, $5) RETURNING *",
        )
        .bind(&input.slug)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&parsed)
        .bind(input.hide_from_public_catalog)
        .fetch_one(pool)
        .await
        .map_err(internal)?
    };
    crate::api::push::notify_federation_global();
    Ok(result.into())
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpServerDeleteInput {
    pub id: Uuid,
}

/// was: delete_mcp_server() in web/components/mcp_server_detail.rs
#[api_mcp_dioxus_server(server = "delete_mcp_server")]
pub async fn mcp_server_delete(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: McpServerDeleteInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    crate::api::push::notify_federation_global();
    crate::api::push::notify_mcp_server_global(input.id).await;
    sqlx::query("DELETE FROM mcp_servers WHERE id = $1")
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

// ── Nix package deps ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NixPackageAddInput {
    pub id: Uuid,
    /// Nix package attribute name to add to the server's dependencies.
    pub package: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NixPackageRemoveInput {
    pub id: Uuid,
    /// Nix package attribute name to remove from the server's dependencies.
    pub package: String,
}

/// was: add_nix_package() in web/components/mcp_server_detail.rs
#[api_mcp_dioxus_server(server = "add_nix_package")]
pub async fn mcp_server_nix_package_add(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: NixPackageAddInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    let pkg = input.package.trim().to_string();
    if pkg.is_empty() {
        return Err(ApiError::bad_request("package name cannot be empty"));
    }
    sqlx::query(
        "UPDATE mcp_servers SET nix_packages = array_append(nix_packages, $1) \
         WHERE id = $2 AND NOT ($1 = ANY(nix_packages))",
    )
    .bind(&pkg)
    .bind(input.id)
    .execute(pool)
    .await
    .map_err(internal)?;
    crate::api::push::notify_federation_global();
    Ok(())
}

/// was: remove_nix_package() in web/components/mcp_server_detail.rs
#[api_mcp_dioxus_server(server = "remove_nix_package")]
pub async fn mcp_server_nix_package_remove(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: NixPackageRemoveInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query(
        "UPDATE mcp_servers SET nix_packages = array_remove(nix_packages, $1) WHERE id = $2",
    )
    .bind(&input.package)
    .bind(input.id)
    .execute(pool)
    .await
    .map_err(internal)?;
    crate::api::push::notify_federation_global();
    Ok(())
}

// ── Dependent skills ────────────────────────────────────────────────────

/// A skill channel that declares a dependency on the MCP server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SkillDepEntry {
    pub skill_slug: String,
    pub channel: String,
    pub skill_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DependentSkillsInput {
    pub mcp_server_id: Uuid,
}

/// was: list_dependent_skills() in web/components/mcp_server_detail.rs
#[api_mcp_dioxus_server(server = "list_dependent_skills")]
pub async fn mcp_server_dependent_skills(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: DependentSkillsInput,
) -> Result<Vec<SkillDepEntry>, ApiError> {
    p.require_admin()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        skill_slug: String,
        channel: String,
        skill_id: Uuid,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT s.slug as skill_slug, sc.channel, s.id as skill_id \
         FROM skill_mcp_dependencies smd \
         JOIN skill_channels sc ON sc.id = smd.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE smd.mcp_server_id = $1 \
         ORDER BY s.slug, sc.channel",
    )
    .bind(input.mcp_server_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    Ok(rows
        .into_iter()
        .map(|r| SkillDepEntry {
            skill_slug: r.skill_slug,
            channel: r.channel,
            skill_id: r.skill_id.to_string(),
        })
        .collect())
}

// ── MCP server options (dropdowns) ──────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpServerOptionsInput {}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpServerOptionsAllInput {}

/// was: list_available_mcp_servers() in web/components/mcp_bundle_detail.rs
#[api_mcp_dioxus_server(server = "list_available_mcp_servers")]
pub async fn mcp_server_options(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: McpServerOptionsInput,
) -> Result<Vec<McpServerOptionEntry>, ApiError> {
    p.require_admin()?;
    sqlx::query_as::<_, McpServerOptionEntry>("SELECT id, slug, name FROM mcp_servers ORDER BY slug")
        .fetch_all(pool)
        .await
        .map_err(internal)
}

/// was: list_all_mcp_servers() in web/components/cluster_mcp_servers.rs
#[api_mcp_dioxus_server(server = "list_all_mcp_servers")]
pub async fn mcp_server_options_all(
    pool: &sqlx::PgPool,
    _p: &Principal,
    _input: McpServerOptionsAllInput,
) -> Result<Vec<McpServerOptionEntry>, ApiError> {
    sqlx::query_as::<_, McpServerOptionEntry>("SELECT id, slug, name FROM mcp_servers ORDER BY slug")
        .fetch_all(pool)
        .await
        .map_err(internal)
}

// ── Remote (skill center) options ───────────────────────────────────────

/// A remote MCP server from an enabled skill center's catalog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RemoteMcpServerOptionEntry {
    pub skill_center_id: String,
    pub skill_center_name: String,
    pub remote_mcp_server_id: String,
    pub slug: String,
    pub name: String,
}

/// A remote MCP bundle from an enabled skill center's catalog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RemoteMcpBundleOptionEntry {
    pub skill_center_id: String,
    pub skill_center_name: String,
    pub remote_bundle_id: String,
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RemoteMcpServerOptionsInput {}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RemoteMcpBundleOptionsInput {}

/// was: list_remote_mcp_server_options() in web/components/cluster_mcp_servers.rs
#[api_mcp_dioxus_server(server = "list_remote_mcp_server_options")]
pub async fn mcp_server_remote_options(
    pool: &sqlx::PgPool,
    _p: &Principal,
    _input: RemoteMcpServerOptionsInput,
) -> Result<Vec<RemoteMcpServerOptionEntry>, ApiError> {
    let cache = crate::skill_center_cache::SkillCenterCache::global()
        .ok_or_else(|| ApiError::internal("skill center cache not initialized"))?;
    let sc_names = enabled_skill_center_names(pool).await.map_err(internal)?;

    let catalogs = cache.get_all().await;
    let mut result = Vec::new();
    for (sc_id, cached) in &catalogs {
        let sc_name = sc_names.get(sc_id).cloned().unwrap_or_default();
        if sc_name.is_empty() {
            continue;
        }
        for m in &cached.catalog.mcp_servers {
            if m.hidden {
                continue;
            }
            result.push(RemoteMcpServerOptionEntry {
                skill_center_id: sc_id.to_string(),
                skill_center_name: sc_name.clone(),
                remote_mcp_server_id: m.id.to_string(),
                slug: m.slug.clone(),
                name: m.name.clone(),
            });
        }
    }
    result.sort_by(|a, b| {
        a.skill_center_name
            .cmp(&b.skill_center_name)
            .then(a.slug.cmp(&b.slug))
    });
    Ok(result)
}

/// was: list_remote_mcp_bundle_options() in web/components/cluster_mcp_servers.rs
#[api_mcp_dioxus_server(server = "list_remote_mcp_bundle_options")]
pub async fn mcp_bundle_remote_options(
    pool: &sqlx::PgPool,
    _p: &Principal,
    _input: RemoteMcpBundleOptionsInput,
) -> Result<Vec<RemoteMcpBundleOptionEntry>, ApiError> {
    let cache = crate::skill_center_cache::SkillCenterCache::global()
        .ok_or_else(|| ApiError::internal("skill center cache not initialized"))?;
    let sc_names = enabled_skill_center_names(pool).await.map_err(internal)?;

    let catalogs = cache.get_all().await;
    let mut result = Vec::new();
    for (sc_id, cached) in &catalogs {
        let sc_name = sc_names.get(sc_id).cloned().unwrap_or_default();
        if sc_name.is_empty() {
            continue;
        }
        for b in &cached.catalog.mcp_bundles {
            if b.hidden {
                continue;
            }
            result.push(RemoteMcpBundleOptionEntry {
                skill_center_id: sc_id.to_string(),
                skill_center_name: sc_name.clone(),
                remote_bundle_id: b.id.to_string(),
                slug: b.slug.clone(),
                name: b.name.clone(),
            });
        }
    }
    result.sort_by(|a, b| {
        a.skill_center_name
            .cmp(&b.skill_center_name)
            .then(a.slug.cmp(&b.slug))
    });
    Ok(result)
}

// ── Cluster ↔ MCP server assignments ────────────────────────────────────

/// A direct MCP server assignment on a cluster (local or remote).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ClusterMcpServerEntry {
    pub cluster_mcp_server_id: Uuid,
    pub server_slug: String,
    pub server_name: String,
    /// Set when the assignment references a remote skill-center server.
    pub skill_center_name: Option<String>,
}

/// An MCP server a cluster gets via an assigned bundle (read-only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BundleMcpServerEntry {
    pub server_slug: String,
    pub server_name: String,
    pub bundle_slug: String,
    /// True if a direct assignment for the same server slug shadows this one.
    pub overwritten: bool,
}

/// An MCP server a cluster gets transitively via a skill dependency.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TransitiveMcpServerEntry {
    pub server_slug: String,
    pub server_name: String,
    pub skill_slug: String,
    pub channel: String,
    /// True if a direct or bundle assignment for the same slug shadows this.
    pub overwritten: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterMcpServersInput {
    pub cluster_id: Uuid,
}

/// was: list_cluster_mcp_servers() in web/components/cluster_mcp_servers.rs
#[api_mcp_dioxus_server(server = "list_cluster_mcp_servers")]
pub async fn mcp_server_cluster_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterMcpServersInput,
) -> Result<Vec<ClusterMcpServerEntry>, ApiError> {
    access::require_cluster_read(pool, p, input.cluster_id).await?;
    sqlx::query_as::<_, ClusterMcpServerEntry>(
        "SELECT cms.id AS cluster_mcp_server_id, \
                COALESCE(ms.slug, cms.slug) AS server_slug, \
                COALESCE(ms.name, cms.mcp_name) AS server_name, \
                sk_center.name AS skill_center_name \
         FROM cluster_mcp_servers cms \
         LEFT JOIN mcp_servers ms ON ms.id = cms.mcp_server_id \
         LEFT JOIN skill_centers sk_center ON sk_center.id = cms.skill_center_id \
         WHERE cms.cluster_id = $1 \
         ORDER BY COALESCE(ms.slug, cms.slug)",
    )
    .bind(input.cluster_id)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

/// was: list_bundle_mcp_servers() in web/components/cluster_mcp_servers.rs
#[api_mcp_dioxus_server(server = "list_bundle_mcp_servers")]
pub async fn mcp_server_cluster_from_bundles(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterMcpServersInput,
) -> Result<Vec<BundleMcpServerEntry>, ApiError> {
    access::require_cluster_read(pool, p, input.cluster_id).await?;

    #[derive(sqlx::FromRow)]
    struct Row {
        server_slug: String,
        server_name: String,
        bundle_slug: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT DISTINCT ms.slug as server_slug, ms.name as server_name, msb.slug as bundle_slug \
         FROM cluster_mcp_bundles cmb \
         JOIN mcp_server_bundle_items msbi ON msbi.bundle_id = cmb.bundle_id \
         JOIN mcp_servers ms ON ms.id = msbi.mcp_server_id \
         JOIN mcp_server_bundles msb ON msb.id = cmb.bundle_id \
         WHERE cmb.cluster_id = $1 \
         ORDER BY ms.slug",
    )
    .bind(input.cluster_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    // Overwritten if a direct assignment exists for the same server slug.
    let direct_slugs: std::collections::HashSet<String> = sqlx::query_scalar::<_, String>(
        "SELECT ms.slug \
         FROM cluster_mcp_servers cms \
         JOIN mcp_servers ms ON ms.id = cms.mcp_server_id \
         WHERE cms.cluster_id = $1",
    )
    .bind(input.cluster_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?
    .into_iter()
    .collect();

    Ok(rows
        .into_iter()
        .map(|r| BundleMcpServerEntry {
            overwritten: direct_slugs.contains(&r.server_slug),
            server_slug: r.server_slug,
            server_name: r.server_name,
            bundle_slug: r.bundle_slug,
        })
        .collect())
}

/// was: list_transitive_mcp_servers() in web/components/cluster_mcp_servers.rs
#[api_mcp_dioxus_server(server = "list_transitive_mcp_servers")]
pub async fn mcp_server_cluster_transitive(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterMcpServersInput,
) -> Result<Vec<TransitiveMcpServerEntry>, ApiError> {
    let cid = input.cluster_id;
    access::require_cluster_read(pool, p, cid).await?;

    // Resolve winning skill channels for this cluster (direct wins over bundle).
    #[derive(sqlx::FromRow)]
    struct WinRow {
        skill_channel_id: Uuid,
        slug: String,
        channel: String,
        is_direct: bool,
    }

    let skill_rows = sqlx::query_as::<_, WinRow>(
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
    .bind(cid)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    // Dedup: direct wins per slug.
    let mut winners: std::collections::HashMap<String, (Uuid, String, String, bool)> =
        std::collections::HashMap::new();
    for r in &skill_rows {
        match winners.get(&r.slug) {
            Some((_, _, _, true)) => {}
            _ => {
                winners.insert(
                    r.slug.clone(),
                    (
                        r.skill_channel_id,
                        r.slug.clone(),
                        r.channel.clone(),
                        r.is_direct,
                    ),
                );
            }
        }
    }

    let channel_ids: Vec<Uuid> = winners.values().map(|(id, _, _, _)| *id).collect();
    if channel_ids.is_empty() {
        return Ok(Vec::new());
    }

    // Build a map from channel_id -> (slug, channel) for labeling.
    let channel_info: std::collections::HashMap<Uuid, (String, String)> = winners
        .into_values()
        .map(|(id, slug, channel, _)| (id, (slug, channel)))
        .collect();

    #[derive(sqlx::FromRow)]
    struct DepRow {
        skill_channel_id: Uuid,
        server_slug: String,
        server_name: String,
    }

    let dep_rows = sqlx::query_as::<_, DepRow>(
        "SELECT smd.skill_channel_id, ms.slug as server_slug, ms.name as server_name \
         FROM skill_mcp_dependencies smd \
         JOIN mcp_servers ms ON ms.id = smd.mcp_server_id \
         WHERE smd.skill_channel_id = ANY($1) \
         ORDER BY ms.slug",
    )
    .bind(&channel_ids)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    // Overwritten if a direct or bundle assignment exists for the same slug.
    let higher_slugs: std::collections::HashSet<String> = sqlx::query_scalar::<_, String>(
        "SELECT ms.slug \
         FROM cluster_mcp_servers cms \
         JOIN mcp_servers ms ON ms.id = cms.mcp_server_id \
         WHERE cms.cluster_id = $1 \
         UNION \
         SELECT ms.slug \
         FROM cluster_mcp_bundles cmb \
         JOIN mcp_server_bundle_items msbi ON msbi.bundle_id = cmb.bundle_id \
         JOIN mcp_servers ms ON ms.id = msbi.mcp_server_id \
         WHERE cmb.cluster_id = $1",
    )
    .bind(cid)
    .fetch_all(pool)
    .await
    .map_err(internal)?
    .into_iter()
    .collect();

    let result = dep_rows
        .into_iter()
        .filter_map(|r| {
            let (skill_slug, channel) = channel_info.get(&r.skill_channel_id)?;
            Some(TransitiveMcpServerEntry {
                overwritten: higher_slugs.contains(&r.server_slug),
                server_slug: r.server_slug,
                server_name: r.server_name,
                skill_slug: skill_slug.clone(),
                channel: channel.clone(),
            })
        })
        .collect();

    Ok(result)
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterMcpServerAttachInput {
    pub cluster_id: Uuid,
    /// Local server id; required unless attaching a remote server.
    pub mcp_server_id: Option<Uuid>,
    /// Skill center id; set (with remote_id + slug) to attach a remote server.
    pub skill_center_id: Option<Uuid>,
    /// Remote catalog id of the server (required for remote attach).
    pub remote_id: Option<Uuid>,
    /// Remote server slug (required for remote attach).
    pub slug: Option<String>,
    /// Remote server display name.
    pub mcp_name: Option<String>,
}

/// was: add_cluster_mcp_server() in web/components/cluster_mcp_servers.rs
#[api_mcp_dioxus_server(server = "add_cluster_mcp_server")]
pub async fn mcp_server_cluster_attach(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterMcpServerAttachInput,
) -> Result<(), ApiError> {
    let cid = input.cluster_id;
    access::require_cluster_write(pool, p, cid).await?;
    if let Some(sc_id) = input.skill_center_id {
        let r_id = input
            .remote_id
            .ok_or_else(|| ApiError::bad_request("remote_id required for remote MCP server"))?;
        let slug = input
            .slug
            .ok_or_else(|| ApiError::bad_request("slug required for remote MCP server"))?;
        sqlx::query(
            "INSERT INTO cluster_mcp_servers (cluster_id, skill_center_id, remote_id, slug, mcp_name) \
             VALUES ($1, $2, $3, $4, $5) ON CONFLICT DO NOTHING",
        )
        .bind(cid)
        .bind(sc_id)
        .bind(r_id)
        .bind(&slug)
        .bind(input.mcp_name.as_deref())
        .execute(pool)
        .await
        .map_err(internal)?;
    } else {
        let msid = input
            .mcp_server_id
            .ok_or_else(|| ApiError::bad_request("mcp_server_id required for local MCP server"))?;
        sqlx::query("INSERT INTO cluster_mcp_servers (cluster_id, mcp_server_id) VALUES ($1, $2)")
            .bind(cid)
            .bind(msid)
            .execute(pool)
            .await
            .map_err(internal)?;
    }
    crate::api::push::notify_global(cid, crate::api::push::PushMessage::SyncMcpServers).await;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterMcpServerDetachInput {
    /// Assignment row id (from cluster_mcp_servers).
    pub cluster_mcp_server_id: Uuid,
}

/// was: remove_cluster_mcp_server() in web/components/cluster_mcp_servers.rs
#[api_mcp_dioxus_server(server = "remove_cluster_mcp_server")]
pub async fn mcp_server_cluster_detach(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterMcpServerDetachInput,
) -> Result<(), ApiError> {
    // Check access to the owning cluster before deleting.
    let owner_cid = sqlx::query_scalar::<_, Uuid>(
        "SELECT cluster_id FROM cluster_mcp_servers WHERE id = $1",
    )
    .bind(input.cluster_mcp_server_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?;
    if let Some(owner_cid) = owner_cid {
        if let Some(ids) = access::writable_cluster_ids(pool, p).await? {
            if !ids.contains(&owner_cid) {
                return Err(ApiError::forbidden("access denied"));
            }
        }
    }
    let cid = sqlx::query_scalar::<_, Uuid>(
        "DELETE FROM cluster_mcp_servers WHERE id = $1 RETURNING cluster_id",
    )
    .bind(input.cluster_mcp_server_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?;
    if let Some(cid) = cid {
        crate::api::push::notify_global(cid, crate::api::push::PushMessage::SyncMcpServers).await;
    }
    Ok(())
}

// ── MCP bundle catalog ──────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpBundlesListInput {}

/// was: list_mcp_bundles() in web/components/mcp_bundle_list.rs
#[api_mcp_dioxus_server(server = "list_mcp_bundles")]
pub async fn mcp_bundle_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: McpBundlesListInput,
) -> Result<Vec<McpCatalogEntry>, ApiError> {
    p.require_admin()?;

    let local = sqlx::query_as::<_, crate::models::McpServerBundle>(
        "SELECT * FROM mcp_server_bundles ORDER BY slug",
    )
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    let mut entries: Vec<McpCatalogEntry> = local
        .into_iter()
        .map(|b| McpCatalogEntry {
            id: b.id,
            slug: b.slug,
            name: b.name,
            description: b.description,
            created_at: Some(b.created_at),
            hide_from_public_catalog: b.hide_from_public_catalog,
            skill_center_name: None,
            route_kind: "mcp_bundle".to_string(),
        })
        .collect();

    if let Some(cache) = crate::skill_center_cache::SkillCenterCache::global() {
        let all = cache.get_all().await;
        let sc_names = enabled_skill_center_names(pool).await.unwrap_or_default();

        for (sc_id, cached) in &all {
            let sc_name = sc_names.get(sc_id).cloned().unwrap_or_default();
            if sc_name.is_empty() {
                continue;
            }
            for bundle in &cached.catalog.mcp_bundles {
                entries.push(McpCatalogEntry {
                    id: bundle.id,
                    slug: bundle.slug.clone(),
                    name: bundle.name.clone(),
                    description: bundle.description.clone(),
                    created_at: None,
                    hide_from_public_catalog: bundle.hidden,
                    skill_center_name: Some(sc_name.clone()),
                    route_kind: "mcp_bundle".to_string(),
                });
            }
        }
    }

    entries.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(entries)
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpBundleGetInput {
    pub id: Uuid,
}

/// was: get_mcp_bundle() in web/components/mcp_bundle_detail.rs
#[api_mcp_dioxus_server(server = "get_mcp_bundle")]
pub async fn mcp_bundle_get(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: McpBundleGetInput,
) -> Result<McpBundleEntry, ApiError> {
    p.require_admin()?;
    let bundle = sqlx::query_as::<_, crate::models::McpServerBundle>(
        "SELECT * FROM mcp_server_bundles WHERE id = $1",
    )
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or_else(|| ApiError::not_found("mcp bundle not found"))?;
    Ok(bundle.into())
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpBundleCreateInput {
    pub slug: String,
    pub name: String,
    pub description: String,
}

/// was: create_mcp_bundle() in web/components/mcp_bundle_form.rs
#[api_mcp_dioxus_server(server = "create_mcp_bundle")]
pub async fn mcp_bundle_create(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: McpBundleCreateInput,
) -> Result<McpBundleEntry, ApiError> {
    p.require_admin()?;
    let bundle = sqlx::query_as::<_, crate::models::McpServerBundle>(
        "INSERT INTO mcp_server_bundles (slug, name, description) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(&input.slug)
    .bind(&input.name)
    .bind(&input.description)
    .fetch_one(pool)
    .await
    .map_err(internal)?;
    crate::api::push::notify_federation_global();
    Ok(bundle.into())
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpBundleUpdateInput {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub hide_from_public_catalog: bool,
}

/// was: update_mcp_bundle() in web/components/mcp_bundle_detail.rs
#[api_mcp_dioxus_server(server = "update_mcp_bundle")]
pub async fn mcp_bundle_update(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: McpBundleUpdateInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query("UPDATE mcp_server_bundles SET name = $1, description = $2, hide_from_public_catalog = $3 WHERE id = $4")
        .bind(&input.name)
        .bind(&input.description)
        .bind(input.hide_from_public_catalog)
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(internal)?;
    crate::api::push::notify_federation_global();
    crate::api::push::notify_mcp_bundle_global(input.id).await;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpBundleDeleteInput {
    pub id: Uuid,
}

/// was: delete_mcp_bundle() in web/components/mcp_bundle_detail.rs
#[api_mcp_dioxus_server(server = "delete_mcp_bundle")]
pub async fn mcp_bundle_delete(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: McpBundleDeleteInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    crate::api::push::notify_federation_global();
    crate::api::push::notify_mcp_bundle_global(input.id).await;
    sqlx::query("DELETE FROM mcp_server_bundles WHERE id = $1")
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

// ── Bundle items ────────────────────────────────────────────────────────

/// An MCP server contained in a bundle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct McpBundleItemEntry {
    pub bundle_item_id: Uuid,
    pub server_slug: String,
    pub server_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpBundleItemsInput {
    pub bundle_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpBundleItemAddInput {
    pub bundle_id: Uuid,
    pub mcp_server_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpBundleItemRemoveInput {
    /// Bundle item row id (from mcp_server_bundle_items).
    pub bundle_item_id: Uuid,
}

/// was: list_mcp_bundle_items() in web/components/mcp_bundle_detail.rs
#[api_mcp_dioxus_server(server = "list_mcp_bundle_items")]
pub async fn mcp_bundle_items(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: McpBundleItemsInput,
) -> Result<Vec<McpBundleItemEntry>, ApiError> {
    p.require_admin()?;
    sqlx::query_as::<_, McpBundleItemEntry>(
        "SELECT msbi.id as bundle_item_id, ms.slug as server_slug, ms.name as server_name \
         FROM mcp_server_bundle_items msbi \
         JOIN mcp_servers ms ON ms.id = msbi.mcp_server_id \
         WHERE msbi.bundle_id = $1 \
         ORDER BY ms.slug",
    )
    .bind(input.bundle_id)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

/// was: add_mcp_bundle_item() in web/components/mcp_bundle_detail.rs
#[api_mcp_dioxus_server(server = "add_mcp_bundle_item")]
pub async fn mcp_bundle_item_add(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: McpBundleItemAddInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query("INSERT INTO mcp_server_bundle_items (bundle_id, mcp_server_id) VALUES ($1, $2)")
        .bind(input.bundle_id)
        .bind(input.mcp_server_id)
        .execute(pool)
        .await
        .map_err(internal)?;
    crate::api::push::notify_federation_global();
    crate::api::push::notify_mcp_bundle_global(input.bundle_id).await;
    Ok(())
}

/// was: remove_mcp_bundle_item() in web/components/mcp_bundle_detail.rs
#[api_mcp_dioxus_server(server = "remove_mcp_bundle_item")]
pub async fn mcp_bundle_item_remove(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: McpBundleItemRemoveInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    let bundle_id: Option<Uuid> =
        sqlx::query_scalar("SELECT bundle_id FROM mcp_server_bundle_items WHERE id = $1")
            .bind(input.bundle_item_id)
            .fetch_optional(pool)
            .await
            .map_err(internal)?;
    sqlx::query("DELETE FROM mcp_server_bundle_items WHERE id = $1")
        .bind(input.bundle_item_id)
        .execute(pool)
        .await
        .map_err(internal)?;
    if let Some(bid) = bundle_id {
        crate::api::push::notify_federation_global();
        crate::api::push::notify_mcp_bundle_global(bid).await;
    }
    Ok(())
}

// ── MCP bundle options (dropdowns) ──────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpBundleOptionsAllInput {}

/// was: list_all_mcp_bundles() in web/components/cluster_mcp_servers.rs
#[api_mcp_dioxus_server(server = "list_all_mcp_bundles")]
pub async fn mcp_bundle_options_all(
    pool: &sqlx::PgPool,
    _p: &Principal,
    _input: McpBundleOptionsAllInput,
) -> Result<Vec<McpBundleOptionEntry>, ApiError> {
    sqlx::query_as::<_, McpBundleOptionEntry>(
        "SELECT id, slug, name FROM mcp_server_bundles ORDER BY slug",
    )
    .fetch_all(pool)
    .await
    .map_err(internal)
}

// ── Cluster ↔ MCP bundle assignments ────────────────────────────────────

/// An MCP bundle assignment on a cluster (local or remote).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ClusterMcpBundleEntry {
    pub cluster_mcp_bundle_id: Uuid,
    pub bundle_slug: String,
    pub bundle_name: String,
    /// Set when the assignment references a remote skill-center bundle.
    pub skill_center_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterMcpBundlesInput {
    pub cluster_id: Uuid,
}

/// was: list_cluster_mcp_bundles() in web/components/cluster_mcp_servers.rs
#[api_mcp_dioxus_server(server = "list_cluster_mcp_bundles")]
pub async fn mcp_bundle_cluster_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterMcpBundlesInput,
) -> Result<Vec<ClusterMcpBundleEntry>, ApiError> {
    access::require_cluster_read(pool, p, input.cluster_id).await?;
    sqlx::query_as::<_, ClusterMcpBundleEntry>(
        "SELECT cmb.id AS cluster_mcp_bundle_id, \
                COALESCE(msb.slug, cmb.slug) AS bundle_slug, \
                COALESCE(msb.name, cmb.bundle_name) AS bundle_name, \
                sk_center.name AS skill_center_name \
         FROM cluster_mcp_bundles cmb \
         LEFT JOIN mcp_server_bundles msb ON msb.id = cmb.bundle_id \
         LEFT JOIN skill_centers sk_center ON sk_center.id = cmb.skill_center_id \
         WHERE cmb.cluster_id = $1 \
         ORDER BY COALESCE(msb.slug, cmb.slug)",
    )
    .bind(input.cluster_id)
    .fetch_all(pool)
    .await
    .map_err(internal)
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterMcpBundleAttachInput {
    pub cluster_id: Uuid,
    /// Local bundle id; required unless attaching a remote bundle.
    pub bundle_id: Option<Uuid>,
    /// Skill center id; set (with remote_id + slug) to attach a remote bundle.
    pub skill_center_id: Option<Uuid>,
    /// Remote catalog id of the bundle (required for remote attach).
    pub remote_id: Option<Uuid>,
    /// Remote bundle slug (required for remote attach).
    pub slug: Option<String>,
    /// Remote bundle display name.
    pub bundle_name: Option<String>,
}

/// was: add_cluster_mcp_bundle() in web/components/cluster_mcp_servers.rs
#[api_mcp_dioxus_server(server = "add_cluster_mcp_bundle")]
pub async fn mcp_bundle_cluster_attach(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterMcpBundleAttachInput,
) -> Result<(), ApiError> {
    let cid = input.cluster_id;
    access::require_cluster_write(pool, p, cid).await?;
    if let Some(sc_id) = input.skill_center_id {
        let r_id = input
            .remote_id
            .ok_or_else(|| ApiError::bad_request("remote_id required for remote MCP bundle"))?;
        let slug = input
            .slug
            .ok_or_else(|| ApiError::bad_request("slug required for remote MCP bundle"))?;
        sqlx::query(
            "INSERT INTO cluster_mcp_bundles (cluster_id, skill_center_id, remote_id, slug, bundle_name) \
             VALUES ($1, $2, $3, $4, $5) ON CONFLICT DO NOTHING",
        )
        .bind(cid)
        .bind(sc_id)
        .bind(r_id)
        .bind(&slug)
        .bind(input.bundle_name.as_deref())
        .execute(pool)
        .await
        .map_err(internal)?;
    } else {
        let bid = input
            .bundle_id
            .ok_or_else(|| ApiError::bad_request("bundle_id required for local MCP bundle"))?;

        // Check for overlap: does the new bundle share any mcp_server with
        // any bundle already assigned to this cluster?
        let overlap = sqlx::query_scalar::<_, String>(
            "SELECT ms.slug \
             FROM mcp_server_bundle_items new_bi \
             JOIN mcp_server_bundle_items existing_bi ON existing_bi.mcp_server_id = new_bi.mcp_server_id \
             JOIN cluster_mcp_bundles cmb ON cmb.bundle_id = existing_bi.bundle_id AND cmb.cluster_id = $1 \
             JOIN mcp_servers ms ON ms.id = new_bi.mcp_server_id \
             WHERE new_bi.bundle_id = $2 \
             LIMIT 1",
        )
        .bind(cid)
        .bind(bid)
        .fetch_optional(pool)
        .await
        .map_err(internal)?;

        if let Some(conflicting) = overlap {
            return Err(ApiError::conflict(format!(
                "bundle conflicts with an already-assigned bundle on MCP server: {conflicting}"
            )));
        }

        sqlx::query("INSERT INTO cluster_mcp_bundles (cluster_id, bundle_id) VALUES ($1, $2)")
            .bind(cid)
            .bind(bid)
            .execute(pool)
            .await
            .map_err(internal)?;
    }
    crate::api::push::notify_global(cid, crate::api::push::PushMessage::SyncMcpServers).await;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterMcpBundleDetachInput {
    /// Assignment row id (from cluster_mcp_bundles).
    pub cluster_mcp_bundle_id: Uuid,
}

/// was: remove_cluster_mcp_bundle() in web/components/cluster_mcp_servers.rs
#[api_mcp_dioxus_server(server = "remove_cluster_mcp_bundle")]
pub async fn mcp_bundle_cluster_detach(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterMcpBundleDetachInput,
) -> Result<(), ApiError> {
    // Check access to the owning cluster before deleting.
    let owner_cid = sqlx::query_scalar::<_, Uuid>(
        "SELECT cluster_id FROM cluster_mcp_bundles WHERE id = $1",
    )
    .bind(input.cluster_mcp_bundle_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?;
    if let Some(owner_cid) = owner_cid {
        if let Some(ids) = access::writable_cluster_ids(pool, p).await? {
            if !ids.contains(&owner_cid) {
                return Err(ApiError::forbidden("access denied"));
            }
        }
    }
    let cid = sqlx::query_scalar::<_, Uuid>(
        "DELETE FROM cluster_mcp_bundles WHERE id = $1 RETURNING cluster_id",
    )
    .bind(input.cluster_mcp_bundle_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?;
    if let Some(cid) = cid {
        crate::api::push::notify_global(cid, crate::api::push::PushMessage::SyncMcpServers).await;
    }
    Ok(())
}

// ── Registration ────────────────────────────────────────────────────────

#[cfg(feature = "server")]
pub fn register(reg: &mut plan_ai_api_mcp::Registry<sqlx::PgPool>) {
    use plan_ai_api_mcp::{OnItem, Risk};

    {
        let mut m = reg.resource("mcp_servers", "mcp_server", "MCP servers");
        m.list(
            "List the MCP server catalog: local servers plus remote servers from enabled skill centers (remote ones carry skill_center_name); admin only.",
            |pool: sqlx::PgPool, p, input: McpServersListInput| async move {
                mcp_server_list(&pool, &p, input).await
            },
        );
        m.get(
            "Get a local MCP server (slug, name, description, launch config, nix packages); admin only.",
            |pool: sqlx::PgPool, p, input: McpServerGetInput| async move {
                mcp_server_get(&pool, &p, input).await
            },
        );
        m.custom(
            "upsert",
            Risk::Mutating,
            OnItem::No,
            "Create (id omitted) or update (id set) an MCP server; config_json is a JSON string validated against the MCP server config schema. Pushes a federation sync. Admin only.",
            |pool: sqlx::PgPool, p, input: McpServerUpsertInput| async move {
                mcp_server_upsert(&pool, &p, input).await
            },
        );
        m.delete(
            "Delete an MCP server from the catalog and push federation + per-cluster MCP syncs; admin only.",
            |pool: sqlx::PgPool, p, input: McpServerDeleteInput| async move {
                mcp_server_delete(&pool, &p, input).await
            },
        );
        m.custom(
            "nix_package_add",
            Risk::Mutating,
            OnItem::Yes,
            "Add a nix package to the MCP server's dependency list (idempotent) and push a federation sync; admin only.",
            |pool: sqlx::PgPool, p, input: NixPackageAddInput| async move {
                mcp_server_nix_package_add(&pool, &p, input).await
            },
        );
        m.custom(
            "nix_package_remove",
            Risk::Mutating,
            OnItem::Yes,
            "Remove a nix package from the MCP server's dependency list and push a federation sync; admin only.",
            |pool: sqlx::PgPool, p, input: NixPackageRemoveInput| async move {
                mcp_server_nix_package_remove(&pool, &p, input).await
            },
        );
        m.custom(
            "dependent_skills",
            Risk::ReadOnly,
            OnItem::No,
            "List skill channels that declare a dependency on the MCP server; admin only.",
            |pool: sqlx::PgPool, p, input: DependentSkillsInput| async move {
                mcp_server_dependent_skills(&pool, &p, input).await
            },
        );
        m.custom(
            "options",
            Risk::ReadOnly,
            OnItem::No,
            "List local MCP servers as id/slug/name options (for bundle item selection); admin only.",
            |pool: sqlx::PgPool, p, input: McpServerOptionsInput| async move {
                mcp_server_options(&pool, &p, input).await
            },
        );
        m.custom(
            "options_all",
            Risk::ReadOnly,
            OnItem::No,
            "List local MCP servers as id/slug/name options (for cluster assignment); any authenticated caller.",
            |pool: sqlx::PgPool, p, input: McpServerOptionsAllInput| async move {
                mcp_server_options_all(&pool, &p, input).await
            },
        );
        m.custom(
            "remote_options",
            Risk::ReadOnly,
            OnItem::No,
            "List non-hidden MCP servers from enabled skill centers' cached catalogs; any authenticated caller.",
            |pool: sqlx::PgPool, p, input: RemoteMcpServerOptionsInput| async move {
                mcp_server_remote_options(&pool, &p, input).await
            },
        );
        m.custom(
            "cluster_list",
            Risk::ReadOnly,
            OnItem::No,
            "List a cluster's direct MCP server assignments (local and remote); requires cluster read.",
            |pool: sqlx::PgPool, p, input: ClusterMcpServersInput| async move {
                mcp_server_cluster_list(&pool, &p, input).await
            },
        );
        m.custom(
            "cluster_from_bundles",
            Risk::ReadOnly,
            OnItem::No,
            "List MCP servers a cluster gets via its assigned bundles, flagging ones shadowed by a direct assignment; requires cluster read.",
            |pool: sqlx::PgPool, p, input: ClusterMcpServersInput| async move {
                mcp_server_cluster_from_bundles(&pool, &p, input).await
            },
        );
        m.custom(
            "cluster_transitive",
            Risk::ReadOnly,
            OnItem::No,
            "List MCP servers a cluster gets transitively via skill dependencies (direct skill assignments win over bundles), flagging ones shadowed by a direct or bundle assignment; requires cluster read.",
            |pool: sqlx::PgPool, p, input: ClusterMcpServersInput| async move {
                mcp_server_cluster_transitive(&pool, &p, input).await
            },
        );
        m.custom(
            "cluster_attach",
            Risk::Mutating,
            OnItem::No,
            "Attach an MCP server to a cluster: local (mcp_server_id) or remote (skill_center_id + remote_id + slug). Pushes a per-cluster MCP sync. Requires cluster write.",
            |pool: sqlx::PgPool, p, input: ClusterMcpServerAttachInput| async move {
                mcp_server_cluster_attach(&pool, &p, input).await
            },
        );
        m.custom(
            "cluster_detach",
            Risk::Mutating,
            OnItem::No,
            "Detach an MCP server assignment from its cluster (by assignment row id) and push a per-cluster MCP sync; requires write access to the owning cluster.",
            |pool: sqlx::PgPool, p, input: ClusterMcpServerDetachInput| async move {
                mcp_server_cluster_detach(&pool, &p, input).await
            },
        );
    }

    {
        let mut b = reg.resource("mcp_bundles", "mcp_bundle", "MCP bundles");
        b.list(
            "List the MCP bundle catalog: local bundles plus remote bundles from enabled skill centers (remote ones carry skill_center_name); admin only.",
            |pool: sqlx::PgPool, p, input: McpBundlesListInput| async move {
                mcp_bundle_list(&pool, &p, input).await
            },
        );
        b.get(
            "Get a local MCP bundle (slug, name, description); admin only.",
            |pool: sqlx::PgPool, p, input: McpBundleGetInput| async move {
                mcp_bundle_get(&pool, &p, input).await
            },
        );
        b.create(
            "Create an MCP bundle with slug, name and description, and push a federation sync; admin only.",
            |pool: sqlx::PgPool, p, input: McpBundleCreateInput| async move {
                mcp_bundle_create(&pool, &p, input).await
            },
        );
        b.update(
            "Update an MCP bundle's name, description and catalog visibility, and push federation + per-cluster MCP syncs; admin only.",
            |pool: sqlx::PgPool, p, input: McpBundleUpdateInput| async move {
                mcp_bundle_update(&pool, &p, input).await
            },
        );
        b.delete(
            "Delete an MCP bundle and push federation + per-cluster MCP syncs; admin only.",
            |pool: sqlx::PgPool, p, input: McpBundleDeleteInput| async move {
                mcp_bundle_delete(&pool, &p, input).await
            },
        );
        b.custom(
            "items",
            Risk::ReadOnly,
            OnItem::No,
            "List the MCP servers contained in a bundle; admin only.",
            |pool: sqlx::PgPool, p, input: McpBundleItemsInput| async move {
                mcp_bundle_items(&pool, &p, input).await
            },
        );
        b.custom(
            "item_add",
            Risk::Mutating,
            OnItem::No,
            "Add an MCP server to a bundle and push federation + per-cluster MCP syncs; admin only.",
            |pool: sqlx::PgPool, p, input: McpBundleItemAddInput| async move {
                mcp_bundle_item_add(&pool, &p, input).await
            },
        );
        b.custom(
            "item_remove",
            Risk::Mutating,
            OnItem::No,
            "Remove an MCP server from a bundle (by bundle item row id) and push federation + per-cluster MCP syncs; admin only.",
            |pool: sqlx::PgPool, p, input: McpBundleItemRemoveInput| async move {
                mcp_bundle_item_remove(&pool, &p, input).await
            },
        );
        b.custom(
            "options_all",
            Risk::ReadOnly,
            OnItem::No,
            "List local MCP bundles as id/slug/name options (for cluster assignment); any authenticated caller.",
            |pool: sqlx::PgPool, p, input: McpBundleOptionsAllInput| async move {
                mcp_bundle_options_all(&pool, &p, input).await
            },
        );
        b.custom(
            "remote_options",
            Risk::ReadOnly,
            OnItem::No,
            "List non-hidden MCP bundles from enabled skill centers' cached catalogs; any authenticated caller.",
            |pool: sqlx::PgPool, p, input: RemoteMcpBundleOptionsInput| async move {
                mcp_bundle_remote_options(&pool, &p, input).await
            },
        );
        b.custom(
            "cluster_list",
            Risk::ReadOnly,
            OnItem::No,
            "List a cluster's MCP bundle assignments (local and remote); requires cluster read.",
            |pool: sqlx::PgPool, p, input: ClusterMcpBundlesInput| async move {
                mcp_bundle_cluster_list(&pool, &p, input).await
            },
        );
        b.custom(
            "cluster_attach",
            Risk::Mutating,
            OnItem::No,
            "Attach an MCP bundle to a cluster: local (bundle_id, rejected if it overlaps an already-assigned bundle) or remote (skill_center_id + remote_id + slug). Pushes a per-cluster MCP sync. Requires cluster write.",
            |pool: sqlx::PgPool, p, input: ClusterMcpBundleAttachInput| async move {
                mcp_bundle_cluster_attach(&pool, &p, input).await
            },
        );
        b.custom(
            "cluster_detach",
            Risk::Mutating,
            OnItem::No,
            "Detach an MCP bundle assignment from its cluster (by assignment row id) and push a per-cluster MCP sync; requires write access to the owning cluster.",
            |pool: sqlx::PgPool, p, input: ClusterMcpBundleDetachInput| async move {
                mcp_bundle_cluster_detach(&pool, &p, input).await
            },
        );
    }
}
