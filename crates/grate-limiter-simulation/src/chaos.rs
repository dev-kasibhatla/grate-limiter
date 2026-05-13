use serde::{Deserialize, Serialize};

/// Configuration for chaos testing scenarios.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaosConfig {
    /// Events to inject during simulation.
    pub events: Vec<ChaosEvent>,
}

/// A chaos event to inject at a specific simulation step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaosEvent {
    /// Simulation step at which to inject this event.
    pub at_step: u64,
    /// Type of chaos to inject.
    pub kind: ChaosKind,
}

/// Types of chaos events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChaosKind {
    /// Provider becomes completely unavailable.
    ProviderDown { provider: String },
    /// Provider latency spikes to the given multiplier.
    LatencySpike { provider: String, multiplier: u64 },
    /// All providers degrade simultaneously.
    GlobalDegradation { error_rate: f64 },
    /// Provider recovers from a previous chaos event.
    ProviderRecover { provider: String },
}
