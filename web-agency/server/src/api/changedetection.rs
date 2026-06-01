//! ChangeDetection.io webhook receiver.
//!
//! `POST /api/changedetection/{secret}` — accept notification from changedetection.io
//!
//! Auth: the secret in the URL is the auth token (stored per sub-URL in
//! `changedetection_suburls`).

use dioxus::fullstack::axum::{
    self as axum, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::post,
};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct ChangeDetectionState {
    pub pool: PgPool,
}

pub fn router(state: ChangeDetectionState) -> Router<()> {
    Router::new()
        .route("/api/changedetection/{secret}", post(receive_notification))
        .with_state(state)
}

#[derive(serde::Deserialize)]
struct NotificationBody {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    body: Option<String>,
}

async fn receive_notification(
    State(state): State<ChangeDetectionState>,
    Path(secret): Path<String>,
    axum::Json(payload): axum::Json<NotificationBody>,
) -> StatusCode {
    let suburl = match sqlx::query_as::<_, (Uuid, Uuid)>(
        "SELECT id, webspace_host_id FROM changedetection_suburls WHERE secret = $1",
    )
    .bind(&secret)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(row) => row,
        Err(e) => {
            tracing::error!("changedetection webhook db error: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    let Some((suburl_id, host_id)) = suburl else {
        return StatusCode::NOT_FOUND;
    };

    if let Err(e) = sqlx::query(
        "INSERT INTO changedetection_notifications (webspace_host_id, suburl_id, title, body) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(host_id)
    .bind(suburl_id)
    .bind(&payload.title)
    .bind(&payload.body)
    .execute(&state.pool)
    .await
    {
        tracing::error!(%host_id, %suburl_id, "failed to insert changedetection notification: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    tracing::debug!(%host_id, %suburl_id, title = ?payload.title, "received changedetection notification");
    StatusCode::OK
}
