//! Interactive fleet chatbot.
//!
//! A context-aware chat agent built on `plan-ai-chat`: multi-turn sessions
//! persisted in the generic chat tables, tools bridged from the api-mcp
//! registry (dispatched with the chat user's Principal, so the agent can only
//! do what the user can do), the healer's risk classing + validator-LLM guard,
//! and per-call human approval for mutating/destructive calls.

use std::sync::Arc;

use anyhow::{Context, Result};
use plan_ai_chat::bridge::api_mcp::{ToolFilter, registry_tools};
use plan_ai_chat::session_loop::{InitialPrompt, SessionHandles, SessionSpec};
use plan_ai_chat::store::pg::{PgChatStore, PgTables};
use plan_ai_chat::tools::{ChatToolContext, NameSessionTool, PinConfig, PinTool, SetPhaseTool};
use plan_ai_chat::{
    ApprovalDecision, ApprovalPolicy, ChatSession, ConnectorConfig, CoreState, DynChatStore,
    GuardRole, SessionManager, StateModel, TimeoutAction, ToolRisk, ValidatedTool,
    ValidationConfig,
};
use plan_ai_api_mcp::{Principal, Registry};
use uuid::Uuid;

use crate::config::ChatConfig;

/// Identity of the chat user, resolved fresh from the web session for every
/// spawn/resume (permissions may change between turns).
#[derive(Clone)]
pub struct ChatUserCtx {
    pub email: String,
    pub is_admin: bool,
    pub principal: Principal,
}

/// Shared chatbot state. Clone-friendly (inner Arc).
#[derive(Clone)]
pub struct ChatState {
    inner: Arc<ChatStateInner>,
}

struct ChatStateInner {
    manager: SessionManager,
    registry: Arc<Registry<sqlx::PgPool>>,
    pool: sqlx::PgPool,
    connector: ConnectorConfig,
    cfg: ChatConfig,
}

/// Stable per-user scope id: chat sessions are grouped by owner.
fn scope_for(email: &str) -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_OID, email.as_bytes())
}

/// Agent-drivable phases the chat agent tracks via `set_phase`.
const CHAT_PHASES: &[&str] = &["planning", "executing", "executed"];

/// Chat session state vocabulary: lifecycle states plus the working phases
/// created → planning → executing → executed. ("running" stays mapped for
/// rows created before phases existed.)
#[derive(Debug, Clone, Copy, Default)]
pub struct ChatStateModel;

impl StateModel for ChatStateModel {
    fn core(&self, state: &str) -> CoreState {
        match state {
            "created" => CoreState::Created,
            "initializing" => CoreState::Initializing,
            "planning" | "executing" | "executed" | "running" => CoreState::Running,
            "awaiting_approval" => CoreState::AwaitingApproval,
            "awaiting_retry" => CoreState::AwaitingRetry,
            "paused" => CoreState::Paused,
            "completed" => CoreState::Completed,
            "failed" => CoreState::Failed,
            "cancelled" => CoreState::Cancelled,
            "needs_attention" => CoreState::NeedsAttention,
            _ => CoreState::Failed,
        }
    }

    fn agent_allowed(&self, name: &str) -> Option<String> {
        CHAT_PHASES.contains(&name).then(|| name.to_string())
    }

    fn initial_running_state(&self) -> &str {
        "planning"
    }

    fn non_resumable_states(&self) -> &[&str] {
        &["completed", "failed", "cancelled", "needs_attention"]
    }
}

impl ChatState {
    /// Run the chat-table migrations, build the session manager, and park any
    /// sessions interrupted by the previous shutdown.
    pub async fn new(
        pool: sqlx::PgPool,
        registry: Arc<Registry<sqlx::PgPool>>,
        connector: ConnectorConfig,
        cfg: ChatConfig,
    ) -> Result<Self> {
        plan_ai_chat::store::migrations::run_migrations(&pool)
            .await
            .context("chat migrations failed")?;

        let store: DynChatStore = Arc::new(PgChatStore::new(pool.clone(), PgTables::chat()));
        let manager = SessionManager::new(store, Arc::new(ChatStateModel));

        // Sweep sessions interrupted by a previous shutdown: no auto-respawn,
        // they resume lazily on the next user message.
        let model = ChatStateModel;
        if let Ok(interrupted) = manager
            .store()
            .find_resumable(model.non_resumable_states(), &[])
            .await
        {
            for sess in interrupted {
                let data = serde_json::json!({"reason": "server_restart"});
                let _ = manager
                    .store()
                    .transition_state(sess.id, "paused", false, &data)
                    .await;
            }
        }

        Ok(Self {
            inner: Arc::new(ChatStateInner {
                manager,
                registry,
                pool,
                connector,
                cfg,
            }),
        })
    }

    pub fn manager(&self) -> &SessionManager {
        &self.inner.manager
    }

    pub fn store(&self) -> &DynChatStore {
        self.inner.manager.store()
    }

    /// Sessions owned by this user (newest first).
    pub async fn list_sessions(&self, email: &str) -> Result<Vec<ChatSession>> {
        self.store().list_sessions(scope_for(email)).await
    }

    /// Fetch a session, enforcing ownership (admins may access any).
    pub async fn get_session_checked(
        &self,
        user: &ChatUserCtx,
        session_id: Uuid,
    ) -> Result<ChatSession> {
        let sess = self
            .store()
            .get_session(session_id)
            .await?
            .context("session not found")?;
        if !user.is_admin && sess.subject != user.email {
            anyhow::bail!("access denied");
        }
        Ok(sess)
    }

    /// Start a new chat session with the user's first message.
    pub async fn start_session(
        &self,
        user: &ChatUserCtx,
        provider: Option<String>,
        model: Option<String>,
        first_message: String,
        page_context: Option<String>,
    ) -> Result<Uuid> {
        // Per-user cap on concurrently running agents.
        let running = self
            .list_sessions(&user.email)
            .await?
            .iter()
            .filter(|s| self.inner.manager.is_running(s.id))
            .count() as u32;
        if running >= self.inner.cfg.max_active_sessions_per_user {
            anyhow::bail!(
                "too many active chat sessions ({running}); close or pause one first"
            );
        }

        let state_data = serde_json::json!({
            "owner": user.email,
            "page_context": page_context,
        });
        let session_id = self
            .store()
            .create_session(plan_ai_chat::NewSession {
                scope_id: scope_for(&user.email),
                subject: &user.email,
                created_by: &user.email,
                initial_context: &serde_json::json!({}),
                state_data: &state_data,
                provider: provider.as_deref(),
                model: model.as_deref(),
                label: None,
            })
            .await?;

        if self.inner.cfg.token_budget > 0 {
            self.store()
                .set_token_budget(session_id, self.inner.cfg.token_budget)
                .await
                .ok();
        }

        self.launch(
            user.clone(),
            session_id,
            provider,
            model,
            InitialPrompt::User(prefix_context(page_context.as_deref(), &first_message)),
            false,
            "planning".to_string(),
        )?;
        Ok(session_id)
    }

    /// Deliver a user message: queued into the running session, or the
    /// session is respawned with the message (lazy resume).
    pub async fn send_message(
        &self,
        user: &ChatUserCtx,
        session_id: Uuid,
        text: String,
        page_context: Option<String>,
    ) -> Result<()> {
        let sess = self.get_session_checked(user, session_id).await?;
        let msg = prefix_context(page_context.as_deref(), &text);
        // Fast path: queue into a running agent.
        if self.inner.manager.send_user_message(session_id, msg.clone()).is_ok() {
            return Ok(());
        }
        // Not running: resumable (paused/awaiting_retry/idle-parked "completed")?
        let model = ChatStateModel;
        let resumable = !model.is_terminal(&sess.state) || sess.state == "completed";
        if !resumable {
            anyhow::bail!("session is in state '{}' and cannot be resumed", sess.state);
        }
        // Budget-exhausted sessions need an explicit extend first.
        if sess.state == "paused"
            && sess.state_data.get("reason").and_then(|v| v.as_str())
                == Some("token_budget_exceeded")
        {
            let budget = self.store().get_token_budget(session_id).await.unwrap_or(0);
            let used = self.store().get_token_usage(session_id).await.unwrap_or(0);
            if budget > 0 && used >= budget {
                anyhow::bail!(
                    "session hit its token budget — extend the budget before continuing"
                );
            }
        }
        // Resume into the phase the session was in, if it was working;
        // parked/paused sessions restart their thinking at "planning".
        let resume_state = if model.is_active(&sess.state) {
            sess.state.clone()
        } else {
            "planning".to_string()
        };
        if self
            .launch(
                user.clone(),
                session_id,
                sess.provider.clone(),
                sess.model.clone(),
                InitialPrompt::User(msg.clone()),
                true,
                resume_state,
            )
            .is_err()
        {
            // Lost a race against a concurrent resume — queue instead.
            return self.inner.manager.send_user_message(session_id, msg);
        }
        Ok(())
    }

    /// Resolve a pending per-call approval.
    pub async fn resolve_approval(
        &self,
        user: &ChatUserCtx,
        session_id: Uuid,
        approval_id: Uuid,
        decision: ApprovalDecision,
    ) -> Result<()> {
        self.get_session_checked(user, session_id).await?;
        // Persist the decision for the audit trail before delivering it.
        let meta = serde_json::json!({
            "approval_id": approval_id,
            "decision": decision.as_str(),
            "by": user.email,
        });
        self.store()
            .append_message(
                session_id,
                "approval_decision",
                &format!("{} by {}", decision.as_str(), user.email),
                Some(&meta),
            )
            .await
            .ok();
        self.inner
            .manager
            .resolve_approval(session_id, approval_id, decision, Some(&user.email))
    }

    /// Session-level standing approval grant ("auto-approve"). Runtime-only:
    /// applies to the currently running agent, like ApproveAllForSession.
    pub async fn set_auto_approve(
        &self,
        user: &ChatUserCtx,
        session_id: Uuid,
        value: bool,
    ) -> Result<()> {
        self.get_session_checked(user, session_id).await?;
        self.inner.manager.set_approve_all(session_id, value)
    }

    /// Current auto-approve state (false when the session is not running).
    pub fn auto_approve(&self, session_id: Uuid) -> bool {
        self.inner.manager.approve_all(session_id).unwrap_or(false)
    }

    pub async fn pause(&self, user: &ChatUserCtx, session_id: Uuid) -> Result<()> {
        self.get_session_checked(user, session_id).await?;
        self.inner.manager.pause_session(session_id)
    }

    pub async fn cancel(&self, user: &ChatUserCtx, session_id: Uuid) -> Result<()> {
        self.get_session_checked(user, session_id).await?;
        self.inner.manager.cancel_session(session_id).await
    }

    /// Extend the session's token budget to 1M tokens.
    pub async fn extend_budget(&self, user: &ChatUserCtx, session_id: Uuid) -> Result<()> {
        self.get_session_checked(user, session_id).await?;
        self.store().set_token_budget(session_id, 1_000_000).await
    }

    pub async fn graceful_shutdown(&self, timeout: std::time::Duration) -> usize {
        self.inner.manager.graceful_shutdown(timeout).await
    }

    /// Build the spec (LLM, tools, prompt) in a spawned task and hand the
    /// session to the generic manager. Mirrors the healer's launch shape.
    fn launch(
        &self,
        user: ChatUserCtx,
        session_id: Uuid,
        provider: Option<String>,
        model: Option<String>,
        initial: InitialPrompt,
        resumed: bool,
        start_state: String,
    ) -> Result<()> {
        let handles = self.inner.manager.register(session_id)?;
        let state = self.clone();
        tokio::spawn(async move {
            match build_session_spec(
                &state, &user, session_id, provider, model, &handles, initial, resumed,
                start_state,
            )
            .await
            {
                Ok(spec) => state.inner.manager.run_detached(spec),
                Err(e) => {
                    tracing::error!(%session_id, err = %e, "chat session failed to start");
                    state.inner.manager.unregister(session_id);
                    let _ = state
                        .inner
                        .manager
                        .store()
                        .fail_session(session_id, &e.to_string(), &serde_json::json!({}))
                        .await;
                    let _ = handles.events_tx.send(plan_ai_chat::ChatEvent::Done {
                        state: "failed".to_string(),
                    });
                }
            }
        });
        Ok(())
    }
}

/// Prepend the UI's current-page context to a user turn.
fn prefix_context(page_context: Option<&str>, text: &str) -> String {
    match page_context {
        Some(ctx) if !ctx.is_empty() => format!("[context: user is viewing {ctx}]\n\n{text}"),
        _ => text.to_string(),
    }
}

fn approval_policy(cfg: &ChatConfig) -> Option<ApprovalPolicy> {
    let threshold = match cfg.risk_threshold.as_str() {
        "never" => return None,
        "destructive" => ToolRisk::Destructive,
        _ => ToolRisk::Mutating,
    };
    Some(ApprovalPolicy {
        gate_threshold: threshold,
        guard_role: GuardRole::Advises,
        timeout: std::time::Duration::from_secs(15 * 60),
        on_timeout: TimeoutAction::Deny,
        allow_approve_all: cfg.allow_approve_all,
    })
}

#[allow(clippy::too_many_arguments)]
async fn build_session_spec(
    state: &ChatState,
    user: &ChatUserCtx,
    session_id: Uuid,
    provider: Option<String>,
    model: Option<String>,
    handles: &SessionHandles,
    initial: InitialPrompt,
    resumed: bool,
    start_state: String,
) -> Result<SessionSpec> {
    let inner = &state.inner;
    let store = inner.manager.store().clone();

    let mut connector = inner.connector.clone();
    connector.validator_provider = inner.cfg.validator_provider.clone();
    connector.validator_model = inner.cfg.validator_model.clone();

    let token_ctx = plan_ai_chat::TokenEventContext {
        store: store.clone(),
        session_id,
        budget_notify: handles.budget_notify.clone(),
    };
    let llm = plan_ai_chat::connector::resolve_llm(
        &connector,
        provider.as_deref(),
        model.as_deref(),
        Some(token_ctx),
    )
    .await
    .context("failed to resolve LLM")?;

    // Validation config: static checks + optional guard LLM + human approval.
    let validation_history = plan_ai_chat::validation::ToolCallHistory::default();
    let validator_token_ctx = connector.validator_provider.as_ref().map(|_| {
        plan_ai_chat::TokenEventContext {
            store: store.clone(),
            session_id,
            budget_notify: handles.budget_notify.clone(),
        }
    });
    let validator_llm =
        plan_ai_chat::validation::build_validator_llm(&connector, validator_token_ctx).await;
    let approval = approval_policy(&inner.cfg).map(|policy| plan_ai_chat::ApprovalHook {
        policy,
        broker: handles.approval_broker.clone(),
        store: store.clone(),
        session_id,
    });
    let validation_config = ValidationConfig {
        validator_llm,
        enabled: true,
        running_tools: handles.running_tools.clone(),
        events_tx: handles.events_tx.clone(),
        approval,
    };

    // Tools: every api-mcp registry endpoint the filter allows, dispatched
    // with THIS user's principal — server-side authorization applies per call.
    let filter = ToolFilter {
        include: inner.cfg.tools_include.clone(),
        exclude: inner.cfg.tools_exclude.clone(),
    };
    let principal = Arc::new(user.principal.clone());
    let mut tools = registry_tools(
        inner.registry.clone(),
        inner.pool.clone(),
        principal,
        &filter,
        16 * 1024,
    );

    // Session-local tools: pinning + naming.
    let tool_ctx = ChatToolContext {
        store: store.clone(),
        session_id,
        events_tx: handles.events_tx.clone(),
        approval_notify: handles.approval_notify.clone(),
    };
    tools.push(PinTool::new_with_risk(tool_ctx.clone(), PinConfig::chat()));
    tools.push(NameSessionTool::new_with_risk(tool_ctx.clone()));
    // Phase tracking. Session-local risk: phase changes are bookkeeping and
    // must not trip the guard or the approval gate.
    let (set_phase, _) = SetPhaseTool::new_with_risk(
        tool_ctx,
        Arc::new(ChatStateModel),
        /* auto_approve */ true,
        /* approval_gated_phase */ None,
        CHAT_PHASES,
        "Track your progress. Phases: planning (deciding what to do),          executing (running tools / making changes), executed (the current          request is done). Call this when you move between phases.",
    );
    tools.push((set_phase, ToolRisk::SessionLocal));

    let wrapped = ValidatedTool::wrap_all(tools, validation_config, validation_history.clone());

    let system_prompt = build_system_prompt(state, user, resumed).await;

    Ok(SessionSpec {
        session_id,
        system_prompt,
        initial_prompt: initial,
        tools: wrapped,
        llm,
        start_state,
        interactive: true,
        idle_timeout: Some(std::time::Duration::from_secs(
            inner.cfg.idle_park_minutes.max(1) * 60,
        )),
        deadline: None,
        deadline_reason: String::new(),
        loop_limit: 50,
        validation_history,
    })
}

/// Compact fleet snapshot for the system prompt (best-effort).
async fn fleet_summary(pool: &sqlx::PgPool) -> String {
    let clusters: i64 = sqlx::query_scalar("SELECT count(*) FROM clusters")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let (instances, online): (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE reported_at > now() - interval '2 minutes') \
         FROM daemon_heartbeats",
    )
    .fetch_one(pool)
    .await
    .unwrap_or((0, 0));
    format!("{clusters} clusters; {instances} known instances ({online} online in the last 2 minutes)")
}

async fn build_system_prompt(state: &ChatState, user: &ChatUserCtx, resumed: bool) -> String {
    let inner = &state.inner;
    let fleet = fleet_summary(&inner.pool).await;

    // One line per resource group — full specs are already in the tool list.
    let mut by_resource: std::collections::BTreeMap<&str, Vec<String>> = Default::default();
    for ep in inner.registry.endpoints() {
        by_resource.entry(ep.resource).or_default().push(ep.tool_name());
    }
    let tool_index = by_resource
        .iter()
        .map(|(res, tools)| format!("- {res}: {}", tools.join(", ")))
        .collect::<Vec<_>>()
        .join("\n");

    let access_line = if user.is_admin {
        "They are a GLOBAL ADMIN with full access.".to_string()
    } else {
        format!(
            "They are NOT a global admin; access is scoped to their organizations. \
             Authorization is enforced server-side on every tool call — a call failing \
             with 'access denied'/'forbidden' means {} lacks access; report it, do not retry.",
            user.email
        )
    };

    let resume_line = if resumed {
        "\nThis session was resumed. The conversation history above contains your previous work — continue from where you left off.\n"
    } else {
        ""
    };

    let threshold = &inner.cfg.risk_threshold;

    format!(
        r#"You are the mac-mgmt fleet assistant: an operations agent for a macOS fleet management platform (clusters of machines running AI workloads, managed by daemons that sync config, skills, MCP servers, and daemon versions from this server).

You are talking to {email}. {access_line}

Fleet snapshot: {fleet}.

## How to work
- Act through your tools; NEVER invent tool names or arguments. Tool names follow <resource>_<action>.
- Every tool call requires a `_reason` argument — one sentence on why you are calling it.
- Read before you write: fetch current state before changing anything.
- Tool calls at or above the "{threshold}" risk level pause and wait for the user to approve them in the chat UI. Explain WHAT you are about to change and WHY before making such calls, so the approval prompt makes sense.
- Track your progress with `set_phase`: planning (deciding what to do), executing (doing it), executed (current request done).
- Pin durable findings with the `pin` tool (slots: notes, plan, summary) so they stay visible in long conversations.
- Name the session early with `name_session` once you understand the topic.
- Answer in the user's language; be concise and concrete. Render lists/tables in markdown.
{resume_line}
## Available tool groups
{tool_index}
"#,
        email = user.email,
    )
}
