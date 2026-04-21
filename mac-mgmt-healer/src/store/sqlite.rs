//! SQLite-backed implementation of [`HealerStore`].
//!
//! Used by the daemon when running the healer locally. The database is stored
//! at `~/.mac-mgmt/healer.db`.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::session::models::{HealerMessage, HealerSession, SessionState, StaffPing};
use super::*;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS healer_sessions (
    id TEXT PRIMARY KEY,
    cluster_id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'created',
    state_data TEXT NOT NULL DEFAULT '{}',
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    error_message TEXT,
    initial_issues TEXT NOT NULL DEFAULT '[]',
    provider TEXT,
    model TEXT,
    label TEXT
);
CREATE TABLE IF NOT EXISTS healer_messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES healer_sessions(id),
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    metadata TEXT,
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS healer_staff_pings (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES healer_sessions(id),
    cluster_id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    category TEXT NOT NULL,
    message TEXT NOT NULL,
    resolved INTEGER NOT NULL DEFAULT 0,
    resolved_by TEXT,
    resolved_at TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_messages_session ON healer_messages(session_id);
CREATE INDEX IF NOT EXISTS idx_pings_session ON healer_staff_pings(session_id);
CREATE INDEX IF NOT EXISTS idx_sessions_instance ON healer_sessions(instance_id);
"#;

/// [`HealerStore`] backed by a local SQLite database.
#[derive(Clone)]
pub struct SqliteHealerStore {
    pool: SqlitePool,
}

impl SqliteHealerStore {
    /// Open (or create) the SQLite database and run migrations.
    pub async fn open(path: &str) -> Result<Self> {
        let pool = SqlitePool::connect(&format!("sqlite:{path}?mode=rwc"))
            .await?;
        // Run schema (idempotent CREATE IF NOT EXISTS)
        sqlx::raw_sql(SCHEMA).execute(&pool).await?;
        Ok(Self { pool })
    }
}

// ── Helpers ────────────────────────────────────────────────────────────

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn new_uuid() -> String {
    Uuid::new_v4().to_string()
}

fn parse_uuid(s: &str) -> Result<Uuid> {
    s.parse().map_err(|e| anyhow::anyhow!("invalid UUID: {e}"))
}

fn parse_datetime(s: &str) -> DateTime<Utc> {
    s.parse::<DateTime<Utc>>()
        .unwrap_or_else(|_| Utc::now())
}

fn parse_json(s: &str) -> serde_json::Value {
    serde_json::from_str(s).unwrap_or(serde_json::Value::Null)
}

fn session_from_row(
    id: String,
    cluster_id: String,
    instance_id: String,
    state: String,
    state_data: String,
    created_by: String,
    created_at: String,
    updated_at: String,
    completed_at: Option<String>,
    error_message: Option<String>,
    initial_issues: String,
    provider: Option<String>,
    model: Option<String>,
    label: Option<String>,
) -> HealerSession {
    HealerSession {
        id: parse_uuid(&id).unwrap_or_default(),
        cluster_id: parse_uuid(&cluster_id).unwrap_or_default(),
        instance_id,
        state: SessionState::from_str(&state).unwrap_or(SessionState::Failed),
        state_data: parse_json(&state_data),
        created_by,
        created_at: parse_datetime(&created_at),
        updated_at: parse_datetime(&updated_at),
        completed_at: completed_at.map(|s| parse_datetime(&s)),
        error_message,
        initial_issues: parse_json(&initial_issues),
        provider,
        model,
        label,
    }
}

fn message_from_row(
    id: String,
    session_id: String,
    role: String,
    content: String,
    metadata: Option<String>,
    created_at: String,
) -> HealerMessage {
    HealerMessage {
        id: parse_uuid(&id).unwrap_or_default(),
        session_id: parse_uuid(&session_id).unwrap_or_default(),
        role,
        content,
        metadata: metadata.map(|s| parse_json(&s)),
        created_at: parse_datetime(&created_at),
    }
}

fn ping_from_row(
    id: String,
    session_id: String,
    cluster_id: String,
    instance_id: String,
    category: String,
    message: String,
    resolved: bool,
    resolved_by: Option<String>,
    resolved_at: Option<String>,
    created_at: String,
) -> StaffPing {
    StaffPing {
        id: parse_uuid(&id).unwrap_or_default(),
        session_id: parse_uuid(&session_id).unwrap_or_default(),
        cluster_id: parse_uuid(&cluster_id).unwrap_or_default(),
        instance_id,
        category,
        message,
        resolved,
        resolved_by,
        resolved_at: resolved_at.map(|s| parse_datetime(&s)),
        created_at: parse_datetime(&created_at),
    }
}

/// Persist a `state_change` message to the session chat log.
async fn append_state_change(
    pool: &SqlitePool,
    session_id: &str,
    state: &str,
    state_data: &serde_json::Value,
) {
    let reason = state_data
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let content = serde_json::json!({ "state": state, "reason": reason }).to_string();
    let metadata = serde_json::to_string(state_data).unwrap_or_default();
    let id = new_uuid();
    let now = now_iso();
    let _ = sqlx::query(
        "INSERT INTO healer_messages (id, session_id, role, content, metadata, created_at) \
         VALUES (?, ?, 'state_change', ?, ?, ?)",
    )
    .bind(&id)
    .bind(session_id)
    .bind(&content)
    .bind(&metadata)
    .bind(&now)
    .execute(pool)
    .await;
}

// ── Trait implementation ───────────────────────────────────────────────

#[async_trait]
impl HealerStore for SqliteHealerStore {
    async fn create_session(
        &self,
        cluster_id: Uuid,
        instance_id: &str,
        created_by: &str,
        initial_issues: &serde_json::Value,
        state_data: &serde_json::Value,
        provider: Option<&str>,
        model: Option<&str>,
        label: Option<&str>,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        let now = now_iso();
        let sd = serde_json::to_string(state_data)?;
        let ii = serde_json::to_string(initial_issues)?;
        sqlx::query(
            "INSERT INTO healer_sessions (id, cluster_id, instance_id, state, state_data, \
             created_by, initial_issues, provider, model, label, created_at, updated_at) \
             VALUES (?, ?, ?, 'created', ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id.to_string())
        .bind(cluster_id.to_string())
        .bind(instance_id)
        .bind(&sd)
        .bind(created_by)
        .bind(&ii)
        .bind(provider)
        .bind(model)
        .bind(label)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    async fn set_label(&self, session_id: Uuid, label: &str) -> Result<()> {
        sqlx::query("UPDATE healer_sessions SET label = ? WHERE id = ?")
            .bind(label)
            .bind(session_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn transition_state(
        &self,
        session_id: Uuid,
        new_state: &SessionState,
        state_data: &serde_json::Value,
    ) -> Result<()> {
        let now = now_iso();
        let completed_at: Option<String> = if new_state.is_terminal() {
            Some(now.clone())
        } else {
            None
        };
        let sd = serde_json::to_string(state_data)?;
        let sid = session_id.to_string();
        sqlx::query(
            "UPDATE healer_sessions SET state = ?, state_data = ?, updated_at = ?, \
             completed_at = COALESCE(?, completed_at) WHERE id = ?",
        )
        .bind(new_state.as_str())
        .bind(&sd)
        .bind(&now)
        .bind(&completed_at)
        .bind(&sid)
        .execute(&self.pool)
        .await?;

        append_state_change(&self.pool, &sid, new_state.as_str(), state_data).await;
        Ok(())
    }

    async fn fail_session(
        &self,
        session_id: Uuid,
        error_message: &str,
        state_data: &serde_json::Value,
    ) -> Result<()> {
        let now = now_iso();
        let sd = serde_json::to_string(state_data)?;
        let sid = session_id.to_string();
        sqlx::query(
            "UPDATE healer_sessions SET state = 'failed', state_data = ?, error_message = ?, \
             updated_at = ?, completed_at = ? WHERE id = ?",
        )
        .bind(&sd)
        .bind(error_message)
        .bind(&now)
        .bind(&now)
        .bind(&sid)
        .execute(&self.pool)
        .await?;

        let data = serde_json::json!({"reason": error_message});
        append_state_change(&self.pool, &sid, "failed", &data).await;
        Ok(())
    }

    async fn get_session(&self, session_id: Uuid) -> Result<Option<HealerSession>> {
        let sid = session_id.to_string();
        let row: Option<(String, String, String, String, String, String, String, String, Option<String>, Option<String>, String, Option<String>, Option<String>, Option<String>)> =
            sqlx::query_as(
                "SELECT id, cluster_id, instance_id, state, state_data, created_by, \
                 created_at, updated_at, completed_at, error_message, initial_issues, \
                 provider, model, label FROM healer_sessions WHERE id = ?",
            )
            .bind(&sid)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| session_from_row(r.0, r.1, r.2, r.3, r.4, r.5, r.6, r.7, r.8, r.9, r.10, r.11, r.12, r.13)))
    }

    async fn list_sessions(&self, cluster_id: Uuid) -> Result<Vec<HealerSession>> {
        let cid = cluster_id.to_string();
        let rows: Vec<(String, String, String, String, String, String, String, String, Option<String>, Option<String>, String, Option<String>, Option<String>, Option<String>)> =
            sqlx::query_as(
                "SELECT id, cluster_id, instance_id, state, state_data, created_by, \
                 created_at, updated_at, completed_at, error_message, initial_issues, \
                 provider, model, label FROM healer_sessions WHERE cluster_id = ? \
                 ORDER BY created_at DESC LIMIT 100",
            )
            .bind(&cid)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|r| session_from_row(r.0, r.1, r.2, r.3, r.4, r.5, r.6, r.7, r.8, r.9, r.10, r.11, r.12, r.13)).collect())
    }

    async fn find_resumable(&self) -> Result<Vec<HealerSession>> {
        let rows: Vec<(String, String, String, String, String, String, String, String, Option<String>, Option<String>, String, Option<String>, Option<String>, Option<String>)> =
            sqlx::query_as(
                "SELECT id, cluster_id, instance_id, state, state_data, created_by, \
                 created_at, updated_at, completed_at, error_message, initial_issues, \
                 provider, model, label FROM healer_sessions \
                 WHERE state NOT IN ('completed', 'done', 'failed', 'cancelled', 'paused', 'needs_human_attention') \
                 ORDER BY created_at ASC",
            )
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|r| session_from_row(r.0, r.1, r.2, r.3, r.4, r.5, r.6, r.7, r.8, r.9, r.10, r.11, r.12, r.13)).collect())
    }

    async fn update_provider_model(&self, session_id: Uuid, provider: &str, model: &str) -> Result<()> {
        sqlx::query("UPDATE healer_sessions SET provider = ?, model = ? WHERE id = ?")
            .bind(provider)
            .bind(model)
            .bind(session_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn append_message(
        &self,
        session_id: Uuid,
        role: &str,
        content: &str,
        metadata: Option<&serde_json::Value>,
    ) -> Result<()> {
        let id = new_uuid();
        let now = now_iso();
        let md = metadata.map(|v| serde_json::to_string(v).unwrap_or_default());
        sqlx::query(
            "INSERT INTO healer_messages (id, session_id, role, content, metadata, created_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(session_id.to_string())
        .bind(role)
        .bind(content)
        .bind(&md)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_messages(&self, session_id: Uuid) -> Result<Vec<HealerMessage>> {
        let sid = session_id.to_string();
        let rows: Vec<(String, String, String, String, Option<String>, String)> = sqlx::query_as(
            "SELECT id, session_id, role, content, metadata, created_at \
             FROM healer_messages WHERE session_id = ? ORDER BY created_at ASC",
        )
        .bind(&sid)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| message_from_row(r.0, r.1, r.2, r.3, r.4, r.5)).collect())
    }

    async fn get_messages_after(&self, session_id: Uuid, after: DateTime<Utc>) -> Result<Vec<HealerMessage>> {
        let sid = session_id.to_string();
        let after_str = after.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let rows: Vec<(String, String, String, String, Option<String>, String)> = sqlx::query_as(
            "SELECT id, session_id, role, content, metadata, created_at \
             FROM healer_messages WHERE session_id = ? AND created_at > ? ORDER BY created_at ASC",
        )
        .bind(&sid)
        .bind(&after_str)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| message_from_row(r.0, r.1, r.2, r.3, r.4, r.5)).collect())
    }

    async fn create_staff_ping(
        &self,
        session_id: Uuid,
        cluster_id: Uuid,
        instance_id: &str,
        category: &str,
        message: &str,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        let now = now_iso();
        sqlx::query(
            "INSERT INTO healer_staff_pings (id, session_id, cluster_id, instance_id, category, message, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id.to_string())
        .bind(session_id.to_string())
        .bind(cluster_id.to_string())
        .bind(instance_id)
        .bind(category)
        .bind(message)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    async fn list_staff_pings(&self, cluster_id: Uuid) -> Result<Vec<StaffPing>> {
        let cid = cluster_id.to_string();
        let rows: Vec<(String, String, String, String, String, String, bool, Option<String>, Option<String>, String)> =
            sqlx::query_as(
                "SELECT id, session_id, cluster_id, instance_id, category, message, \
                 resolved, resolved_by, resolved_at, created_at \
                 FROM healer_staff_pings WHERE cluster_id = ? \
                 ORDER BY resolved ASC, created_at DESC LIMIT 100",
            )
            .bind(&cid)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|r| ping_from_row(r.0, r.1, r.2, r.3, r.4, r.5, r.6, r.7, r.8, r.9)).collect())
    }

    async fn list_session_pings(&self, session_id: Uuid) -> Result<Vec<StaffPing>> {
        let sid = session_id.to_string();
        let rows: Vec<(String, String, String, String, String, String, bool, Option<String>, Option<String>, String)> =
            sqlx::query_as(
                "SELECT id, session_id, cluster_id, instance_id, category, message, \
                 resolved, resolved_by, resolved_at, created_at \
                 FROM healer_staff_pings WHERE session_id = ? ORDER BY created_at ASC",
            )
            .bind(&sid)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|r| ping_from_row(r.0, r.1, r.2, r.3, r.4, r.5, r.6, r.7, r.8, r.9)).collect())
    }

    async fn resolve_staff_ping(&self, ping_id: Uuid, resolved_by: &str) -> Result<()> {
        let now = now_iso();
        sqlx::query(
            "UPDATE healer_staff_pings SET resolved = 1, resolved_by = ?, resolved_at = ? WHERE id = ?",
        )
        .bind(resolved_by)
        .bind(&now)
        .bind(ping_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn has_running_session(&self, instance_id: &str) -> Result<bool> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM healer_sessions WHERE instance_id = ? AND state NOT IN \
             ('completed', 'done', 'failed', 'cancelled', 'needs_human_attention')",
        )
        .bind(instance_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count.0 > 0)
    }

    async fn has_recent_session(&self, instance_id: &str) -> Result<bool> {
        let one_hour_ago = (Utc::now() - chrono::Duration::hours(1))
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM healer_sessions WHERE instance_id = ? AND created_at > ?",
        )
        .bind(instance_id)
        .bind(&one_hour_ago)
        .fetch_one(&self.pool)
        .await?;
        Ok(count.0 > 0)
    }

    // -- Cluster settings (stub for local mode) ---------------------------

    async fn get_config(&self, _cluster_id: Uuid) -> Result<Option<serde_json::Value>> {
        Ok(None)
    }

    async fn save_config(&self, _cluster_id: Uuid, _config: &serde_json::Value) -> Result<()> {
        anyhow::bail!("config management not available in local mode")
    }

    async fn list_skills(&self, _cluster_id: Uuid) -> Result<Vec<SkillEntry>> {
        Ok(Vec::new())
    }

    async fn add_skill(&self, _cluster_id: Uuid, _skill_channel_id: Uuid) -> Result<()> {
        anyhow::bail!("skill management not available in local mode")
    }

    async fn remove_skill(&self, _cluster_id: Uuid, _skill_channel_id: Uuid) -> Result<bool> {
        anyhow::bail!("skill management not available in local mode")
    }

    async fn list_mcp_servers(&self, _cluster_id: Uuid) -> Result<Vec<McpServerEntry>> {
        Ok(Vec::new())
    }

    async fn add_mcp_server(&self, _cluster_id: Uuid, _mcp_server_id: Uuid) -> Result<()> {
        anyhow::bail!("MCP server management not available in local mode")
    }

    async fn remove_mcp_server(&self, _cluster_id: Uuid, _mcp_server_id: Uuid) -> Result<bool> {
        anyhow::bail!("MCP server management not available in local mode")
    }
}
