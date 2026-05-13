use serde::{Deserialize, Serialize};

/// Weights for the composite scoring algorithm.
///
/// All weights should sum to 1.0 for normalized scoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringWeights {
    /// Weight for remaining quota percentage (anticipatory).
    pub quota: f32,
    /// Weight for provider health score.
    pub health: f32,
    /// Weight for capability-level priority.
    pub priority: f32,
    /// Weight for latency score.
    pub latency: f32,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            quota: 0.40,
            health: 0.35,
            priority: 0.20,
            latency: 0.05,
        }
    }
}

/// Trait for pluggable scoring strategies.
///
/// Implement this to customize how providers are ranked.
pub trait ScoringStrategy: Send + Sync {
    /// Score a provider given its context. Returns a value in [0.0, 1.0].
    fn score(&self, ctx: &ProviderScoreContext) -> f32;
}

/// Context provided to the scoring strategy for a single provider.
#[derive(Debug, Clone)]
pub struct ProviderScoreContext {
    /// Remaining quota as a ratio [0.0, 1.0]. 1.0 = fully available.
    pub quota_remaining_ratio: f64,
    /// Predicted seconds until quota exhaustion.
    pub predicted_exhaustion_secs: f64,
    /// Burn rate in units per second.
    pub burn_rate: f64,
    /// Health score [0.0, 1.0].
    pub health_score: f32,
    /// Capability-level priority for this provider (higher = preferred).
    pub priority: u16,
    /// Maximum priority across all providers for this capability.
    pub max_priority: u16,
    /// EWMA latency in milliseconds.
    pub latency_ms: f64,
    /// Maximum observed latency across candidates (for normalization).
    pub max_latency_ms: f64,
}

/// Default weighted composite scorer.
pub(crate) struct WeightedScorer {
    pub(crate) weights: ScoringWeights,
}

impl WeightedScorer {
    pub(crate) fn new(weights: ScoringWeights) -> Self {
        Self { weights }
    }

    /// Compute the quota sub-score with anticipatory exhaustion prediction.
    fn quota_score(ctx: &ProviderScoreContext) -> f32 {
        let base = ctx.quota_remaining_ratio as f32;

        // Anticipatory penalty: if exhaustion is predicted soon, reduce score aggressively
        let exhaustion_penalty = if ctx.predicted_exhaustion_secs < 10.0 {
            0.8 // Severe penalty — exhaustion imminent
        } else if ctx.predicted_exhaustion_secs < 30.0 {
            0.5
        } else if ctx.predicted_exhaustion_secs < 60.0 {
            0.3
        } else if ctx.predicted_exhaustion_secs < 120.0 {
            0.1
        } else {
            0.0
        };

        // Burn rate penalty: fast consumption rate reduces confidence
        let burn_penalty = if ctx.burn_rate > 0.0 && ctx.quota_remaining_ratio < 0.5 {
            0.1
        } else {
            0.0
        };

        (base - exhaustion_penalty - burn_penalty).max(0.0)
    }

    /// Compute the priority sub-score normalized to [0.0, 1.0].
    fn priority_score(ctx: &ProviderScoreContext) -> f32 {
        if ctx.max_priority == 0 {
            return 0.5;
        }
        ctx.priority as f32 / ctx.max_priority as f32
    }

    /// Compute the latency sub-score (lower latency = higher score).
    fn latency_score(ctx: &ProviderScoreContext) -> f32 {
        if ctx.max_latency_ms <= 0.0 || ctx.latency_ms <= 0.0 {
            return 1.0; // No latency data — assume fine
        }
        (1.0 - (ctx.latency_ms / ctx.max_latency_ms) as f32).max(0.0)
    }
}

impl ScoringStrategy for WeightedScorer {
    fn score(&self, ctx: &ProviderScoreContext) -> f32 {
        let qs = Self::quota_score(ctx);
        let hs = ctx.health_score;
        let ps = Self::priority_score(ctx);
        let ls = Self::latency_score(ctx);

        let final_score =
            qs * self.weights.quota
            + hs * self.weights.health
            + ps * self.weights.priority
            + ls * self.weights.latency;

        final_score.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_ctx() -> ProviderScoreContext {
        ProviderScoreContext {
            quota_remaining_ratio: 1.0,
            predicted_exhaustion_secs: f64::INFINITY,
            burn_rate: 0.0,
            health_score: 1.0,
            priority: 10,
            max_priority: 10,
            latency_ms: 100.0,
            max_latency_ms: 200.0,
        }
    }

    #[test]
    fn perfect_provider_scores_high() {
        let scorer = WeightedScorer::new(ScoringWeights::default());
        let ctx = default_ctx();
        let score = scorer.score(&ctx);
        assert!(score > 0.9, "score={score}");
    }

    #[test]
    fn exhausted_provider_scores_low() {
        let scorer = WeightedScorer::new(ScoringWeights::default());
        let ctx = ProviderScoreContext {
            quota_remaining_ratio: 0.05,
            predicted_exhaustion_secs: 5.0,
            health_score: 0.5,
            ..default_ctx()
        };
        let score = scorer.score(&ctx);
        assert!(score < 0.5, "score={score}");
    }

    #[test]
    fn unhealthy_provider_scores_low() {
        let scorer = WeightedScorer::new(ScoringWeights::default());
        let ctx = ProviderScoreContext {
            health_score: 0.2,
            ..default_ctx()
        };
        let score = scorer.score(&ctx);
        assert!(score < 0.8, "score={score}");
    }

    #[test]
    fn low_priority_scores_lower() {
        let scorer = WeightedScorer::new(ScoringWeights::default());
        let high = scorer.score(&default_ctx());
        let low = scorer.score(&ProviderScoreContext {
            priority: 2,
            ..default_ctx()
        });
        assert!(high > low);
    }

    #[test]
    fn anticipatory_penalty_kicks_in() {
        let scorer = WeightedScorer::new(ScoringWeights::default());

        // Provider with lots of remaining quota but fast burn rate
        let fast_burn = ProviderScoreContext {
            quota_remaining_ratio: 0.3,
            predicted_exhaustion_secs: 20.0, // will exhaust in 20s
            burn_rate: 50.0,
            ..default_ctx()
        };
        let slow_burn = ProviderScoreContext {
            quota_remaining_ratio: 0.3,
            predicted_exhaustion_secs: 300.0,
            burn_rate: 1.0,
            ..default_ctx()
        };

        let fast_score = scorer.score(&fast_burn);
        let slow_score = scorer.score(&slow_burn);
        assert!(slow_score > fast_score, "slow={slow_score} fast={fast_score}");
    }

    #[test]
    fn score_always_bounded() {
        let scorer = WeightedScorer::new(ScoringWeights::default());

        // Worst case
        let ctx = ProviderScoreContext {
            quota_remaining_ratio: 0.0,
            predicted_exhaustion_secs: 0.0,
            burn_rate: 1000.0,
            health_score: 0.0,
            priority: 0,
            max_priority: 10,
            latency_ms: 5000.0,
            max_latency_ms: 5000.0,
        };
        let score = scorer.score(&ctx);
        assert!(score >= 0.0 && score <= 1.0, "score={score}");

        // Best case
        let score = scorer.score(&default_ctx());
        assert!(score >= 0.0 && score <= 1.0, "score={score}");
    }
}
