use std::sync::atomic::{AtomicU64, Ordering};

use crate::clock::Timestamp;
use crate::quota::Window;
use crate::quota::strategy::QuotaTracker;

/// Fixed window quota strategy.
///
/// Simple counter that resets at fixed intervals. Least complex strategy but susceptible
/// to boundary burst issues (double the rate at window edges).
#[allow(dead_code)]
pub(crate) struct FixedWindow {
    capacity: u64,
    window_nanos: u64,
    count: AtomicU64,
    window_start: AtomicU64,
}

#[allow(dead_code)]
impl FixedWindow {
    pub(crate) fn new(capacity: u64, window: Window, now: Timestamp) -> Self {
        Self {
            capacity,
            window_nanos: window.as_nanos(),
            count: AtomicU64::new(0),
            window_start: AtomicU64::new(now.0),
        }
    }

    /// Check if the window has expired and reset if necessary.
    fn maybe_reset(&self, now: Timestamp) {
        let ws = self.window_start.load(Ordering::Acquire);
        if now.0.saturating_sub(ws) >= self.window_nanos {
            self.count.store(0, Ordering::Release);
            // Align to window boundary
            let windows_elapsed = now.0.saturating_sub(ws) / self.window_nanos;
            let new_start = ws + windows_elapsed * self.window_nanos;
            self.window_start.store(new_start, Ordering::Release);
        }
    }
}

impl QuotaTracker for FixedWindow {
    fn check(&self, amount: u64, now: Timestamp) -> bool {
        self.maybe_reset(now);
        let current = self.count.load(Ordering::Acquire);
        current + amount <= self.capacity
    }

    fn record(&self, amount: u64, now: Timestamp) {
        self.maybe_reset(now);
        self.count.fetch_add(amount, Ordering::AcqRel);
    }

    fn remaining(&self, now: Timestamp) -> u64 {
        self.maybe_reset(now);
        let used = self.count.load(Ordering::Acquire);
        self.capacity.saturating_sub(used)
    }

    fn capacity(&self) -> u64 {
        self.capacity
    }

    fn burn_rate(&self, now: Timestamp) -> f64 {
        self.maybe_reset(now);
        let ws = self.window_start.load(Ordering::Acquire);
        let elapsed_secs = now.0.saturating_sub(ws) as f64 / 1_000_000_000.0;
        if elapsed_secs < 0.001 {
            return 0.0;
        }
        let count = self.count.load(Ordering::Acquire);
        count as f64 / elapsed_secs
    }

    fn reset(&self, now: Timestamp) {
        self.count.store(0, Ordering::Release);
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
    fn new_window_full_capacity() {
        let fw = FixedWindow::new(100, Window::Minute, ts(0));
        assert_eq!(fw.remaining(ts(0)), 100);
    }

    #[test]
    fn record_reduces_remaining() {
        let fw = FixedWindow::new(100, Window::Minute, ts(0));
        fw.record(60, ts(0));
        assert_eq!(fw.remaining(ts(0)), 40);
    }

    #[test]
    fn window_resets_after_expiry() {
        let fw = FixedWindow::new(100, Window::Minute, ts(0));
        fw.record(100, ts(0));
        assert_eq!(fw.remaining(ts(0)), 0);

        // After the window expires, capacity should be restored
        assert_eq!(fw.remaining(ts(60_000)), 100);
    }

    #[test]
    fn check_respects_capacity() {
        let fw = FixedWindow::new(100, Window::Minute, ts(0));
        assert!(fw.check(100, ts(0)));
        assert!(!fw.check(101, ts(0)));

        fw.record(90, ts(0));
        assert!(fw.check(10, ts(0)));
        assert!(!fw.check(11, ts(0)));
    }

    #[test]
    fn multiple_windows() {
        let fw = FixedWindow::new(10, Window::Second, ts(0));
        fw.record(10, ts(0));
        assert_eq!(fw.remaining(ts(0)), 0);
        assert_eq!(fw.remaining(ts(1_000)), 10); // next second
        fw.record(5, ts(1_000));
        assert_eq!(fw.remaining(ts(1_000)), 5);
        assert_eq!(fw.remaining(ts(2_000)), 10); // next window
    }
}
