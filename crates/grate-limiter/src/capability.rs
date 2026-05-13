use serde::{Deserialize, Serialize};

/// Configuration for a capability (e.g., "chat-completion", "image-generation").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityConfig {
    /// Unique capability name.
    pub name: String,
    /// Providers that can fulfill this capability, with per-capability priority.
    pub providers: Vec<CapabilityProvider>,
}

/// A provider registered under a capability with its priority for that capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityProvider {
    /// Provider name (must be registered via `upsert_provider`).
    pub provider: String,
    /// Priority for this capability (higher = preferred). Overrides provider-level priority.
    pub priority: u16,
}
