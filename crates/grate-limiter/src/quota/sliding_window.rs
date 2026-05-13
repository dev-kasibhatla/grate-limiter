use std::sync::atomic::{AtomicU64, Ordering};

use crate::clock::Timestamp;
use crate::quota::Window;
use crate::quota::strategy::QuotaTracker;

/// Sliding window counter quota strategy.
///
/// Approximates a true sliding window by interpolating between the previous and current
/// fixed windows. More accurate than a pure fixed window for bursty traffic.
#[allow(dead_code)]
pub(crate) struct SlidingWindowCounter {
    capacity: u64,
    window_nanos: u64,
    /// Count in the current window.
    current_count: AtomicU64,
    /// Count in the previous window.
    previous_count: AtomicU64,
    /// Start of the current window (nanos).
    window_start: AtomicU64,
}

#[allow(dead_code)]
impl SlidingWindowCounter {
    pub(crate) fn new(capacity: u64, window: Window, now: Timestamp) -> Self {
        Self {
            capacity,
            window_nanos: window.as_nanos(),
            current_count: AtomicU64::new(0),
            previous_count: AtomicU64::new(0),
            window_start: AtomicU64::new(now.0),
        }
    }

    /// Rotate windows if needed and return the interpolated count.
    fn rotate_and_count(&self, now: Timestamp) -> u64 {
        let ws = self.window_start.load(Ordering::Acquire);
        let elapsed = now.0.saturating_sub(ws);

        if elapsed >= 2 * self.window_nanos {
            // More than 2 windows passed — fully reset
            self.previous_count.store(0, Ordering::Release);
            self.current_count.store(0, Ordering::Release);
            self.window_start.store(now.0, Ordering::Release);
            return 0;
        }

        if elapsed >= self.window_nanos {
            // Rotate: current becomes previous, start new window
            let current = self.current_count.load(Ordering::Acquire);
            self.previous_count.store(current, Ordering::Release);
            self.current_count.store(0, Ordering::Release);
            let new_start = ws + self.window_nanos;
            self.window_start.store(new_start, Ordering::Release);

            // Recalculate position in new window
            let new_elapsed = now.0.saturating_sub(new_start);
            let fraction_of_prev = 1.0 - (new_elapsed as f64 / self.window_nanos as f64);
            let weighted_prev = (current as f64 * fraction_of_prev) as u64;
            return weighted_prev;
        }

        // Within current window — interpolate
        let prev = self.previous_count.load(Ordering::Acquire);
        let curr = self.current_count.load(Ordering::Acquire);
        let fraction_of_prev = 1.0 - (elapsed as f64 / self.window_nanos as f64);
        let weighted_prev = (prev as f64 * fraction_of_prev) as u64;
        weighted_prev + curr
    }
}

impl QuotaTracker for SlidingWindowCounter {
    fn check(&self, amount: u64, now: Timestamp) -> bool {
        let current_usage = self.rotate_and_count(now);
        current_usage + amount <= self.capacity
    }

    fn record(&self, amount: u64, now: Timestamp) {
        // Ensure window rotation happens
        self.rotate_and_count(now);
        self.current_count.fetch_add(amount, Ordering::AcqRel);
    }

    fn remaining(&self, now: Timestamp) -> u64 {
        let used = self.rotate_and_count(now);
        self.capacity.saturating_sub(used)
    }

    fn capacity(&self) -> u64 {
        self.capacity
    }

    fn burn_rate(&self, now: Timestamp) -> f64 {
        let ws = self.window_start.load(Ordering::Acquire);
        let elapsed_secs = now.0.saturating_sub(ws) as f64 / 1_000_000_000.0;
        if elapsed_secs < 0.001 {
            return 0.0;
        }
        let curr = self.current_count.load(Ordering::Acquire);
        curr as f64 / elapsed_secs
    }

    fn reset(&self, now: Timestamp) {
        self.current_count.store(0, Ordering::Release);
        self.previous_count.store(0, Ordering::Release);
        self.window_start.store(now.0, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(ms: u64) -> Timestamp {
        Timestamp(ms * 1_000_000)
    }

    #[test]
    fn new_window_has_full_capacity() {
        let sw = SlidingWindowCounter::new(100, Window::Minute, ts(0));
        assert_eq!(sw.remaining(ts(0)), 100);
    }

    #[test]
    fn record_reduces_remaining() {
        let sw = SlidingWindowCounter::new(100, Window::Minute, ts(0));
        sw.record(40, ts(0));
        assert_eq!(sw.remaining(ts(0)), 60);
    }

    #[test]
    fn window_rotation_interpolates() {
        let sw = SlidingWindowCounter::new(100, Window::Minute, ts(0));
        sw.record(80, ts(0));

        // At 60s, window rotates. Previous window had 80.
        // At 90s (halfway through new window), 50% of previous should count.
        let remaining = sw.remaining(ts(90_000));
        // ~50% of 80 = 40 used from previous, so ~60 remaining
        assert!((55..=65).contains(&remaining), "remaining={remaining}");
    }

    #[test]
    fn full_window_resets() {
        let sw = SlidingWindowCounter::new(100, Window::Minute, ts(0));
        sw.record(100, ts(0));
        assert_eq!(sw.remaining(ts(0)), 0);

        // After 2 full windows, everything should be reset
        assert_eq!(sw.remaining(ts(120_001)), 100);
    }

    #[test]
    fn check_respects_capacity() {
        let sw = SlidingWindowCounter::new(100, Window::Minute, ts(0));
        assert!(sw.check(100, ts(0)));
        assert!(!sw.check(101, ts(0)));

        sw.record(50, ts(0));
        assert!(sw.check(50, ts(0)));
        assert!(!sw.check(51, ts(0)));
    }
}
