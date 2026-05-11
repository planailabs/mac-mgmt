use mac_mgmt_healer::agent::InstanceInfo;
use mac_mgmt_healer::{HealerState, SpawnRequest};
use rocket::http::Status;
use rocket::response::stream::{Event, EventStream};
use rocket::serde::json::Json;
use rocket::{Shutdown, State, get, post};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::auth::{AdminAuth, SettingAuth};

/// Fetch per-cluster healer settings from the dedicated table.
pub(crate) async fn cluster_healer_config(
    pool: &PgPool,
    cluster_id: Uuid,
) -> mac_mgmt_common::HealerClusterConfig {
    #[derive(sqlx::FromRow)]
    struct Row {
        enabled: bool,
        auto_trigger: Option<bool>,
        auto_trigger_provider: Option<String>,
        auto_trigger_model: Option<String>,
        auto_approve: Option<bool>,
        fix_provider: Option<String>,
        fix_model: Option<String>,
    }
    let row = sqlx::query_as::<_, Row>(
        "SELECT enabled, auto_trigger, auto_trigger_provider, auto_trigger_model, \
                auto_approve, fix_provider, fix_model \
         FROM healer_cluster_settings WHERE cluster_id = $1",
    )
    .bind(cluster_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    match row {
        Some(r) if r.enabled => mac_mgmt_common::HealerClusterConfig {
            auto_trigger: r.auto_trigger,
            auto_trigger_provider: r.auto_trigger_provider,
            auto_trigger_model: r.auto_trigger_model,
            auto_approve: r.auto_approve,
            fix_provider: r.fix_provider,
            fix_model: r.fix_model,
        },
        _ => mac_mgmt_common::HealerClusterConfig::default(),
    }
}

// ── Request/Response types ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateSessionBody {
    pub instance_id: String,
    #[serde(default)]
    pub user_message: Option<String>,
    /// Force a specific LLM provider ("ollama", "anthropic", or "openrouter").
    #[serde(default)]
    pub provider: Option<String>,
    /// Force a specific model name (e.g. "gemma4", "claude-sonnet-4-6").
    #[serde(default)]
    pub model: Option<String>,
    /// Auto-approve remediation (skip approval gate). Default: false.
    #[serde(default)]
    pub auto_approve: bool,
    /// Provider for the fix-model (remediation phase).
    #[serde(default)]
    pub fix_provider: Option<String>,
    /// Model for the fix-model (remediation phase).
    #[serde(default)]
    pub fix_model: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub auto_approve: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_model: Option<String>,
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
    pg_store: &State<std::sync::Arc<mac_mgmt_healer::store::pg::PgHealerStore>>,
    body: Json<CreateSessionBody>,
) -> Result<(Status, Json<SessionCreated>), Status> {
    let cluster_id = auth.cluster_id;
    let body = body.into_inner();

    // Look up instance from heartbeats
    let hb = sqlx::query_as::<_, HeartbeatRow>(
        "SELECT relay_proxy_url, services_extended, \
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

    let healer_scopes: &[&str] = &["files:read", "files:write", "shell:exec", "logs:read"];
    let (proxy_token, proxy_expires) = pg_store
        .mint_proxy_token_scoped(cluster_id, None, Some(healer_scopes))
        .await
        .map_err(|_| Status::InternalServerError)?;
    let relay_client = std::sync::Arc::new(
        mac_mgmt_healer::relay_client::RelayClient::new(relay_url.clone(), proxy_token),
    );
    let instance_prefix: String = body.instance_id.chars().take(12).collect();
    let instance_access: mac_mgmt_healer::DynInstanceAccess = std::sync::Arc::new(
        mac_mgmt_healer::relay_client::RelayInstanceAccess::new(
            relay_client.clone(),
            instance_prefix,
        ),
    );
    let cluster_access: Option<mac_mgmt_healer::DynClusterAccess> = Some(std::sync::Arc::new(
        mac_mgmt_healer::relay_client::RelayClusterAccess::new(relay_client),
    ));
    let metrics_url = Some(format!("{}/metrics", relay_url));

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

    // Per-cluster healer overrides (auto_approve, fix_provider, fix_model).
    let cluster_healer = cluster_healer_config(pool.inner(), cluster_id).await;
    let server_cfg = crate::config::load();

    // Look up per-model token budget from the configured model list
    let per_model_budget = if let (Some(provider), Some(model)) = (&body.provider, &body.model) {
        let models = if crate::config::load().healer.models.is_empty() {
            crate::config::default_healer_models()
        } else {
            crate::config::load().healer.models.clone()
        };
        models.iter()
            .find(|m| m.provider == *provider && m.model == *model)
            .and_then(|m| m.token_budget)
    } else {
        None
    };

    let req = SpawnRequest {
        cluster_id,
        instance_id: body.instance_id,
        created_by: format!("api:setting-token"),
        user_message: body.user_message,
        instance_access,
        cluster_access,
        metrics_url,
        services_extended,
        sample: hb.sample,
        file_tunnels: hb.file_tunnels.unwrap_or_default(),
        shell_tunnels: hb.shell_tunnels.unwrap_or_default(),
        cluster_instances,
        cluster_name,
        hostname: hb.hostname.unwrap_or_default(),
        skip_cooldown: false,
        provider: body.provider,
        model: body.model,
        label: None,
        token_budget: per_model_budget,
        proxy_expires: Some(proxy_expires),
        // Priority: request body > cluster config > server global
        auto_approve: body.auto_approve || cluster_healer.auto_approve.unwrap_or(false),
        fix_provider: body.fix_provider
            .or(cluster_healer.fix_provider)
            .or_else(|| server_cfg.healer.fix_provider.clone()),
        fix_model: body.fix_model
            .or(cluster_healer.fix_model)
            .or_else(|| server_cfg.healer.fix_model.clone()),
        validator_provider: None,
        validator_model: None,
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
            .map(|s| {
                let auto_approve = s.state_data.get("auto_approve").and_then(|v| v.as_bool()).unwrap_or(false);
                let fix_provider = s.state_data.get("fix_provider").and_then(|v| v.as_str()).map(String::from);
                let fix_model = s.state_data.get("fix_model").and_then(|v| v.as_str()).map(String::from);
                SessionSummary {
                    id: s.id,
                    instance_id: s.instance_id,
                    state: s.state.as_str().to_string(),
                    created_by: s.created_by,
                    created_at: s.created_at,
                    completed_at: s.completed_at,
                    error_message: s.error_message,
                    provider: s.provider,
                    model: s.model,
                    label: s.label,
                    auto_approve,
                    fix_provider,
                    fix_model,
                }
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

    let auto_approve = session.state_data.get("auto_approve").and_then(|v| v.as_bool()).unwrap_or(false);
    let fix_provider = session.state_data.get("fix_provider").and_then(|v| v.as_str()).map(String::from);
    let fix_model = session.state_data.get("fix_model").and_then(|v| v.as_str()).map(String::from);

    Ok(Json(SessionDetail {
        session: SessionSummary {
            id: session.id,
            instance_id: session.instance_id,
            state: session.state.as_str().to_string(),
            created_by: session.created_by,
            created_at: session.created_at,
            completed_at: session.completed_at,
            error_message: session.error_message,
            provider: session.provider,
            model: session.model,
            label: session.label,
            auto_approve,
            fix_provider,
            fix_model,
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

/// Extend the token budget for a running session to 1 million tokens.
/// Admin-only — allows a paused-for-budget session to continue.
#[post("/healer/sessions/<id>/extend-budget")]
pub async fn extend_budget(
    _auth: AdminAuth,
    healer: &State<HealerState>,
    id: &str,
) -> Result<Status, Status> {
    let session_id: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    healer.extend_budget(session_id).await.map_err(|e| {
        tracing::error!(err = %e, "failed to extend budget");
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

/// Approve remediation for a session awaiting approval.
#[post("/healer/sessions/<id>/approve")]
pub async fn approve_session(
    _auth: SettingAuth,
    healer: &State<HealerState>,
    id: &str,
) -> Result<Status, Status> {
    let session_id: Uuid = id.parse().map_err(|_| Status::BadRequest)?;
    healer.approve_session(session_id).await.map_err(|e| {
        tracing::error!(err = %e, "failed to approve healer session");
        Status::BadRequest
    })?;
    Ok(Status::Ok)
}

// ── Per-cluster healer settings ──────────────────────────────────────

/// Get healer settings for a cluster.
#[get("/healer/settings")]
pub async fn get_healer_settings(
    auth: SettingAuth,
    pool: &State<PgPool>,
) -> Result<Json<mac_mgmt_common::HealerClusterConfig>, Status> {
    let cfg = cluster_healer_config(pool.inner(), auth.cluster_id).await;
    Ok(Json(cfg))
}

/// Update healer settings for a cluster (upsert).
#[derive(Debug, Deserialize)]
pub struct HealerSettingsBody {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub auto_trigger: Option<bool>,
    #[serde(default)]
    pub auto_trigger_provider: Option<String>,
    #[serde(default)]
    pub auto_trigger_model: Option<String>,
    #[serde(default)]
    pub auto_approve: Option<bool>,
    #[serde(default)]
    pub fix_provider: Option<String>,
    #[serde(default)]
    pub fix_model: Option<String>,
}

fn default_true() -> bool {
    true
}

#[post("/healer/settings", data = "<body>")]
pub async fn put_healer_settings(
    auth: SettingAuth,
    pool: &State<PgPool>,
    body: Json<HealerSettingsBody>,
) -> Result<Status, Status> {
    let body = body.into_inner();
    sqlx::query(
        "INSERT INTO healer_cluster_settings \
            (cluster_id, enabled, auto_trigger, auto_trigger_provider, auto_trigger_model, \
             auto_approve, fix_provider, fix_model, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now()) \
         ON CONFLICT (cluster_id) DO UPDATE SET \
            enabled = EXCLUDED.enabled, \
            auto_trigger = EXCLUDED.auto_trigger, \
            auto_trigger_provider = EXCLUDED.auto_trigger_provider, \
            auto_trigger_model = EXCLUDED.auto_trigger_model, \
            auto_approve = EXCLUDED.auto_approve, \
            fix_provider = EXCLUDED.fix_provider, \
            fix_model = EXCLUDED.fix_model, \
            updated_at = now()",
    )
    .bind(auth.cluster_id)
    .bind(body.enabled)
    .bind(body.auto_trigger)
    .bind(&body.auto_trigger_provider)
    .bind(&body.auto_trigger_model)
    .bind(body.auto_approve)
    .bind(&body.fix_provider)
    .bind(&body.fix_model)
    .execute(pool.inner())
    .await
    .map_err(|e| {
        tracing::error!(err = %e, "failed to save healer settings");
        Status::InternalServerError
    })?;
    Ok(Status::Ok)
}

/// SSE stream of live events for a running session.
#[get("/healer/sessions/<id>/stream")]
pub async fn stream_session(
    _auth: SettingAuth,
    healer: &State<HealerState>,
    id: &str,
    mut shutdown: Shutdown,
) -> Option<EventStream![]> {
    let session_id: Uuid = id.parse().ok()?;
    let store = healer.store().clone();

    // Load existing messages first
    let existing = store.get_messages(session_id)
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
            let mut keepalive = tokio::time::interval(std::time::Duration::from_secs(30));
            keepalive.tick().await; // consume the immediate first tick
            loop {
                tokio::select! {
                    _ = keepalive.tick() => {
                        yield Event::data("{}").event("ping");
                    }
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
                                if let Ok(missed) = store.get_messages_after(
                                    session_id, last_seen_at,
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
            if let Ok(Some(sess)) = store.get_session(session_id).await {
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
