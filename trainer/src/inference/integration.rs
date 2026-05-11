use uuid::Uuid;

/// Contextual information about the current session for inference.
pub struct SessionContext {
    /// Encoded token IDs for the session so far.
    pub tokens: Vec<u32>,
    /// Role IDs corresponding to each token.
    pub roles: Vec<u32>,
    /// Recent tool call history (tool class indices).
    pub tool_history: Vec<usize>,
    /// Current session phase token ID.
    pub phase: u32,
    /// Initial issues JSON text (for embedding).
    pub initial_issues_text: String,
}

/// A predicted outcome distribution.
pub struct OutcomePrediction {
    /// Probability of success (done/completed).
    pub p_success: f32,
    /// Probability of failure.
    pub p_failure: f32,
    /// Probability of needs_human/escalation.
    pub p_escalation: f32,
}

/// Recommends tools based on learned patterns from past sessions.
pub trait ToolRecommender: Send + Sync {
    /// Returns ranked tool recommendations with confidence scores.
    fn recommend_tools(&self, context: &SessionContext) -> Vec<(String, f32)>;
}

/// Estimates session outcome from early signals.
pub trait OutcomeEstimator: Send + Sync {
    /// Predicts session outcome probabilities.
    fn estimate_outcome(&self, context: &SessionContext) -> OutcomePrediction;
}

/// Finds similar past sessions via embedding similarity.
pub trait SessionMatcher: Send + Sync {
    /// Returns (session_id, similarity_score) pairs for the K most similar sessions.
    fn find_similar(&self, context: &SessionContext, k: usize) -> Vec<(Uuid, f32)>;
}
