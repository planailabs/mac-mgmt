pub mod agent;
pub mod connector;
pub mod instance_access;
pub mod instance_data;
pub mod relay_client;
pub mod session;
pub mod settings_tools;
pub mod store;
pub mod tools;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde_json::json;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub use connector::ConnectorConfig;
pub use instance_access::{
    ClusterAccess, DynClusterAccess, DynInstanceAccess, FileReadResult, InstanceAccess,
    ShellOutput,
};
pub use instance_data::{DynInstanceData, InstanceDataSource};
pub use session::models::{HealerEvent, HealerMessage, HealerSession, SessionState};
pub use store::{DynStore, HealerStore};

use agent::InstanceInfo;
use mac_mgmt_common::ServiceExtState;
use swiftide::traits::ToolBox as _;

// ── Session factory trait ──────────────────────────────────────────────

/// Data returned by [`SessionFactory::build_access`] for a new or resumed session.
pub struct SessionAccess {
    pub instance: DynInstanceAccess,
    pub cluster: Option<DynClusterAccess>,
    pub metrics_url: Option<String>,
    pub proxy_expires: Option<DateTime<Utc>>,
}

/// Factory that creates instance/cluster access handles for a healer session.
///
/// * **Server mode** — mints a proxy token, builds relay-based access.
/// * **Daemon mode** — returns local access (no token needed).
#[async_trait::async_trait]
pub trait SessionFactory: Send + Sync + 'static {
    /// Build instance access for a session (new or resumed).
    async fn build_access(&self, session: &HealerSession) -> Result<SessionAccess>;
}

/// Shared state for the healer subsystem. Clone-friendly (inner Arc).
#[derive(Clone)]
pub struct HealerState {
    inner: Arc<HealerStateInner>,
}

struct HealerStateInner {
    store: DynStore,
    instance_data: DynInstanceData,
    session_factory: Arc<dyn SessionFactory>,
    connector_config: ConnectorConfig,
    running: DashMap<Uuid, RunningSession>,
    shutting_down: AtomicBool,
    push_fn: Option<tools::PushFn>,
    /// Budget extensions for sessions that were extended while not running
    /// (e.g. paused). Consumed when the session resumes.
    extended_budgets: DashMap<Uuid, u64>,
}

#[allow(dead_code)] // approval_notify is kept alive here; used via Arc in the spawned task
struct RunningSession {
    cancel: CancellationToken,
    pause_notify: Arc<tokio::sync::Notify>,
    budget_limit: Arc<std::sync::atomic::AtomicU64>,
    events_tx: broadcast::Sender<HealerEvent>,
    /// Currently executing tools (in-memory only, not persisted).
    running_tools: Arc<std::sync::Mutex<Vec<session::RunningTool>>>,
    /// Fired by the set_phase tool when transitioning to AwaitingApproval.
    approval_notify: Arc<tokio::sync::Notify>,
}

/// Request to spawn a new healer session.
pub struct SpawnRequest {
    pub cluster_id: Uuid,
    pub instance_id: String,
    pub created_by: String,
    pub user_message: Option<String>,
    pub instance_access: DynInstanceAccess,
    pub cluster_access: Option<DynClusterAccess>,
    /// Optional metrics URL for the `get_metrics` tool (server mode only).
    pub metrics_url: Option<String>,
    pub services_extended: Vec<ServiceExtState>,
    pub sample: Option<serde_json::Value>,
    pub file_tunnels: serde_json::Value,
    pub shell_tunnels: serde_json::Value,
    pub cluster_instances: Vec<InstanceInfo>,
    pub cluster_name: String,
    pub hostname: String,
    /// Skip the 1-hour cooldown (set when DEV_ONLY_NO_AUTH=1).
    pub skip_cooldown: bool,
    /// Force a specific LLM provider ("ollama", "anthropic", or "openrouter").
    /// If None, auto-detect (ollama first, anthropic fallback, then openrouter).
    pub provider: Option<String>,
    /// Force a specific model name. If None, use the configured default.
    pub model: Option<String>,
    /// Initial label for the session (e.g. "auto-triggered").
    pub label: Option<String>,
    /// Per-model token budget override. If set, overrides the global `token_budget`.
    pub token_budget: Option<u64>,
    /// Optional proxy token expiry time. If set, the session will pause
    /// when the token is about to expire (server mode only).
    pub proxy_expires: Option<DateTime<Utc>>,
    /// When false, the session pauses for human approval before remediation.
    /// Mutating tools are not available until approved. Default: true.
    pub auto_approve: bool,
}

impl HealerState {
    pub fn new(
        store: DynStore,
        instance_data: DynInstanceData,
        session_factory: Arc<dyn SessionFactory>,
        connector_config: ConnectorConfig,
    ) -> Self {
        Self {
            inner: Arc::new(HealerStateInner {
                store,
                instance_data,
                session_factory,
                connector_config,
                running: DashMap::new(),
                shutting_down: AtomicBool::new(false),
                push_fn: None,
                extended_budgets: DashMap::new(),
            }),
        }
    }

    /// Get the underlying store (for server-side direct access).
    pub fn store(&self) -> &DynStore {
        &self.inner.store
    }

    /// Get the instance data source.
    pub fn instance_data(&self) -> &DynInstanceData {
        &self.inner.instance_data
    }

    /// Set the push callback for sending SSE events to daemons.
    /// Must be called after construction, before spawning sessions.
    pub fn set_push_fn(&mut self, f: tools::PushFn) {
        Arc::get_mut(&mut self.inner)
            .expect("set_push_fn must be called before cloning HealerState")
            .push_fn = Some(f);
    }

    /// Spawn a new healer session. Returns the session ID immediately.
    pub async fn spawn_session(&self, req: SpawnRequest) -> Result<Uuid> {
        // Guard: no concurrent sessions for the same instance
        if self.inner.store.has_running_session(&req.instance_id).await? {
            anyhow::bail!("a healer session is already running for this instance");
        }

        // Guard: 1-hour cooldown between sessions (skip in dev mode)
        if !req.skip_cooldown && self.inner.store.has_recent_session(&req.instance_id).await? {
            anyhow::bail!(
                "a healer session was created for this instance in the last hour — \
                 please wait before starting another"
            );
        }

        // 1. Build initial issues snapshot
        let initial_issues =
            serde_json::to_value(&req.services_extended).unwrap_or_else(|_| json!([]));

        let state_data = json!({
            "instance_id": req.instance_id,
            "cluster_name": req.cluster_name,
            "hostname": req.hostname,
            "file_tunnels": req.file_tunnels,
            "shell_tunnels": req.shell_tunnels,
            "sample": req.sample,
        });

        // 2. Create session row
        let session_id = self.inner.store.create_session(
            req.cluster_id,
            &req.instance_id,
            &req.created_by,
            &initial_issues,
            &state_data,
            req.provider.as_deref(),
            req.model.as_deref(),
            req.label.as_deref(),
        )
        .await
        .context("failed to create healer session")?;

        // 3. Transition to Initializing
        self.inner.store.transition_state(
            session_id,
            &SessionState::Initializing,
            &state_data,
        )
        .await?;

        // 4. Set up cancellation and event broadcasting
        let cancel = CancellationToken::new();
        let pause_notify = Arc::new(tokio::sync::Notify::new());
        let approval_notify = Arc::new(tokio::sync::Notify::new());
        let (events_tx, _) = broadcast::channel::<HealerEvent>(4096);
        let running_tools = Arc::new(std::sync::Mutex::new(Vec::new()));
        let effective_budget = req.token_budget.unwrap_or(self.inner.connector_config.token_budget);
        let budget_limit = Arc::new(std::sync::atomic::AtomicU64::new(effective_budget));
        self.inner.running.insert(
            session_id,
            RunningSession {
                cancel: cancel.clone(),
                pause_notify: pause_notify.clone(),
                budget_limit: budget_limit.clone(),
                events_tx: events_tx.clone(),
                running_tools: running_tools.clone(),
                approval_notify: approval_notify.clone(),
            },
        );

        // 5. Spawn the agent task
        let state = self.clone();
        let connector_config = self.inner.connector_config.clone();
        tokio::spawn(async move {
            let result = run_agent_session(
                &state,
                session_id,
                req,
                cancel.clone(),
                pause_notify,
                approval_notify,
                budget_limit,
                events_tx.clone(),
                running_tools,
                connector_config,
                None, // no restored history
            )
            .await;

            // Clean up
            state.inner.running.remove(&session_id);

            if let Err(e) = &result {
                tracing::error!(session_id = %session_id, err = %e, "healer session failed");
                let _ = state.inner.store.fail_session(
                    session_id,
                    &e.to_string(),
                    &json!({}),
                )
                .await;
            }

            // Broadcast done event
            let final_state = state.inner.store.get_session(session_id)
                .await
                .ok()
                .flatten()
                .map(|s| s.state.as_str().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let _ = events_tx.send(HealerEvent::Done { state: final_state });
        });

        Ok(session_id)
    }

    /// Resume all interrupted sessions on server startup.
    pub async fn resume_interrupted(&self) -> Result<usize> {
        let sessions = self.inner.store.find_resumable().await?;
        let count = sessions.len();

        for sess in sessions {
            let sid = sess.id;
            tracing::info!(
                session_id = %sid,
                state = sess.state.as_str(),
                instance = %sess.instance_id,
                "resuming interrupted healer session"
            );

            if let Err(e) = self.resume_session_internal(sess).await {
                tracing::error!(session_id = %sid, err = %e, "failed to resume session");
            }
        }

        Ok(count)
    }

    /// Resume a paused session (manual resume via API).
    ///
    /// If the session was paused due to token budget exhaustion, refuses
    /// to resume unless `extend_budget()` was called first (the extended
    /// budget is stored in `self.extended_budgets`).
    pub async fn resume_session(&self, session_id: Uuid) -> Result<()> {
        let sess = self.inner.store.get_session(session_id)
            .await?
            .context("session not found")?;

        if sess.state != SessionState::Paused {
            anyhow::bail!(
                "session {} is in state {:?}, can only resume from Paused",
                session_id,
                sess.state
            );
        }

        // Block resume if paused for budget exhaustion and budget wasn't extended.
        let reason = sess
            .state_data
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if reason == "token_budget_exceeded" {
            // Check if an extend_budget was applied (stored on the running session
            // if one exists, or in the extended_budgets map otherwise).
            let extended = self
                .inner
                .running
                .get(&session_id)
                .is_some_and(|r| r.budget_limit.load(Ordering::Relaxed) > self.inner.connector_config.token_budget)
                || self.inner.extended_budgets.contains_key(&session_id);
            if !extended {
                anyhow::bail!(
                    "session was paused for token budget exhaustion — \
                     use 'More Tokens' to extend the budget before resuming"
                );
            }
        }

        self.resume_session_internal(sess).await
    }

    async fn resume_session_internal(&self, sess: HealerSession) -> Result<()> {
        let session_id = sess.id;

        // Load messages for history restoration
        let messages = self.inner.store.get_messages(session_id).await?;

        // Extract context from state_data
        let instance_id = sess.instance_id.clone();
        let cluster_name = sess.state_data["cluster_name"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let hostname = sess.state_data["hostname"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let file_tunnels = sess.state_data["file_tunnels"].clone();
        let shell_tunnels = sess.state_data["shell_tunnels"].clone();
        let sample = sess.state_data.get("sample").cloned();

        // Build instance access via the session factory
        let access = self.inner.session_factory.build_access(&sess).await
            .context("failed to build instance access for resumed session")?;

        // Transition back to a running state
        let resume_state = match sess
            .state_data
            .get("last_state")
            .and_then(|v| v.as_str())
            .and_then(SessionState::from_str)
        {
            Some(s) if s.is_active() => s,
            _ => SessionState::Diagnosing,
        };
        self.inner.store.transition_state(
            session_id,
            &resume_state,
            &sess.state_data,
        )
        .await?;

        // Set up cancellation and events
        let cancel = CancellationToken::new();
        let pause_notify = Arc::new(tokio::sync::Notify::new());
        let approval_notify = Arc::new(tokio::sync::Notify::new());
        let (events_tx, _) = broadcast::channel::<HealerEvent>(4096);
        let running_tools = Arc::new(std::sync::Mutex::new(Vec::new()));
        // Use extended budget if one was granted, otherwise default
        let extended = self.inner.extended_budgets.remove(&session_id);
        let budget = extended.map(|(_, v)| v).unwrap_or(self.inner.connector_config.token_budget);
        let budget_limit = Arc::new(std::sync::atomic::AtomicU64::new(budget));
        self.inner.running.insert(
            session_id,
            RunningSession {
                cancel: cancel.clone(),
                pause_notify: pause_notify.clone(),
                budget_limit: budget_limit.clone(),
                events_tx: events_tx.clone(),
                running_tools: running_tools.clone(),
                approval_notify: approval_notify.clone(),
            },
        );

        // Build a SpawnRequest from saved state
        let req = SpawnRequest {
            cluster_id: sess.cluster_id,
            instance_id,
            created_by: sess.created_by,
            user_message: None,
            instance_access: access.instance,
            cluster_access: access.cluster,
            metrics_url: access.metrics_url,
            services_extended: serde_json::from_value(sess.initial_issues.clone())
                .unwrap_or_default(),
            sample,
            file_tunnels,
            shell_tunnels,
            cluster_instances: Vec::new(),
            cluster_name,
            hostname,
            skip_cooldown: true, // resuming — cooldown doesn't apply
            provider: sess.provider.clone(),
            model: sess.model.clone(),
            label: sess.label.clone(),
            token_budget: None, // resume uses the budget from the running session state
            proxy_expires: access.proxy_expires,
            // After approval, remediation tools are unlocked. For regular
            // resumes (from Paused), keep the original behavior (auto_approve
            // was true for auto-triggered sessions, determined by current state).
            auto_approve: sess.state == SessionState::Remediating || sess.state == SessionState::Verifying,
        };

        let state = self.clone();
        let connector_config = self.inner.connector_config.clone();
        let restored_history = Some(messages);

        tokio::spawn(async move {
            let result = run_agent_session(
                &state,
                session_id,
                req,
                cancel,
                pause_notify,
                approval_notify,
                budget_limit,
                events_tx.clone(),
                running_tools,
                connector_config,
                restored_history,
            )
            .await;

            state.inner.running.remove(&session_id);

            if let Err(e) = &result {
                tracing::error!(session_id = %session_id, err = %e, "resumed healer session failed");
                let _ = state.inner.store.fail_session(
                    session_id,
                    &e.to_string(),
                    &json!({}),
                )
                .await;
            }

            let final_state = state.inner.store.get_session(session_id)
                .await
                .ok()
                .flatten()
                .map(|s| s.state.as_str().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let _ = events_tx.send(HealerEvent::Done { state: final_state });
        });

        Ok(())
    }

    /// Request a running session to pause immediately.
    /// The agent future is dropped, then the session transitions to Paused.
    pub fn pause_session(&self, session_id: Uuid) -> Result<()> {
        if let Some(entry) = self.inner.running.get(&session_id) {
            entry.pause_notify.notify_one();
            Ok(())
        } else {
            Err(anyhow::anyhow!("session is not running"))
        }
    }

    /// Cancel a running session.
    pub async fn cancel_session(&self, session_id: Uuid) -> Result<()> {
        if let Some(entry) = self.inner.running.get(&session_id) {
            entry.cancel.cancel();
            Ok(())
        } else {
            // Session might not be running — mark it cancelled in DB directly
            self.inner.store.transition_state(
                session_id,
                &SessionState::Cancelled,
                &json!({}),
            )
            .await
        }
    }

    /// Approve remediation for a session awaiting approval.
    /// Transitions to Remediating and resumes the agent with full tools.
    pub async fn approve_session(&self, session_id: Uuid) -> Result<()> {
        let mut sess = self
            .inner
            .store
            .get_session(session_id)
            .await?
            .context("session not found")?;

        if sess.state != SessionState::AwaitingApproval {
            anyhow::bail!(
                "session {} is in state {:?}, can only approve from AwaitingApproval",
                session_id,
                sess.state
            );
        }

        // Transition to Remediating before resuming — the resumed agent will
        // have auto_approve=true so all tools are registered.
        self.inner
            .store
            .transition_state(session_id, &SessionState::Remediating, &json!({"reason": "approved"}))
            .await?;
        sess.state = SessionState::Remediating;

        self.resume_session_internal(sess).await
    }

    /// Increase the token budget for a session to 1 million tokens.
    /// Works for both running and paused sessions.
    pub fn extend_budget(&self, session_id: Uuid) -> Result<()> {
        if let Some(entry) = self.inner.running.get(&session_id) {
            entry
                .budget_limit
                .store(1_000_000, Ordering::Relaxed);
        }
        // Also record for paused sessions that will resume later
        self.inner.extended_budgets.insert(session_id, 1_000_000);
        tracing::info!("extended token budget to 1M for session {session_id}");
        Ok(())
    }

    /// List sessions for a cluster.
    pub async fn list_sessions(&self, cluster_id: Uuid) -> Result<Vec<HealerSession>> {
        self.inner.store.list_sessions(cluster_id).await
    }

    /// Get a session with its messages.
    pub async fn get_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<(HealerSession, Vec<HealerMessage>)>> {
        let Some(sess) = self.inner.store.get_session(session_id).await? else {
            return Ok(None);
        };
        let msgs = self.inner.store.get_messages(session_id).await?;
        Ok(Some((sess, msgs)))
    }

    /// Subscribe to live events for a running session.
    pub fn subscribe(&self, session_id: Uuid) -> Option<broadcast::Receiver<HealerEvent>> {
        self.inner
            .running
            .get(&session_id)
            .map(|entry| entry.events_tx.subscribe())
    }

    /// Get the current running tools snapshot for a session.
    pub fn running_tools(&self, session_id: Uuid) -> Vec<session::RunningTool> {
        self.inner
            .running
            .get(&session_id)
            .map(|entry| entry.running_tools.lock().unwrap().clone())
            .unwrap_or_default()
    }

    /// Graceful shutdown: signal all sessions to stop at the next safe point,
    /// wait for them to checkpoint, with a timeout.
    pub async fn graceful_shutdown(&self, timeout: std::time::Duration) -> usize {
        self.inner.shutting_down.store(true, Ordering::SeqCst);

        let deadline = tokio::time::Instant::now() + timeout;
        let initial_count = self.inner.running.len();

        // Wait for all sessions to drain
        loop {
            if self.inner.running.is_empty() {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                tracing::warn!(
                    remaining = self.inner.running.len(),
                    "healer shutdown timed out, some sessions may not be cleanly checkpointed"
                );
                // Force-cancel remaining sessions
                for entry in self.inner.running.iter() {
                    entry.cancel.cancel();
                }
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }

        initial_count
    }

    /// Check if the system is shutting down.
    pub fn is_shutting_down(&self) -> bool {
        self.inner.shutting_down.load(Ordering::SeqCst)
    }
}

/// Run the agent session. This is the main async task that drives the LLM agent.
async fn run_agent_session(
    state: &HealerState,
    session_id: Uuid,
    req: SpawnRequest,
    cancel: CancellationToken,
    pause_notify: Arc<tokio::sync::Notify>,
    approval_notify: Arc<tokio::sync::Notify>,
    budget_limit: Arc<std::sync::atomic::AtomicU64>,
    events_tx: broadcast::Sender<HealerEvent>,
    running_tools: Arc<std::sync::Mutex<Vec<session::RunningTool>>>,
    connector_config: ConnectorConfig,
    restored_history: Option<Vec<HealerMessage>>,
) -> Result<()> {
    let store = &state.inner.store;

    // 1. Resolve LLM (use forced provider/model if specified in request)
    let llm = connector::resolve_llm(
        &connector_config,
        req.provider.as_deref(),
        req.model.as_deref(),
    )
    .await
    .context("failed to resolve LLM")?;

    // Persist the actual resolved provider/model to the session row so we can
    // resume with the same LLM later and display it in the UI.
    store
        .update_provider_model(session_id, llm.resolved_provider.as_str(), &llm.resolved_model)
        .await
        .ok();

    // 2. Extract tunnel names
    let file_tunnel_names: Vec<String> = req
        .file_tunnels
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let shell_command_names: Vec<String> = req
        .shell_tunnels
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let cluster_instance_prefixes: Vec<String> = req
        .cluster_instances
        .iter()
        .map(|i| i.instance_prefix.clone())
        .collect();

    // 3. Create native swiftide tools (no MCP transport needed)
    let tool_ctx = tools::ToolContext {
        instance: req.instance_access.clone(),
        instance_data: state.inner.instance_data.clone(),
        cluster: req.cluster_access.clone(),
        target_instance: req.instance_id.chars().take(12).collect(),
        cluster_instances: cluster_instance_prefixes,
        file_tunnels: file_tunnel_names.clone(),
        file_tunnels_full: req.file_tunnels.clone(),
        shell_commands: shell_command_names.clone(),
        shell_commands_full: req.shell_tunnels.clone(),
        store: store.clone(),
        session_id,
        cluster_id: req.cluster_id,
        instance_id: req.instance_id.clone(),
        push_fn: state.inner.push_fn.clone(),
        events_tx: events_tx.clone(),
        metrics_url: req.metrics_url.clone(),
        auto_approve: req.auto_approve,
        approval_notify: approval_notify.clone(),
    };

    // In approval mode, the agent starts with diagnosis-only tools (no mutating
    // tools). On resume after approval, auto_approve is true so all tools are
    // registered.
    let diagnosis_only = !req.auto_approve;
    let healer_tools = tools::all_tools(tool_ctx.clone(), diagnosis_only);
    let settings_tools = settings_tools::all_settings_tools(tool_ctx, diagnosis_only);

    // 4. Build system prompt
    let sample_summary = req
        .sample
        .as_ref()
        .map(|s| agent::format_sample_summary(s))
        .unwrap_or_default();

    let resume_context = if restored_history.is_some() {
        Some(
            "This session has been resumed from a previous checkpoint. The conversation history above contains your previous work. Continue from where you left off.",
        )
    } else {
        None
    };

    let system_prompt = agent::build_system_prompt(
        &req.cluster_name,
        &req.cluster_id.to_string(),
        &req.instance_id,
        &req.hostname,
        &req.cluster_instances,
        &req.services_extended,
        &sample_summary,
        &file_tunnel_names,
        &shell_command_names,
        resume_context,
        req.auto_approve,
    );

    // 5. Transition to Diagnosing
    let transition = |new_state: &SessionState, data: &serde_json::Value| {
        let store = store.clone();
        let events_tx = events_tx.clone();
        let data = data.clone();
        let new_state = new_state.clone();
        async move {
            store
                .transition_state(session_id, &new_state, &data)
                .await
                .ok();
            session::emit_state_change(&events_tx, new_state.as_str(), &data);
        }
    };

    transition(&SessionState::Diagnosing, &json!({})).await;

    // 6. Build and run agent
    let store_msg = store.clone();
    let events_tx_msg = events_tx.clone();
    let is_cloud = llm.is_cloud;
    let proxy_expires = req.proxy_expires;

    // Determine initial prompt
    let initial_prompt = if restored_history.is_some() {
        "You have been resumed. Review your previous conversation above and continue your work from where you left off.".to_string()
    } else {
        let msg = req.user_message.unwrap_or_else(|| {
            "Diagnose and fix the detected issues. Start by listing available tools and reading logs.".to_string()
        });
        // Persist the initial user message so it appears in the chat log
        store.append_message(session_id, "user", &msg, None)
            .await
            .ok();
        let _ = events_tx.send(session::HealerEvent::Message {
            role: "user".to_string(),
            content: msg.clone(),
            metadata: None,
            created_at: chrono::Utc::now(),
        });
        msg
    };

    // Build agent with swiftide
    let mut agent = {
        use swiftide::agents;

        let mut builder = agents::Agent::builder();

        // Set LLM provider
        match &llm.provider {
            connector::LlmProvider::Ollama(o) => {
                builder.llm(o);
            }
            connector::LlmProvider::Anthropic(a) => {
                builder.llm(a);
            }
            connector::LlmProvider::OpenRouter(o) => {
                builder.llm(o);
            }
        }

        // Extract role/content from ChatMessage enum for persistence
        /// Extract role/content from ChatMessage. Returns None for ToolOutput
        /// (handled by before_tool/after_tool hooks to avoid duplicates).
        fn extract_role_content(
            msg: &swiftide::chat_completion::ChatMessage,
        ) -> Option<(String, String)> {
            match msg {
                swiftide::chat_completion::ChatMessage::System(s) => {
                    Some(("system".to_string(), s.clone()))
                }
                swiftide::chat_completion::ChatMessage::User(s) => {
                    Some(("user".to_string(), s.clone()))
                }
                swiftide::chat_completion::ChatMessage::Assistant(s, tool_calls) => {
                    let mut content = s.clone().unwrap_or_default();
                    // Include tool call names in the assistant message for visibility
                    if let Some(calls) = tool_calls {
                        if !calls.is_empty() && content.is_empty() {
                            content = calls
                                .iter()
                                .map(|tc| format!("`{}`", tc.name()))
                                .collect::<Vec<_>>()
                                .join(", ");
                        }
                    }
                    Some(("assistant".to_string(), content))
                }
                // Skip ToolOutput — persisted by after_tool hook
                swiftide::chat_completion::ChatMessage::ToolOutput(_, _) => None,
                swiftide::chat_completion::ChatMessage::Summary(s) => {
                    Some(("summary".to_string(), s.clone()))
                }
                swiftide::chat_completion::ChatMessage::UserWithParts(parts) => {
                    let content = parts
                        .iter()
                        .filter_map(|p| {
                            if let swiftide::chat_completion::ChatMessageContentPart::Text { text } = p {
                                Some(text.as_str())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    Some(("user".to_string(), content))
                }
                swiftide::chat_completion::ChatMessage::Reasoning(_) => None,
            }
        }

        for tool in healer_tools {
            builder.add_tool(tool);
        }
        for tool in settings_tools {
            builder.add_tool(tool);
        }

        // Connect to Context7 MCP server if API key is configured.
        // Wrap MCP tools with sanitized names — Anthropic rejects colons
        // but swiftide formats MCP tool names as "server:tool".
        let mut _mcp_toolbox = None;
        if let Some(api_key) = &connector_config.context7_api_key {
            match connect_context7(api_key).await {
                Ok(toolbox) => {
                    tracing::info!("Context7 MCP connected");
                    match toolbox.available_tools().await {
                        Ok(tools) => {
                            for tool in tools {
                                builder.add_tool(RenamedTool::wrap(tool));
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Context7: failed to list tools: {e:#}");
                        }
                    }
                    _mcp_toolbox = Some(toolbox);
                }
                Err(e) => {
                    tracing::warn!("Context7 MCP connection failed: {e:#}");
                }
            }
        }

        let events_tx_before_tool = events_tx.clone();
        let running_tools_before = running_tools.clone();
        let store_after_tool = store.clone();
        let events_tx_after_tool = events_tx.clone();
        let running_tools_after = running_tools.clone();

        builder
            .system_prompt(system_prompt)
            .on_new_message(move |_agent, msg| {
                let store = store_msg.clone();
                let events_tx = events_tx_msg.clone();
                let extracted = extract_role_content(msg);
                Box::pin(async move {
                    let Some((role, content)) = extracted else {
                        return Ok(()); // ToolOutput handled by after_tool
                    };
                    if content.is_empty() {
                        return Ok(());
                    }
                    // Persist message
                    store.append_message(session_id, &role, &content, None)
                        .await
                        .ok();

                    // Broadcast to SSE subscribers
                    let _ = events_tx.send(HealerEvent::Message {
                        role,
                        content,
                        metadata: None,
                        created_at: Utc::now(),
                    });
                    Ok(())
                })
            })
            .before_tool(move |_agent, tool_call| {
                let events_tx = events_tx_before_tool.clone();
                let running_tools = running_tools_before.clone();
                let name = tool_call.name().to_string();
                let args = tool_call.args().map(String::from);
                Box::pin(async move {
                    // Add to in-memory running tools and broadcast snapshot
                    let tool = session::RunningTool {
                        name: name.clone(),
                        args: args.clone(),
                        started_at: Utc::now(),
                    };
                    let snapshot = {
                        let mut tools = running_tools.lock().unwrap();
                        tools.push(tool);
                        tools.clone()
                    };
                    let _ = events_tx.send(HealerEvent::RunningTools { tools: snapshot });
                    Ok(())
                })
            })
            .after_tool(move |_agent, tool_call, result| {
                let store = store_after_tool.clone();
                let events_tx = events_tx_after_tool.clone();
                let running_tools = running_tools_after.clone();
                let name = tool_call.name().to_string();
                let args = tool_call.args().map(String::from);
                let (status, output) = match result {
                    Ok(out) => ("ok", out.to_string()),
                    Err(e) => ("error", e.to_string()),
                };
                Box::pin(async move {
                    // Remove from running tools and broadcast snapshot
                    let snapshot = {
                        let mut tools = running_tools.lock().unwrap();
                        tools.retain(|t| t.name != name);
                        tools.clone()
                    };
                    let _ = events_tx.send(HealerEvent::RunningTools { tools: snapshot });

                    // Persist and broadcast the result
                    let content = format!("{name}: {output}");
                    let metadata = serde_json::json!({
                        "tool_name": name,
                        "tool_args": args.as_deref().unwrap_or("{}"),
                        "status": status,
                    });
                    store
                        .append_message(
                            session_id,
                            "tool_result",
                            &content,
                            Some(&metadata),
                        )
                        .await
                    .ok();
                    let _ = events_tx.send(HealerEvent::Message {
                        role: "tool_result".to_string(),
                        content,
                        metadata: Some(metadata),
                        created_at: Utc::now(),
                    });
                    Ok(())
                })
            })
            .limit(50);

        builder.build().context("failed to build agent")?
    };

    // Race agent execution against control signals.
    // When a signal fires, the agent future is dropped — immediately stopping
    // all LLM calls and tool execution. This replaces the broken
    // before_completion hook approach (swiftide swallows hook errors).

    let store_select = store.clone();
    let events_tx_select = events_tx.clone();

    enum StopReason {
        Completed,
        AgentError(anyhow::Error),
        Cancelled,
        Paused,
        AwaitingApproval,
        Shutdown,
        ProxyExpiring,
        BudgetExceeded { used: u64, limit: u64 },
    }

    let reason = tokio::select! {
        result = agent.query(initial_prompt) => {
            match result {
                Ok(()) => StopReason::Completed,
                Err(e) => StopReason::AgentError(e.into()),
            }
        }
        _ = cancel.cancelled() => StopReason::Cancelled,
        _ = pause_notify.notified() => StopReason::Paused,
        _ = approval_notify.notified() => StopReason::AwaitingApproval,
        _ = shutdown_signal(state) => StopReason::Shutdown,
        _ = proxy_expiry_signal(proxy_expires) => StopReason::ProxyExpiring,
        _ = budget_exceeded_signal(&llm.token_usage, &budget_limit, is_cloud) => {
            let used = llm.token_usage.load(Ordering::Relaxed);
            let limit = budget_limit.load(Ordering::Relaxed);
            StopReason::BudgetExceeded { used, limit }
        }
    };

    match reason {
        StopReason::Completed => {
            let data = json!({"reason": "agent finished"});
            store_select
                .transition_state(session_id, &SessionState::Completed, &data)
                .await?;
            session::emit_state_change(&events_tx_select, "completed", &data);
        }
        StopReason::AgentError(e) => {
            return Err(e.context("agent query failed"));
        }
        StopReason::Cancelled => {
            let data = json!({"reason": "cancelled"});
            store_select
                .transition_state(session_id, &SessionState::Cancelled, &data)
                .await
                .ok();
            session::emit_state_change(&events_tx_select, "cancelled", &data);
        }
        StopReason::Paused => {
            let data = json!({"reason": "manual_pause"});
            store_select
                .transition_state(session_id, &SessionState::Paused, &data)
                .await
                .ok();
            session::emit_state_change(&events_tx_select, "paused", &data);
        }
        StopReason::AwaitingApproval => {
            // State already transitioned by set_phase tool — nothing to do.
        }
        StopReason::Shutdown => {
            let data = json!({"reason": "server_shutdown"});
            store_select
                .transition_state(session_id, &SessionState::AwaitingRetry, &data)
                .await
                .ok();
            session::emit_state_change(&events_tx_select, "awaiting_retry", &data);
        }
        StopReason::ProxyExpiring => {
            let data = json!({"reason": "proxy_token_expiring"});
            store_select
                .transition_state(session_id, &SessionState::AwaitingRetry, &data)
                .await
                .ok();
            session::emit_state_change(&events_tx_select, "awaiting_retry", &data);
        }
        StopReason::BudgetExceeded { used, limit } => {
            let data = json!({
                "reason": "token_budget_exceeded",
                "tokens_used": used,
                "budget_limit": limit,
            });
            store_select
                .transition_state(session_id, &SessionState::Paused, &data)
                .await
                .ok();
            session::emit_state_change(&events_tx_select, "paused", &data);
        }
    }

    Ok(())
}

/// Wrapper that sanitizes tool names for Anthropic compatibility.
/// Replaces colons with hyphens (e.g. "Context7:query-docs" → "context7-query-docs").
#[derive(Clone)]
struct RenamedTool {
    inner: Box<dyn swiftide::chat_completion::Tool>,
    name: String,
    spec: swiftide::chat_completion::ToolSpec,
}

impl RenamedTool {
    fn wrap(tool: Box<dyn swiftide::chat_completion::Tool>) -> Box<dyn swiftide::chat_completion::Tool> {
        let orig_name = tool.name().to_string();
        let sanitized = orig_name
            .replace(':', "-")
            .replace(' ', "_")
            .to_lowercase();
        let mut spec = tool.tool_spec();
        spec.name = sanitized.clone();
        Box::new(Self {
            inner: tool,
            name: sanitized,
            spec,
        })
    }
}

#[async_trait::async_trait]
impl swiftide::chat_completion::Tool for RenamedTool {
    fn name(&self) -> std::borrow::Cow<'_, str> {
        std::borrow::Cow::Borrowed(&self.name)
    }

    fn tool_spec(&self) -> swiftide::chat_completion::ToolSpec {
        self.spec.clone()
    }

    async fn invoke(
        &self,
        agent_context: &dyn swiftide::traits::AgentContext,
        tool_call: &swiftide::chat_completion::ToolCall,
    ) -> Result<swiftide::chat_completion::ToolOutput, swiftide::chat_completion::errors::ToolError> {
        self.inner.invoke(agent_context, tool_call).await
    }
}

/// Wait until the HealerState signals a server shutdown.
async fn shutdown_signal(state: &HealerState) {
    loop {
        if state.is_shutting_down() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

/// Wait until the proxy token is about to expire (30 minutes before).
/// Returns `pending` if no expiry is set.
async fn proxy_expiry_signal(expires: Option<DateTime<Utc>>) {
    let Some(expires) = expires else {
        return std::future::pending().await;
    };
    let deadline = expires - chrono::Duration::minutes(30);
    let now = Utc::now();
    if now >= deadline {
        return;
    }
    let dur = (deadline - now).to_std().unwrap_or_default();
    tokio::time::sleep(dur).await;
}

/// Wait until token usage exceeds the budget. Polls every 5 seconds.
/// Returns `pending` if not a cloud provider or budget is 0.
async fn budget_exceeded_signal(
    token_usage: &std::sync::atomic::AtomicU64,
    budget_limit: &std::sync::atomic::AtomicU64,
    is_cloud: bool,
) {
    if !is_cloud {
        return std::future::pending().await;
    }
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        let budget = budget_limit.load(Ordering::Relaxed);
        if budget == 0 {
            continue;
        }
        let used = token_usage.load(Ordering::Relaxed);
        if used >= budget {
            return;
        }
    }
}

/// Connect to the Context7 documentation MCP server via SSE.
async fn connect_context7(
    api_key: &str,
) -> Result<swiftide::agents::tools::mcp::McpToolbox> {
    let url = format!("https://mcp.context7.com/mcp?api_key={api_key}");
    let transport = rmcp::transport::StreamableHttpClientTransport::<reqwest::Client>::from_uri(url);
    let mut toolbox =
        swiftide::agents::tools::mcp::McpToolbox::try_from_transport(transport)
            .await
            .context("Context7 MCP handshake failed")?;
    toolbox.with_name("Context7");
    Ok(toolbox)
}
