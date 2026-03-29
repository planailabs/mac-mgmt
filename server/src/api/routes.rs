use std::collections::HashMap;

use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::auth::{SettingAuth, SyncAuth};

// ── Existing sync routes ───────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct McpServerRow {
    slug: String,
    config_json: serde_json::Value,
    nix_packages: Vec<String>,
    is_direct: bool,
}

#[derive(serde::Serialize)]
pub(crate) struct McpServerEntry {
    config: serde_json::Value,
    nix_packages: Vec<String>,
}

#[rocket::get("/mcp-servers")]
pub async fn get_mcp_servers(
    auth: SyncAuth,
    pool: &State<PgPool>,
) -> Result<Json<HashMap<String, McpServerEntry>>, Status> {
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
    let mut result: HashMap<String, (McpServerEntry, bool)> = HashMap::new();
    for row in &rows {
        match result.get(&row.slug) {
            Some((_, true)) => {} // already have a direct assignment
            _ => {
                result.insert(
                    row.slug.clone(),
                    (McpServerEntry {
                        config: row.config_json.clone(),
                        nix_packages: row.nix_packages.clone(),
                    }, row.is_direct),
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
    slug: String,
    channel: String,
    is_direct: bool,
}

#[rocket::get("/skills?<arch>")]
pub async fn get_skills(
    auth: SyncAuth,
    pool: &State<PgPool>,
    arch: String,
) -> Result<Json<HashMap<String, String>>, Status> {
    // Fetch all skill+channel pairs with a flag indicating direct vs bundle.
    // Direct assignments win: for each slug we pick the direct row if present.
    let rows = sqlx::query_as::<_, SkillSlugChannel>(
        "SELECT s.slug, sc.channel, true AS is_direct \
         FROM customer_skills cs \
         JOIN skill_channels sc ON sc.id = cs.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE cs.customer_id = $1 \
         UNION ALL \
         SELECT s.slug, sc.channel, false AS is_direct \
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

    // For each slug, prefer the direct assignment's channel over bundle's.
    let mut slug_channel: HashMap<String, (String, bool)> = HashMap::new();
    for row in &rows {
        match slug_channel.get(&row.slug) {
            Some((_, true)) => {} // already have a direct assignment, keep it
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

// -- Skills --

#[derive(Serialize, sqlx::FromRow)]
pub(crate) struct CustomerSkillRow {
    customer_skill_id: Uuid,
    skill_slug: String,
    channel: String,
}

#[rocket::get("/setting/skills")]
pub async fn setting_list_skills(
    auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<CustomerSkillRow>>, Status> {
    let rows = sqlx::query_as::<_, CustomerSkillRow>(
        "SELECT cs.id as customer_skill_id, s.slug as skill_slug, sc.channel \
         FROM customer_skills cs \
         JOIN skill_channels sc ON sc.id = cs.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE cs.customer_id = $1 \
         ORDER BY s.slug, sc.channel",
    )
    .bind(auth.customer_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct AddSkillBody {
    skill_channel_id: Uuid,
}

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

#[derive(Serialize, sqlx::FromRow)]
pub(crate) struct CustomerBundleRow {
    customer_bundle_id: Uuid,
    bundle_slug: String,
    bundle_name: String,
}

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

#[derive(Deserialize)]
pub struct AddBundleBody {
    bundle_id: Uuid,
}

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

#[derive(Serialize, sqlx::FromRow)]
pub(crate) struct CustomerMcpServerRow {
    customer_mcp_server_id: Uuid,
    server_slug: String,
    server_name: String,
}

#[rocket::get("/setting/mcp-servers")]
pub async fn setting_list_mcp_servers(
    auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<CustomerMcpServerRow>>, Status> {
    let rows = sqlx::query_as::<_, CustomerMcpServerRow>(
        "SELECT cms.id as customer_mcp_server_id, ms.slug as server_slug, ms.name as server_name \
         FROM customer_mcp_servers cms \
         JOIN mcp_servers ms ON ms.id = cms.mcp_server_id \
         WHERE cms.customer_id = $1 \
         ORDER BY ms.slug",
    )
    .bind(auth.customer_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct AddMcpServerBody {
    mcp_server_id: Uuid,
}

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

#[derive(Serialize, sqlx::FromRow)]
pub(crate) struct CustomerMcpBundleRow {
    customer_mcp_bundle_id: Uuid,
    bundle_slug: String,
    bundle_name: String,
}

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

#[derive(Deserialize)]
pub struct AddMcpBundleBody {
    bundle_id: Uuid,
}

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

#[derive(Serialize, sqlx::FromRow)]
pub(crate) struct SkillChannelRow {
    id: Uuid,
    skill_slug: String,
    channel: String,
}

#[rocket::get("/setting/available/skill-channels")]
pub async fn setting_available_skill_channels(
    _auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<SkillChannelRow>>, Status> {
    let rows = sqlx::query_as::<_, SkillChannelRow>(
        "SELECT sc.id, s.slug as skill_slug, sc.channel \
         FROM skill_channels sc \
         JOIN skills s ON s.id = sc.skill_id \
         ORDER BY s.slug, sc.channel",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(rows))
}

#[derive(Serialize, sqlx::FromRow)]
pub(crate) struct OptionRow {
    id: Uuid,
    slug: String,
    name: String,
}

#[rocket::get("/setting/available/bundles")]
pub async fn setting_available_bundles(
    _auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<OptionRow>>, Status> {
    let rows = sqlx::query_as::<_, OptionRow>(
        "SELECT id, slug, name FROM bundles ORDER BY slug",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(rows))
}

#[rocket::get("/setting/available/mcp-servers")]
pub async fn setting_available_mcp_servers(
    _auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<OptionRow>>, Status> {
    let rows = sqlx::query_as::<_, OptionRow>(
        "SELECT id, slug, name FROM mcp_servers ORDER BY slug",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(rows))
}

#[rocket::get("/setting/available/mcp-bundles")]
pub async fn setting_available_mcp_bundles(
    _auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<Json<Vec<OptionRow>>, Status> {
    let rows = sqlx::query_as::<_, OptionRow>(
        "SELECT id, slug, name FROM mcp_server_bundles ORDER BY slug",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(rows))
}
