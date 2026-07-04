//! Run results and live-progress snapshots. Wasm-safe: these cross the wire
//! to the UI and double as the API/MCP output schemas.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Ok,
    Skipped,
    Failed,
}

/// Outcome of one step. For loop steps `inputs`/`output` are arrays with one
/// element per iteration (null for skipped iterations).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StepReport {
    pub name: String,
    pub action: String,
    pub status: StepStatus,
    /// Fully rendered inputs, recorded before dispatch (None when skipped).
    #[serde(default)]
    pub inputs: Option<serde_json::Value>,
    #[serde(default)]
    pub output: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<String>,
}

/// Final result of a run. `steps` ends at the first failed step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RunReport {
    pub ok: bool,
    pub steps: Vec<StepReport>,
    /// Final variable state (inputs + step outputs).
    pub variables: serde_json::Map<String, serde_json::Value>,
}

/// One debug-log line, globally ordered by `seq` within a run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RunLogEvent {
    pub seq: u64,
    pub step_index: u32,
    pub message: String,
}

/// Live snapshot of a run, served to long-polling progress UIs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RunStatus {
    pub run_id: Uuid,
    pub template: String,
    /// 0-based index of the step currently executing (or, when done, the
    /// count of executed steps).
    pub current_step: u32,
    pub total_steps: u32,
    pub current_step_name: String,
    pub done: bool,
    /// Set when done.
    #[serde(default)]
    pub ok: Option<bool>,
    /// Log events with `seq` greater than the poller's `after_seq`.
    pub events: Vec<RunLogEvent>,
    #[serde(default)]
    pub report: Option<RunReport>,
}
