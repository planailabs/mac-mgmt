use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::models::{HealerMessage, HealerSession, SessionState};

/// Create a new healer session in the `created` state.
pub async fn create_session(
    pool: &PgPool,
    cluster_id: Uuid,
    instance_id: &str,
    created_by: &str,
    initial_issues: &serde_json::Value,
    state_data: &serde_json::Value,
) -> Result<Uuid> {
    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO healer_sessions (cluster_id, instance_id, state, state_data, created_by, initial_issues) \
         VALUES ($1, $2, 'created', $3, $4, $5) \
         RETURNING id",
    )
    .bind(cluster_id)
    .bind(instance_id)
    .bind(state_data)
    .bind(created_by)
    .bind(initial_issues)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// Atomically transition a session to a new state.
pub async fn transition_state(
    pool: &PgPool,
    session_id: Uuid,
    new_state: &SessionState,
    state_data: &serde_json::Value,
) -> Result<()> {
    let completed_at: Option<DateTime<Utc>> = if new_state.is_terminal() {
        Some(Utc::now())
    } else {
        None
    };
    sqlx::query(
        "UPDATE healer_sessions \
         SET state = $1, state_data = $2, updated_at = now(), completed_at = COALESCE($3, completed_at) \
         WHERE id = $4",
    )
    .bind(new_state.as_str())
    .bind(state_data)
    .bind(completed_at)
    .bind(session_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Transition to failed state with an error message.
pub async fn fail_session(
    pool: &PgPool,
    session_id: Uuid,
    error_message: &str,
    state_data: &serde_json::Value,
) -> Result<()> {
    sqlx::query(
        "UPDATE healer_sessions \
         SET state = 'failed', state_data = $1, error_message = $2, \
             updated_at = now(), completed_at = now() \
         WHERE id = $3",
    )
    .bind(state_data)
    .bind(error_message)
    .bind(session_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Append a message to a session's conversation log.
pub async fn append_message(
    pool: &PgPool,
    session_id: Uuid,
    role: &str,
    content: &str,
    metadata: Option<&serde_json::Value>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO healer_messages (session_id, role, content, metadata) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(session_id)
    .bind(role)
    .bind(content)
    .bind(metadata)
    .execute(pool)
    .await?;
    Ok(())
}

/// Get a single session by ID.
pub async fn get_session(pool: &PgPool, session_id: Uuid) -> Result<Option<HealerSession>> {
    let row = sqlx::query_as::<_, SessionRow>(
        "SELECT id, cluster_id, instance_id, state, state_data, created_by, \
                created_at, updated_at, completed_at, error_message, initial_issues \
         FROM healer_sessions WHERE id = $1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(Into::into))
}

/// List sessions for a cluster, newest first.
pub async fn list_sessions(pool: &PgPool, cluster_id: Uuid) -> Result<Vec<HealerSession>> {
    let rows = sqlx::query_as::<_, SessionRow>(
        "SELECT id, cluster_id, instance_id, state, state_data, created_by, \
                created_at, updated_at, completed_at, error_message, initial_issues \
         FROM healer_sessions WHERE cluster_id = $1 \
         ORDER BY created_at DESC LIMIT 100",
    )
    .bind(cluster_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// Get all messages for a session, oldest first.
pub async fn get_messages(pool: &PgPool, session_id: Uuid) -> Result<Vec<HealerMessage>> {
    let rows = sqlx::query_as::<_, MessageRow>(
        "SELECT id, session_id, role, content, metadata, created_at \
         FROM healer_messages WHERE session_id = $1 \
         ORDER BY created_at ASC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// Find sessions that should be auto-resumed on server startup.
/// Excludes terminal states AND paused (paused requires manual resume).
pub async fn find_resumable(pool: &PgPool) -> Result<Vec<HealerSession>> {
    let rows = sqlx::query_as::<_, SessionRow>(
        "SELECT id, cluster_id, instance_id, state, state_data, created_by, \
                created_at, updated_at, completed_at, error_message, initial_issues \
         FROM healer_sessions \
         WHERE state NOT IN ('completed', 'failed', 'cancelled', 'paused') \
         ORDER BY created_at ASC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

// ── Internal row types for sqlx mapping ────────────────────────────────

#[derive(sqlx::FromRow)]
struct SessionRow {
    id: Uuid,
    cluster_id: Uuid,
    instance_id: String,
    state: String,
    state_data: serde_json::Value,
    created_by: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    error_message: Option<String>,
    initial_issues: serde_json::Value,
}

impl From<SessionRow> for HealerSession {
    fn from(r: SessionRow) -> Self {
        Self {
            id: r.id,
            cluster_id: r.cluster_id,
            instance_id: r.instance_id,
            state: SessionState::from_str(&r.state).unwrap_or(SessionState::Failed),
            state_data: r.state_data,
            created_by: r.created_by,
            created_at: r.created_at,
            updated_at: r.updated_at,
            completed_at: r.completed_at,
            error_message: r.error_message,
            initial_issues: r.initial_issues,
        }
    }
}

#[derive(sqlx::FromRow)]
struct MessageRow {
    id: Uuid,
    session_id: Uuid,
    role: String,
    content: String,
    metadata: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
}

impl From<MessageRow> for HealerMessage {
    fn from(r: MessageRow) -> Self {
        Self {
            id: r.id,
            session_id: r.session_id,
            role: r.role,
            content: r.content,
            metadata: r.metadata,
            created_at: r.created_at,
        }
    }
}
