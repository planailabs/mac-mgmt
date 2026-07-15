//! Interactive fleet chatbot — mac-mgmt's instantiation of the reusable
//! [`plan_ai_chat::service`] chat agent: the fleet-specific system prompt
//! plus the mapping from `[chat]` config onto the service knobs. Session
//! lifecycle, tool bridging, validation, and approvals live in the service.

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use plan_ai_chat::ConnectorConfig;
use plan_ai_chat::service::{ChatServiceConfig, SystemPromptBuilder, tool_index};
use plan_ai_api_mcp::Registry;

pub use plan_ai_chat::service::{ChatState, ChatStateModel, ChatUserCtx};

use crate::config::ChatConfig;

/// Build the chat service from the `[chat]` config section.
pub async fn init(
    pool: sqlx::PgPool,
    registry: Arc<Registry<sqlx::PgPool>>,
    connector: ConnectorConfig,
    cfg: &ChatConfig,
) -> Result<ChatState> {
    let service_cfg = ChatServiceConfig {
        token_budget: cfg.token_budget,
        risk_threshold: cfg.risk_threshold.clone(),
        allow_approve_all: cfg.allow_approve_all,
        max_active_sessions_per_user: cfg.max_active_sessions_per_user,
        idle_park_minutes: cfg.idle_park_minutes,
        tools_include: cfg.tools_include.clone(),
        tools_exclude: cfg.tools_exclude.clone(),
        validator_provider: cfg.validator_provider.clone(),
        validator_model: cfg.validator_model.clone(),
    };
    ChatState::new(
        pool,
        registry,
        connector,
        service_cfg,
        Arc::new(FleetPromptBuilder),
    )
    .await
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
    format!(
        "{clusters} clusters; {instances} known instances ({online} online in the last 2 minutes)"
    )
}

struct FleetPromptBuilder;

#[async_trait]
impl SystemPromptBuilder for FleetPromptBuilder {
    async fn build(&self, user: &ChatUserCtx, resumed: bool, state: &ChatState) -> String {
        let fleet = fleet_summary(state.pool()).await;
        let tool_index = tool_index(state.registry());

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

        let threshold = &state.config().risk_threshold;

        format!(
            r#"You are the mac-mgmt fleet assistant: an operations agent for a macOS fleet management platform (clusters of machines running AI workloads, managed by daemons that sync config, skills, MCP servers, and daemon versions from this server).

You are talking to {email}. {access_line}

Fleet snapshot: {fleet}.

## How to work
- Act through your tools; NEVER invent tool names or arguments. Tool names follow <resource>_<action>.
- Every tool call requires a `_reason` argument — one sentence on why you are calling it.
- Read before you write: fetch current state before changing anything.
- When platform behavior is unclear (config semantics, healer, rollouts, MCP servers, ...), read the built-in documentation: `doc_list` shows the topics, `doc_get` returns a page as markdown. Prefer checking the docs over guessing.
- When a TERM is unclear (what is a bundle? a skill channel? a staff ping?), look it up in the glossary: `glossary_search` / `glossary_get`.
- Cluster config changes: BEFORE any `cluster_config_save`, first load the JSON Schema with `cluster_config_schema` and the current document with `cluster_config_get`. Edit minimally and make sure the result validates against the schema — invalid documents are rejected.
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
}
