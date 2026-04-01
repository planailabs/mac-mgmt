use std::collections::HashMap;

use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use super::auth::{AdminAuth, AuthenticatedCustomer, SettingAuth, SyncAuth};

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

#[derive(Serialize, ToSchema)]
pub(crate) struct McpServerEntry {
    config: serde_json::Value,
    nix_packages: Vec<String>,
}

#[utoipa::path(
    get,
    path = "/api/mcp-servers",
    tag = "Sync",
    summary = "List MCP servers for daemon sync",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Map of slug to MCP server entry", body = HashMap<String, McpServerEntry>),
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
    let pins = crate::xzar::fetch_pins(&cfg.xzar.url, &cfg.xzar.token)
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
    body: Json<AddSkillBody>,
) -> Result<Status, Status> {
    sqlx::query("INSERT INTO customer_skills (customer_id, skill_channel_id) VALUES ($1, $2)")
        .bind(auth.customer_id)
        .bind(body.skill_channel_id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
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
    _auth: SettingAuth,
    pool: &State<PgPool>,
    id: &str,
) -> Result<Status, Status> {
    let uuid: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    sqlx::query("DELETE FROM customer_skills WHERE id = $1")
        .bind(uuid)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    Ok(Status::NoContent)
}

// -- Bundles --

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub(crate) struct CustomerBundleRow {
    customer_bundle_id: Uuid,
    bundle_slug: String,
    bundle_name: String,
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
        "SELECT cb.id as customer_bundle_id, b.slug as bundle_slug, b.name as bundle_name \
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
    body: Json<AddBundleBody>,
) -> Result<Status, Status> {
    sqlx::query("INSERT INTO customer_bundles (customer_id, bundle_id) VALUES ($1, $2)")
        .bind(auth.customer_id)
        .bind(body.bundle_id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
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
    _auth: SettingAuth,
    pool: &State<PgPool>,
    id: &str,
) -> Result<Status, Status> {
    let uuid: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    sqlx::query("DELETE FROM customer_bundles WHERE id = $1")
        .bind(uuid)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
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
    body: Json<AddMcpServerBody>,
) -> Result<Status, Status> {
    sqlx::query("INSERT INTO customer_mcp_servers (customer_id, mcp_server_id) VALUES ($1, $2)")
        .bind(auth.customer_id)
        .bind(body.mcp_server_id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
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
    _auth: SettingAuth,
    pool: &State<PgPool>,
    id: &str,
) -> Result<Status, Status> {
    let uuid: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    sqlx::query("DELETE FROM customer_mcp_servers WHERE id = $1")
        .bind(uuid)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    Ok(Status::NoContent)
}

// -- MCP Bundles --

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub(crate) struct CustomerMcpBundleRow {
    customer_mcp_bundle_id: Uuid,
    bundle_slug: String,
    bundle_name: String,
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
        "SELECT cmb.id as customer_mcp_bundle_id, msb.slug as bundle_slug, msb.name as bundle_name \
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
    body: Json<AddMcpBundleBody>,
) -> Result<Status, Status> {
    sqlx::query("INSERT INTO customer_mcp_bundles (customer_id, bundle_id) VALUES ($1, $2)")
        .bind(auth.customer_id)
        .bind(body.bundle_id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
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
    _auth: SettingAuth,
    pool: &State<PgPool>,
    id: &str,
) -> Result<Status, Status> {
    let uuid: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    sqlx::query("DELETE FROM customer_mcp_bundles WHERE id = $1")
        .bind(uuid)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    Ok(Status::NoContent)
}

// -- Available resources (for dropdowns) --

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub(crate) struct SkillChannelRow {
    id: Uuid,
    skill_slug: String,
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
        "SELECT sc.id, s.slug as skill_slug, sc.channel, \
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
    installed: bool,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct McpServerOptionRow {
    id: Uuid,
    slug: String,
    name: String,
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
        "SELECT b.id, b.slug, b.name, \
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
        "SELECT ms.id, ms.slug, ms.name, \
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
        "SELECT msb.id, msb.slug, msb.name, \
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

// -- Catalog (combined view) --

#[derive(Serialize, ToSchema)]
pub(crate) struct Catalog {
    skill_channels: Vec<SkillChannelRow>,
    bundles: Vec<OptionRow>,
    mcp_servers: Vec<McpServerOptionRow>,
    mcp_bundles: Vec<OptionRow>,
}

#[utoipa::path(
    get,
    path = "/api/setting/catalog",
    tag = "Setting — Available",
    summary = "List all available resources with install status",
    description = "Returns all skill channels, bundles, MCP servers and MCP bundles in a single response, each with an `installed` flag indicating whether the customer has it assigned.",
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
    let bundles = build_bundle_rows(cid, p).await?;
    let mcp_servers = build_mcp_server_options(cid, p).await?;
    let mcp_bundles = build_mcp_bundle_rows(cid, p).await?;

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
        "SELECT smd.id, smd.skill_channel_id, smd.mcp_server_id, ms.slug AS mcp_server_slug \
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
