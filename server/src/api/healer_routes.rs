use mac_mgmt_healer::agent::InstanceInfo;
use mac_mgmt_healer::{HealerState, SpawnRequest};
use rocket::http::Status;
use rocket::response::stream::{Event, EventStream};
use rocket::serde::json::Json;
use rocket::{Shutdown, State, get, post};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::auth::SettingAuth;

// ── Request/Response types ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateSessionBody {
    pub instance_id: String,
    #[serde(default)]
    pub user_message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SessionCreated {
    pub session_id: Uuid,
    pub state: String,
}

#[derive(Debug, Serialize)]
pub struct SessionSummary {
    pub id: Uuid,
    pub instance_id: String,
    pub state: String,
    pub created_by: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SessionDetail {
    pub session: SessionSummary,
    pub messages: Vec<MessageRow>,
}

#[derive(Debug, Serialize)]
pub struct MessageRow {
    pub id: Uuid,
    pub role: String,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// ── Routes ─────────────────────────────────────────────────────────────

/// Create a new healer session for the given instance.
#[post("/healer/sessions", data = "<body>")]
pub async fn create_session(
    auth: SettingAuth,
    pool: &State<PgPool>,
    healer: &State<HealerState>,
    body: Json<CreateSessionBody>,
) -> Result<(Status, Json<SessionCreated>), Status> {
    let cluster_id = auth.cluster_id;
    let body = body.into_inner();

    // Look up instance from heartbeats
    let hb = sqlx::query_as::<_, HeartbeatRow>(
        "SELECT instance_id, relay_proxy_url, services_extended, \
                file_tunnels, shell_tunnels, sample, hostname \
         FROM daemon_heartbeats WHERE cluster_id = $1 AND instance_id = $2",
    )
    .bind(cluster_id)
    .bind(&body.instance_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?
    .ok_or(Status::NotFound)?;

    let relay_url = hb.relay_proxy_url.ok_or(Status::BadRequest)?;

    // Get all instances in this cluster for cross-instance tools
    let cluster_instances: Vec<InstanceInfo> = sqlx::query_as::<_, InstanceRow>(
        "SELECT instance_id, hostname FROM daemon_heartbeats \
         WHERE cluster_id = $1 AND instance_id != $2 \
         AND reported_at > now() - interval '5 minutes'",
    )
    .bind(cluster_id)
    .bind(&body.instance_id)
    .fetch_all(pool.inner())
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| InstanceInfo {
        instance_prefix: r.instance_id.chars().take(12).collect(),
        hostname: r.hostname.unwrap_or_default(),
        healthy: true,
    })
    .collect();

    // Get cluster name
    let cluster_name: String = sqlx::query_scalar("SELECT name FROM clusters WHERE id = $1")
        .bind(cluster_id)
        .fetch_optional(pool.inner())
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| cluster_id.to_string());

    // Parse services_extended
    let services_extended: Vec<mac_mgmt_common::ServiceExtState> =
        serde_json::from_value(hb.services_extended.unwrap_or_default()).unwrap_or_default();

    let req = SpawnRequest {
        cluster_id,
        instance_id: body.instance_id,
        created_by: format!("api:setting-token"),
        user_message: body.user_message,
        relay_url,
        services_extended,
        sample: hb.sample,
        file_tunnels: hb.file_tunnels.unwrap_or_default(),
        shell_tunnels: hb.shell_tunnels.unwrap_or_default(),
        cluster_instances,
        cluster_name,
        hostname: hb.hostname.unwrap_or_default(),
        skip_cooldown: false,
    };

    let session_id = healer.spawn_session(req).await.map_err(|e| {
        tracing::error!(err = %e, "failed to spawn healer session");
        Status::InternalServerError
    })?;

    Ok((
        Status::Created,
        Json(SessionCreated {
            session_id,
            state: "created".to_string(),
        }),
    ))
}

/// List healer sessions for the cluster.
#[get("/healer/sessions")]
pub async fn list_sessions(
    auth: SettingAuth,
    healer: &State<HealerState>,
) -> Result<Json<Vec<SessionSummary>>, Status> {
    let sessions = healer
        .list_sessions(auth.cluster_id)
        .await
        .map_err(|_| Status::InternalServerError)?;

    Ok(Json(
        sessions
            .into_iter()
            .map(|s| SessionSummary {
                id: s.id,
                instance_id: s.instance_id,
                state: s.state.as_str().to_string(),
                created_by: s.created_by,
                created_at: s.created_at,
                completed_at: s.completed_at,
                error_message: s.error_message,
            })
            .collect(),
    ))
}

/// Get a single session with its messages.
#[get("/healer/sessions/<id>")]
pub async fn get_session(
    _auth: SettingAuth,
    healer: &State<HealerState>,
    id: &str,
) -> Result<Json<SessionDetail>, Status> {
    let session_id: Uuid = id.parse().map_err(|_| Status::BadRequest)?;

    let (session, messages) = healer
        .get_session(session_id)
        .await
        .map_err(|_| Status::InternalServerError)?
        .ok_or(Status::NotFound)?;

    Ok(Json(SessionDetail {
        session: SessionSummary {
            id: session.id,
            instance_id: session.instance_id,
            state: session.state.as_str().to_string(),
            created_by: session.created_by,
            created_at: session.created_at,
            completed_at: session.completed_at,
            error_message: session.error_message,
        },
        messages: messages
            .into_iter()
            .map(|m| MessageRow {
                id: m.id,
                role: m.role,
                content: m.content,
                metadata: m.metadata,
                created_at: m.created_at,
            })
            .collect(),
    }))
}

/// Cancel a running healer session.
#[post("/healer/sessions/<id>/cancel")]
pub async fn cancel_session(
    _auth: SettingAuth,
    healer: &State<HealerState>,
    id: &str,
) -> Result<Status, Status> {
    let session_id: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    healer
        .cancel_session(session_id)
        .await
        .map_err(|_| Status::InternalServerError)?;
    Ok(Status::Ok)
}

/// Pause a running healer session at its next checkpoint.
#[post("/healer/sessions/<id>/pause")]
pub async fn pause_session(
    _auth: SettingAuth,
    healer: &State<HealerState>,
    id: &str,
) -> Result<Status, Status> {
    let session_id: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    healer.pause_session(session_id).map_err(|e| {
        tracing::error!(err = %e, "failed to pause healer session");
        Status::BadRequest
    })?;
    Ok(Status::Ok)
}

/// Resume a paused healer session.
#[post("/healer/sessions/<id>/resume")]
pub async fn resume_session(
    _auth: SettingAuth,
    healer: &State<HealerState>,
    id: &str,
) -> Result<Status, Status> {
    let session_id: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    healer.resume_session(session_id).await.map_err(|e| {
        tracing::error!(err = %e, "failed to resume healer session");
        Status::BadRequest
    })?;
    Ok(Status::Ok)
}

/// SSE stream of live events for a running session.
#[get("/healer/sessions/<id>/stream")]
pub async fn stream_session(
    _auth: SettingAuth,
    healer: &State<HealerState>,
    pool: &State<PgPool>,
    id: &str,
    mut shutdown: Shutdown,
) -> Option<EventStream![]> {
    let session_id: Uuid = id.parse().ok()?;
    let pool = pool.inner().clone();

    // Load existing messages first
    let existing = mac_mgmt_healer::session::store::get_messages(&pool, session_id)
        .await
        .ok()
        .unwrap_or_default();

    // Subscribe to live events (may not exist if session is not running)
    let rx = healer.subscribe(session_id);

    Some(EventStream! {
        // Track the latest replayed timestamp so we can recover from lag
        let mut last_seen_at = chrono::DateTime::<chrono::Utc>::MIN_UTC;

        // Replay existing messages
        for msg in existing {
            if msg.created_at > last_seen_at {
                last_seen_at = msg.created_at;
            }
            let event = mac_mgmt_healer::HealerEvent::Message {
                role: msg.role,
                content: msg.content,
                metadata: msg.metadata,
                created_at: msg.created_at,
            };
            let json = serde_json::to_string(&event).unwrap_or_default();
            yield Event::data(json);
        }

        // Stream live events if session is running
        if let Some(mut rx) = rx {
            loop {
                tokio::select! {
                    msg = rx.recv() => {
                        match msg {
                            Ok(event) => {
                                if let mac_mgmt_healer::HealerEvent::Message { created_at, .. } = &event {
                                    if *created_at > last_seen_at {
                                        last_seen_at = *created_at;
                                    }
                                }
                                let is_done = matches!(&event, mac_mgmt_healer::HealerEvent::Done { .. });
                                let json = serde_json::to_string(&event).unwrap_or_default();
                                yield Event::data(json);
                                if is_done { break; }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                tracing::warn!("healer SSE client lagged, skipped {n} events — replaying from DB");
                                // Recover dropped messages from the database
                                if let Ok(missed) = mac_mgmt_healer::session::store::get_messages_after(
                                    &pool, session_id, last_seen_at,
                                ).await {
                                    for msg in missed {
                                        if msg.created_at > last_seen_at {
                                            last_seen_at = msg.created_at;
                                        }
                                        let event = mac_mgmt_healer::HealerEvent::Message {
                                            role: msg.role,
                                            content: msg.content,
                                            metadata: msg.metadata,
                                            created_at: msg.created_at,
                                        };
                                        let json = serde_json::to_string(&event).unwrap_or_default();
                                        yield Event::data(json);
                                    }
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        }
                    }
                    _ = &mut shutdown => break,
                }
            }
        } else {
            // Session not running — send current state as done
            if let Ok(Some(sess)) = mac_mgmt_healer::session::store::get_session(&pool, session_id).await {
                let event = mac_mgmt_healer::HealerEvent::Done {
                    state: sess.state.as_str().to_string(),
                };
                let json = serde_json::to_string(&event).unwrap_or_default();
                yield Event::data(json);
            }
        }
    })
}

// ── Internal sqlx row types ────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct HeartbeatRow {
    #[allow(dead_code)]
    instance_id: String,
    relay_proxy_url: Option<String>,
    services_extended: Option<serde_json::Value>,
    file_tunnels: Option<serde_json::Value>,
    shell_tunnels: Option<serde_json::Value>,
    sample: Option<serde_json::Value>,
    hostname: Option<String>,
}

#[derive(sqlx::FromRow)]
struct InstanceRow {
    instance_id: String,
    hostname: Option<String>,
}
