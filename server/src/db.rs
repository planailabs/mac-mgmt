use rocket::http::Status;
use sqlx::PgPool;

pub async fn connect(url: &str) -> PgPool {
    PgPool::connect(url)
        .await
        .expect("failed to connect to database")
}

/// The client-error status for a write that Postgres rejected on its own
/// rules: **404** when a foreign key points at a row that does not exist,
/// **409** when the row is already there. `None` means it really is an
/// internal error.
///
/// Postgres reports both as a generic database error, so a request body that
/// names an unknown id used to surface as a 500 (t4t finding #3,
/// `POST /api/setting/bundles`). Use this on writes that bind an id from the
/// request; `/api/v1` maps the same cases through `api_mcp::access::db_write`.
pub fn write_status(e: &sqlx::Error) -> Option<Status> {
    match e {
        sqlx::Error::Database(dbe) if dbe.code().as_deref() == Some("23503") => {
            Some(Status::NotFound)
        }
        sqlx::Error::Database(dbe) if dbe.code().as_deref() == Some("23505") => {
            Some(Status::Conflict)
        }
        sqlx::Error::RowNotFound => Some(Status::NotFound),
        _ => None,
    }
}
