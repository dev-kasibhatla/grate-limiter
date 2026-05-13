use std::sync::atomic::{AtomicU64, Ordering};

use crate::clock::Timestamp;
use crate::quota::strategy::QuotaTracker;

/// Concurrency limiter — tracks in-flight requests rather than rate.
///
/// Unlike rate-based quotas, concurrency limits don't have a time window.
/// The count is incremented on acquire and decremented on release.
pub(crate) struct ConcurrencyLimiter {
    capacity: u64,
    active: AtomicU64,
}

impl ConcurrencyLimiter {
    pub(crate) fn new(capacity: u64) -> Self {
        Self {
            capacity,
            active: AtomicU64::new(0),
        }
    }

    /// Release a concurrency slot. Called when a request completes.
    #[allow(dead_code)]
    pub(crate) fn release(&self, amount: u64) {
        let current = self.active.load(Ordering::Acquire);
        let new = current.saturating_sub(amount);
        self.active.store(new, Ordering::Release);
    }
}

impl QuotaTracker for ConcurrencyLimiter {
    fn check(&self, amount: u64, _now: Timestamp) -> bool {
        let current = self.active.load(Ordering::Acquire);
        current + amount <= self.capacity
    }

    fn record(&self, amount: u64, _now: Timestamp) {
        self.active.fetch_add(amount, Ordering::AcqRel);
    }

    fn remaining(&self, _now: Timestamp) -> u64 {
        let active = self.active.load(Ordering::Acquire);
        self.capacity.saturating_sub(active)
    }

    fn capacity(&self) -> u64 {
        self.capacity
    }

    fn burn_rate(&self, _now: Timestamp) -> f64 {
        // Concurrency doesn't have a meaningful burn rate
        0.0
    }

    fn reset(&self, _now: Timestamp) {
        self.active.store(0, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(ms: u64) -> Timestamp {
        Timestamp(ms * 1_000_000)
    }

    #[test]
    fn new_has_full_capacity() {
        let cl = ConcurrencyLimiter::new(10);
        assert_eq!(cl.remaining(ts(0)), 10);
    }

    #[test]
    fn record_occupies_slots() {
        let cl = ConcurrencyLimiter::new(10);
        cl.record(3, ts(0));
        assert_eq!(cl.remaining(ts(0)), 7);
    }

    #[test]
    fn release_frees_slots() {
        let cl = ConcurrencyLimiter::new(10);
        cl.record(5, ts(0));
        assert_eq!(cl.remaining(ts(0)), 5);

        cl.release(3);
        assert_eq!(cl.remaining(ts(0)), 8);
    }

    #[test]
    fn check_respects_capacity() {
        let cl = ConcurrencyLimiter::new(5);
        assert!(cl.check(5, ts(0)));
        assert!(!cl.check(6, ts(0)));

        cl.record(3, ts(0));
        assert!(cl.check(2, ts(0)));
        assert!(!cl.check(3, ts(0)));
    }

    #[test]
    fn time_does_not_affect_concurrency() {
        let cl = ConcurrencyLimiter::new(10);
        cl.record(10, ts(0));
        assert_eq!(cl.remaining(ts(0)), 0);
        // Time passing doesn't free slots
        assert_eq!(cl.remaining(ts(60_000)), 0);
    }
}
