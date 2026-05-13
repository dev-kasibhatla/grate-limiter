use std::sync::atomic::{AtomicU64, Ordering};

/// Engine-level metrics counters for observability.
///
/// All counters are monotonically increasing and lock-free.
pub struct Metrics {
    pub(crate) selects: AtomicU64,
    pub(crate) observations: AtomicU64,
    pub(crate) cooldowns_triggered: AtomicU64,
    pub(crate) no_provider_available: AtomicU64,
}

impl Metrics {
    pub(crate) fn new() -> Self {
        Self {
            selects: AtomicU64::new(0),
            observations: AtomicU64::new(0),
            cooldowns_triggered: AtomicU64::new(0),
            no_provider_available: AtomicU64::new(0),
        }
    }

    /// Total `select()` calls.
    pub fn selects(&self) -> u64 {
        self.selects.load(Ordering::Relaxed)
    }

    /// Total `observe()` calls.
    pub fn observations(&self) -> u64 {
        self.observations.load(Ordering::Relaxed)
    }

    /// Total times a provider was put on cooldown.
    pub fn cooldowns_triggered(&self) -> u64 {
        self.cooldowns_triggered.load(Ordering::Relaxed)
    }

    /// Total times `select()` found no available providers.
    pub fn no_provider_available(&self) -> u64 {
        self.no_provider_available.load(Ordering::Relaxed)
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}
