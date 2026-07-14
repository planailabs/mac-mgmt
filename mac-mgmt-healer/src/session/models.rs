use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Generic chat types, re-exported under their healer-era names so consumers
// (server, daemon) keep compiling with minimal churn. Wire formats identical.
pub use plan_ai_chat::ChatEvent as HealerEvent;
pub use plan_ai_chat::ChatMessage as HealerMessage;
pub use plan_ai_chat::{RunningTool, RunningToolValidation};

/// Session state machine states.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Created,
    Initializing,
    Diagnosing,
    Remediating,
    Verifying,
    Completed,
    Done,
    Failed,
    Cancelled,
    AwaitingRetry,
    AwaitingApproval,
    Paused,
    NeedsHumanAttention,
}

impl SessionState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Initializing => "initializing",
            Self::Diagnosing => "diagnosing",
            Self::Remediating => "remediating",
            Self::Verifying => "verifying",
            Self::Completed => "completed",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::AwaitingRetry => "awaiting_retry",
            Self::AwaitingApproval => "awaiting_approval",
            Self::Paused => "paused",
            Self::NeedsHumanAttention => "needs_human_attention",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "created" => Some(Self::Created),
            "initializing" => Some(Self::Initializing),
            "diagnosing" => Some(Self::Diagnosing),
            "remediating" => Some(Self::Remediating),
            "verifying" => Some(Self::Verifying),
            "completed" => Some(Self::Completed),
            "done" => Some(Self::Done),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            "awaiting_retry" => Some(Self::AwaitingRetry),
            "awaiting_approval" => Some(Self::AwaitingApproval),
            "paused" => Some(Self::Paused),
            "needs_human_attention" => Some(Self::NeedsHumanAttention),
            _ => None,
        }
    }

    /// Whether this is a terminal state (no further transitions possible).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed
                | Self::Done
                | Self::Failed
                | Self::Cancelled
                | Self::NeedsHumanAttention
        )
    }

    /// Whether this session is actively running (has a spawned agent task).
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Initializing | Self::Diagnosing | Self::Remediating | Self::Verifying
        )
    }

    /// States the AI agent is allowed to transition to.
    pub fn agent_allowed(name: &str) -> Option<Self> {
        match name {
            "diagnosing" => Some(Self::Diagnosing),
            "remediating" => Some(Self::Remediating),
            "verifying" => Some(Self::Verifying),
            "done" => Some(Self::Done),
            "needs_human_attention" => Some(Self::NeedsHumanAttention),
            _ => None,
        }
    }
}

/// State strings that make a session non-resumable. Must match the historical
/// SQL semantics exactly (note: `done` IS non-resumable here even though the
/// partial index from migration 038 does not exclude it).
pub const NON_RESUMABLE_STATES: &[&str] = &[
    "completed",
    "done",
    "failed",
    "cancelled",
    "paused",
    "needs_human_attention",
];

/// State strings that do NOT count as "running" for the concurrent-session
/// guard (paused sessions still block new spawns, matching historical SQL).
pub const INACTIVE_STATES: &[&str] = &[
    "completed",
    "done",
    "failed",
    "cancelled",
    "needs_human_attention",
];

/// [`plan_ai_chat::StateModel`] over the healer state vocabulary.
#[derive(Debug, Clone, Copy, Default)]
pub struct HealerStateModel;

impl plan_ai_chat::StateModel for HealerStateModel {
    fn core(&self, state: &str) -> plan_ai_chat::CoreState {
        use plan_ai_chat::CoreState;
        match SessionState::from_str(state) {
            Some(SessionState::Created) => CoreState::Created,
            Some(SessionState::Initializing) => CoreState::Initializing,
            Some(
                SessionState::Diagnosing | SessionState::Remediating | SessionState::Verifying,
            ) => CoreState::Running,
            Some(SessionState::Completed | SessionState::Done) => CoreState::Completed,
            Some(SessionState::Failed) | None => CoreState::Failed,
            Some(SessionState::Cancelled) => CoreState::Cancelled,
            Some(SessionState::AwaitingRetry) => CoreState::AwaitingRetry,
            Some(SessionState::AwaitingApproval) => CoreState::AwaitingApproval,
            Some(SessionState::Paused) => CoreState::Paused,
            Some(SessionState::NeedsHumanAttention) => CoreState::NeedsAttention,
        }
    }

    fn agent_allowed(&self, name: &str) -> Option<String> {
        SessionState::agent_allowed(name).map(|s| s.as_str().to_string())
    }

    fn initial_running_state(&self) -> &str {
        "diagnosing"
    }

    fn non_resumable_states(&self) -> &[&str] {
        NON_RESUMABLE_STATES
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealerSession {
    pub id: Uuid,
    pub cluster_id: Uuid,
    pub instance_id: String,
    pub state: SessionState,
    pub state_data: serde_json::Value,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub initial_issues: serde_json::Value,
    /// LLM provider used for this session (e.g. "ollama", "anthropic").
    pub provider: Option<String>,
    /// LLM model used for this session (e.g. "gemma4", "claude-sonnet-4-6").
    pub model: Option<String>,
    /// Human-readable label for this session (set by the LLM or auto-trigger).
    pub label: Option<String>,
}

impl From<plan_ai_chat::ChatSession> for HealerSession {
    fn from(s: plan_ai_chat::ChatSession) -> Self {
        Self {
            id: s.id,
            cluster_id: s.scope_id,
            instance_id: s.subject,
            state: SessionState::from_str(&s.state).unwrap_or(SessionState::Failed),
            state_data: s.state_data,
            created_by: s.created_by,
            created_at: s.created_at,
            updated_at: s.updated_at,
            completed_at: s.completed_at,
            error_message: s.error_message,
            initial_issues: s.initial_context,
            provider: s.provider,
            model: s.model,
            label: s.label,
        }
    }
}

impl From<HealerSession> for plan_ai_chat::ChatSession {
    fn from(s: HealerSession) -> Self {
        Self {
            id: s.id,
            scope_id: s.cluster_id,
            subject: s.instance_id,
            state: s.state.as_str().to_string(),
            state_data: s.state_data,
            created_by: s.created_by,
            created_at: s.created_at,
            updated_at: s.updated_at,
            completed_at: s.completed_at,
            error_message: s.error_message,
            initial_context: s.initial_issues,
            provider: s.provider,
            model: s.model,
            label: s.label,
        }
    }
}

/// A staff ping: actionable notification from the healer to admins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffPing {
    pub id: Uuid,
    pub session_id: Uuid,
    pub cluster_id: Uuid,
    pub instance_id: String,
    pub category: String,
    pub message: String,
    pub resolved: bool,
    pub resolved_by: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Staff ping categories.
pub const PING_CATEGORIES: &[&str] = &[
    "hardware",
    "network",
    "disk_space",
    "config_error",
    "service_crash",
    "model_issue",
    "permission",
    "dependency",
    "security",
    "performance",
    "tool_needed",
    "other",
];
