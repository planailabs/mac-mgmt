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
}

#[rocket::get("/skills")]
pub async fn get_skills(
    auth: AuthenticatedCustomer,
    pool: &State<PgPool>,
) -> Result<Json<HashMap<String, String>>, Status> {
    let rows = sqlx::query_as::<_, SkillSlugChannel>(
        "SELECT DISTINCT s.slug, sc.channel \
         FROM skill_channels sc \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE sc.id IN ( \
             SELECT skill_channel_id FROM customer_skills WHERE customer_id = $1 \
             UNION \
             SELECT bi.skill_channel_id FROM customer_bundles cb \
             JOIN bundle_items bi ON bi.bundle_id = cb.bundle_id \
             WHERE cb.customer_id = $1 \
         )",
    )
    .bind(auth.customer_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    if rows.is_empty() {
        return Ok(Json(HashMap::new()));
    }

    let cfg = crate::config::config();
    let pins = crate::xzar::fetch_pins(&cfg.xzar.url, &cfg.xzar.token)
        .await
        .map_err(|e| {
            tracing::error!("xzar fetch failed: {e}");
            Status::InternalServerError
        })?;

    let skills: Vec<(String, String)> = rows.into_iter().map(|r| (r.slug, r.channel)).collect();
    let result = crate::xzar::resolve_store_paths(&pins, &skills);

    Ok(Json(result))
}
