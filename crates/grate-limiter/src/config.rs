use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::clock::{self, Clock};
use crate::health::HealthConfig;
use crate::scoring::ScoringWeights;

/// Top-level engine configuration.
#[derive(Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    /// Scoring weights for provider selection.
    pub scoring: ScoringWeights,

    /// Health engine configuration.
    pub health: HealthConfig,

    /// Minimum health score before a provider is excluded from selection.
    pub minimum_health_score: f32,

    /// Time (in seconds) that a provider should remain on cooldown after being triggered.
    pub default_cooldown_seconds: u64,

    /// Internal clock (not serialized). Use [`EngineConfig::with_clock`].
    #[serde(skip)]
    pub(crate) clock: Option<Arc<dyn Clock>>,
}

impl EngineConfig {
    /// Inject a custom clock (e.g., [`MockClock`](crate::MockClock) for deterministic testing).
    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = Some(clock);
        self
    }

    pub(crate) fn clock(&self) -> Arc<dyn Clock> {
        self.clock.clone().unwrap_or_else(clock::default_clock)
    }
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            scoring: ScoringWeights::default(),
            health: HealthConfig::default(),
            minimum_health_score: 0.2,
            default_cooldown_seconds: 60,
            clock: None,
        }
    }
}
