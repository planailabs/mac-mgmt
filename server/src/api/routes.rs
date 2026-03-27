use rocket::http::Status;
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
