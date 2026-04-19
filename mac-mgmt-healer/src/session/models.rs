use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealerMessage {
    pub id: Uuid,
    pub session_id: Uuid,
    pub role: String,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

/// Event emitted during a running session, consumed by SSE streams.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HealerEvent {
    Message {
        role: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<serde_json::Value>,
        created_at: DateTime<Utc>,
    },
    /// Snapshot of currently executing tools. Sent on every tool start/end.
    RunningTools {
        tools: Vec<RunningTool>,
    },
    /// Ephemeral status message (not persisted). Shown in UI but cleared on reload.
    Status {
        message: String,
    },
    State {
        state: String,
        state_data: serde_json::Value,
    },
    Done {
        state: String,
    },
}

/// A tool currently being executed by the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunningTool {
    pub name: String,
    pub args: Option<String>,
    pub started_at: DateTime<Utc>,
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
    "other",
];
