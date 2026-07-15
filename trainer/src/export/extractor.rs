use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// A fully exported session with all its messages and staff pings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedSession {
    pub id: Uuid,
    pub cluster_id: Uuid,
    pub instance_id: String,
    pub state: String,
    pub created_by: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub error_message: Option<String>,
    pub initial_issues: serde_json::Value,
    pub state_data: serde_json::Value,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub label: Option<String>,
    pub messages: Vec<ExportedMessage>,
    pub staff_pings: Vec<ExportedStaffPing>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedMessage {
    pub role: String,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedStaffPing {
    pub category: String,
    pub message: String,
    pub resolved: bool,
}

/// Extract all terminal sessions with at least `min_messages` messages.
pub async fn extract_all(pool: &PgPool, min_messages: usize) -> Result<Vec<ExportedSession>> {
    // Fetch all terminal sessions
    let sessions = sqlx::query_as::<_, SessionRow>(
        "SELECT id, scope_id AS cluster_id, subject AS instance_id, state, state_data, \
                created_by, created_at, updated_at, completed_at, error_message, \
                initial_context AS initial_issues, provider, model, label \
         FROM chat_sessions \
         WHERE session_type = 'healer' \
           AND state IN ('done', 'completed', 'failed', 'needs_human_attention', 'cancelled', 'paused') \
         ORDER BY created_at ASC",
    )
    .fetch_all(pool)
    .await?;

    tracing::info!("found {} terminal sessions", sessions.len());

    let mut result = Vec::new();

    for sess in sessions {
        // Fetch messages
        let messages = sqlx::query_as::<_, MessageRow>(
            "SELECT role, content, metadata, created_at \
             FROM chat_messages WHERE session_id = $1 \
             ORDER BY created_at ASC",
        )
        .bind(sess.id)
        .fetch_all(pool)
        .await?;

        if messages.len() < min_messages {
            continue;
        }

        // Fetch staff pings
        let pings = sqlx::query_as::<_, PingRow>(
            "SELECT category, message, resolved \
             FROM healer_staff_pings WHERE session_id = $1",
        )
        .bind(sess.id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        result.push(ExportedSession {
            id: sess.id,
            cluster_id: sess.cluster_id,
            instance_id: sess.instance_id,
            state: sess.state,
            created_by: sess.created_by,
            created_at: sess.created_at,
            completed_at: sess.completed_at,
            error_message: sess.error_message,
            initial_issues: sess.initial_issues,
            state_data: sess.state_data,
            provider: sess.provider,
            model: sess.model,
            label: sess.label,
            messages: messages
                .into_iter()
                .map(|m| ExportedMessage {
                    role: m.role,
                    content: m.content,
                    metadata: m.metadata,
                    created_at: m.created_at,
                })
                .collect(),
            staff_pings: pings
                .into_iter()
                .map(|p| ExportedStaffPing {
                    category: p.category,
                    message: p.message,
                    resolved: p.resolved,
                })
                .collect(),
        });
    }

    Ok(result)
}

#[derive(sqlx::FromRow)]
struct SessionRow {
    id: Uuid,
    cluster_id: Uuid,
    instance_id: String,
    state: String,
    state_data: serde_json::Value,
    created_by: String,
    created_at: chrono::DateTime<chrono::Utc>,
    #[allow(dead_code)]
    updated_at: chrono::DateTime<chrono::Utc>,
    completed_at: Option<chrono::DateTime<chrono::Utc>>,
    error_message: Option<String>,
    initial_issues: serde_json::Value,
    provider: Option<String>,
    model: Option<String>,
    label: Option<String>,
}

#[derive(sqlx::FromRow)]
struct MessageRow {
    role: String,
    content: String,
    metadata: Option<serde_json::Value>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(sqlx::FromRow)]
struct PingRow {
    category: String,
    message: String,
    resolved: bool,
}
