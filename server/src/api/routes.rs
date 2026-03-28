use std::collections::HashMap;

use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use sqlx::PgPool;

use super::auth::AuthenticatedCustomer;

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
    auth: AuthenticatedCustomer,
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
    auth: AuthenticatedCustomer,
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
    auth: AuthenticatedCustomer,
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
