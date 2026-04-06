use std::collections::HashMap;

use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use base64::Engine;
use sha2::{Sha256, Digest};

use super::auth::{AdminAuth, AuthenticatedCustomer, SettingAuth, SyncAuth};
use super::push::{self, PushChannels, PushMessage};

// ── Common routes (any valid token) ────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub(crate) struct SelfInfo {
    customer_id: Option<Uuid>,
    customer_name: Option<String>,
    token_kind: String,
}

#[utoipa::path(
    get,
    path = "/api/self",
    tag = "Common",
    summary = "Get current token identity",
    description = "Returns customer info and token kind for the authenticated token.",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Token identity", body = SelfInfo),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[rocket::get("/self")]
pub async fn get_self(
    auth: AuthenticatedCustomer,
    pool: &State<PgPool>,
) -> Result<Json<SelfInfo>, Status> {
    let name = match auth.customer_id {
        Some(cid) => {
            let n = sqlx::query_scalar::<_, String>(
                "SELECT name FROM customers WHERE id = $1",
            )
            .bind(cid)
            .fetch_one(pool.inner())
            .await
            .map_err(|_| Status::InternalServerError)?;
            Some(n)
        }
        None => None,
    };

    Ok(Json(SelfInfo {
        customer_id: auth.customer_id,
        customer_name: name,
        token_kind: auth.token_kind,
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
         FROM customer_mcp_servers cms \
         JOIN mcp_servers ms ON ms.id = cms.mcp_server_id \
         WHERE cms.customer_id = $1 \
         UNION ALL \
         SELECT ms.slug, ms.config_json, ms.nix_packages, false AS is_direct \
         FROM customer_mcp_bundles cmb \
         JOIN mcp_server_bundle_items msbi ON msbi.bundle_id = cmb.bundle_id \
         JOIN mcp_servers ms ON ms.id = msbi.mcp_server_id \
         WHERE cmb.customer_id = $1",
    )
    .bind(auth.customer_id)
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
    let winning_channels = resolve_winning_skill_channels(auth.customer_id, pool.inner()).await?;
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
    summary = "Get customer config TOML for daemon sync",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Config TOML string", body = String),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Sync token required"),
        (status = 404, description = "No config saved"),
    ),
)]
#[rocket::get("/config")]
pub async fn get_config(
    auth: SyncAuth,
    pool: &State<PgPool>,
) -> Result<String, Status> {
    let config = sqlx::query_scalar::<_, String>(
        "SELECT config_toml FROM customer_configs \
         WHERE customer_id = $1 \
         ORDER BY created_at DESC \
         LIMIT 1",
    )
    .bind(auth.customer_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    match config {
        Some(toml) => Ok(toml),
        None => Err(Status::NotFound),
    }
}

// ── Update target (for daemon self-update) ──────────────────────────

pub(crate) use mac_mgmt_common::UpdateTarget;

#[utoipa::path(
    get,
    path = "/api/update",
    tag = "Sync",
    summary = "Get the target version for this daemon",
    description = "Returns the version the daemon should update to. If an active rollout targets this customer, returns the rollout's version; otherwise returns the customer's pinned version. Null means stay on current version.",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Update target"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Sync token required"),
    ),
)]
#[rocket::get("/update")]
pub async fn get_update_target(
    auth: SyncAuth,
    pool: &State<PgPool>,
) -> Result<Json<UpdateTarget>, Status> {
    // Check for active rollout targeting this customer
    let rollout_version = sqlx::query_scalar::<_, String>(
        "SELECT r.target_version FROM rollouts r \
         JOIN rollout_stages rs ON rs.rollout_id = r.id \
         JOIN rollout_group_members rgm ON rgm.group_id = rs.group_id \
         WHERE rgm.customer_id = $1 \
           AND r.status = 'rolling' \
           AND rs.status = 'rolling' \
         ORDER BY r.created_at DESC LIMIT 1",
    )
    .bind(auth.customer_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    if let Some(ver) = rollout_version {
        return Ok(Json(UpdateTarget {
            target_version: Some(ver),
        }));
    }

    // Fall back to customer's pinned version
    let pinned = sqlx::query_scalar::<_, Option<String>>(
        "SELECT pinned_version FROM customers WHERE id = $1",
    )
    .bind(auth.customer_id)
    .fetch_one(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    Ok(Json(UpdateTarget {
        target_version: pinned,
    }))
}

#[derive(sqlx::FromRow)]
struct SkillSlugChannel {
    skill_channel_id: Uuid,
    slug: String,
    channel: String,
    is_direct: bool,
}

/// Resolve the winning skill_channel_id per slug for a customer.
/// Direct assignments beat bundle assignments for the same slug.
async fn resolve_winning_skill_channels(
    customer_id: Uuid,
    pool: &PgPool,
) -> Result<HashMap<String, Uuid>, Status> {
    let rows = sqlx::query_as::<_, SkillSlugChannel>(
        "SELECT sc.id AS skill_channel_id, s.slug, sc.channel, true AS is_direct \
         FROM customer_skills cs \
         JOIN skill_channels sc ON sc.id = cs.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE cs.customer_id = $1 \
         UNION ALL \
         SELECT sc.id AS skill_channel_id, s.slug, sc.channel, false AS is_direct \
         FROM customer_bundles cb \
         JOIN bundle_items bi ON bi.bundle_id = cb.bundle_id \
         JOIN skill_channels sc ON sc.id = bi.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE cb.customer_id = $1",
    )
    .bind(customer_id)
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
         FROM customer_skills cs \
         JOIN skill_channels sc ON sc.id = cs.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE cs.customer_id = $1 \
         UNION ALL \
         SELECT sc.id AS skill_channel_id, s.slug, sc.channel, false AS is_direct \
         FROM customer_bundles cb \
         JOIN bundle_items bi ON bi.bundle_id = cb.bundle_id \
         JOIN skill_channels sc ON sc.id = bi.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE cb.customer_id = $1",
    )
    .bind(auth.customer_id)
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
    summary = "Get JSON Schema for customer config",
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
    let schema = schemars::schema_for!(mac_mgmt_common::CustomerConfig);
    (
        rocket::http::ContentType::JSON,
        serde_json::to_string_pretty(&schema).unwrap(),
    )
}

#[utoipa::path(
    get,
    path = "/api/setting/config",
    tag = "Setting — Config",
    summary = "Get customer config TOML",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Config TOML string", body = String),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
        (status = 404, description = "No config saved"),
    ),
)]
#[rocket::get("/setting/config")]
pub async fn setting_get_config(
    auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<String, Status> {
    let config = sqlx::query_scalar::<_, String>(
        "SELECT config_toml FROM customer_configs \
         WHERE customer_id = $1 \
         ORDER BY created_at DESC \
         LIMIT 1",
    )
    .bind(auth.customer_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    match config {
        Some(toml) => Ok(toml),
        None => Err(Status::NotFound),
    }
}

#[derive(Deserialize, ToSchema)]
pub struct SetConfigBody {
    config_toml: String,
}

#[utoipa::path(
    put,
    path = "/api/setting/config",
    tag = "Setting — Config",
    summary = "Set customer config TOML",
    security(("bearer" = [])),
    request_body = SetConfigBody,
    responses(
        (status = 201, description = "Config saved"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
        (status = 422, description = "Invalid TOML config"),
    ),
)]
#[rocket::put("/setting/config", data = "<body>")]
pub async fn setting_set_config(
    auth: SettingAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    body: Json<SetConfigBody>,
) -> Result<Status, Status> {
    mac_mgmt_common::CustomerConfig::from_toml(&body.config_toml)
        .map_err(|_| Status::UnprocessableEntity)?;

    sqlx::query("INSERT INTO customer_configs (customer_id, config_toml) VALUES ($1, $2)")
        .bind(auth.customer_id)
        .bind(&body.config_toml)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;

    push::notify(channels, auth.customer_id, PushMessage::SyncConfig).await;
    Ok(Status::Created)
}

// -- Skills --

#[utoipa::path(
    get,
    path = "/api/setting/skills",
    tag = "Setting — Skills",
    summary = "List customer skill assignments",
    description = "Returns installed skill channels with `installed_bundle` indicating whether the skill comes from a bundle. `customer_skill_id` is present only for direct assignments.",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Customer skills", body = Vec<SkillChannelRow>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::get("/setting/skills")]
pub async fn setting_list_skills(
    auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<SkillChannelRow>>, Status> {
    let rows = build_skill_channel_rows(auth.customer_id, pool.inner())
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
    sqlx::query("INSERT INTO customer_skills (customer_id, skill_channel_id) VALUES ($1, $2)")
        .bind(auth.customer_id)
        .bind(body.skill_channel_id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    push::notify(channels, auth.customer_id, PushMessage::SyncSkills).await;
    Ok(Status::Created)
}

#[utoipa::path(
    delete,
    path = "/api/setting/skills/{id}",
    tag = "Setting — Skills",
    summary = "Remove a direct skill assignment",
    security(("bearer" = [])),
    params(("id" = Uuid, Path, description = "Customer skill assignment ID")),
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
    sqlx::query("DELETE FROM customer_skills WHERE id = $1")
        .bind(uuid)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    push::notify(channels, auth.customer_id, PushMessage::SyncSkills).await;
    Ok(Status::NoContent)
}

// -- Bundles --

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub(crate) struct CustomerBundleRow {
    customer_bundle_id: Uuid,
    bundle_slug: String,
    bundle_name: String,
    bundle_description: String,
}

#[utoipa::path(
    get,
    path = "/api/setting/bundles",
    tag = "Setting — Bundles",
    summary = "List customer bundle assignments",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Customer bundles", body = Vec<CustomerBundleRow>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::get("/setting/bundles")]
pub async fn setting_list_bundles(
    auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<CustomerBundleRow>>, Status> {
    let rows = sqlx::query_as::<_, CustomerBundleRow>(
        "SELECT cb.id as customer_bundle_id, b.slug as bundle_slug, b.name as bundle_name, b.description as bundle_description \
         FROM customer_bundles cb \
         JOIN bundles b ON b.id = cb.bundle_id \
         WHERE cb.customer_id = $1 \
         ORDER BY b.slug",
    )
    .bind(auth.customer_id)
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
    sqlx::query("INSERT INTO customer_bundles (customer_id, bundle_id) VALUES ($1, $2)")
        .bind(auth.customer_id)
        .bind(body.bundle_id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    push::notify(channels, auth.customer_id, PushMessage::SyncSkills).await;
    Ok(Status::Created)
}

#[utoipa::path(
    delete,
    path = "/api/setting/bundles/{id}",
    tag = "Setting — Bundles",
    summary = "Remove a bundle assignment",
    security(("bearer" = [])),
    params(("id" = Uuid, Path, description = "Customer bundle assignment ID")),
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
    sqlx::query("DELETE FROM customer_bundles WHERE id = $1")
        .bind(uuid)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    push::notify(channels, auth.customer_id, PushMessage::SyncSkills).await;
    Ok(Status::NoContent)
}

// -- MCP Servers --

#[utoipa::path(
    get,
    path = "/api/setting/mcp-servers",
    tag = "Setting — MCP Servers",
    summary = "List customer MCP server assignments",
    description = "Returns installed MCP servers with `installed_bundle` and `installed_transitive` flags. `customer_mcp_server_id` is present only for direct assignments.",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Customer MCP servers", body = Vec<McpServerOptionRow>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::get("/setting/mcp-servers")]
pub async fn setting_list_mcp_servers(
    auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<McpServerOptionRow>>, Status> {
    let rows = build_mcp_server_options(auth.customer_id, pool.inner())
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
    sqlx::query("INSERT INTO customer_mcp_servers (customer_id, mcp_server_id) VALUES ($1, $2)")
        .bind(auth.customer_id)
        .bind(body.mcp_server_id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    push::notify(channels, auth.customer_id, PushMessage::SyncMcpServers).await;
    Ok(Status::Created)
}

#[utoipa::path(
    delete,
    path = "/api/setting/mcp-servers/{id}",
    tag = "Setting — MCP Servers",
    summary = "Remove a direct MCP server assignment",
    security(("bearer" = [])),
    params(("id" = Uuid, Path, description = "Customer MCP server assignment ID")),
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
    sqlx::query("DELETE FROM customer_mcp_servers WHERE id = $1")
        .bind(uuid)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    push::notify(channels, auth.customer_id, PushMessage::SyncMcpServers).await;
    Ok(Status::NoContent)
}

// -- MCP Bundles --

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub(crate) struct CustomerMcpBundleRow {
    customer_mcp_bundle_id: Uuid,
    bundle_slug: String,
    bundle_name: String,
    bundle_description: String,
}

#[utoipa::path(
    get,
    path = "/api/setting/mcp-bundles",
    tag = "Setting — MCP Bundles",
    summary = "List customer MCP bundle assignments",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Customer MCP bundles", body = Vec<CustomerMcpBundleRow>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Setting token required"),
    ),
)]
#[rocket::get("/setting/mcp-bundles")]
pub async fn setting_list_mcp_bundles(
    auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<CustomerMcpBundleRow>>, Status> {
    let rows = sqlx::query_as::<_, CustomerMcpBundleRow>(
        "SELECT cmb.id as customer_mcp_bundle_id, msb.slug as bundle_slug, msb.name as bundle_name, msb.description as bundle_description \
         FROM customer_mcp_bundles cmb \
         JOIN mcp_server_bundles msb ON msb.id = cmb.bundle_id \
         WHERE cmb.customer_id = $1 \
         ORDER BY msb.slug",
    )
    .bind(auth.customer_id)
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
    sqlx::query("INSERT INTO customer_mcp_bundles (customer_id, bundle_id) VALUES ($1, $2)")
        .bind(auth.customer_id)
        .bind(body.bundle_id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    push::notify(channels, auth.customer_id, PushMessage::SyncMcpServers).await;
    Ok(Status::Created)
}

#[utoipa::path(
    delete,
    path = "/api/setting/mcp-bundles/{id}",
    tag = "Setting — MCP Bundles",
    summary = "Remove an MCP bundle assignment",
    security(("bearer" = [])),
    params(("id" = Uuid, Path, description = "Customer MCP bundle assignment ID")),
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
    sqlx::query("DELETE FROM customer_mcp_bundles WHERE id = $1")
        .bind(uuid)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    push::notify(channels, auth.customer_id, PushMessage::SyncMcpServers).await;
    Ok(Status::NoContent)
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
    customer_skill_id: Option<Uuid>,
}

async fn build_skill_channel_rows(
    customer_id: Uuid,
    pool: &PgPool,
) -> Result<Vec<SkillChannelRow>, Status> {
    sqlx::query_as::<_, SkillChannelRow>(
        "SELECT sc.id, s.slug as skill_slug, s.name as skill_name, s.description as skill_description, sc.channel, \
                (cs.id IS NOT NULL OR bi.id IS NOT NULL) as installed, \
                (bi.id IS NOT NULL) as installed_bundle, \
                cs.id as customer_skill_id \
         FROM skill_channels sc \
         JOIN skills s ON s.id = sc.skill_id \
         LEFT JOIN customer_skills cs ON cs.skill_channel_id = sc.id AND cs.customer_id = $1 \
         LEFT JOIN bundle_items bi ON bi.skill_channel_id = sc.id \
              AND bi.bundle_id IN (SELECT bundle_id FROM customer_bundles WHERE customer_id = $1) \
         ORDER BY s.slug, sc.channel",
    )
    .bind(customer_id)
    .fetch_all(pool)
    .await
    .map_err(|_| Status::InternalServerError)
}

#[utoipa::path(
    get,
    path = "/api/setting/available/skill-channels",
    tag = "Setting — Available",
    summary = "List all skill channels",
    description = "Returns all skill channels with an `installed` flag indicating whether the customer has this skill channel assigned (directly or via a bundle).",
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
    Ok(Json(build_skill_channel_rows(auth.customer_id, pool.inner()).await?))
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
    customer_mcp_server_id: Option<Uuid>,
}

async fn build_bundle_rows(
    customer_id: Uuid,
    pool: &PgPool,
) -> Result<Vec<OptionRow>, Status> {
    sqlx::query_as::<_, OptionRow>(
        "SELECT b.id, b.slug, b.name, b.description, \
                (cb.id IS NOT NULL) as installed \
         FROM bundles b \
         LEFT JOIN customer_bundles cb ON cb.bundle_id = b.id AND cb.customer_id = $1 \
         ORDER BY b.slug",
    )
    .bind(customer_id)
    .fetch_all(pool)
    .await
    .map_err(|_| Status::InternalServerError)
}

#[utoipa::path(
    get,
    path = "/api/setting/available/bundles",
    tag = "Setting — Available",
    summary = "List all skill bundles",
    description = "Returns all skill bundles with an `installed` flag indicating whether the customer has this bundle assigned.",
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
    Ok(Json(build_bundle_rows(auth.customer_id, pool.inner()).await?))
}

/// Fetch the set of MCP server IDs transitively required by a customer's winning skill channels.
async fn transitive_mcp_server_ids(
    customer_id: Uuid,
    pool: &PgPool,
) -> Result<std::collections::HashSet<Uuid>, Status> {
    let winning = resolve_winning_skill_channels(customer_id, pool).await?;
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
    customer_mcp_server_id: Option<Uuid>,
}

/// Build the full MCP server option list with all install flags.
async fn build_mcp_server_options(
    customer_id: Uuid,
    pool: &PgPool,
) -> Result<Vec<McpServerOptionRow>, Status> {
    let base_rows = sqlx::query_as::<_, McpServerBaseRow>(
        "SELECT ms.id, ms.slug, ms.name, ms.description, \
                (cms.id IS NOT NULL) as installed_direct, \
                (msbi.id IS NOT NULL) as installed_bundle, \
                cms.id as customer_mcp_server_id \
         FROM mcp_servers ms \
         LEFT JOIN customer_mcp_servers cms ON cms.mcp_server_id = ms.id AND cms.customer_id = $1 \
         LEFT JOIN mcp_server_bundle_items msbi ON msbi.mcp_server_id = ms.id \
              AND msbi.bundle_id IN (SELECT bundle_id FROM customer_mcp_bundles WHERE customer_id = $1) \
         ORDER BY ms.slug",
    )
    .bind(customer_id)
    .fetch_all(pool)
    .await
    .map_err(|_| Status::InternalServerError)?;

    let transitive_ids = transitive_mcp_server_ids(customer_id, pool).await?;

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
                customer_mcp_server_id: r.customer_mcp_server_id,
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
    let rows = build_mcp_server_options(auth.customer_id, pool.inner()).await?;
    Ok(Json(rows))
}

async fn build_mcp_bundle_rows(
    customer_id: Uuid,
    pool: &PgPool,
) -> Result<Vec<OptionRow>, Status> {
    sqlx::query_as::<_, OptionRow>(
        "SELECT msb.id, msb.slug, msb.name, msb.description, \
                (cmb.id IS NOT NULL) as installed \
         FROM mcp_server_bundles msb \
         LEFT JOIN customer_mcp_bundles cmb ON cmb.bundle_id = msb.id AND cmb.customer_id = $1 \
         ORDER BY msb.slug",
    )
    .bind(customer_id)
    .fetch_all(pool)
    .await
    .map_err(|_| Status::InternalServerError)
}

#[utoipa::path(
    get,
    path = "/api/setting/available/mcp-bundles",
    tag = "Setting — Available",
    summary = "List all MCP bundles",
    description = "Returns all MCP bundles with an `installed` flag indicating whether the customer has this bundle assigned.",
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
    Ok(Json(build_mcp_bundle_rows(auth.customer_id, pool.inner()).await?))
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
    let cid = auth.customer_id;
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
pub(crate) struct AdminCustomerRow {
    id: Uuid,
    name: String,
    created_at: DateTime<Utc>,
}

#[utoipa::path(
    get,
    path = "/api/admin/customers",
    tag = "Admin",
    summary = "List all customers",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "All customers", body = Vec<AdminCustomerRow>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin token required"),
    ),
)]
#[rocket::get("/admin/customers")]
pub async fn admin_list_customers(
    _auth: AdminAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<AdminCustomerRow>>, Status> {
    let rows = sqlx::query_as::<_, AdminCustomerRow>(
        "SELECT id, name, created_at FROM customers ORDER BY name",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(rows))
}

#[derive(Deserialize, ToSchema)]
pub struct CreateTokenForCustomerBody {
    label: String,
    kind: String,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct CreatedToken {
    token: String,
}

#[utoipa::path(
    post,
    path = "/api/admin/customers/{customer_id}/tokens",
    tag = "Admin",
    summary = "Create a sync or setting token for a customer",
    security(("bearer" = [])),
    params(("customer_id" = Uuid, Path, description = "Customer ID")),
    request_body = CreateTokenForCustomerBody,
    responses(
        (status = 201, description = "Token created", body = CreatedToken),
        (status = 400, description = "Invalid kind (must be sync or setting)"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Admin token required"),
    ),
)]
#[rocket::post("/admin/customers/<customer_id>/tokens", data = "<body>")]
pub async fn admin_create_token(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    customer_id: &str,
    body: Json<CreateTokenForCustomerBody>,
) -> Result<(Status, Json<CreatedToken>), Status> {
    use rand::Rng;
    use sha2::{Digest, Sha256};

    let cid: Uuid = customer_id.parse().map_err(|_| Status::BadRequest)?;

    if body.kind != "sync" && body.kind != "setting" {
        return Err(Status::BadRequest);
    }

    let label = body.label.trim();
    if label.is_empty() {
        return Err(Status::BadRequest);
    }

    let raw_token: String = hex::encode(rand::rng().random::<[u8; 32]>());
    let hash = hex::encode(Sha256::digest(raw_token.as_bytes()));

    sqlx::query("INSERT INTO tokens (customer_id, token_hash, label, kind) VALUES ($1, $2, $3, $4)")
        .bind(cid)
        .bind(&hash)
        .bind(label)
        .bind(&body.kind)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;

    Ok((Status::Created, Json(CreatedToken { token: raw_token })))
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
        "SELECT public_key FROM customer_ssh_keys WHERE customer_id = $1",
    )
    .bind(auth.customer_id)
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
    summary = "List customer SSH keys",
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
         FROM customer_ssh_keys WHERE customer_id = $1 ORDER BY created_at",
    )
    .bind(auth.customer_id)
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
        "INSERT INTO customer_ssh_keys (customer_id, public_key, comment, fingerprint) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(auth.customer_id)
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

    push::notify(channels, auth.customer_id, PushMessage::SyncSshKeys).await;
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
    sqlx::query("DELETE FROM customer_ssh_keys WHERE id = $1 AND customer_id = $2")
        .bind(uuid)
        .bind(auth.customer_id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    push::notify(channels, auth.customer_id, PushMessage::SyncSshKeys).await;
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
    sqlx::query(
        "INSERT INTO daemon_heartbeats (customer_id, instance_id, version, services) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (customer_id, instance_id) \
         DO UPDATE SET version = $3, services = $4, reported_at = now()",
    )
    .bind(auth.customer_id)
    .bind(&body.instance_id)
    .bind(&body.version)
    .bind(&body.services)
    .execute(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    Ok(Status::Ok)
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
    customer_id: Uuid,
}

#[utoipa::path(
    post,
    path = "/api/admin/rollout-groups/{group_id}/members",
    tag = "Admin — Rollouts",
    summary = "Add customer to rollout group",
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
    sqlx::query("INSERT INTO rollout_group_members (group_id, customer_id) VALUES ($1, $2)")
        .bind(gid)
        .bind(body.customer_id)
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
    path = "/api/admin/rollout-groups/{group_id}/members/{customer_id}",
    tag = "Admin — Rollouts",
    summary = "Remove customer from rollout group",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Member removed"),
    ),
)]
#[rocket::delete("/admin/rollout-groups/<group_id>/members/<customer_id>")]
pub async fn admin_remove_group_member(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    group_id: &str,
    customer_id: &str,
) -> Result<Status, Status> {
    let gid: Uuid = group_id.parse().map_err(|_| Status::BadRequest)?;
    let cid: Uuid = customer_id.parse().map_err(|_| Status::BadRequest)?;
    sqlx::query("DELETE FROM rollout_group_members WHERE group_id = $1 AND customer_id = $2")
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
    /// Target version to roll out (semver, e.g., "0.1.6")
    target_version: String,
    /// Ordered list of group IDs for the rollout stages
    group_ids: Vec<Uuid>,
}

#[utoipa::path(
    post,
    path = "/api/admin/rollouts",
    tag = "Admin — Rollouts",
    summary = "Create a new version rollout",
    description = "Creates a staged rollout to move groups of customers to a target daemon version. Validates the version is semver and prevents downgrades.",
    security(("bearer" = [])),
    request_body = CreateRolloutBody,
    responses(
        (status = 201, description = "Rollout created", body = String),
        (status = 400, description = "Bad request"),
        (status = 409, description = "Would downgrade some customers"),
    ),
)]
#[rocket::post("/admin/rollouts", data = "<body>")]
pub async fn admin_create_rollout(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    body: Json<CreateRolloutBody>,
) -> Result<Json<serde_json::Value>, Status> {
    if body.target_version.trim().is_empty() {
        return Err(Status::BadRequest);
    }

    // Validate semver
    let target_parts: Vec<u64> = body
        .target_version
        .split('.')
        .filter_map(|p| p.parse().ok())
        .collect();
    if target_parts.len() < 3 {
        return Err(Status::BadRequest);
    }

    // Check which customers would be skipped (already at higher version)
    #[derive(sqlx::FromRow)]
    struct SkippedCustomer { name: String, pinned_version: String }

    let has_all = body.group_ids.is_empty(); // "all" represented as empty group_ids
    let skipped = sqlx::query_as::<_, SkippedCustomer>(
        "SELECT c.name, c.pinned_version FROM customers c \
         WHERE c.pinned_version IS NOT NULL \
           AND c.pinned_version > $1 \
           AND ($2 OR c.id IN (\
             SELECT rgm.customer_id FROM rollout_group_members rgm \
             WHERE rgm.group_id = ANY($3)))",
    )
    .bind(&body.target_version)
    .bind(has_all)
    .bind(&body.group_ids)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    let skipped_names: Vec<String> = skipped.iter().map(|s| {
        format!("{} (v{})", s.name, s.pinned_version)
    }).collect();

    let rollout_id = Uuid::new_v4();

    let mut tx = pool.inner().begin().await.map_err(|_| Status::InternalServerError)?;

    sqlx::query("INSERT INTO rollouts (id, target_version) VALUES ($1, $2)")
        .bind(rollout_id)
        .bind(&body.target_version)
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
        result["skipped_customers"] = serde_json::json!(skipped_names);
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
    target_version: String,
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
    struct RolloutRow2 { id: Uuid, target_version: String, status: String, created_at: DateTime<Utc>, updated_at: DateTime<Utc> }

    let rollout = sqlx::query_as::<_, RolloutRow2>("SELECT id, target_version, status, created_at, updated_at FROM rollouts WHERE id = $1")
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
    super::push::notify_rollout_customers(channels.inner(), pool.inner(), rid, super::push::PushMessage::SelfUpdate).await;
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
    super::push::notify_rollout_customers(channels.inner(), pool.inner(), rid, super::push::PushMessage::SelfUpdate).await;
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
    summary = "Complete a rollout and persist config to all targeted customers",
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
    let target_version: String = sqlx::query_scalar(
        "SELECT target_version FROM rollouts WHERE id = $1",
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

    // Pin version for all targeted customers.
    sqlx::query(
        "UPDATE customers SET pinned_version = $1 WHERE id IN (\
         SELECT DISTINCT rgm.customer_id FROM rollout_stages rs \
         JOIN rollout_group_members rgm ON rgm.group_id = rs.group_id \
         WHERE rs.rollout_id = $2)",
    )
    .bind(&target_version)
    .bind(rid)
    .execute(&mut *tx)
    .await
    .map_err(|_| Status::InternalServerError)?;

    tx.commit().await.map_err(|_| Status::InternalServerError)?;
    super::push::notify_all_rollout_customers(channels.inner(), pool.inner(), rid, super::push::PushMessage::SelfUpdate).await;
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
    super::push::notify_rollout_customers(channels.inner(), pool.inner(), rid, super::push::PushMessage::SelfUpdate).await;
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
    customer_id: Uuid,
    customer_name: String,
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
    struct MRow { member_id: Uuid, customer_id: Uuid, customer_name: String }

    let members = sqlx::query_as::<_, MRow>(
        "SELECT rgm.id AS member_id, rgm.customer_id, c.name AS customer_name \
         FROM rollout_group_members rgm \
         JOIN customers c ON c.id = rgm.customer_id \
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
            customer_id: m.customer_id,
            customer_name: m.customer_name,
        }).collect(),
    }))
}
