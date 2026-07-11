//! Chaos-test support endpoints for the mmrc harness.
//!
//! Two groups:
//! - **Chaos-node registration** (`chaos_*`): marks disposable test instances
//!   so they never count toward rollout/fleet health. Mounted only when
//!   `[chaos] enabled = true` — off in production, on in the antithesis cluster.
//! - **Admin push & probes read-back** (`admin_push`, `admin_list_probes`):
//!   always mounted in mgmt/monolith mode; used by mmrc to drive and observe a
//!   cluster.

use chrono::{DateTime, Utc};
use rocket::State;
use rocket::http::Status;
use rocket::serde::json::Json;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::auth::AdminAuth;
use super::push::{self, PushChannels, PushMessage};

// ── Chaos-node registration (gated) ─────────────────────────────────

#[derive(Deserialize)]
pub struct RegisterChaosNodeBody {
    /// Pre-computed daemon instance id (sha256 of the ssh host pubkey), known
    /// before the node first heartbeats.
    pub instance_id: String,
    #[serde(default)]
    pub label: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ChaosNodeRow {
    pub instance_id: String,
    pub label: String,
    pub created_at: DateTime<Utc>,
    /// Last heartbeat time if the node has enrolled, else null.
    pub reported_at: Option<DateTime<Utc>>,
    pub version: Option<String>,
}

#[rocket::post(
    "/admin/clusters/<cluster_id>/chaos-nodes",
    data = "<body>",
    format = "json"
)]
pub async fn chaos_register_node(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    cluster_id: &str,
    body: Json<RegisterChaosNodeBody>,
) -> Result<Status, Status> {
    let cid: Uuid = cluster_id.parse().map_err(|_| Status::BadRequest)?;
    if body.instance_id.trim().is_empty() {
        return Err(Status::BadRequest);
    }
    sqlx::query(
        "INSERT INTO chaos_nodes (cluster_id, instance_id, label) VALUES ($1, $2, $3) \
         ON CONFLICT (cluster_id, instance_id) DO UPDATE SET label = EXCLUDED.label",
    )
    .bind(cid)
    .bind(&body.instance_id)
    .bind(&body.label)
    .execute(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    // If the node already heartbeated (e.g. registration raced enrollment),
    // stamp the existing heartbeat row so fleet counting excludes it now.
    let _ = sqlx::query(
        "UPDATE daemon_heartbeats SET chaos = TRUE WHERE cluster_id = $1 AND instance_id = $2",
    )
    .bind(cid)
    .bind(&body.instance_id)
    .execute(pool.inner())
    .await;
    Ok(Status::Created)
}

#[rocket::get("/admin/clusters/<cluster_id>/chaos-nodes")]
pub async fn chaos_list_nodes(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    cluster_id: &str,
) -> Result<Json<Vec<ChaosNodeRow>>, Status> {
    let cid: Uuid = cluster_id.parse().map_err(|_| Status::BadRequest)?;
    let rows = sqlx::query_as::<_, ChaosNodeRow>(
        "SELECT c.instance_id, c.label, c.created_at, h.reported_at, h.version \
         FROM chaos_nodes c \
         LEFT JOIN daemon_heartbeats h \
           ON h.cluster_id = c.cluster_id AND h.instance_id = c.instance_id \
         WHERE c.cluster_id = $1 ORDER BY c.created_at DESC",
    )
    .bind(cid)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(rows))
}

#[rocket::delete("/admin/clusters/<cluster_id>/chaos-nodes/<instance_id>")]
pub async fn chaos_delete_node(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    cluster_id: &str,
    instance_id: &str,
) -> Result<Status, Status> {
    let cid: Uuid = cluster_id.parse().map_err(|_| Status::BadRequest)?;
    // Reap: drop the registration and the heartbeat row so the instance
    // disappears from the fleet entirely once mmrcd has destroyed the container.
    sqlx::query("DELETE FROM chaos_nodes WHERE cluster_id = $1 AND instance_id = $2")
        .bind(cid)
        .bind(instance_id)
        .execute(pool.inner())
        .await
        .map_err(|_| Status::InternalServerError)?;
    let _ = sqlx::query("DELETE FROM daemon_heartbeats WHERE cluster_id = $1 AND instance_id = $2")
        .bind(cid)
        .bind(instance_id)
        .execute(pool.inner())
        .await;
    Ok(Status::NoContent)
}

// ── Admin push (always mounted in mgmt/monolith) ────────────────────

#[derive(Deserialize)]
pub struct AdminPushBody {
    /// snake_case event name (see `PushEvent::from_wire_name`).
    pub event: String,
    /// When set, deliver only to this instance (wrapped in `Targeted`).
    #[serde(default)]
    pub instance_id: Option<String>,
}

#[derive(Serialize)]
pub struct AdminPushResult {
    pub ok: bool,
    /// Number of connected daemons the event was delivered to (0 = none).
    pub receivers: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[rocket::post("/admin/clusters/<cluster_id>/push", data = "<body>", format = "json")]
pub async fn admin_push(
    _auth: AdminAuth,
    channels: &State<PushChannels>,
    cluster_id: &str,
    body: Json<AdminPushBody>,
) -> Result<Json<AdminPushResult>, Status> {
    let cid: Uuid = cluster_id.parse().map_err(|_| Status::BadRequest)?;
    let mut msg = match PushMessage::from_wire_name(&body.event) {
        Some(m) => m,
        None => {
            return Ok(Json(AdminPushResult {
                ok: false,
                receivers: 0,
                message: Some(format!("unknown event: {}", body.event)),
            }));
        }
    };
    if let Some(instance_id) = &body.instance_id {
        msg = PushMessage::Targeted {
            instance_id: instance_id.clone(),
            event: Box::new(msg),
        };
    }
    let receivers = push::notify_counted(channels.inner(), cid, msg).await;
    Ok(Json(AdminPushResult {
        ok: true,
        receivers,
        message: None,
    }))
}

// ── Probes read-back (always mounted in mgmt/monolith) ──────────────

#[derive(Serialize, sqlx::FromRow)]
pub struct ProbeRow {
    pub instance_id: String,
    pub service: String,
    pub kind: String,
    pub ok: bool,
    pub error_class: Option<String>,
    pub error_detail: Option<String>,
    pub collected_at: DateTime<Utc>,
}

#[rocket::get("/admin/clusters/<cluster_id>/probes")]
pub async fn admin_list_probes(
    _auth: AdminAuth,
    pool: &State<PgPool>,
    cluster_id: &str,
) -> Result<Json<Vec<ProbeRow>>, Status> {
    let cid: Uuid = cluster_id.parse().map_err(|_| Status::BadRequest)?;
    // Latest probe row per (instance_id, service, kind).
    let rows = sqlx::query_as::<_, ProbeRow>(
        "SELECT DISTINCT ON (instance_id, service, kind) \
                instance_id, service, kind, ok, error_class, error_detail, collected_at \
         FROM assessment_probes \
         WHERE cluster_id = $1 \
         ORDER BY instance_id, service, kind, collected_at DESC",
    )
    .bind(cid)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;
    Ok(Json(rows))
}
