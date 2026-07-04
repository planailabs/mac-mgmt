//! Endpoint modules. Each holds the shared handlers plus their macro-generated
//! Dioxus `#[server]` wrappers for one entity.

/// Map any error to `ApiError::Internal` — the catch-all for DB and
/// upstream-API failures in endpoint handlers.
#[cfg(feature = "server")]
pub(crate) fn internal<E: std::fmt::Display>(e: E) -> plan_ai_api_mcp::ApiError {
    plan_ai_api_mcp::ApiError::internal(e.to_string())
}

/// Fetch the owning organization of a row in `table`, or 404 naming `what`.
/// The standard first step of org-write-guarded mutations.
#[cfg(feature = "server")]
pub(crate) async fn owning_org(
    pool: &sqlx::PgPool,
    table: &str,
    id: uuid::Uuid,
    what: &str,
) -> Result<uuid::Uuid, plan_ai_api_mcp::ApiError> {
    sqlx::query_scalar::<_, uuid::Uuid>(&format!(
        "SELECT organization_id FROM {table} WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or_else(|| plan_ai_api_mcp::ApiError::not_found(format!("{what} not found")))
}

pub mod action_templates;
pub mod basic_auth;
pub mod billing;
pub mod certificates;
pub mod changedetection;
pub mod contacts;
pub mod credentials;
pub mod domains;
pub mod organizations;
pub mod sync;
pub mod tokens;
pub mod users;
pub mod webspace_hosts;
pub mod webspaces;
