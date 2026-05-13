use serde::{Deserialize, Serialize};

/// The result of a provider selection decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    /// Selected provider name.
    pub provider: String,
    /// Composite score [0.0, 1.0].
    pub score: f32,
    /// Detailed score breakdown for debugging/observability.
    pub reasoning: ScoreBreakdown,
    /// Alternative providers ranked by score (excluding the selected one).
    pub alternatives: Vec<Alternative>,
}

/// Detailed breakdown of how a provider was scored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    /// Quota sub-score (remaining capacity + anticipatory penalty).
    pub quota_score: f32,
    /// Health sub-score.
    pub health_score: f32,
    /// Priority sub-score.
    pub priority_score: f32,
    /// Latency sub-score.
    pub latency_score: f32,
}

/// An alternative provider candidate with its score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alternative {
    /// Provider name.
    pub provider: String,
    /// Composite score.
    pub score: f32,
}
