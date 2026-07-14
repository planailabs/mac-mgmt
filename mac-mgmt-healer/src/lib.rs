pub mod agent;
pub mod connector;
pub mod instance_access;
pub mod instance_data;
pub mod relay_client;
pub mod session;
pub mod settings_tools;
pub mod store;
pub mod tools;
pub mod validation;

use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::json;
use tokio::sync::broadcast;
use uuid::Uuid;

pub use connector::ConnectorConfig;
pub use instance_access::{
    ClusterAccess, DynClusterAccess, DynInstanceAccess, FileReadResult, InstanceAccess, ShellOutput,
};
pub use instance_data::{DynInstanceData, InstanceDataSource};
pub use plan_ai_chat::{
    ApprovalDecision, DynChatStore, PendingApproval, SessionManager,
};
pub use session::models::{
    HealerEvent, HealerMessage, HealerSession, HealerStateModel, SessionState,
};
pub use store::{DynStore, HealerStore};

use agent::InstanceInfo;
use mac_mgmt_common::ServiceExtState;
use plan_ai_chat::session_loop::{InitialPrompt, SessionHandles, SessionSpec};
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
///
/// A thin domain wrapper over the generic [`plan_ai_chat::SessionManager`]:
/// spawn/resume guards, instance access construction, tool assembly, and the
/// healer system prompt live here; the agent loop, control signals, approvals
/// and persistence hooks live in the generic crate.
#[derive(Clone)]
pub struct HealerState {
    inner: Arc<HealerStateInner>,
}

struct HealerStateInner {
    store: DynStore,
    manager: SessionManager,
    instance_data: DynInstanceData,
    session_factory: Arc<dyn SessionFactory>,
    connector_config: ConnectorConfig,
    push_fn: Option<tools::PushFn>,
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
    /// Host-level failure signals that triggered / accompany this session.
    pub failure_signals: Vec<mac_mgmt_common::FailureSignal>,
    pub sample: Option<serde_json::Value>,
    pub file_tunnels: serde_json::Value,
    pub shell_tunnels: serde_json::Value,
    pub cluster_instances: Vec<InstanceInfo>,
    pub cluster_name: String,
    pub hostname: String,
    /// Skip the 1-hour cooldown (set when DEV_ONLY_NO_AUTH=1).
    pub skip_cooldown: bool,
    /// Force a specific LLM provider ("ollama", "anthropic", "openrouter",
    /// or the name of a configured OpenAI-compatible source).
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
    /// Optional provider override for the remediation phase (fix-model).
    pub fix_provider: Option<String>,
    /// Optional model override for the remediation phase (fix-model).
    pub fix_model: Option<String>,
    /// Validator LLM provider for tool-call validation. If None, static checks only.
    pub validator_provider: Option<String>,
    /// Validator LLM model name.
    pub validator_model: Option<String>,
    /// Optional ML model hints (tool recommendations, outcome prediction, similar sessions).
    pub ml_hints: Option<agent::MlHints>,
}

impl HealerState {
    pub fn new(
        store: DynStore,
        chat_store: DynChatStore,
        instance_data: DynInstanceData,
        session_factory: Arc<dyn SessionFactory>,
        connector_config: ConnectorConfig,
    ) -> Self {
        let manager = SessionManager::new(chat_store, Arc::new(HealerStateModel));
        Self {
            inner: Arc::new(HealerStateInner {
                store,
                manager,
                instance_data,
                session_factory,
                connector_config,
                push_fn: None,
            }),
        }
    }

    /// Get the underlying store (for server-side direct access).
    pub fn store(&self) -> &DynStore {
        &self.inner.store
    }

    /// The generic session manager driving the agent loops.
    pub fn manager(&self) -> &SessionManager {
        &self.inner.manager
    }

    /// Get the instance data source.
    pub fn instance_data(&self) -> &DynInstanceData {
        &self.inner.instance_data
    }

    /// Get the session factory (for building instance access).
    pub fn session_factory(&self) -> &dyn SessionFactory {
        &*self.inner.session_factory
    }

    /// Get the push function (for sending SSE events to daemons).
    pub fn push_fn(&self) -> Option<&tools::PushFn> {
        self.inner.push_fn.as_ref()
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
        if self
            .inner
            .store
            .has_running_session(&req.instance_id)
            .await?
        {
            anyhow::bail!("a healer session is already running for this instance");
        }

        // Guard: 1-hour cooldown between sessions (skip in dev mode)
        if !req.skip_cooldown
            && self
                .inner
                .store
                .has_recent_session(&req.instance_id)
                .await?
        {
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
            "failure_signals": req.failure_signals,
            "auto_approve": req.auto_approve,
            "fix_provider": req.fix_provider,
            "fix_model": req.fix_model,
        });

        // 2. Create session row
        let session_id = self
            .inner
            .store
            .create_session(
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
        self.inner
            .store
            .transition_state(session_id, &SessionState::Initializing, &state_data)
            .await?;

        // Set initial token budget on the session row. All cloud providers
        // (including OpenAI-compatible sources) report usage via
        // on_usage_async, so budgets are enforceable everywhere.
        let effective_budget = req
            .token_budget
            .unwrap_or(self.inner.connector_config.token_budget);
        if effective_budget > 0 {
            self.inner
                .store
                .set_token_budget(session_id, effective_budget)
                .await
                .ok();
        }

        // 4. Register control handles + spawn the build/run task.
        self.launch(session_id, req, /* resumed */ false, "diagnosing".into())?;

        Ok(session_id)
    }

    /// Build the session spec (LLM resolution, tools, prompt) and hand off to
    /// the generic manager. Heavy work (Ollama probing, metrics fetch) runs in
    /// a spawned task, matching the old behavior of failing asynchronously.
    /// Fails if the session already has a running agent.
    fn launch(
        &self,
        session_id: Uuid,
        req: SpawnRequest,
        resumed: bool,
        start_state: String,
    ) -> Result<()> {
        let handles = self.inner.manager.register(session_id)?;
        let state = self.clone();

        let mut connector_config = self.inner.connector_config.clone();
        // Override validator provider/model from the spawn request (UI picker).
        if req.validator_provider.is_some() {
            connector_config.validator_provider = req.validator_provider.clone();
            connector_config.validator_model = req.validator_model.clone();
        }

        tokio::spawn(async move {
            match build_session_spec(
                &state,
                session_id,
                req,
                &handles,
                connector_config,
                resumed,
                start_state,
            )
            .await
            {
                Ok(spec) => state.inner.manager.run_detached(spec),
                Err(e) => {
                    tracing::error!(session_id = %session_id, err = %e, "healer session failed to start");
                    state.inner.manager.unregister(session_id);
                    let _ = state
                        .inner
                        .store
                        .fail_session(session_id, &e.to_string(), &json!({}))
                        .await;
                    let _ = handles.events_tx.send(HealerEvent::Done {
                        state: "failed".to_string(),
                    });
                }
            }
        });
        Ok(())
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
    /// to resume unless `extend_budget()` was called first.
    pub async fn resume_session(&self, session_id: Uuid) -> Result<()> {
        let sess = self
            .inner
            .store
            .get_session(session_id)
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
            let budget = self
                .inner
                .store
                .get_token_budget(session_id)
                .await
                .unwrap_or(0);
            let used = self
                .inner
                .store
                .get_token_usage(session_id)
                .await
                .unwrap_or(0);
            if budget > 0 && used >= budget {
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
        let access = self
            .inner
            .session_factory
            .build_access(&sess)
            .await
            .context("failed to build instance access for resumed session")?;

        // Resume into the last active state (post-approval sessions resume
        // straight into Remediating), else restart diagnosis.
        let resume_state = match sess
            .state_data
            .get("last_state")
            .and_then(|v| v.as_str())
            .and_then(SessionState::from_str)
        {
            Some(s) if s.is_active() => s,
            _ if sess.state.is_active() => sess.state.clone(),
            _ => SessionState::Diagnosing,
        };

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
            failure_signals: sess
                .state_data
                .get("failure_signals")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
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
            auto_approve: sess
                .state_data
                .get("auto_approve")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            fix_provider: None,
            fix_model: None,
            validator_provider: None,
            validator_model: None,
            ml_hints: None,
        };

        self.launch(
            session_id,
            req,
            /* resumed */ true,
            resume_state.as_str().to_string(),
        )
    }

    /// Request a running session to pause immediately.
    /// The agent future is dropped, then the session transitions to Paused.
    pub fn pause_session(&self, session_id: Uuid) -> Result<()> {
        self.inner.manager.pause_session(session_id)
    }

    /// Cancel a running session.
    pub async fn cancel_session(&self, session_id: Uuid) -> Result<()> {
        self.inner.manager.cancel_session(session_id).await
    }

    /// Approve remediation for a session awaiting approval.
    /// Transitions to Remediating and resumes the agent with full tools.
    /// If a fix-model was configured, the resumed session uses it.
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

        // If a fix-model was stored in state_data, override the session's
        // provider/model so the resumed agent uses the fix-model for remediation.
        let fix_provider = sess
            .state_data
            .get("fix_provider")
            .and_then(|v| v.as_str())
            .map(String::from);
        let fix_model = sess
            .state_data
            .get("fix_model")
            .and_then(|v| v.as_str())
            .map(String::from);
        if fix_provider.is_some() || fix_model.is_some() {
            if let Some(p) = &fix_provider {
                sess.provider = Some(p.clone());
            }
            if let Some(m) = &fix_model {
                sess.model = Some(m.clone());
            }
            tracing::info!(
                session_id = %session_id,
                fix_provider = ?fix_provider,
                fix_model = ?fix_model,
                "using fix-model for remediation phase"
            );
        }

        // Merge the approval into the existing state_data (instead of
        // replacing it) so the resumed session keeps its tunnels/context AND
        // registers the full tool set: the pre-approval state_data carried
        // auto_approve=false, which previously leaked into the resumed
        // session and re-gated remediation.
        let mut data = sess.state_data.clone();
        if let Some(obj) = data.as_object_mut() {
            obj.insert("auto_approve".to_string(), json!(true));
            obj.insert("reason".to_string(), json!("approved"));
        }

        self.inner
            .store
            .transition_state(session_id, &SessionState::Remediating, &data)
            .await?;
        sess.state = SessionState::Remediating;
        sess.state_data = data;

        self.resume_session_internal(sess).await
    }

    /// Increase the token budget for a session to 1 million tokens.
    /// Works for both running and paused sessions. Updates the DB directly.
    pub async fn extend_budget(&self, session_id: Uuid) -> Result<()> {
        self.inner
            .store
            .set_token_budget(session_id, 1_000_000)
            .await?;
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
        self.inner.manager.subscribe(session_id)
    }

    /// Get the current running tools snapshot for a session.
    pub fn running_tools(&self, session_id: Uuid) -> Vec<session::RunningTool> {
        self.inner.manager.running_tools(session_id)
    }

    /// Deliver a user message to a running session (queued while the agent is
    /// mid-turn; interactive sessions pick it up at the next idle point).
    pub fn send_user_message(&self, session_id: Uuid, text: String) -> Result<()> {
        self.inner.manager.send_user_message(session_id, text)
    }

    /// Pending per-call human approvals for a running session.
    pub fn pending_approvals(&self, session_id: Uuid) -> Vec<PendingApproval> {
        self.inner.manager.pending_approvals(session_id)
    }

    /// Resolve a pending per-call approval with a human decision.
    pub fn resolve_approval(
        &self,
        session_id: Uuid,
        approval_id: Uuid,
        decision: ApprovalDecision,
        by: Option<&str>,
    ) -> Result<()> {
        self.inner
            .manager
            .resolve_approval(session_id, approval_id, decision, by)
    }

    /// Graceful shutdown: signal all sessions to stop at the next safe point,
    /// wait for them to checkpoint, with a timeout.
    pub async fn graceful_shutdown(&self, timeout: std::time::Duration) -> usize {
        self.inner.manager.graceful_shutdown(timeout).await
    }

    /// Check if the system is shutting down.
    pub fn is_shutting_down(&self) -> bool {
        self.inner.manager.is_shutting_down()
    }
}

/// Assemble the [`SessionSpec`] for a healer session: resolve the LLM, build
/// the tool set (with validation wrappers), and render the system prompt.
async fn build_session_spec(
    state: &HealerState,
    session_id: Uuid,
    req: SpawnRequest,
    handles: &SessionHandles,
    connector_config: ConnectorConfig,
    resumed: bool,
    start_state: String,
) -> Result<SessionSpec> {
    let store = &state.inner.store;
    let chat_store = state.inner.manager.store().clone();

    // 1. Resolve LLM (use forced provider/model if specified in request)
    let token_ctx = connector::TokenEventContext {
        store: chat_store.clone(),
        session_id,
        budget_notify: handles.budget_notify.clone(),
    };
    let llm = connector::resolve_llm(
        &connector_config,
        req.provider.as_deref(),
        req.model.as_deref(),
        Some(token_ctx),
    )
    .await
    .context("failed to resolve LLM")?;

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
        events_tx: handles.events_tx.clone(),
        metrics_url: req.metrics_url.clone(),
        auto_approve: req.auto_approve,
        approval_notify: handles.approval_notify.clone(),
    };

    // In approval mode, the agent starts with diagnosis-only tools (no mutating
    // tools). On resume after approval, auto_approve is true so all tools are
    // registered.
    let diagnosis_only = !req.auto_approve;

    // Set up validation layer
    let validation_history = validation::ToolCallHistory::default();
    let validator_token_ctx = if connector_config.validator_provider.is_some() {
        Some(connector::TokenEventContext {
            store: chat_store.clone(),
            session_id,
            budget_notify: handles.budget_notify.clone(),
        })
    } else {
        None
    };
    let validator_llm =
        validation::build_validator_llm(&connector_config, validator_token_ctx).await;
    let validation_config = validation::ValidationConfig {
        validator_llm,
        enabled: true,
        running_tools: handles.running_tools.clone(),
        events_tx: handles.events_tx.clone(),
        // The healer keeps its phase-level approval gate (set_phase →
        // AwaitingApproval); per-call human approval stays disabled here.
        approval: None,
    };

    let mut all_tools = validation::ValidatedTool::wrap_all(
        tools::all_tools_with_risk(tool_ctx.clone(), diagnosis_only),
        validation_config.clone(),
        validation_history.clone(),
    );
    all_tools.extend(validation::ValidatedTool::wrap_all(
        settings_tools::all_settings_tools_with_risk(tool_ctx, diagnosis_only),
        validation_config.clone(),
        validation_history.clone(),
    ));

    // Connect to Context7 MCP server if API key is configured.
    // Wrap MCP tools with sanitized names — Anthropic rejects colons
    // but swiftide formats MCP tool names as "server:tool".
    if let Some(api_key) = &connector_config.context7_api_key {
        match plan_ai_chat::tools::connect_context7(api_key).await {
            Ok(toolbox) => {
                tracing::info!("Context7 MCP connected");
                match toolbox.available_tools().await {
                    Ok(tools) => {
                        for tool in tools {
                            all_tools.push(validation::ValidatedTool::wrap(
                                plan_ai_chat::tools::RenamedTool::wrap(tool),
                                tools::ToolRisk::ReadOnly,
                                validation_config.clone(),
                                validation_history.clone(),
                            ));
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Context7: failed to list tools: {e:#}");
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Context7 MCP connection failed: {e:#}");
            }
        }
    }

    // 4. Build system prompt
    let sample_summary = req
        .sample
        .as_ref()
        .map(|s| agent::format_sample_summary(s))
        .unwrap_or_default();

    let resume_context = if resumed {
        Some(
            "This session has been resumed from a previous checkpoint. The conversation history above contains your previous work. Continue from where you left off.",
        )
    } else {
        None
    };

    // Fetch a metrics snapshot for the system prompt (best-effort).
    let metrics_summary = if let Some(url) = &req.metrics_url {
        match reqwest::Client::new()
            .get(url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                let body = resp.text().await.unwrap_or_default();
                // Keep only our own metrics, skip comments, truncate to ~10KB.
                let filtered: String = body
                    .lines()
                    .filter(|l| l.starts_with("mac_mgmt_"))
                    .collect::<Vec<_>>()
                    .join("\n");
                if filtered.len() > 10_000 {
                    filtered[..10_000].to_string()
                } else {
                    filtered
                }
            }
            _ => String::new(),
        }
    } else {
        String::new()
    };

    let system_prompt = agent::build_system_prompt(
        &req.cluster_name,
        &req.cluster_id.to_string(),
        &req.instance_id,
        &req.hostname,
        &req.cluster_instances,
        &req.services_extended,
        &req.failure_signals,
        &sample_summary,
        &file_tunnel_names,
        &shell_command_names,
        resume_context,
        req.auto_approve,
        diagnosis_only,
        &metrics_summary,
        req.ml_hints.as_ref(),
    );

    // 5. Determine initial prompt
    let initial_prompt = if resumed {
        InitialPrompt::Resume(
            "You have been resumed. Review your previous conversation above and continue your work from where you left off.".to_string(),
        )
    } else {
        InitialPrompt::User(req.user_message.unwrap_or_else(|| {
            "Diagnose and fix the detected issues. Start by listing available tools and reading logs.".to_string()
        }))
    };

    // The session parks 30 minutes before the proxy token expires.
    let deadline = req
        .proxy_expires
        .map(|expires| expires - chrono::Duration::minutes(30));

    Ok(SessionSpec {
        session_id,
        system_prompt,
        initial_prompt,
        tools: all_tools,
        llm,
        start_state,
        interactive: false,
        idle_timeout: None,
        deadline,
        deadline_reason: "proxy_token_expiring".to_string(),
        loop_limit: 50,
        validation_history,
    })
}
