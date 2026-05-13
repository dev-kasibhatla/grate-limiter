use crate::clock::Timestamp;

/// Internal trait for quota tracking strategies.
///
/// All implementations must be thread-safe (Send + Sync).
#[allow(dead_code)]
pub(crate) trait QuotaTracker: Send + Sync {
    /// Check if `amount` units can be consumed without exceeding the quota.
    fn check(&self, amount: u64, now: Timestamp) -> bool;

    /// Record consumption of `amount` units.
    fn record(&self, amount: u64, now: Timestamp);

    /// Current remaining capacity.
    fn remaining(&self, now: Timestamp) -> u64;

    /// Total capacity (limit).
    fn capacity(&self) -> u64;

    /// Usage ratio [0.0, 1.0] — higher means more consumed.
    fn usage_ratio(&self, now: Timestamp) -> f64 {
        let cap = self.capacity();
        if cap == 0 {
            return 1.0;
        }
        let rem = self.remaining(now);
        1.0 - (rem as f64 / cap as f64)
    }

    /// Estimated burn rate in units per second, based on recent observations.
    fn burn_rate(&self, now: Timestamp) -> f64;

    /// Predicted seconds until exhaustion at current burn rate.
    /// Returns `f64::INFINITY` if burn rate is zero.
    fn predicted_exhaustion_secs(&self, now: Timestamp) -> f64 {
        let rate = self.burn_rate(now);
        if rate <= 0.0 {
            return f64::INFINITY;
        }
        let rem = self.remaining(now);
        rem as f64 / rate
    }

    /// Reset the tracker to its initial state.
    fn reset(&self, now: Timestamp);
}
