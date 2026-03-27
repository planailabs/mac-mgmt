use std::collections::HashMap;

use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use sqlx::PgPool;

use super::auth::AuthenticatedCustomer;

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

#[rocket::get("/skills")]
pub async fn get_skills(
    auth: AuthenticatedCustomer,
    pool: &State<PgPool>,
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
    let result = crate::xzar::resolve_store_paths(&pins, &skills);

    Ok(Json(result))
}
