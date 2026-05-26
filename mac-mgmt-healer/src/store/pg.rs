//! Postgres-backed implementation of [`HealerStore`].

use anyhow::{Context as _, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::*;
use crate::instance_data::InstanceDataSource;
use crate::session::models::{HealerMessage, HealerSession, SessionState, StaffPing};

/// [`HealerStore`] backed by a Postgres connection pool.
#[derive(Clone)]
pub struct PgHealerStore {
    pool: PgPool,
}

impl PgHealerStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Return the underlying pool (escape hatch for callers that still need it).
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

// ── Internal helpers ───────────────────────────────────────────────────

/// Persist a `state_change` message to the session chat log.
async fn append_state_change(
    pool: &PgPool,
    session_id: Uuid,
    state: &str,
    state_data: &serde_json::Value,
) {
    let reason = state_data
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let content = serde_json::json!({
        "state": state,
        "reason": reason,
    })
    .to_string();
    let _ = sqlx::query(
        "INSERT INTO healer_messages (session_id, role, content, metadata) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(session_id)
    .bind("state_change")
    .bind(&content)
    .bind(Some(state_data))
    .execute(pool)
    .await;
}

// ── sqlx row types ─────────────────────────────────────────────────────

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
    provider: Option<String>,
    model: Option<String>,
    label: Option<String>,
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
            provider: r.provider,
            model: r.model,
            label: r.label,
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

#[derive(sqlx::FromRow)]
struct StaffPingRow {
    id: Uuid,
    session_id: Uuid,
    cluster_id: Uuid,
    instance_id: String,
    category: String,
    message: String,
    resolved: bool,
    resolved_by: Option<String>,
    resolved_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

impl From<StaffPingRow> for StaffPing {
    fn from(r: StaffPingRow) -> Self {
        Self {
            id: r.id,
            session_id: r.session_id,
            cluster_id: r.cluster_id,
            instance_id: r.instance_id,
            category: r.category,
            message: r.message,
            resolved: r.resolved,
            resolved_by: r.resolved_by,
            resolved_at: r.resolved_at,
            created_at: r.created_at,
        }
    }
}

// ── Trait implementation ───────────────────────────────────────────────

#[async_trait]
impl HealerStore for PgHealerStore {
    // -- Session lifecycle ------------------------------------------------

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
        let id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO healer_sessions (cluster_id, instance_id, state, state_data, created_by, initial_issues, provider, model, label) \
             VALUES ($1, $2, 'created', $3, $4, $5, $6, $7, $8) \
             RETURNING id",
        )
        .bind(cluster_id)
        .bind(instance_id)
        .bind(state_data)
        .bind(created_by)
        .bind(initial_issues)
        .bind(provider)
        .bind(model)
        .bind(label)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    async fn set_label(&self, session_id: Uuid, label: &str) -> Result<()> {
        sqlx::query("UPDATE healer_sessions SET label = $1 WHERE id = $2")
            .bind(label)
            .bind(session_id)
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
        .execute(&self.pool)
        .await?;

        append_state_change(&self.pool, session_id, new_state.as_str(), state_data).await;
        Ok(())
    }

    async fn fail_session(
        &self,
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
        .execute(&self.pool)
        .await?;

        let data = serde_json::json!({"reason": error_message});
        append_state_change(&self.pool, session_id, "failed", &data).await;
        Ok(())
    }

    async fn get_session(&self, session_id: Uuid) -> Result<Option<HealerSession>> {
        let row = sqlx::query_as::<_, SessionRow>(
            "SELECT id, cluster_id, instance_id, state, state_data, created_by, \
                    created_at, updated_at, completed_at, error_message, initial_issues, \
                    provider, model, label \
             FROM healer_sessions WHERE id = $1",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    async fn list_sessions(&self, cluster_id: Uuid) -> Result<Vec<HealerSession>> {
        let rows = sqlx::query_as::<_, SessionRow>(
            "SELECT id, cluster_id, instance_id, state, state_data, created_by, \
                    created_at, updated_at, completed_at, error_message, initial_issues, \
                    provider, model, label \
             FROM healer_sessions WHERE cluster_id = $1 \
             ORDER BY created_at DESC LIMIT 100",
        )
        .bind(cluster_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_resumable(&self) -> Result<Vec<HealerSession>> {
        let rows = sqlx::query_as::<_, SessionRow>(
            "SELECT id, cluster_id, instance_id, state, state_data, created_by, \
                    created_at, updated_at, completed_at, error_message, initial_issues, \
                    provider, model, label \
             FROM healer_sessions \
             WHERE state NOT IN ('completed', 'done', 'failed', 'cancelled', 'paused', 'needs_human_attention') \
               AND created_by != 'admin-mcp' \
             ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn update_provider_model(
        &self,
        session_id: Uuid,
        provider: &str,
        model: &str,
    ) -> Result<()> {
        sqlx::query("UPDATE healer_sessions SET provider = $1, model = $2 WHERE id = $3")
            .bind(provider)
            .bind(model)
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // -- Messages ---------------------------------------------------------

    async fn append_message(
        &self,
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
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_messages(&self, session_id: Uuid) -> Result<Vec<HealerMessage>> {
        let rows = sqlx::query_as::<_, MessageRow>(
            "SELECT id, session_id, role, content, metadata, created_at \
             FROM healer_messages WHERE session_id = $1 \
             ORDER BY created_at ASC",
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn get_messages_after(
        &self,
        session_id: Uuid,
        after: DateTime<Utc>,
    ) -> Result<Vec<HealerMessage>> {
        let rows = sqlx::query_as::<_, MessageRow>(
            "SELECT id, session_id, role, content, metadata, created_at \
             FROM healer_messages WHERE session_id = $1 AND created_at > $2 \
             ORDER BY created_at ASC",
        )
        .bind(session_id)
        .bind(after)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    // -- Staff pings ------------------------------------------------------

    async fn create_staff_ping(
        &self,
        session_id: Uuid,
        cluster_id: Uuid,
        instance_id: &str,
        category: &str,
        message: &str,
    ) -> Result<Uuid> {
        let id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO healer_staff_pings (session_id, cluster_id, instance_id, category, message) \
             VALUES ($1, $2, $3, $4, $5) RETURNING id",
        )
        .bind(session_id)
        .bind(cluster_id)
        .bind(instance_id)
        .bind(category)
        .bind(message)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    async fn list_staff_pings(&self, cluster_id: Uuid) -> Result<Vec<StaffPing>> {
        let rows = sqlx::query_as::<_, StaffPingRow>(
            "SELECT id, session_id, cluster_id, instance_id, category, message, \
                    resolved, resolved_by, resolved_at, created_at \
             FROM healer_staff_pings WHERE cluster_id = $1 \
             ORDER BY resolved ASC, created_at DESC LIMIT 100",
        )
        .bind(cluster_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn list_instance_pings(&self, instance_id: &str) -> Result<Vec<StaffPing>> {
        let rows = sqlx::query_as::<_, StaffPingRow>(
            "SELECT id, session_id, cluster_id, instance_id, category, message, \
                    resolved, resolved_by, resolved_at, created_at \
             FROM healer_staff_pings WHERE instance_id = $1 AND NOT resolved \
             ORDER BY created_at DESC LIMIT 50",
        )
        .bind(instance_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn list_session_pings(&self, session_id: Uuid) -> Result<Vec<StaffPing>> {
        let rows = sqlx::query_as::<_, StaffPingRow>(
            "SELECT id, session_id, cluster_id, instance_id, category, message, \
                    resolved, resolved_by, resolved_at, created_at \
             FROM healer_staff_pings WHERE session_id = $1 \
             ORDER BY created_at ASC",
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn resolve_staff_ping(&self, ping_id: Uuid, resolved_by: &str) -> Result<()> {
        sqlx::query(
            "UPDATE healer_staff_pings SET resolved = true, resolved_by = $1, resolved_at = now() \
             WHERE id = $2",
        )
        .bind(resolved_by)
        .bind(ping_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // -- Session guards ---------------------------------------------------

    async fn has_running_session(&self, instance_id: &str) -> Result<bool> {
        Ok(sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM healer_sessions \
             WHERE instance_id = $1 AND state NOT IN \
             ('completed', 'done', 'failed', 'cancelled', 'needs_human_attention'))",
        )
        .bind(instance_id)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(false))
    }

    async fn has_recent_session(&self, instance_id: &str) -> Result<bool> {
        Ok(sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM healer_sessions \
             WHERE instance_id = $1 AND created_at > now() - interval '1 hour')",
        )
        .bind(instance_id)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(false))
    }

    async fn find_mcp_session(&self) -> Result<Option<HealerSession>> {
        let row = sqlx::query_as::<_, SessionRow>(
            "SELECT id, cluster_id, instance_id, state, state_data, created_by, \
                    created_at, updated_at, completed_at, error_message, initial_issues, \
                    provider, model, label \
             FROM healer_sessions \
             WHERE created_by = 'admin-mcp' \
               AND state NOT IN ('completed', 'done', 'failed', 'cancelled', 'needs_human_attention') \
             ORDER BY created_at DESC \
             LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    // -- Cluster settings -------------------------------------------------

    async fn get_config(&self, cluster_id: Uuid) -> Result<Option<serde_json::Value>> {
        Ok(sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT config_json FROM cluster_configs \
             WHERE cluster_id = $1 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(cluster_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    async fn save_config(&self, cluster_id: Uuid, config: &serde_json::Value) -> Result<()> {
        sqlx::query("INSERT INTO cluster_configs (cluster_id, config_json) VALUES ($1, $2)")
            .bind(cluster_id)
            .bind(config)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn list_skills(&self, cluster_id: Uuid) -> Result<Vec<SkillEntry>> {
        let rows = sqlx::query_as::<_, (Uuid, String, String)>(
            "SELECT sc.id, s.slug, sc.channel \
             FROM cluster_skills cs \
             JOIN skill_channels sc ON sc.id = cs.skill_channel_id \
             JOIN skills s ON s.id = sc.skill_id \
             WHERE cs.cluster_id = $1 \
             ORDER BY s.slug, sc.channel",
        )
        .bind(cluster_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(id, slug, channel)| SkillEntry { id, slug, channel })
            .collect())
    }

    async fn add_skill(&self, cluster_id: Uuid, skill_channel_id: Uuid) -> Result<()> {
        sqlx::query(
            "INSERT INTO cluster_skills (cluster_id, skill_channel_id) VALUES ($1, $2) \
             ON CONFLICT DO NOTHING",
        )
        .bind(cluster_id)
        .bind(skill_channel_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn remove_skill(&self, cluster_id: Uuid, skill_channel_id: Uuid) -> Result<bool> {
        let r = sqlx::query(
            "DELETE FROM cluster_skills WHERE cluster_id = $1 AND skill_channel_id = $2",
        )
        .bind(cluster_id)
        .bind(skill_channel_id)
        .execute(&self.pool)
        .await?;
        Ok(r.rows_affected() > 0)
    }

    async fn list_mcp_servers(&self, cluster_id: Uuid) -> Result<Vec<McpServerEntry>> {
        let rows = sqlx::query_as::<_, (Uuid, String, String)>(
            "SELECT m.id, m.slug, m.name \
             FROM cluster_mcp_servers cms \
             JOIN mcp_servers m ON m.id = cms.mcp_server_id \
             WHERE cms.cluster_id = $1 \
             ORDER BY m.slug",
        )
        .bind(cluster_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(id, slug, name)| McpServerEntry { id, slug, name })
            .collect())
    }

    async fn add_mcp_server(&self, cluster_id: Uuid, mcp_server_id: Uuid) -> Result<()> {
        sqlx::query(
            "INSERT INTO cluster_mcp_servers (cluster_id, mcp_server_id) VALUES ($1, $2) \
             ON CONFLICT DO NOTHING",
        )
        .bind(cluster_id)
        .bind(mcp_server_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn remove_mcp_server(&self, cluster_id: Uuid, mcp_server_id: Uuid) -> Result<bool> {
        let r = sqlx::query(
            "DELETE FROM cluster_mcp_servers WHERE cluster_id = $1 AND mcp_server_id = $2",
        )
        .bind(cluster_id)
        .bind(mcp_server_id)
        .execute(&self.pool)
        .await?;
        Ok(r.rows_affected() > 0)
    }

    // -- Token usage tracking ------------------------------------------------

    async fn append_token_event(
        &self,
        session_id: Uuid,
        provider: &str,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<u64> {
        let total = (input_tokens + output_tokens) as i64;
        // Insert event and atomically update denormalized total.
        sqlx::query(
            "INSERT INTO healer_token_events (session_id, provider, model, input_tokens, output_tokens) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(session_id)
        .bind(provider)
        .bind(model)
        .bind(input_tokens as i32)
        .bind(output_tokens as i32)
        .execute(&self.pool)
        .await?;

        let new_total: i64 = sqlx::query_scalar(
            "UPDATE healer_sessions SET tokens_used = tokens_used + $1 WHERE id = $2 \
             RETURNING tokens_used",
        )
        .bind(total)
        .bind(session_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(new_total as u64)
    }

    async fn get_token_usage(&self, session_id: Uuid) -> Result<u64> {
        let used: i64 = sqlx::query_scalar("SELECT tokens_used FROM healer_sessions WHERE id = $1")
            .bind(session_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(used as u64)
    }

    async fn set_token_budget(&self, session_id: Uuid, budget: u64) -> Result<()> {
        sqlx::query("UPDATE healer_sessions SET token_budget = $1 WHERE id = $2")
            .bind(budget as i64)
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn get_token_budget(&self, session_id: Uuid) -> Result<u64> {
        let budget: i64 =
            sqlx::query_scalar("SELECT token_budget FROM healer_sessions WHERE id = $1")
                .bind(session_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(budget as u64)
    }
}

// ── Instance data (InstanceDataSource) ────────────────────────────────

#[async_trait]
impl InstanceDataSource for PgHealerStore {
    async fn get_probe_status(&self, instance_id: &str) -> Result<Option<ProbeStatus>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            services_extended: Option<serde_json::Value>,
            sample: Option<serde_json::Value>,
            reported_at: DateTime<Utc>,
        }
        let row = sqlx::query_as::<_, Row>(
            "SELECT services_extended, sample, reported_at \
             FROM daemon_heartbeats WHERE instance_id = $1 LIMIT 1",
        )
        .bind(instance_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| ProbeStatus {
            services_extended: r.services_extended,
            sample: r.sample,
            reported_at: r.reported_at,
        }))
    }

    async fn get_system_sample(&self, instance_id: &str) -> Result<Option<SystemSample>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            sample: Option<serde_json::Value>,
            reported_at: DateTime<Utc>,
        }
        let row = sqlx::query_as::<_, Row>(
            "SELECT sample, reported_at \
             FROM daemon_heartbeats WHERE instance_id = $1 LIMIT 1",
        )
        .bind(instance_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| SystemSample {
            sample: r.sample,
            reported_at: r.reported_at,
        }))
    }

    async fn get_inventory(&self, instance_id: &str) -> Result<Option<Inventory>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            inventory: serde_json::Value,
            security: serde_json::Value,
            collected_at: DateTime<Utc>,
        }
        let row = sqlx::query_as::<_, Row>(
            "SELECT inventory, security, collected_at \
             FROM assessments WHERE instance_id = $1 \
             ORDER BY collected_at DESC LIMIT 1",
        )
        .bind(instance_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| Inventory {
            inventory: r.inventory,
            security: r.security,
            collected_at: r.collected_at,
        }))
    }

    async fn get_probe_history(
        &self,
        instance_id: &str,
        service: Option<&str>,
        limit: i64,
    ) -> Result<Vec<ProbeHistoryEntry>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            service: String,
            kind: String,
            ok: bool,
            duration_ms: i64,
            collected_at: DateTime<Utc>,
            model: Option<String>,
            error_class: Option<String>,
            error_detail: Option<String>,
            tokens_in: Option<i32>,
            tokens_out: Option<i32>,
            first_token_ms: Option<i64>,
        }

        let rows = if let Some(svc) = service {
            sqlx::query_as::<_, Row>(
                "SELECT service, kind, ok, duration_ms, collected_at, model, \
                        error_class, error_detail, tokens_in, tokens_out, first_token_ms \
                 FROM assessment_probes WHERE instance_id = $1 AND service = $2 \
                 ORDER BY collected_at DESC LIMIT $3",
            )
            .bind(instance_id)
            .bind(svc)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, Row>(
                "SELECT service, kind, ok, duration_ms, collected_at, model, \
                        error_class, error_detail, tokens_in, tokens_out, first_token_ms \
                 FROM assessment_probes WHERE instance_id = $1 \
                 ORDER BY collected_at DESC LIMIT $2",
            )
            .bind(instance_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|r| ProbeHistoryEntry {
                service: r.service,
                kind: r.kind,
                ok: r.ok,
                duration_ms: r.duration_ms,
                collected_at: r.collected_at,
                model: r.model,
                error_class: r.error_class,
                error_detail: r.error_detail,
                tokens_in: r.tokens_in,
                tokens_out: r.tokens_out,
                first_token_ms: r.first_token_ms,
            })
            .collect())
    }

    async fn get_heartbeat_json(&self, instance_id: &str) -> Result<Option<serde_json::Value>> {
        let row = sqlx::query_as::<_, (serde_json::Value,)>(
            "SELECT row_to_json(h) FROM daemon_heartbeats h WHERE instance_id = $1 LIMIT 1",
        )
        .bind(instance_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(v,)| v))
    }

    async fn get_version_info(&self, instance_id: &str) -> Result<VersionInfo> {
        #[derive(sqlx::FromRow)]
        struct HbRow {
            version: String,
            git_sha: Option<String>,
            reported_at: DateTime<Utc>,
        }
        let hb = sqlx::query_as::<_, HbRow>(
            "SELECT version, git_sha, reported_at FROM daemon_heartbeats \
             WHERE instance_id = $1 LIMIT 1",
        )
        .bind(instance_id)
        .fetch_optional(&self.pool)
        .await?;

        #[derive(sqlx::FromRow)]
        struct DvRow {
            version: String,
            system: String,
            store_path: Option<String>,
            created_at: DateTime<Utc>,
        }
        let dvs = sqlx::query_as::<_, DvRow>(
            "SELECT version, system, store_path, created_at FROM daemon_versions \
             ORDER BY created_at DESC LIMIT 10",
        )
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        Ok(VersionInfo {
            heartbeat: hb.map(|h| HeartbeatVersion {
                version: h.version,
                git_sha: h.git_sha,
                reported_at: h.reported_at,
            }),
            daemon_versions: dvs
                .into_iter()
                .map(|d| DaemonVersion {
                    version: d.version,
                    system: d.system,
                    store_path: d.store_path,
                    created_at: d.created_at,
                })
                .collect(),
        })
    }

    async fn get_cluster_instances(&self, cluster_id: Uuid) -> Result<Vec<ClusterInstance>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            instance_id: String,
            version: String,
            hostname: Option<String>,
            reported_at: DateTime<Utc>,
            services_extended: Option<serde_json::Value>,
        }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT instance_id, version, hostname, reported_at, services_extended \
             FROM daemon_heartbeats WHERE cluster_id = $1 \
             ORDER BY reported_at DESC",
        )
        .bind(cluster_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| ClusterInstance {
                instance_id: r.instance_id,
                version: r.version,
                hostname: r.hostname,
                reported_at: r.reported_at,
                services_extended: r.services_extended,
            })
            .collect())
    }

    async fn get_service_state(&self, instance_id: &str) -> Result<Option<ServiceState>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            services: serde_json::Value,
            services_extended: Option<serde_json::Value>,
            reported_at: DateTime<Utc>,
        }
        let row = sqlx::query_as::<_, Row>(
            "SELECT services, services_extended, reported_at \
             FROM daemon_heartbeats WHERE instance_id = $1 LIMIT 1",
        )
        .bind(instance_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| ServiceState {
            services: r.services,
            services_extended: r.services_extended,
            reported_at: r.reported_at,
        }))
    }

    async fn has_recent_heartbeat(&self, instance_id: &str) -> Result<bool> {
        PgHealerStore::has_recent_heartbeat(self, instance_id).await
    }

    async fn get_relay_proxy_url(&self, instance_id: &str) -> Result<Option<String>> {
        PgHealerStore::get_relay_proxy_url(self, instance_id).await
    }
}

// ── Server-only helpers (not trait methods) ────────────────────────────

impl PgHealerStore {
    pub async fn mint_proxy_token(
        &self,
        cluster_id: Uuid,
        organization_id: Option<Uuid>,
    ) -> Result<(String, DateTime<Utc>)> {
        self.mint_proxy_token_scoped(cluster_id, organization_id, None)
            .await
    }

    pub async fn mint_proxy_token_scoped(
        &self,
        cluster_id: Uuid,
        organization_id: Option<Uuid>,
        scopes: Option<&[&str]>,
    ) -> Result<(String, DateTime<Utc>)> {
        use rand::Rng;
        use sha2::{Digest, Sha256};

        let raw_token = hex::encode(rand::rng().random::<[u8; 32]>());
        let hash = hex::encode(Sha256::digest(raw_token.as_bytes()));
        let expires_at = Utc::now() + chrono::Duration::hours(6);
        let scopes_json = scopes.map(|s| serde_json::json!(s));

        sqlx::query(
            "INSERT INTO tokens (cluster_id, organization_id, token_hash, label, kind, expires_at, scopes) \
             VALUES ($1, $2, $3, 'healer', 'proxy', $4, $5)",
        )
        .bind(cluster_id)
        .bind(organization_id)
        .bind(&hash)
        .bind(expires_at)
        .bind(&scopes_json)
        .execute(&self.pool)
        .await
        .context("failed to mint proxy token")?;

        Ok((raw_token, expires_at))
    }

    pub async fn get_relay_proxy_url(&self, instance_id: &str) -> Result<Option<String>> {
        Ok(sqlx::query_scalar(
            "SELECT relay_proxy_url FROM daemon_heartbeats \
             WHERE instance_id = $1 AND relay_proxy_url IS NOT NULL AND relay_proxy_url != '' \
             LIMIT 1",
        )
        .bind(instance_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn has_recent_heartbeat(&self, instance_id: &str) -> Result<bool> {
        Ok(sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM daemon_heartbeats \
             WHERE instance_id = $1 AND reported_at > now() - interval '2 minutes')",
        )
        .bind(instance_id)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(false))
    }
}
