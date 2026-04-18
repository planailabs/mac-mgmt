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
    Failed,
    Cancelled,
    AwaitingRetry,
    Paused,
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
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::AwaitingRetry => "awaiting_retry",
            Self::Paused => "paused",
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
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            "awaiting_retry" => Some(Self::AwaitingRetry),
            "paused" => Some(Self::Paused),
            _ => None,
        }
    }

    /// Whether this is a terminal state (no further transitions possible).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// Whether this session is actively running (has a spawned agent task).
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Initializing | Self::Diagnosing | Self::Remediating | Self::Verifying
        )
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
    State {
        state: String,
        state_data: serde_json::Value,
    },
    Done {
        state: String,
    },
}
