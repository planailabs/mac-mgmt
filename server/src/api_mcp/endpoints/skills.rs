//! Skills + bundles endpoints: skill catalog (local + skill-center remote),
//! xzar sync, skill channels (nix packages, MCP dependencies, store paths),
//! bundles CRUD with bundle items, and per-cluster skill/bundle assignment.

use std::collections::HashMap;

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

/// A catalog entry: a local skill/bundle or a remote one from a skill center.
/// Local mirror of `web::components::table_utils::CatalogEntry` (which cannot
/// carry `JsonSchema`); call sites map between the two.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CatalogEntryDto {
    /// For local items the real id; for remote items the remote catalog id.
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub hide_from_public_catalog: bool,
    /// Non-None for remote items from skill centers.
    pub skill_center_name: Option<String>,
    /// Route discriminant for local items ("skill" or "bundle").
    pub route_kind: String,
}

/// A skill (JsonSchema mirror of `models::Skill`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct SkillDto {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub hide_from_public_catalog: bool,
}

/// A skill channel (JsonSchema mirror of `models::SkillChannel`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct SkillChannelDto {
    pub id: Uuid,
    pub skill_id: Uuid,
    pub channel: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub nix_packages: Vec<String>,
}

/// A bundle (JsonSchema mirror of `models::Bundle`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct BundleDto {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub hide_from_public_catalog: bool,
}

// ── Skill catalog ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SkillListInput {}

/// was: list_skills() in web/components/skill_list.rs
#[api_mcp_dioxus_server(server = "list_skills")]
pub async fn skill_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: SkillListInput,
) -> Result<Vec<CatalogEntryDto>, ApiError> {
    p.require_admin()?;

    let local = sqlx::query_as::<_, SkillDto>("SELECT * FROM skills ORDER BY slug")
        .fetch_all(pool)
        .await
        .map_err(internal)?;

    let mut entries: Vec<CatalogEntryDto> = local
        .into_iter()
        .map(|s| CatalogEntryDto {
            id: s.id,
            slug: s.slug,
            name: s.name,
            description: s.description,
            created_at: Some(s.created_at),
            hide_from_public_catalog: s.hide_from_public_catalog,
            skill_center_name: None,
            route_kind: "skill".to_string(),
        })
        .collect();

    if let Some(cache) = crate::skill_center_cache::SkillCenterCache::global() {
        let all = cache.get_all().await;
        let sc_names: HashMap<Uuid, String> = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, name FROM skill_centers WHERE enabled = true",
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();

        for (sc_id, cached) in &all {
            let sc_name = sc_names.get(sc_id).cloned().unwrap_or_default();
            if sc_name.is_empty() {
                continue;
            }
            for ch in &cached.catalog.skill_channels {
                entries.push(CatalogEntryDto {
                    id: ch.id,
                    slug: ch.skill_slug.clone(),
                    name: ch.skill_name.clone(),
                    description: ch.skill_description.clone(),
                    created_at: None,
                    hide_from_public_catalog: ch.hidden,
                    skill_center_name: Some(sc_name.clone()),
                    route_kind: "skill".to_string(),
                });
            }
        }
    }

    entries.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(entries)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SkillSyncInput {}

/// Result of an xzar skill sync (mirror of `xzar::SyncResult`, visible in
/// WASM builds).
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SyncResult {
    pub created_skills: u32,
    pub created_channels: u32,
    pub removed_channels: u32,
    pub removed_skills: u32,
}

/// was: sync_from_xzar() in web/components/skill_list.rs
///
/// Fetch all pins from xzar, parse `skill/{slug}/{channel}` pins, and upsert
/// skills + channels into the database.
#[api_mcp_dioxus_server(server = "sync_from_xzar")]
pub async fn skill_sync_from_xzar(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: SkillSyncInput,
) -> Result<SyncResult, ApiError> {
    p.require_admin()?;
    let r = crate::xzar::sync_skills_db(pool).await.map_err(internal)?;
    Ok(SyncResult {
        created_skills: r.created_skills,
        created_channels: r.created_channels,
        removed_channels: r.removed_channels,
        removed_skills: r.removed_skills,
    })
}

// ── Skill CRUD ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SkillGetInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SkillUpdateInput {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    /// Hide the skill from the public catalog.
    pub hide_from_public_catalog: bool,
}

/// was: get_skill() in web/components/skill_detail.rs
#[api_mcp_dioxus_server(server = "get_skill")]
pub async fn skill_get(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: SkillGetInput,
) -> Result<SkillDto, ApiError> {
    p.require_admin()?;
    let skill = sqlx::query_as::<_, SkillDto>("SELECT * FROM skills WHERE id = $1")
        .bind(input.id)
        .fetch_optional(pool)
        .await
        .map_err(internal)?
        .ok_or_else(|| ApiError::not_found("skill not found"))?;
    Ok(skill)
}

/// was: update_skill() in web/components/skill_detail.rs
#[api_mcp_dioxus_server(server = "update_skill")]
pub async fn skill_update(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: SkillUpdateInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query(
        "UPDATE skills SET name = $1, description = $2, hide_from_public_catalog = $3 WHERE id = $4",
    )
    .bind(&input.name)
    .bind(&input.description)
    .bind(input.hide_from_public_catalog)
    .bind(input.id)
    .execute(pool)
    .await
    .map_err(internal)?;
    crate::api::push::notify_federation_global();
    Ok(())
}

// ── Skill channels ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SkillChannelsInput {
    /// Skill id.
    pub id: Uuid,
}

/// was: list_channels() in web/components/skill_detail.rs
#[api_mcp_dioxus_server(server = "list_channels")]
pub async fn skill_channels(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: SkillChannelsInput,
) -> Result<Vec<SkillChannelDto>, ApiError> {
    p.require_admin()?;
    let channels = sqlx::query_as::<_, SkillChannelDto>(
        "SELECT * FROM skill_channels WHERE skill_id = $1 ORDER BY channel",
    )
    .bind(input.id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(channels)
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ChannelPathsInput {
    /// Skill id.
    pub id: Uuid,
}

/// A per-architecture nix store path for a skill channel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ChannelArchPath {
    pub arch: String,
    pub path: String,
}

/// was: resolve_channel_paths() in web/components/skill_detail.rs
///
/// Resolve store paths for all channels of a skill from xzar.
#[api_mcp_dioxus_server(server = "resolve_channel_paths")]
pub async fn skill_channel_paths(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ChannelPathsInput,
) -> Result<HashMap<String, Vec<ChannelArchPath>>, ApiError> {
    p.require_admin()?;

    let slug = sqlx::query_scalar::<_, String>("SELECT slug FROM skills WHERE id = $1")
        .bind(input.id)
        .fetch_one(pool)
        .await
        .map_err(internal)?;

    let channels =
        sqlx::query_scalar::<_, String>("SELECT channel FROM skill_channels WHERE skill_id = $1")
            .bind(input.id)
            .fetch_all(pool)
            .await
            .map_err(internal)?;

    if channels.is_empty() {
        return Ok(HashMap::new());
    }

    let cfg = crate::config::config();
    let xzar = cfg
        .xzar
        .as_ref()
        .ok_or_else(|| ApiError::internal("xzar not configured"))?;
    let pins = crate::xzar::fetch_pins(&xzar.url, &xzar.token)
        .await
        .map_err(|e| ApiError::internal(format!("xzar error: {e}")))?;

    let mut result: HashMap<String, Vec<ChannelArchPath>> = HashMap::new();
    let prefix = format!("skill/{slug}/");
    for pin in &pins {
        if pin.abandoned || pin.roots.is_empty() {
            continue;
        }
        if let Some(rest) = pin.name.strip_prefix(&prefix) {
            if let Some((channel, arch)) = rest.split_once('/') {
                if channels.contains(&channel.to_string()) {
                    let path = crate::xzar::store_path_for_pin(&pins, &pin.name);
                    if let Some(path) = path {
                        result
                            .entry(channel.to_string())
                            .or_default()
                            .push(ChannelArchPath {
                                arch: arch.to_string(),
                                path,
                            });
                    }
                }
            }
        }
    }

    Ok(result)
}

// ── Channel MCP dependencies ────────────────────────────────────────────

/// An MCP server dependency of a skill channel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ChannelMcpDep {
    pub dep_id: Uuid,
    pub mcp_server_id: Uuid,
    pub mcp_server_slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ChannelMcpDepsInput {
    pub skill_channel_id: Uuid,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpServerOptionsInput {}

/// An MCP server option for the channel-dependency picker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct McpServerOption {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ChannelMcpDepAddInput {
    pub skill_channel_id: Uuid,
    pub mcp_server_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ChannelMcpDepRemoveInput {
    /// Row id of the skill_mcp_dependencies entry.
    pub dep_id: Uuid,
}

/// was: list_channel_mcp_deps() in web/components/skill_detail.rs
#[api_mcp_dioxus_server(server = "list_channel_mcp_deps")]
pub async fn skill_channel_mcp_deps(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ChannelMcpDepsInput,
) -> Result<Vec<ChannelMcpDep>, ApiError> {
    p.require_admin()?;
    let deps = sqlx::query_as::<_, ChannelMcpDep>(
        "SELECT smd.id as dep_id, smd.mcp_server_id, ms.slug as mcp_server_slug \
         FROM skill_mcp_dependencies smd \
         JOIN mcp_servers ms ON ms.id = smd.mcp_server_id \
         WHERE smd.skill_channel_id = $1 \
         ORDER BY ms.slug",
    )
    .bind(input.skill_channel_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(deps)
}

/// was: list_all_mcp_servers() in web/components/skill_detail.rs
#[api_mcp_dioxus_server(server = "list_all_mcp_servers")]
pub async fn skill_mcp_server_options(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: McpServerOptionsInput,
) -> Result<Vec<McpServerOption>, ApiError> {
    p.require_admin()?;
    let rows =
        sqlx::query_as::<_, McpServerOption>("SELECT id, slug, name FROM mcp_servers ORDER BY slug")
            .fetch_all(pool)
            .await
            .map_err(internal)?;
    Ok(rows)
}

/// was: add_channel_mcp_dep() in web/components/skill_detail.rs
#[api_mcp_dioxus_server(server = "add_channel_mcp_dep")]
pub async fn skill_channel_mcp_dep_add(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ChannelMcpDepAddInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query(
        "INSERT INTO skill_mcp_dependencies (skill_channel_id, mcp_server_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(input.skill_channel_id)
    .bind(input.mcp_server_id)
    .execute(pool)
    .await
    .map_err(internal)?;
    crate::api::push::notify_federation_global();
    Ok(())
}

/// was: remove_channel_mcp_dep() in web/components/skill_detail.rs
#[api_mcp_dioxus_server(server = "remove_channel_mcp_dep")]
pub async fn skill_channel_mcp_dep_remove(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ChannelMcpDepRemoveInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query("DELETE FROM skill_mcp_dependencies WHERE id = $1")
        .bind(input.dep_id)
        .execute(pool)
        .await
        .map_err(internal)?;
    crate::api::push::notify_federation_global();
    Ok(())
}

// ── Channel nix packages ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ChannelNixPackagesInput {
    pub channel_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ChannelNixPackageAddInput {
    pub channel_id: Uuid,
    /// Nix package attribute name.
    pub package: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ChannelNixPackageRemoveInput {
    pub channel_id: Uuid,
    pub package: String,
}

/// was: get_channel_nix_packages() in web/components/skill_detail.rs
#[api_mcp_dioxus_server(server = "get_channel_nix_packages")]
pub async fn skill_channel_nix_packages(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ChannelNixPackagesInput,
) -> Result<Vec<String>, ApiError> {
    p.require_admin()?;
    let pkgs: Vec<String> =
        sqlx::query_scalar("SELECT unnest(nix_packages) FROM skill_channels WHERE id = $1")
            .bind(input.channel_id)
            .fetch_all(pool)
            .await
            .map_err(internal)?;
    Ok(pkgs)
}

/// was: add_channel_nix_package() in web/components/skill_detail.rs
#[api_mcp_dioxus_server(server = "add_channel_nix_package")]
pub async fn skill_channel_nix_package_add(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ChannelNixPackageAddInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query(
        "UPDATE skill_channels SET nix_packages = array_append(nix_packages, $1) \
         WHERE id = $2 AND NOT ($1 = ANY(nix_packages))",
    )
    .bind(&input.package)
    .bind(input.channel_id)
    .execute(pool)
    .await
    .map_err(internal)?;
    crate::api::push::notify_federation_global();
    crate::api::push::notify_skill_channels_global(&[input.channel_id]).await;
    Ok(())
}

/// was: remove_channel_nix_package() in web/components/skill_detail.rs
#[api_mcp_dioxus_server(server = "remove_channel_nix_package")]
pub async fn skill_channel_nix_package_remove(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ChannelNixPackageRemoveInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query(
        "UPDATE skill_channels SET nix_packages = array_remove(nix_packages, $1) WHERE id = $2",
    )
    .bind(&input.package)
    .bind(input.channel_id)
    .execute(pool)
    .await
    .map_err(internal)?;
    crate::api::push::notify_federation_global();
    crate::api::push::notify_skill_channels_global(&[input.channel_id]).await;
    Ok(())
}

// ── Bundle catalog / CRUD ───────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BundleListInput {}

/// was: list_bundles() in web/components/bundle_list.rs
#[api_mcp_dioxus_server(server = "list_bundles")]
pub async fn bundle_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: BundleListInput,
) -> Result<Vec<CatalogEntryDto>, ApiError> {
    p.require_admin()?;

    let local = sqlx::query_as::<_, BundleDto>("SELECT * FROM bundles ORDER BY slug")
        .fetch_all(pool)
        .await
        .map_err(internal)?;

    let mut entries: Vec<CatalogEntryDto> = local
        .into_iter()
        .map(|b| CatalogEntryDto {
            id: b.id,
            slug: b.slug,
            name: b.name,
            description: b.description,
            created_at: Some(b.created_at),
            hide_from_public_catalog: b.hide_from_public_catalog,
            skill_center_name: None,
            route_kind: "bundle".to_string(),
        })
        .collect();

    if let Some(cache) = crate::skill_center_cache::SkillCenterCache::global() {
        let all = cache.get_all().await;
        let sc_names: HashMap<Uuid, String> = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, name FROM skill_centers WHERE enabled = true",
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();

        for (sc_id, cached) in &all {
            let sc_name = sc_names.get(sc_id).cloned().unwrap_or_default();
            if sc_name.is_empty() {
                continue;
            }
            for bundle in &cached.catalog.bundles {
                entries.push(CatalogEntryDto {
                    id: bundle.id,
                    slug: bundle.slug.clone(),
                    name: bundle.name.clone(),
                    description: bundle.description.clone(),
                    created_at: None,
                    hide_from_public_catalog: bundle.hidden,
                    skill_center_name: Some(sc_name.clone()),
                    route_kind: "bundle".to_string(),
                });
            }
        }
    }

    entries.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(entries)
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BundleGetInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BundleCreateInput {
    pub slug: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BundleUpdateInput {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    /// Hide the bundle from the public catalog.
    pub hide_from_public_catalog: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BundleDeleteInput {
    pub id: Uuid,
}

/// was: get_bundle() in web/components/bundle_detail.rs
#[api_mcp_dioxus_server(server = "get_bundle")]
pub async fn bundle_get(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: BundleGetInput,
) -> Result<BundleDto, ApiError> {
    p.require_admin()?;
    let bundle = sqlx::query_as::<_, BundleDto>("SELECT * FROM bundles WHERE id = $1")
        .bind(input.id)
        .fetch_optional(pool)
        .await
        .map_err(internal)?
        .ok_or_else(|| ApiError::not_found("bundle not found"))?;
    Ok(bundle)
}

/// was: create_bundle() in web/components/bundle_form.rs
#[api_mcp_dioxus_server(server = "create_bundle")]
pub async fn bundle_create(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: BundleCreateInput,
) -> Result<BundleDto, ApiError> {
    p.require_admin()?;
    let bundle = sqlx::query_as::<_, BundleDto>(
        "INSERT INTO bundles (slug, name, description) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(&input.slug)
    .bind(&input.name)
    .bind(&input.description)
    .fetch_one(pool)
    .await
    .map_err(internal)?;
    crate::api::push::notify_federation_global();
    Ok(bundle)
}

/// was: update_bundle() in web/components/bundle_detail.rs
#[api_mcp_dioxus_server(server = "update_bundle")]
pub async fn bundle_update(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: BundleUpdateInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query(
        "UPDATE bundles SET name = $1, description = $2, hide_from_public_catalog = $3 WHERE id = $4",
    )
    .bind(&input.name)
    .bind(&input.description)
    .bind(input.hide_from_public_catalog)
    .bind(input.id)
    .execute(pool)
    .await
    .map_err(internal)?;
    crate::api::push::notify_federation_global();
    crate::api::push::notify_skill_bundle_global(input.id).await;
    Ok(())
}

/// was: delete_bundle() in web/components/bundle_detail.rs
#[api_mcp_dioxus_server(server = "delete_bundle")]
pub async fn bundle_delete(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: BundleDeleteInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    crate::api::push::notify_federation_global();
    crate::api::push::notify_skill_bundle_global(input.id).await;
    sqlx::query("DELETE FROM bundles WHERE id = $1")
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

// ── Bundle items ────────────────────────────────────────────────────────

/// A skill channel with its skill slug for display/pickers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct SkillChannelDisplay {
    pub id: Uuid,
    pub skill_slug: String,
    pub channel: String,
}

/// A bundle item joined with skill info for display.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct BundleItemDisplay {
    pub bundle_item_id: Uuid,
    pub skill_slug: String,
    pub channel: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BundleItemsInput {
    /// Bundle id.
    pub id: Uuid,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AvailableChannelsInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BundleItemAddInput {
    /// Bundle id.
    pub id: Uuid,
    pub skill_channel_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BundleItemRemoveInput {
    /// Row id of the bundle_items entry.
    pub bundle_item_id: Uuid,
}

/// was: list_bundle_items() in web/components/bundle_detail.rs
#[api_mcp_dioxus_server(server = "list_bundle_items")]
pub async fn bundle_items(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: BundleItemsInput,
) -> Result<Vec<BundleItemDisplay>, ApiError> {
    p.require_admin()?;
    let items = sqlx::query_as::<_, BundleItemDisplay>(
        "SELECT bi.id as bundle_item_id, s.slug as skill_slug, sc.channel \
         FROM bundle_items bi \
         JOIN skill_channels sc ON sc.id = bi.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE bi.bundle_id = $1 \
         ORDER BY s.slug, sc.channel",
    )
    .bind(input.id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(items)
}

/// was: list_available_skill_channels() in web/components/bundle_detail.rs
#[api_mcp_dioxus_server(server = "list_available_skill_channels")]
pub async fn bundle_available_channels(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: AvailableChannelsInput,
) -> Result<Vec<SkillChannelDisplay>, ApiError> {
    p.require_admin()?;
    let channels = sqlx::query_as::<_, SkillChannelDisplay>(
        "SELECT sc.id, s.slug as skill_slug, sc.channel \
         FROM skill_channels sc \
         JOIN skills s ON s.id = sc.skill_id \
         ORDER BY s.slug, sc.channel",
    )
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(channels)
}

/// was: add_bundle_item() in web/components/bundle_detail.rs
#[api_mcp_dioxus_server(server = "add_bundle_item")]
pub async fn bundle_item_add(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: BundleItemAddInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query("INSERT INTO bundle_items (bundle_id, skill_channel_id) VALUES ($1, $2)")
        .bind(input.id)
        .bind(input.skill_channel_id)
        .execute(pool)
        .await
        .map_err(internal)?;
    crate::api::push::notify_federation_global();
    crate::api::push::notify_skill_bundle_global(input.id).await;
    Ok(())
}

/// was: remove_bundle_item() in web/components/bundle_detail.rs
#[api_mcp_dioxus_server(server = "remove_bundle_item")]
pub async fn bundle_item_remove(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: BundleItemRemoveInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    let bundle_id: Option<Uuid> =
        sqlx::query_scalar("SELECT bundle_id FROM bundle_items WHERE id = $1")
            .bind(input.bundle_item_id)
            .fetch_optional(pool)
            .await
            .map_err(internal)?;
    sqlx::query("DELETE FROM bundle_items WHERE id = $1")
        .bind(input.bundle_item_id)
        .execute(pool)
        .await
        .map_err(internal)?;
    if let Some(bid) = bundle_id {
        crate::api::push::notify_federation_global();
        crate::api::push::notify_skill_bundle_global(bid).await;
    }
    Ok(())
}

// ── Cluster skill assignment ────────────────────────────────────────────

/// Direct skill assignment display.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ClusterSkillDisplay {
    pub cluster_skill_id: Uuid,
    pub skill_slug: String,
    pub channel: String,
    #[serde(default)]
    pub skill_center_name: Option<String>,
}

/// Skill coming from a bundle (read-only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BundleSkillDisplay {
    pub skill_slug: String,
    pub channel: String,
    pub bundle_slug: String,
    pub overwritten: bool,
}

/// Bundle assignment display.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ClusterBundleDisplay {
    pub cluster_bundle_id: Uuid,
    pub bundle_slug: String,
    pub bundle_name: String,
    #[serde(default)]
    pub skill_center_name: Option<String>,
}

/// A bundle option for the assignment dropdown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct BundleOption {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
}

/// Remote skill option for add-item dropdown (from skill center catalogs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RemoteSkillOption {
    pub skill_center_id: String,
    pub skill_center_name: String,
    pub remote_skill_channel_id: String,
    pub skill_slug: String,
    pub skill_name: String,
    pub channel: String,
}

/// Remote bundle option for add-item dropdown (from skill center catalogs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RemoteBundleOption {
    pub skill_center_id: String,
    pub skill_center_name: String,
    pub remote_bundle_id: String,
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterSkillsListInput {
    pub cluster_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterBundlesListInput {
    pub cluster_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterBundleSkillsInput {
    pub cluster_id: Uuid,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SkillChannelOptionsInput {}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BundleOptionsInput {}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RemoteSkillOptionsInput {}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RemoteBundleOptionsInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterSkillAddInput {
    pub cluster_id: Uuid,
    /// Local skill channel to assign (local skills).
    pub skill_channel_id: Option<Uuid>,
    /// Skill center id (remote skills).
    pub skill_center_id: Option<Uuid>,
    /// Remote skill channel id (remote skills).
    pub remote_id: Option<Uuid>,
    /// Skill slug (remote skills).
    pub slug: Option<String>,
    /// Channel name (remote skills).
    pub channel: Option<String>,
    /// Display name (remote skills).
    pub skill_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterSkillRemoveInput {
    /// Row id of the cluster_skills entry.
    pub cluster_skill_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterBundleAddInput {
    pub cluster_id: Uuid,
    /// Local bundle to assign (local bundles).
    pub bundle_id: Option<Uuid>,
    /// Skill center id (remote bundles).
    pub skill_center_id: Option<Uuid>,
    /// Remote bundle id (remote bundles).
    pub remote_id: Option<Uuid>,
    /// Bundle slug (remote bundles).
    pub slug: Option<String>,
    /// Display name (remote bundles).
    pub bundle_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterBundleRemoveInput {
    /// Row id of the cluster_bundles entry.
    pub cluster_bundle_id: Uuid,
}

/// was: list_cluster_skills() in web/components/cluster_skills.rs
#[api_mcp_dioxus_server(server = "list_cluster_skills")]
pub async fn cluster_skill_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterSkillsListInput,
) -> Result<Vec<ClusterSkillDisplay>, ApiError> {
    access::require_cluster_read(pool, p, input.cluster_id).await?;
    let skills = sqlx::query_as::<_, ClusterSkillDisplay>(
        "SELECT cs.id AS cluster_skill_id, \
                COALESCE(s.slug, cs.slug) AS skill_slug, \
                COALESCE(sc_ch.channel, cs.channel) AS channel, \
                sk_center.name AS skill_center_name \
         FROM cluster_skills cs \
         LEFT JOIN skill_channels sc_ch ON sc_ch.id = cs.skill_channel_id \
         LEFT JOIN skills s ON s.id = sc_ch.skill_id \
         LEFT JOIN skill_centers sk_center ON sk_center.id = cs.skill_center_id \
         WHERE cs.cluster_id = $1 \
         ORDER BY COALESCE(s.slug, cs.slug), COALESCE(sc_ch.channel, cs.channel)",
    )
    .bind(input.cluster_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(skills)
}

/// was: list_cluster_bundles() in web/components/cluster_skills.rs
#[api_mcp_dioxus_server(server = "list_cluster_bundles")]
pub async fn cluster_bundle_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterBundlesListInput,
) -> Result<Vec<ClusterBundleDisplay>, ApiError> {
    access::require_cluster_read(pool, p, input.cluster_id).await?;
    let bundles = sqlx::query_as::<_, ClusterBundleDisplay>(
        "SELECT cb.id AS cluster_bundle_id, \
                COALESCE(b.slug, cb.slug) AS bundle_slug, \
                COALESCE(b.name, cb.bundle_name) AS bundle_name, \
                sk_center.name AS skill_center_name \
         FROM cluster_bundles cb \
         LEFT JOIN bundles b ON b.id = cb.bundle_id \
         LEFT JOIN skill_centers sk_center ON sk_center.id = cb.skill_center_id \
         WHERE cb.cluster_id = $1 \
         ORDER BY COALESCE(b.slug, cb.slug)",
    )
    .bind(input.cluster_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(bundles)
}

/// was: list_bundle_skills() in web/components/cluster_skills.rs
#[api_mcp_dioxus_server(server = "list_bundle_skills")]
pub async fn cluster_bundle_skills(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterBundleSkillsInput,
) -> Result<Vec<BundleSkillDisplay>, ApiError> {
    access::require_cluster_read(pool, p, input.cluster_id).await?;

    #[derive(sqlx::FromRow)]
    struct Row {
        skill_slug: String,
        channel: String,
        bundle_slug: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT DISTINCT s.slug as skill_slug, sc.channel, b.slug as bundle_slug \
         FROM cluster_bundles cb \
         JOIN bundle_items bi ON bi.bundle_id = cb.bundle_id \
         JOIN skill_channels sc ON sc.id = bi.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         JOIN bundles b ON b.id = cb.bundle_id \
         WHERE cb.cluster_id = $1 \
         ORDER BY s.slug, sc.channel",
    )
    .bind(input.cluster_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    let direct_slugs: std::collections::HashSet<String> = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT s.slug \
         FROM cluster_skills cs \
         JOIN skill_channels sc ON sc.id = cs.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE cs.cluster_id = $1",
    )
    .bind(input.cluster_id)
    .fetch_all(pool)
    .await
    .map_err(internal)?
    .into_iter()
    .collect();

    Ok(rows
        .into_iter()
        .map(|r| BundleSkillDisplay {
            overwritten: direct_slugs.contains(&r.skill_slug),
            skill_slug: r.skill_slug,
            channel: r.channel,
            bundle_slug: r.bundle_slug,
        })
        .collect())
}

/// was: list_all_skill_channels() in web/components/cluster_skills.rs
/// (any authenticated user; no extra permission check, as before)
#[api_mcp_dioxus_server(server = "list_all_skill_channels")]
pub async fn skill_channel_options(
    pool: &sqlx::PgPool,
    _p: &Principal,
    _input: SkillChannelOptionsInput,
) -> Result<Vec<SkillChannelDisplay>, ApiError> {
    let channels = sqlx::query_as::<_, SkillChannelDisplay>(
        "SELECT sc.id, s.slug as skill_slug, sc.channel \
         FROM skill_channels sc \
         JOIN skills s ON s.id = sc.skill_id \
         ORDER BY s.slug, sc.channel",
    )
    .fetch_all(pool)
    .await
    .map_err(internal)?;
    Ok(channels)
}

/// was: list_all_bundles() in web/components/cluster_skills.rs
/// (any authenticated user; no extra permission check, as before)
#[api_mcp_dioxus_server(server = "list_all_bundles")]
pub async fn bundle_options(
    pool: &sqlx::PgPool,
    _p: &Principal,
    _input: BundleOptionsInput,
) -> Result<Vec<BundleOption>, ApiError> {
    let bundles =
        sqlx::query_as::<_, BundleOption>("SELECT id, slug, name FROM bundles ORDER BY slug")
            .fetch_all(pool)
            .await
            .map_err(internal)?;
    Ok(bundles)
}

/// was: add_cluster_skill() in web/components/cluster_skills.rs
#[api_mcp_dioxus_server(server = "add_cluster_skill")]
pub async fn cluster_skill_add(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterSkillAddInput,
) -> Result<(), ApiError> {
    let cid = input.cluster_id;
    access::require_cluster_write(pool, p, cid).await?;
    if let Some(sc_id) = input.skill_center_id {
        let r_id = input
            .remote_id
            .ok_or_else(|| ApiError::bad_request("remote_id required for remote skill"))?;
        let slug = input
            .slug
            .ok_or_else(|| ApiError::bad_request("slug required for remote skill"))?;
        let channel = input
            .channel
            .ok_or_else(|| ApiError::bad_request("channel required for remote skill"))?;
        sqlx::query(
            "INSERT INTO cluster_skills (cluster_id, skill_center_id, remote_id, slug, channel, skill_name) \
             VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING",
        )
        .bind(cid)
        .bind(sc_id)
        .bind(r_id)
        .bind(&slug)
        .bind(&channel)
        .bind(input.skill_name.as_deref())
        .execute(pool)
        .await
        .map_err(internal)?;
    } else {
        let scid = input
            .skill_channel_id
            .ok_or_else(|| ApiError::bad_request("skill_channel_id required for local skill"))?;
        sqlx::query("INSERT INTO cluster_skills (cluster_id, skill_channel_id) VALUES ($1, $2)")
            .bind(cid)
            .bind(scid)
            .execute(pool)
            .await
            .map_err(internal)?;
    }
    crate::api::push::notify_global(cid, crate::api::push::PushMessage::SyncSkills).await;
    Ok(())
}

/// was: remove_cluster_skill() in web/components/cluster_skills.rs
#[api_mcp_dioxus_server(server = "remove_cluster_skill")]
pub async fn cluster_skill_remove(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterSkillRemoveInput,
) -> Result<(), ApiError> {
    let owner_cid =
        sqlx::query_scalar::<_, Uuid>("SELECT cluster_id FROM cluster_skills WHERE id = $1")
            .bind(input.cluster_skill_id)
            .fetch_optional(pool)
            .await
            .map_err(internal)?;
    if let Some(owner_cid) = owner_cid {
        access::require_cluster_write(pool, p, owner_cid).await?;
    }
    let cid = sqlx::query_scalar::<_, Uuid>(
        "DELETE FROM cluster_skills WHERE id = $1 RETURNING cluster_id",
    )
    .bind(input.cluster_skill_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?;
    if let Some(cid) = cid {
        crate::api::push::notify_global(cid, crate::api::push::PushMessage::SyncSkills).await;
    }
    Ok(())
}

/// was: add_cluster_bundle() in web/components/cluster_skills.rs
#[api_mcp_dioxus_server(server = "add_cluster_bundle")]
pub async fn cluster_bundle_add(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterBundleAddInput,
) -> Result<(), ApiError> {
    let cid = input.cluster_id;
    access::require_cluster_write(pool, p, cid).await?;
    if let Some(sc_id) = input.skill_center_id {
        let r_id = input
            .remote_id
            .ok_or_else(|| ApiError::bad_request("remote_id required for remote bundle"))?;
        let slug = input
            .slug
            .ok_or_else(|| ApiError::bad_request("slug required for remote bundle"))?;
        sqlx::query(
            "INSERT INTO cluster_bundles (cluster_id, skill_center_id, remote_id, slug, bundle_name) \
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
            .ok_or_else(|| ApiError::bad_request("bundle_id required for local bundle"))?;

        let overlap = sqlx::query_scalar::<_, String>(
            "SELECT s.slug || '/' || sc.channel \
             FROM bundle_items new_bi \
             JOIN bundle_items existing_bi ON existing_bi.skill_channel_id = new_bi.skill_channel_id \
             JOIN cluster_bundles cb ON cb.bundle_id = existing_bi.bundle_id AND cb.cluster_id = $1 \
             JOIN skill_channels sc ON sc.id = new_bi.skill_channel_id \
             JOIN skills s ON s.id = sc.skill_id \
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
                "bundle conflicts with an already-assigned bundle on skill channel: {conflicting}"
            )));
        }

        sqlx::query("INSERT INTO cluster_bundles (cluster_id, bundle_id) VALUES ($1, $2)")
            .bind(cid)
            .bind(bid)
            .execute(pool)
            .await
            .map_err(internal)?;
    }
    crate::api::push::notify_global(cid, crate::api::push::PushMessage::SyncSkills).await;
    Ok(())
}

/// was: remove_cluster_bundle() in web/components/cluster_skills.rs
#[api_mcp_dioxus_server(server = "remove_cluster_bundle")]
pub async fn cluster_bundle_remove(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterBundleRemoveInput,
) -> Result<(), ApiError> {
    let owner_cid =
        sqlx::query_scalar::<_, Uuid>("SELECT cluster_id FROM cluster_bundles WHERE id = $1")
            .bind(input.cluster_bundle_id)
            .fetch_optional(pool)
            .await
            .map_err(internal)?;
    if let Some(owner_cid) = owner_cid {
        access::require_cluster_write(pool, p, owner_cid).await?;
    }
    let cid = sqlx::query_scalar::<_, Uuid>(
        "DELETE FROM cluster_bundles WHERE id = $1 RETURNING cluster_id",
    )
    .bind(input.cluster_bundle_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?;
    if let Some(cid) = cid {
        crate::api::push::notify_global(cid, crate::api::push::PushMessage::SyncSkills).await;
    }
    Ok(())
}

/// was: list_remote_skill_options() in web/components/cluster_skills.rs
/// (any authenticated user; no extra permission check, as before)
#[api_mcp_dioxus_server(server = "list_remote_skill_options")]
pub async fn remote_skill_options(
    pool: &sqlx::PgPool,
    _p: &Principal,
    _input: RemoteSkillOptionsInput,
) -> Result<Vec<RemoteSkillOption>, ApiError> {
    let cache = crate::skill_center_cache::SkillCenterCache::global()
        .ok_or_else(|| ApiError::internal("skill center cache not initialized"))?;

    let sc_rows: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT id, name FROM skill_centers WHERE enabled = true")
            .fetch_all(pool)
            .await
            .map_err(internal)?;
    let sc_names: HashMap<Uuid, String> = sc_rows.into_iter().collect();

    let catalogs = cache.get_all().await;
    let mut result = Vec::new();
    for (sc_id, cached) in &catalogs {
        let sc_name = sc_names.get(sc_id).cloned().unwrap_or_default();
        if sc_name.is_empty() {
            continue;
        }
        for sc in &cached.catalog.skill_channels {
            if sc.hidden {
                continue;
            }
            result.push(RemoteSkillOption {
                skill_center_id: sc_id.to_string(),
                skill_center_name: sc_name.clone(),
                remote_skill_channel_id: sc.id.to_string(),
                skill_slug: sc.skill_slug.clone(),
                skill_name: sc.skill_name.clone(),
                channel: sc.channel.clone(),
            });
        }
    }
    result.sort_by(|a, b| {
        a.skill_center_name
            .cmp(&b.skill_center_name)
            .then(a.skill_slug.cmp(&b.skill_slug))
            .then(a.channel.cmp(&b.channel))
    });
    Ok(result)
}

/// was: list_remote_bundle_options() in web/components/cluster_skills.rs
/// (any authenticated user; no extra permission check, as before)
#[api_mcp_dioxus_server(server = "list_remote_bundle_options")]
pub async fn remote_bundle_options(
    pool: &sqlx::PgPool,
    _p: &Principal,
    _input: RemoteBundleOptionsInput,
) -> Result<Vec<RemoteBundleOption>, ApiError> {
    let cache = crate::skill_center_cache::SkillCenterCache::global()
        .ok_or_else(|| ApiError::internal("skill center cache not initialized"))?;

    let sc_rows: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT id, name FROM skill_centers WHERE enabled = true")
            .fetch_all(pool)
            .await
            .map_err(internal)?;
    let sc_names: HashMap<Uuid, String> = sc_rows.into_iter().collect();

    let catalogs = cache.get_all().await;
    let mut result = Vec::new();
    for (sc_id, cached) in &catalogs {
        let sc_name = sc_names.get(sc_id).cloned().unwrap_or_default();
        if sc_name.is_empty() {
            continue;
        }
        for b in &cached.catalog.bundles {
            if b.hidden {
                continue;
            }
            result.push(RemoteBundleOption {
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

// ── Registration ────────────────────────────────────────────────────────
