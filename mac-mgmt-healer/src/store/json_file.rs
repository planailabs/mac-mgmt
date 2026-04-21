//! JSON file-backed implementation of [`HealerStore`].
//!
//! Each session is a single JSON file containing the session metadata,
//! messages, and staff pings. Zero external dependencies beyond serde_json.
//!
//! Layout:
//! ```text
//! {dir}/
//!   {session_uuid}.json   # SessionFile { session, messages, pings }
//! ```

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context as _, Result};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::session::models::{HealerMessage, HealerSession, SessionState, StaffPing};
use super::*;

/// On-disk format: one file per session.
#[derive(Serialize, Deserialize)]
struct SessionFile {
    session: HealerSession,
    messages: Vec<HealerMessage>,
    pings: Vec<StaffPing>,
}

/// [`HealerStore`] backed by JSON files in a directory.
pub struct JsonFileStore {
    dir: PathBuf,
    /// Serialize writes to avoid torn reads during concurrent appends.
    lock: Mutex<()>,
}

impl JsonFileStore {
    /// Open (or create) the store directory.
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create healer store dir: {}", dir.display()))?;
        Ok(Self {
            dir,
            lock: Mutex::new(()),
        })
    }

    fn session_path(&self, id: Uuid) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    fn read_file(&self, id: Uuid) -> Result<Option<SessionFile>> {
        let path = self.session_path(id);
        if !path.exists() {
            return Ok(None);
        }
        let data = std::fs::read(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let sf: SessionFile = serde_json::from_slice(&data)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        Ok(Some(sf))
    }

    fn write_file(&self, id: Uuid, sf: &SessionFile) -> Result<()> {
        let path = self.session_path(id);
        let data = serde_json::to_vec_pretty(sf)?;
        atomic_write(&path, &data)
    }

    fn mutate<F>(&self, id: Uuid, f: F) -> Result<()>
    where
        F: FnOnce(&mut SessionFile),
    {
        let _guard = self.lock.lock().unwrap();
        let mut sf = self
            .read_file(id)?
            .ok_or_else(|| anyhow::anyhow!("session {id} not found"))?;
        f(&mut sf);
        self.write_file(id, &sf)
    }

    /// Read all session files in the directory.
    fn all_sessions(&self) -> Result<Vec<SessionFile>> {
        let mut out = Vec::new();
        let entries = std::fs::read_dir(&self.dir)?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                match std::fs::read(&path) {
                    Ok(data) => {
                        if let Ok(sf) = serde_json::from_slice::<SessionFile>(&data) {
                            out.push(sf);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("failed to read {}: {e}", path.display());
                    }
                }
            }
        }
        Ok(out)
    }
}

fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, data)
        .with_context(|| format!("failed to write {}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("failed to rename {} → {}", tmp.display(), path.display()))?;
    Ok(())
}

fn append_state_change(sf: &mut SessionFile, state: &str, state_data: &serde_json::Value) {
    let reason = state_data
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    sf.messages.push(HealerMessage {
        id: Uuid::new_v4(),
        session_id: sf.session.id,
        role: "state_change".to_string(),
        content: serde_json::json!({ "state": state, "reason": reason }).to_string(),
        metadata: Some(state_data.clone()),
        created_at: Utc::now(),
    });
}

// ── Trait implementation ───────────────────────────────────────────────

#[async_trait]
impl HealerStore for JsonFileStore {
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
        let now = Utc::now();
        let sf = SessionFile {
            session: HealerSession {
                id,
                cluster_id,
                instance_id: instance_id.to_string(),
                state: SessionState::Created,
                state_data: state_data.clone(),
                created_by: created_by.to_string(),
                created_at: now,
                updated_at: now,
                completed_at: None,
                error_message: None,
                initial_issues: initial_issues.clone(),
                provider: provider.map(String::from),
                model: model.map(String::from),
                label: label.map(String::from),
            },
            messages: Vec::new(),
            pings: Vec::new(),
        };
        let _guard = self.lock.lock().unwrap();
        self.write_file(id, &sf)?;
        Ok(id)
    }

    async fn set_label(&self, session_id: Uuid, label: &str) -> Result<()> {
        let label = label.to_string();
        self.mutate(session_id, |sf| {
            sf.session.label = Some(label);
            sf.session.updated_at = Utc::now();
        })
    }

    async fn transition_state(
        &self,
        session_id: Uuid,
        new_state: &SessionState,
        state_data: &serde_json::Value,
    ) -> Result<()> {
        let new_state = new_state.clone();
        let state_data = state_data.clone();
        self.mutate(session_id, |sf| {
            sf.session.state = new_state.clone();
            sf.session.state_data = state_data.clone();
            sf.session.updated_at = Utc::now();
            if new_state.is_terminal() {
                sf.session.completed_at = Some(Utc::now());
            }
            append_state_change(sf, new_state.as_str(), &state_data);
        })
    }

    async fn fail_session(
        &self,
        session_id: Uuid,
        error_message: &str,
        state_data: &serde_json::Value,
    ) -> Result<()> {
        let error_message = error_message.to_string();
        let state_data = state_data.clone();
        self.mutate(session_id, |sf| {
            sf.session.state = SessionState::Failed;
            sf.session.state_data = state_data.clone();
            sf.session.error_message = Some(error_message.clone());
            sf.session.updated_at = Utc::now();
            sf.session.completed_at = Some(Utc::now());
            let data = serde_json::json!({"reason": error_message});
            append_state_change(sf, "failed", &data);
        })
    }

    async fn get_session(&self, session_id: Uuid) -> Result<Option<HealerSession>> {
        Ok(self.read_file(session_id)?.map(|sf| sf.session))
    }

    async fn list_sessions(&self, cluster_id: Uuid) -> Result<Vec<HealerSession>> {
        let mut sessions: Vec<HealerSession> = self
            .all_sessions()?
            .into_iter()
            .filter(|sf| sf.session.cluster_id == cluster_id)
            .map(|sf| sf.session)
            .collect();
        sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        sessions.truncate(100);
        Ok(sessions)
    }

    async fn find_resumable(&self) -> Result<Vec<HealerSession>> {
        let mut sessions: Vec<HealerSession> = self
            .all_sessions()?
            .into_iter()
            .filter(|sf| {
                !matches!(
                    sf.session.state,
                    SessionState::Completed
                        | SessionState::Done
                        | SessionState::Failed
                        | SessionState::Cancelled
                        | SessionState::Paused
                        | SessionState::NeedsHumanAttention
                )
            })
            .map(|sf| sf.session)
            .collect();
        sessions.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(sessions)
    }

    async fn update_provider_model(
        &self,
        session_id: Uuid,
        provider: &str,
        model: &str,
    ) -> Result<()> {
        let provider = provider.to_string();
        let model = model.to_string();
        self.mutate(session_id, |sf| {
            sf.session.provider = Some(provider);
            sf.session.model = Some(model);
            sf.session.updated_at = Utc::now();
        })
    }

    async fn append_message(
        &self,
        session_id: Uuid,
        role: &str,
        content: &str,
        metadata: Option<&serde_json::Value>,
    ) -> Result<()> {
        let msg = HealerMessage {
            id: Uuid::new_v4(),
            session_id,
            role: role.to_string(),
            content: content.to_string(),
            metadata: metadata.cloned(),
            created_at: Utc::now(),
        };
        self.mutate(session_id, |sf| {
            sf.messages.push(msg);
        })
    }

    async fn get_messages(&self, session_id: Uuid) -> Result<Vec<HealerMessage>> {
        Ok(self
            .read_file(session_id)?
            .map(|sf| sf.messages)
            .unwrap_or_default())
    }

    async fn get_messages_after(
        &self,
        session_id: Uuid,
        after: DateTime<Utc>,
    ) -> Result<Vec<HealerMessage>> {
        Ok(self
            .read_file(session_id)?
            .map(|sf| {
                sf.messages
                    .into_iter()
                    .filter(|m| m.created_at > after)
                    .collect()
            })
            .unwrap_or_default())
    }

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
        self.mutate(session_id, |sf| {
            sf.pings.push(ping);
        })?;
        Ok(ping_id)
    }

    async fn list_staff_pings(&self, cluster_id: Uuid) -> Result<Vec<StaffPing>> {
        let mut pings: Vec<StaffPing> = self
            .all_sessions()?
            .into_iter()
            .filter(|sf| sf.session.cluster_id == cluster_id)
            .flat_map(|sf| sf.pings)
            .collect();
        pings.sort_by(|a, b| {
            a.resolved
                .cmp(&b.resolved)
                .then(b.created_at.cmp(&a.created_at))
        });
        pings.truncate(100);
        Ok(pings)
    }

    async fn list_session_pings(&self, session_id: Uuid) -> Result<Vec<StaffPing>> {
        Ok(self
            .read_file(session_id)?
            .map(|sf| sf.pings)
            .unwrap_or_default())
    }

    async fn resolve_staff_ping(&self, ping_id: Uuid, resolved_by: &str) -> Result<()> {
        let resolved_by = resolved_by.to_string();
        // Scan all sessions to find the ping
        let all = self.all_sessions()?;
        for sf in &all {
            if sf.pings.iter().any(|p| p.id == ping_id) {
                return self.mutate(sf.session.id, |sf| {
                    if let Some(p) = sf.pings.iter_mut().find(|p| p.id == ping_id) {
                        p.resolved = true;
                        p.resolved_by = Some(resolved_by.clone());
                        p.resolved_at = Some(Utc::now());
                    }
                });
            }
        }
        anyhow::bail!("staff ping {ping_id} not found")
    }

    async fn has_running_session(&self, instance_id: &str) -> Result<bool> {
        Ok(self.all_sessions()?.iter().any(|sf| {
            sf.session.instance_id == instance_id
                && !matches!(
                    sf.session.state,
                    SessionState::Completed
                        | SessionState::Done
                        | SessionState::Failed
                        | SessionState::Cancelled
                        | SessionState::NeedsHumanAttention
                )
        }))
    }

    async fn has_recent_session(&self, instance_id: &str) -> Result<bool> {
        let cutoff = Utc::now() - Duration::hours(1);
        Ok(self
            .all_sessions()?
            .iter()
            .any(|sf| sf.session.instance_id == instance_id && sf.session.created_at > cutoff))
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
}
