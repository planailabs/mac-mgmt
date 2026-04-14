use std::collections::HashMap;

use rocket::http::{ContentType, Header, Status};
use rocket::serde::json::Json;
use rocket::State;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use base64::Engine;
use sha2::{Sha256, Digest};

use super::auth::{AdminAuth, AuthenticatedToken, SettingAuth, SyncAuth};
use super::push::{self, PushChannels, PushMessage};

// ── Common routes (any valid token) ────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub(crate) struct SelfInfo {
    cluster_id: Option<Uuid>,
    cluster_name: Option<String>,
    organization_id: Option<Uuid>,
    token_kind: String,
    /// All cluster IDs this token can access.
    cluster_ids: Vec<Uuid>,
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
            let n = sqlx::query_scalar::<_, String>(
                "SELECT name FROM clusters WHERE id = $1",
            )
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
    }))
}

// ── Existing sync routes ───────────────────────────────────────────────

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
                    (McpServerEntry {
                        config: row.config_json.clone(),
                        nix_packages: row.nix_packages.clone(),
                    }, prec),
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
                    (McpServerEntry {
                        config: row.config_json.clone(),
                        nix_packages: row.nix_packages.clone(),
                    }, 0),
                );
            }
        }
    }

    let servers: HashMap<String, McpServerEntry> = result
        .into_iter()
        .map(|(slug, (entry, _))| (slug, entry))
        .collect();

    Ok(Json(servers))
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
        Some(json) => Ok(Json(json)),
        None => Err(Status::NotFound),
    }
}

// ── Update target (for daemon self-update) ──────────────────────────

pub(crate) use mac_mgmt_common::{NixpkgsPin, UpdateTarget};

#[utoipa::path(
    get,
    path = "/api/update",
    tag = "Sync",
    summary = "Get the target version for this daemon",
    description = "Returns the version the daemon should update to. If an active rollout targets this cluster, returns the rollout's version; otherwise returns the cluster's pinned version. Null means stay on current version.",
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
    // Check for active rollout targeting this cluster. The outer Option is
    // "rollout row found?", inner is the nullable column (a rollout may carry
    // only nixpkgs_commit and leave target_version unset).
    let rollout_version: Option<Option<String>> = sqlx::query_scalar(
        "SELECT r.target_version FROM rollouts r \
         JOIN rollout_stages rs ON rs.rollout_id = r.id \
         WHERE (rs.group_id = '00000000-0000-0000-0000-000000000000'::uuid \
                OR rs.group_id IN (SELECT group_id FROM rollout_group_members WHERE cluster_id = $1)) \
           AND r.status = 'rolling' \
           AND rs.status = 'rolling' \
         ORDER BY r.created_at DESC LIMIT 1",
    )
    .bind(auth.cluster_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    let chosen: Option<String> = if let Some(Some(ver)) = rollout_version {
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
            // No pinned version: fall back to the latest semver from daemon_versions.
            None => sqlx::query_scalar::<_, String>(
                "SELECT version FROM daemon_versions ORDER BY \
                 string_to_array(version, '.')::int[] DESC LIMIT 1",
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

#[utoipa::path(
    get,
    path = "/api/nixpkgs",
    tag = "Sync",
    summary = "Get the target nixpkgs commit for this daemon",
    description = "Returns the commit the daemon should pin nixpkgs to. If an active rollout targeting this cluster carries a nixpkgs_commit, that wins; otherwise returns the cluster's persistent pin. Null means use the rolling default source.",
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
    // Outer Option = "rollout row found?", inner Option = nullable column.
    let rollout_commit: Option<Option<String>> = sqlx::query_scalar(
        "SELECT r.nixpkgs_commit FROM rollouts r \
         JOIN rollout_stages rs ON rs.rollout_id = r.id \
         WHERE (rs.group_id = '00000000-0000-0000-0000-000000000000'::uuid \
                OR rs.group_id IN (SELECT group_id FROM rollout_group_members WHERE cluster_id = $1)) \
           AND r.status = 'rolling' \
           AND rs.status = 'rolling' \
         ORDER BY r.created_at DESC LIMIT 1",
    )
    .bind(auth.cluster_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    if let Some(Some(commit)) = rollout_commit {
        return Ok(Json(NixpkgsPin { commit: Some(commit) }));
    }

    // Fall back to cluster's persistent pin.
    let pinned = sqlx::query_scalar::<_, Option<String>>(
        "SELECT nixpkgs_commit FROM clusters WHERE id = $1",
    )
    .bind(auth.cluster_id)
    .fetch_one(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    Ok(Json(NixpkgsPin { commit: pinned }))
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
                winners.insert(
                    row.slug.clone(),
                    (row.skill_channel_id, row.is_direct),
                );
            }
        }
    }

    Ok(winners.into_iter().map(|(slug, (id, _))| (slug, id)).collect())
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
                slug_channel.insert(
                    row.slug.clone(),
                    (row.channel.clone(), row.is_direct),
                );
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
    let result = crate::xzar::resolve_store_paths(&pins, &skills, &arch);

    Ok(Json(result))
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
pub async fn setting_config_schema(
    _auth: SettingAuth,
) -> (rocket::http::ContentType, String) {
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
    // Validate by deserializing into ClusterConfig
    let _: mac_mgmt_common::ClusterConfig = serde_json::from_value(body.config.clone())
        .map_err(|_| Status::UnprocessableEntity)?;

    sqlx::query("INSERT INTO cluster_configs (cluster_id, config_json) VALUES ($1, $2)")
        .bind(auth.cluster_id)
        .bind(&body.config)
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
    let _: mac_mgmt_common::ClusterConfig = serde_json::from_value(config.clone())
        .map_err(|_| Status::UnprocessableEntity)?;

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
    let mut tx = pool.inner().begin().await.map_err(|_| Status::InternalServerError)?;
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
    let mut tx = pool.inner().begin().await.map_err(|_| Status::InternalServerError)?;
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
    let mut tx = pool.inner().begin().await.map_err(|_| Status::InternalServerError)?;
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
    let mut tx = pool.inner().begin().await.map_err(|_| Status::InternalServerError)?;
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
    Ok(Json(build_skill_channel_rows(auth.cluster_id, pool.inner()).await?))
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

async fn build_bundle_rows(
    cluster_id: Uuid,
    pool: &PgPool,
) -> Result<Vec<OptionRow>, Status> {
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
    Ok(Json(build_bundle_rows(auth.cluster_id, pool.inner()).await?))
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

async fn build_mcp_bundle_rows(
    cluster_id: Uuid,
    pool: &PgPool,
) -> Result<Vec<OptionRow>, Status> {
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
    Ok(Json(build_mcp_bundle_rows(auth.cluster_id, pool.inner()).await?))
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
) -> Result<Json<Catalog>, Status> {
    let cid = auth.cluster_id;
    let p = pool.inner();

    let skill_channels = build_skill_channel_rows(cid, p).await?;
    let bundle_rows = build_bundle_rows(cid, p).await?;
    let mcp_servers = build_mcp_server_options(cid, p).await?;
    let mcp_bundle_rows = build_mcp_bundle_rows(cid, p).await?;

    // Fetch bundle membership links
    let skill_links = sqlx::query_as::<_, BundleItemLink>(
        "SELECT bundle_id, skill_channel_id FROM bundle_items",
    )
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

    sqlx::query("INSERT INTO tokens (cluster_id, token_hash, label, kind) VALUES ($1, $2, $3, $4)")
        .bind(cid)
        .bind(&hash)
        .bind(label)
        .bind(&body.kind)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;

    Ok((Status::Created, Json(CreatedToken { token: raw_token })))
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
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM organizations WHERE id = $1)",
    )
    .bind(oid)
    .fetch_one(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    if !exists {
        return Err(Status::NotFound);
    }

    let raw_token: String = hex::encode(rand::rng().random::<[u8; 32]>());
    let hash = hex::encode(Sha256::digest(raw_token.as_bytes()));

    sqlx::query("INSERT INTO tokens (organization_id, token_hash, label, kind) VALUES ($1, $2, $3, 'setting')")
        .bind(oid)
        .bind(&hash)
        .bind(label)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;

    Ok((Status::Created, Json(CreatedToken { token: raw_token })))
}

// ── Proxy token creation ─────────────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub struct ProxyTokenResponse {
    pub proxy_token: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// Create a short-lived proxy token for browser-based tunnel access.
/// Accepts admin or setting tokens. The proxy token inherits the cluster
/// scope of the creating token.
#[utoipa::path(
    post,
    path = "/api/proxy-token",
    tag = "Common",
    summary = "Create a temporary proxy token",
    description = "Creates a short-lived token (15 minutes) for accessing TCP tunnels through the relay proxy.",
    security(("bearer" = [])),
    responses(
        (status = 201, description = "Proxy token created", body = ProxyTokenResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
)]
#[rocket::post("/proxy-token")]
pub async fn create_proxy_token(
    auth: AuthenticatedToken,
    pool: &State<PgPool>,
) -> Result<(Status, Json<ProxyTokenResponse>), Status> {
    use rand::Rng;
    use sha2::{Digest, Sha256};

    if auth.token_kind != "admin" && auth.token_kind != "setting" {
        return Err(Status::Forbidden);
    }

    let raw_token: String = hex::encode(rand::rng().random::<[u8; 32]>());
    let hash = hex::encode(Sha256::digest(raw_token.as_bytes()));
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(6);

    sqlx::query(
        "INSERT INTO tokens (cluster_id, organization_id, token_hash, label, kind, expires_at) \
         VALUES ($1, $2, $3, 'proxy', 'proxy', $4)",
    )
    .bind(auth.cluster_id)
    .bind(auth.organization_id)
    .bind(&hash)
    .bind(expires_at)
    .execute(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    Ok((Status::Created, Json(ProxyTokenResponse {
        proxy_token: raw_token,
        expires_at,
    })))
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
#[rocket::post("/admin/skill-channels/<skill_channel_id>/mcp-dependencies", data = "<body>")]
pub async fn admin_add_skill_mcp_dep(
    _auth: AdminAuth,
    pool: &State<PgPool>,
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
        Ok(_) => Ok(Status::Created),
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
    skill_channel_id: &str,
    dep_id: &str,
) -> Result<Status, Status> {
    let sc_id: Uuid = skill_channel_id.parse().map_err(|_| Status::BadRequest)?;
    let d_id: Uuid = dep_id.parse().map_err(|_| Status::BadRequest)?;
    let res = sqlx::query(
        "DELETE FROM skill_mcp_dependencies WHERE id = $1 AND skill_channel_id = $2",
    )
    .bind(d_id)
    .bind(sc_id)
    .execute(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    if res.rows_affected() == 0 {
        Err(Status::NotFound)
    } else {
        Ok(Status::Ok)
    }
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

    Ok(Json(keys.into_iter().map(|k| SshKeySyncEntry { public_key: k }).collect()))
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
    let fingerprint = format!("SHA256:{}", base64::engine::general_purpose::STANDARD.encode(Sha256::digest(&b64_data)));
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

    let sample_json = body.sample.as_ref().and_then(|s| serde_json::to_value(s).ok());
    let svc_ext_json = if body.services_extended.is_empty() {
        None
    } else {
        serde_json::to_value(&body.services_extended).ok()
    };

    sqlx::query(
        "INSERT INTO daemon_heartbeats (cluster_id, instance_id, version, hostname, environment, services, tunnels, relay_proxy_hostname, nixpkgs_commit, sample, services_extended) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) \
         ON CONFLICT (cluster_id, instance_id) \
         DO UPDATE SET version = $3, hostname = $4, environment = $5, services = $6, tunnels = $7, relay_proxy_hostname = $8, nixpkgs_commit = $9, sample = $10, services_extended = $11, reported_at = now()",
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
    .execute(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

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
    let raw_pk = extract_ed25519_pubkey(&pk_bytes)
        .ok_or("invalid SSH ed25519 public key format")?;

    let verifying_key = VerifyingKey::from_bytes(raw_pk)
        .map_err(|_| "invalid ed25519 public key")?;

    // Decode signature
    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(&body.signature)
        .map_err(|_| "invalid base64 in signature")?;
    let signature = Signature::from_slice(&sig_bytes)
        .map_err(|_| "invalid ed25519 signature format")?;

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

    let raw_pk = extract_ed25519_pubkey(&pk_bytes)
        .ok_or("invalid SSH ed25519 public key format")?;
    let verifying_key = VerifyingKey::from_bytes(raw_pk)
        .map_err(|_| "invalid ed25519 public key")?;

    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature)
        .map_err(|_| "invalid base64 in signature")?;
    let signature = Signature::from_slice(&sig_bytes)
        .map_err(|_| "invalid ed25519 signature format")?;

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

    let inventory_json =
        serde_json::to_value(&body.inventory).map_err(|_| Status::BadRequest)?;
    let security_json =
        serde_json::to_value(&body.security).map_err(|_| Status::BadRequest)?;
    let collected_at = DateTime::<Utc>::from_timestamp(body.collected_at, 0)
        .ok_or(Status::BadRequest)?;

    sqlx::query(
        "INSERT INTO assessments (cluster_id, instance_id, collected_at, inventory, security) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(auth.cluster_id)
    .bind(&body.instance_id)
    .bind(collected_at)
    .bind(&inventory_json)
    .bind(&security_json)
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

    let collected_at = DateTime::<Utc>::from_timestamp(body.collected_at, 0)
        .ok_or(Status::BadRequest)?;

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
    struct Row { id: Uuid, name: String, description: String, member_count: i64 }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT rg.id, rg.name, rg.description, COUNT(rgm.id) AS member_count \
         FROM rollout_groups rg \
         LEFT JOIN rollout_group_members rgm ON rgm.group_id = rg.id \
         WHERE rg.id != '00000000-0000-0000-0000-000000000000'::uuid \
         GROUP BY rg.id ORDER BY rg.name"
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    Ok(Json(rows.into_iter().map(|r| RolloutGroupRow {
        id: r.id, name: r.name, description: r.description, member_count: r.member_count,
    }).collect()))
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

    // Validate semver if a target version is given.
    if let Some(ver) = &target_version {
        let target_parts: Vec<u64> = ver.split('.').filter_map(|p| p.parse().ok()).collect();
        if target_parts.len() < 3 {
            return Err(Status::BadRequest);
        }
    }

    // Normalize + validate nixpkgs_commit if present: 7-40 hex chars.
    let nixpkgs_commit: Option<String> = body
        .nixpkgs_commit
        .as_ref()
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty());
    if let Some(commit) = &nixpkgs_commit {
        let valid = (7..=40).contains(&commit.len())
            && commit.chars().all(|c| c.is_ascii_hexdigit());
        if !valid {
            return Err(Status::BadRequest);
        }
    }

    // Require at least one of target_version or nixpkgs_commit.
    if target_version.is_none() && nixpkgs_commit.is_none() {
        return Err(Status::BadRequest);
    }

    // Check which clusters would be skipped (already at higher version).
    // Only meaningful when a target_version is given.
    let skipped_names: Vec<String> = if let Some(ver) = &target_version {
        #[derive(sqlx::FromRow)]
        struct SkippedCluster { name: String, pinned_version: String }

        let skipped = sqlx::query_as::<_, SkippedCluster>(
            "SELECT c.name, c.pinned_version FROM clusters c \
             WHERE c.pinned_version IS NOT NULL \
               AND c.pinned_version > $1 \
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

        skipped.iter().map(|s| format!("{} (v{})", s.name, s.pinned_version)).collect()
    } else {
        Vec::new()
    };

    let rollout_id = Uuid::new_v4();

    let mut tx = pool.inner().begin().await.map_err(|_| Status::InternalServerError)?;

    sqlx::query("INSERT INTO rollouts (id, target_version, nixpkgs_commit) VALUES ($1, $2, $3)")
        .bind(rollout_id)
        .bind(&target_version)
        .bind(&nixpkgs_commit)
        .execute(&mut *tx)
        .await
        .map_err(|_| Status::InternalServerError)?;

    if body.group_ids.is_empty() {
        return Err(Status::UnprocessableEntity);
    }

    for (i, group_id) in body.group_ids.iter().enumerate() {
        sqlx::query("INSERT INTO rollout_stages (rollout_id, group_id, stage_order) VALUES ($1, $2, $3)")
            .bind(rollout_id)
            .bind(group_id)
            .bind(i as i32)
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
    struct Row { id: Uuid, status: String, created_at: DateTime<Utc>, updated_at: DateTime<Utc>, stage_count: i64 }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT r.id, r.status, r.created_at, r.updated_at, COUNT(rs.id) AS stage_count \
         FROM rollouts r \
         LEFT JOIN rollout_stages rs ON rs.rollout_id = r.id \
         GROUP BY r.id ORDER BY r.created_at DESC"
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    Ok(Json(rows.into_iter().map(|r| RolloutRow {
        id: r.id, status: r.status, created_at: r.created_at, updated_at: r.updated_at, stage_count: r.stage_count,
    }).collect()))
}

#[derive(Serialize, ToSchema)]
pub(crate) struct RolloutDetail {
    id: Uuid,
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
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
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
    struct RolloutRow2 { id: Uuid, target_version: Option<String>, nixpkgs_commit: Option<String>, status: String, created_at: DateTime<Utc>, updated_at: DateTime<Utc> }

    let rollout = sqlx::query_as::<_, RolloutRow2>("SELECT id, target_version, nixpkgs_commit, status, created_at, updated_at FROM rollouts WHERE id = $1")
        .bind(rid)
        .fetch_optional(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?
        .ok_or(Status::NotFound)?;

    #[derive(sqlx::FromRow)]
    struct StageRow { id: Uuid, group_name: String, stage_order: i32, status: String, started_at: Option<DateTime<Utc>>, completed_at: Option<DateTime<Utc>> }

    let stages = sqlx::query_as::<_, StageRow>(
        "SELECT rs.id, rg.name AS group_name, rs.stage_order, rs.status, rs.started_at, rs.completed_at \
         FROM rollout_stages rs JOIN rollout_groups rg ON rg.id = rs.group_id \
         WHERE rs.rollout_id = $1 ORDER BY rs.stage_order"
    )
    .bind(rid)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    Ok(Json(RolloutDetail {
        id: rollout.id,
        target_version: rollout.target_version,
        nixpkgs_commit: rollout.nixpkgs_commit,
        status: rollout.status,
        created_at: rollout.created_at,
        updated_at: rollout.updated_at,
        stages: stages.into_iter().map(|s| StageDetail {
            id: s.id, group_name: s.group_name, stage_order: s.stage_order,
            status: s.status, started_at: s.started_at, completed_at: s.completed_at,
        }).collect(),
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

    let mut tx = pool.inner().begin().await.map_err(|_| Status::InternalServerError)?;

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
    super::push::notify_rollout_clusters(channels.inner(), pool.inner(), rid, super::push::PushMessage::SelfUpdate).await;
    super::push::notify_rollout_clusters(channels.inner(), pool.inner(), rid, super::push::PushMessage::SyncNixpkgs).await;
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
    struct StageInfo { stage_order: i32 }

    let current = sqlx::query_as::<_, StageInfo>(
        "SELECT stage_order FROM rollout_stages WHERE rollout_id = $1 AND status = 'rolling' LIMIT 1"
    )
    .bind(rid)
    .fetch_optional(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?
    .ok_or(Status::Conflict)?;

    let mut tx = pool.inner().begin().await.map_err(|_| Status::InternalServerError)?;

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
    super::push::notify_rollout_clusters(channels.inner(), pool.inner(), rid, super::push::PushMessage::SelfUpdate).await;
    super::push::notify_rollout_clusters(channels.inner(), pool.inner(), rid, super::push::PushMessage::SyncNixpkgs).await;
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

    let mut tx = pool.inner().begin().await.map_err(|_| Status::InternalServerError)?;

    let updated = sqlx::query("UPDATE rollout_stages SET status = 'paused' WHERE rollout_id = $1 AND status = 'rolling'")
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
    let (target_version, nixpkgs_commit): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT target_version, nixpkgs_commit FROM rollouts WHERE id = $1",
    )
    .bind(rid)
    .fetch_optional(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?
    .ok_or(Status::NotFound)?;

    let mut tx = pool.inner().begin().await.map_err(|_| Status::InternalServerError)?;

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
    super::push::notify_all_rollout_clusters(channels.inner(), pool.inner(), rid, super::push::PushMessage::SelfUpdate).await;
    if nixpkgs_commit.is_some() {
        super::push::notify_all_rollout_clusters(channels.inner(), pool.inner(), rid, super::push::PushMessage::SyncNixpkgs).await;
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

    let mut tx = pool.inner().begin().await.map_err(|_| Status::InternalServerError)?;

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
    super::push::notify_rollout_clusters(channels.inner(), pool.inner(), rid, super::push::PushMessage::SelfUpdate).await;
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
    struct GRow { id: Uuid, name: String, description: String }

    let group = sqlx::query_as::<_, GRow>(
        "SELECT id, name, description FROM rollout_groups WHERE id = $1",
    )
    .bind(gid)
    .fetch_optional(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?
    .ok_or(Status::NotFound)?;

    #[derive(sqlx::FromRow)]
    struct MRow { member_id: Uuid, cluster_id: Uuid, cluster_name: String }

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
        members: members.into_iter().map(|m| GroupMemberRow {
            member_id: m.member_id,
            cluster_id: m.cluster_id,
            cluster_name: m.cluster_name,
        }).collect(),
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
pub async fn download_daemon(
    version: &str,
    system: &str,
) -> Result<BinaryDownload, Status> {
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

    let bin_path = std::path::Path::new(&store_path).join("bin").join("mac-mgmt");
    let body = tokio::fs::read(&bin_path).await.map_err(|e| {
        tracing::error!("failed to read {}: {e}", bin_path.display());
        Status::InternalServerError
    })?;

    let filename = format!("mac-mgmt-{version}-{system}");
    Ok(BinaryDownload { body, filename })
}
