use serde::{Deserialize, Serialize};

use crate::quota::QuotaConfig;

/// Configuration for a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Unique provider name (e.g., "openai", "anthropic").
    pub name: String,
    /// Quota configurations for this provider.
    pub quotas: Vec<QuotaConfig>,
    /// Default priority (higher = preferred). Overridden at capability level.
    pub priority: u16,
    /// Weight multiplier for scoring (default 1.0).
    pub weight: f32,
    /// Cooldown duration in seconds after repeated failures.
    pub cooldown_seconds: u64,
}
