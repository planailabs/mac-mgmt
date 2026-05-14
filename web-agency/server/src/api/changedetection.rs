//! ChangeDetection.io webhook receiver.
//!
//! `POST /api/changedetection/{secret}` — accept notification from changedetection.io
//!
//! Auth: the secret in the URL is the auth token (stored per-webspace).

use dioxus::fullstack::axum::{
    self as axum,
    Router,
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
    let ws_id: Option<Uuid> = match sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM webspaces WHERE changedetection_secret = $1",
    )
    .bind(&secret)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("changedetection webhook db error: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    let Some(ws_id) = ws_id else {
        return StatusCode::NOT_FOUND;
    };

    if let Err(e) = sqlx::query(
        "INSERT INTO changedetection_notifications (webspace_id, title, body) VALUES ($1, $2, $3)",
    )
    .bind(ws_id)
    .bind(&payload.title)
    .bind(&payload.body)
    .execute(&state.pool)
    .await
    {
        tracing::error!(%ws_id, "failed to insert changedetection notification: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    tracing::debug!(%ws_id, title = ?payload.title, "received changedetection notification");
    StatusCode::OK
}
