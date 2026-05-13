use grate_limiter::{Dimension, ProviderConfig, QuotaConfig, StatusClass, Window};
use rand::Rng;

/// A simulated provider with configurable failure behaviors.
#[derive(Debug, Clone)]
pub struct SimulatedProvider {
    pub name: String,
    pub quota_limit: u64,
    pub window: Window,
    pub behaviors: Vec<SimulatedBehavior>,
    pub base_latency_ms: u64,
}

/// Simulated failure behavior for a provider.
#[derive(Debug, Clone)]
pub enum SimulatedBehavior {
    /// Hard rate limit at the given request count per window.
    HardQuota { limit: u64 },
    /// Hidden soft throttling starts before the documented limit.
    HiddenThrottling { soft_limit_ratio: f64 },
    /// Random server errors at the given probability.
    RandomServerErrors { probability: f64 },
    /// Latency spikes at the given probability, multiplying base latency.
    LatencySpikes { probability: f64, multiplier: u64 },
    /// Progressive throttling: 429 probability increases with usage.
    ProgressiveThrottling,
}

impl SimulatedProvider {
    pub fn to_provider_config(&self) -> ProviderConfig {
        ProviderConfig {
            name: self.name.clone(),
            quotas: vec![QuotaConfig {
                dimension: Dimension::Requests,
                limit: self.quota_limit,
                window: Some(self.window),
            }],
            priority: 5,
            weight: 1.0,
            cooldown_seconds: 30,
        }
    }

    /// Simulate a response for this provider.
    /// Returns (status, latency_ms).
    pub fn simulate_response(&self, request_number: u64, rng: &mut impl Rng) -> (StatusClass, u64) {
        let mut latency = self.base_latency_ms;
        let mut status = StatusClass::Success;

        for behavior in &self.behaviors {
            match behavior {
                SimulatedBehavior::HardQuota { limit } => {
                    if request_number % self.quota_limit > *limit {
                        status = StatusClass::RateLimited;
                    }
                }
                SimulatedBehavior::HiddenThrottling { soft_limit_ratio } => {
                    let soft_limit = (self.quota_limit as f64 * soft_limit_ratio) as u64;
                    let usage_in_window = request_number % self.quota_limit;
                    if usage_in_window > soft_limit {
                        let over_ratio = (usage_in_window - soft_limit) as f64
                            / (self.quota_limit - soft_limit) as f64;
                        if rng.random::<f64>() < over_ratio {
                            status = StatusClass::RateLimited;
                        }
                    }
                }
                SimulatedBehavior::RandomServerErrors { probability } => {
                    if rng.random::<f64>() < *probability {
                        status = StatusClass::ServerError;
                    }
                }
                SimulatedBehavior::LatencySpikes {
                    probability,
                    multiplier,
                } => {
                    if rng.random::<f64>() < *probability {
                        latency *= multiplier;
                    }
                }
                SimulatedBehavior::ProgressiveThrottling => {
                    let usage_ratio =
                        (request_number % self.quota_limit) as f64 / self.quota_limit as f64;
                    if usage_ratio > 0.7 && rng.random::<f64>() < (usage_ratio - 0.7) * 3.0 {
                        status = StatusClass::RateLimited;
                    }
                }
            }
        }

        (status, latency)
    }
}
