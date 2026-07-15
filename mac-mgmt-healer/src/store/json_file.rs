//! JSON file-backed implementation of [`HealerStore`], built on the generic
//! [`plan_ai_chat::store::json_file::JsonFileChatStore`].
//!
//! Generic session/message/token persistence lives in the shared store; only
//! healer-specific data keeps logic here: staff pings ride in each session
//! file's extension blob under the `pings` key (the pre-unification on-disk
//! layout — old files parse unchanged via the ChatSession field aliases).

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use plan_ai_chat::store::json_file::{JsonFileChatStore, SessionFile};
use plan_ai_chat::{ChatStore, NewSession};
use std::path::PathBuf;
use uuid::Uuid;

use super::*;
use crate::session::models::{
    HealerMessage, HealerSession, INACTIVE_STATES, NON_RESUMABLE_STATES, SessionState, StaffPing,
};

/// [`HealerStore`] backed by JSON files in a directory.
pub struct JsonFileStore {
    inner: JsonFileChatStore,
}

const PINGS_KEY: &str = "pings";

fn pings_of(sf: &SessionFile) -> Vec<StaffPing> {
    sf.extra
        .get(PINGS_KEY)
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

fn set_pings(sf: &mut SessionFile, pings: &[StaffPing]) {
    sf.extra.insert(
        PINGS_KEY.to_string(),
        serde_json::to_value(pings).unwrap_or_default(),
    );
}

impl JsonFileStore {
    /// Open (or create) the store directory.
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self> {
        Ok(Self {
            inner: JsonFileChatStore::open(dir)?,
        })
    }
}

// ── Trait implementation ───────────────────────────────────────────────

#[async_trait]
impl HealerStore for JsonFileStore {
    // -- Session lifecycle (delegated to the generic file store) ------------

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
        self.inner
            .create_session(NewSession {
                scope_id: cluster_id,
                subject: instance_id,
                created_by,
                initial_context: initial_issues,
                state_data,
                provider,
                model,
                label,
            })
            .await
    }

    async fn set_label(&self, session_id: Uuid, label: &str) -> Result<()> {
        ChatStore::set_label(&self.inner, session_id, label).await
    }

    async fn transition_state(
        &self,
        session_id: Uuid,
        new_state: &SessionState,
        state_data: &serde_json::Value,
    ) -> Result<()> {
        self.inner
            .transition_state(
                session_id,
                new_state.as_str(),
                new_state.is_terminal(),
                state_data,
            )
            .await
    }

    async fn fail_session(
        &self,
        session_id: Uuid,
        error_message: &str,
        state_data: &serde_json::Value,
    ) -> Result<()> {
        ChatStore::fail_session(&self.inner, session_id, error_message, state_data).await
    }

    async fn get_session(&self, session_id: Uuid) -> Result<Option<HealerSession>> {
        Ok(ChatStore::get_session(&self.inner, session_id)
            .await?
            .map(Into::into))
    }

    async fn list_sessions(&self, cluster_id: Uuid) -> Result<Vec<HealerSession>> {
        Ok(ChatStore::list_sessions(&self.inner, cluster_id)
            .await?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    async fn find_resumable(&self) -> Result<Vec<HealerSession>> {
        Ok(self
            .inner
            .find_resumable(NON_RESUMABLE_STATES, &["admin-mcp"])
            .await?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    async fn update_provider_model(
        &self,
        session_id: Uuid,
        provider: &str,
        model: &str,
    ) -> Result<()> {
        ChatStore::update_provider_model(&self.inner, session_id, provider, model).await
    }

    // -- Messages -----------------------------------------------------------

    async fn append_message(
        &self,
        session_id: Uuid,
        role: &str,
        content: &str,
        metadata: Option<&serde_json::Value>,
    ) -> Result<()> {
        ChatStore::append_message(&self.inner, session_id, role, content, metadata).await
    }

    async fn get_messages(&self, session_id: Uuid) -> Result<Vec<HealerMessage>> {
        ChatStore::get_messages(&self.inner, session_id).await
    }

    async fn get_messages_after(
        &self,
        session_id: Uuid,
        after: DateTime<Utc>,
    ) -> Result<Vec<HealerMessage>> {
        ChatStore::get_messages_after(&self.inner, session_id, after).await
    }

    // -- Staff pings (healer-specific, stored in the session file) ----------

    async fn create_staff_ping(
        &self,
        session_id: Uuid,
        cluster_id: Uuid,
        instance_id: &str,
        category: &str,
        message: &str,
    ) -> Result<Uuid> {
        let ping_id = Uuid::new_v4();
        let ping = StaffPing {
            id: ping_id,
            session_id,
            cluster_id,
            instance_id: instance_id.to_string(),
            category: category.to_string(),
            message: message.to_string(),
            resolved: false,
            resolved_by: None,
            resolved_at: None,
            created_at: Utc::now(),
        };
        self.inner.mutate(session_id, |sf| {
            let mut pings = pings_of(sf);
            pings.push(ping);
            set_pings(sf, &pings);
        })?;
        Ok(ping_id)
    }

    async fn list_staff_pings(&self, cluster_id: Uuid) -> Result<Vec<StaffPing>> {
        let mut pings: Vec<StaffPing> = self
            .inner
            .all_sessions()?
            .iter()
            .filter(|sf| sf.session.scope_id == cluster_id)
            .flat_map(pings_of)
            .collect();
        pings.sort_by(|a, b| {
            a.resolved
                .cmp(&b.resolved)
                .then(b.created_at.cmp(&a.created_at))
        });
        pings.truncate(100);
        Ok(pings)
    }

    async fn list_instance_pings(&self, instance_id: &str) -> Result<Vec<StaffPing>> {
        let mut pings: Vec<StaffPing> = self
            .inner
            .all_sessions()?
            .iter()
            .flat_map(pings_of)
            .filter(|p| p.instance_id == instance_id && !p.resolved)
            .collect();
        pings.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        pings.truncate(50);
        Ok(pings)
    }

    async fn list_session_pings(&self, session_id: Uuid) -> Result<Vec<StaffPing>> {
        Ok(self
            .inner
            .read_file(session_id)?
            .map(|sf| pings_of(&sf))
            .unwrap_or_default())
    }

    async fn resolve_staff_ping(&self, ping_id: Uuid, resolved_by: &str) -> Result<()> {
        let resolved_by = resolved_by.to_string();
        // Scan all sessions to find the ping
        let all = self.inner.all_sessions()?;
        for sf in &all {
            if pings_of(sf).iter().any(|p| p.id == ping_id) {
                return self.inner.mutate(sf.session.id, |sf| {
                    let mut pings = pings_of(sf);
                    if let Some(p) = pings.iter_mut().find(|p| p.id == ping_id) {
                        p.resolved = true;
                        p.resolved_by = Some(resolved_by.clone());
                        p.resolved_at = Some(Utc::now());
                    }
                    set_pings(sf, &pings);
                });
            }
        }
        anyhow::bail!("staff ping {ping_id} not found")
    }

    // -- Session guards ---------------------------------------------------

    async fn has_running_session(&self, instance_id: &str) -> Result<bool> {
        ChatStore::has_running_session(&self.inner, instance_id, INACTIVE_STATES).await
    }

    async fn has_recent_session(&self, instance_id: &str) -> Result<bool> {
        let cutoff = Utc::now() - Duration::hours(1);
        Ok(self
            .inner
            .all_sessions()?
            .iter()
            .any(|sf| sf.session.subject == instance_id && sf.session.created_at > cutoff))
    }

    async fn find_mcp_session(&self) -> Result<Option<HealerSession>> {
        let mut sessions: Vec<HealerSession> = self
            .inner
            .all_sessions()?
            .into_iter()
            .filter(|sf| {
                sf.session.created_by == "admin-mcp"
                    && !INACTIVE_STATES.contains(&sf.session.state.as_str())
            })
            .map(|sf| sf.session.into())
            .collect();
        sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(sessions.into_iter().next())
    }

    // -- Cluster settings (not applicable in local mode) ------------------

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

    // -- Token usage tracking (delegated) -----------------------------------

    async fn append_token_event(
        &self,
        session_id: Uuid,
        provider: &str,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<u64> {
        ChatStore::append_token_event(
            &self.inner,
            session_id,
            provider,
            model,
            input_tokens,
            output_tokens,
        )
        .await
    }

    async fn get_token_usage(&self, session_id: Uuid) -> Result<u64> {
        ChatStore::get_token_usage(&self.inner, session_id).await
    }

    async fn set_token_budget(&self, session_id: Uuid, budget: u64) -> Result<()> {
        ChatStore::set_token_budget(&self.inner, session_id, budget).await
    }

    async fn get_token_budget(&self, session_id: Uuid) -> Result<u64> {
        ChatStore::get_token_budget(&self.inner, session_id).await
    }
}

// ── ChatStore (generic view, for SessionManager / session loop) ─────────

#[async_trait]
impl ChatStore for JsonFileStore {
    async fn create_session(&self, new: NewSession<'_>) -> Result<Uuid> {
        self.inner.create_session(new).await
    }

    async fn set_label(&self, session_id: Uuid, label: &str) -> Result<()> {
        ChatStore::set_label(&self.inner, session_id, label).await
    }

    async fn transition_state(
        &self,
        session_id: Uuid,
        new_state: &str,
        terminal: bool,
        state_data: &serde_json::Value,
    ) -> Result<()> {
        // Reject unknown states before they land on disk — the healer state
        // machine only understands its own vocabulary.
        SessionState::from_str(new_state)
            .ok_or_else(|| anyhow::anyhow!("unknown session state '{new_state}'"))?;
        ChatStore::transition_state(&self.inner, session_id, new_state, terminal, state_data).await
    }

    async fn fail_session(
        &self,
        session_id: Uuid,
        error_message: &str,
        state_data: &serde_json::Value,
    ) -> Result<()> {
        ChatStore::fail_session(&self.inner, session_id, error_message, state_data).await
    }

    async fn get_session(&self, session_id: Uuid) -> Result<Option<plan_ai_chat::ChatSession>> {
        ChatStore::get_session(&self.inner, session_id).await
    }

    async fn list_sessions(&self, scope_id: Uuid) -> Result<Vec<plan_ai_chat::ChatSession>> {
        ChatStore::list_sessions(&self.inner, scope_id).await
    }

    async fn find_resumable(
        &self,
        non_resumable: &[&str],
        exclude_created_by: &[&str],
    ) -> Result<Vec<plan_ai_chat::ChatSession>> {
        ChatStore::find_resumable(&self.inner, non_resumable, exclude_created_by).await
    }

    async fn update_provider_model(
        &self,
        session_id: Uuid,
        provider: &str,
        model: &str,
    ) -> Result<()> {
        ChatStore::update_provider_model(&self.inner, session_id, provider, model).await
    }

    async fn has_running_session(&self, subject: &str, inactive_states: &[&str]) -> Result<bool> {
        ChatStore::has_running_session(&self.inner, subject, inactive_states).await
    }

    async fn append_message(
        &self,
        session_id: Uuid,
        role: &str,
        content: &str,
        metadata: Option<&serde_json::Value>,
    ) -> Result<()> {
        ChatStore::append_message(&self.inner, session_id, role, content, metadata).await
    }

    async fn get_messages(&self, session_id: Uuid) -> Result<Vec<HealerMessage>> {
        ChatStore::get_messages(&self.inner, session_id).await
    }

    async fn get_messages_after(
        &self,
        session_id: Uuid,
        after: DateTime<Utc>,
    ) -> Result<Vec<HealerMessage>> {
        ChatStore::get_messages_after(&self.inner, session_id, after).await
    }

    async fn append_token_event(
        &self,
        session_id: Uuid,
        provider: &str,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<u64> {
        ChatStore::append_token_event(
            &self.inner,
            session_id,
            provider,
            model,
            input_tokens,
            output_tokens,
        )
        .await
    }

    async fn get_token_usage(&self, session_id: Uuid) -> Result<u64> {
        ChatStore::get_token_usage(&self.inner, session_id).await
    }

    async fn set_token_budget(&self, session_id: Uuid, budget: u64) -> Result<()> {
        ChatStore::set_token_budget(&self.inner, session_id, budget).await
    }

    async fn get_token_budget(&self, session_id: Uuid) -> Result<u64> {
        ChatStore::get_token_budget(&self.inner, session_id).await
    }
}
