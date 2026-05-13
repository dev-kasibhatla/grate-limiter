use std::sync::atomic::{AtomicU64, Ordering};

use crate::clock::Timestamp;
use crate::quota::strategy::QuotaTracker;
use crate::quota::Window;

/// Token bucket quota strategy.
///
/// Tokens refill continuously over the window period. This provides smooth rate limiting
/// that naturally handles bursts while enforcing the average rate.
pub(crate) struct TokenBucket {
    /// Maximum tokens (= limit).
    capacity: u64,
    /// Window duration in nanoseconds.
    window_nanos: u64,
    /// Tokens remaining, stored as fixed-point (value * PRECISION).
    tokens: AtomicU64,
    /// Last refill timestamp in nanoseconds.
    last_refill: AtomicU64,
    /// Total consumed since last window reset (for burn rate).
    consumed_in_window: AtomicU64,
    /// Window start for burn rate calculation.
    window_start: AtomicU64,
}

const PRECISION: u64 = 1_000_000; // Fixed-point precision for fractional tokens

impl TokenBucket {
    pub(crate) fn new(capacity: u64, window: Window, now: Timestamp) -> Self {
        Self {
            capacity,
            window_nanos: window.as_nanos(),
            tokens: AtomicU64::new(capacity * PRECISION),
            last_refill: AtomicU64::new(now.0),
            consumed_in_window: AtomicU64::new(0),
            window_start: AtomicU64::new(now.0),
        }
    }

    /// Refill tokens based on elapsed time. Returns current token count (fixed-point).
    fn refill(&self, now: Timestamp) -> u64 {
        let last = self.last_refill.load(Ordering::Acquire);
        let elapsed = now.0.saturating_sub(last);
        if elapsed == 0 {
            return self.tokens.load(Ordering::Acquire);
        }

        // Calculate tokens to add: (elapsed / window) * capacity
        let tokens_to_add =
            (elapsed as u128 * self.capacity as u128 * PRECISION as u128) / self.window_nanos as u128;
        let tokens_to_add = tokens_to_add as u64;

        if tokens_to_add == 0 {
            return self.tokens.load(Ordering::Acquire);
        }

        // CAS loop to update last_refill and tokens atomically-ish
        // This is best-effort; concurrent refills may slightly over-count,
        // but the cap ensures we never exceed capacity.
        self.last_refill.store(now.0, Ordering::Release);
        let max_tokens = self.capacity * PRECISION;
        let current = self.tokens.fetch_add(tokens_to_add, Ordering::AcqRel);
        let new_tokens = current.saturating_add(tokens_to_add).min(max_tokens);

        // Clamp to max if we exceeded
        if current.saturating_add(tokens_to_add) > max_tokens {
            self.tokens.store(max_tokens, Ordering::Release);
        }

        // Reset burn rate window if it's been a full window
        let ws = self.window_start.load(Ordering::Acquire);
        if now.0.saturating_sub(ws) >= self.window_nanos {
            self.consumed_in_window.store(0, Ordering::Release);
            self.window_start.store(now.0, Ordering::Release);
        }

        new_tokens
    }
}

impl QuotaTracker for TokenBucket {
    fn check(&self, amount: u64, now: Timestamp) -> bool {
        let available = self.refill(now);
        available >= amount * PRECISION
    }

    fn record(&self, amount: u64, now: Timestamp) {
        self.refill(now);
        let cost = amount * PRECISION;
        self.tokens.fetch_sub(cost.min(self.tokens.load(Ordering::Acquire)), Ordering::AcqRel);
        self.consumed_in_window.fetch_add(amount, Ordering::Relaxed);
    }

    fn remaining(&self, now: Timestamp) -> u64 {
        self.refill(now) / PRECISION
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
        let consumed = self.consumed_in_window.load(Ordering::Acquire);
        consumed as f64 / elapsed_secs
    }

    fn reset(&self, now: Timestamp) {
        self.tokens.store(self.capacity * PRECISION, Ordering::Release);
        self.last_refill.store(now.0, Ordering::Release);
        self.consumed_in_window.store(0, Ordering::Release);
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
    fn new_bucket_is_full() {
        let bucket = TokenBucket::new(100, Window::Minute, ts(0));
        assert_eq!(bucket.remaining(ts(0)), 100);
        assert!(bucket.check(100, ts(0)));
        assert!(!bucket.check(101, ts(0)));
    }

    #[test]
    fn consume_reduces_remaining() {
        let bucket = TokenBucket::new(100, Window::Minute, ts(0));
        bucket.record(30, ts(0));
        assert_eq!(bucket.remaining(ts(0)), 70);
    }

    #[test]
    fn tokens_refill_over_time() {
        let bucket = TokenBucket::new(60, Window::Minute, ts(0));
        bucket.record(60, ts(0));
        assert_eq!(bucket.remaining(ts(0)), 0);

        // After 30 seconds, half the tokens should refill
        assert_eq!(bucket.remaining(ts(30_000)), 30);

        // After 60 seconds, fully refilled
        assert_eq!(bucket.remaining(ts(60_000)), 60);
    }

    #[test]
    fn never_exceeds_capacity() {
        let bucket = TokenBucket::new(100, Window::Minute, ts(0));
        // Wait way longer than a window
        assert_eq!(bucket.remaining(ts(120_000)), 100);
    }

    #[test]
    fn burn_rate_tracks_consumption() {
        let bucket = TokenBucket::new(100, Window::Minute, ts(0));
        bucket.record(10, ts(1_000)); // 10 units at t=1s
        bucket.record(10, ts(2_000)); // 10 more at t=2s

        let rate = bucket.burn_rate(ts(5_000)); // measure at t=5s
        // 20 units consumed over 5 seconds = 4.0/s
        assert!((rate - 4.0).abs() < 0.1);
    }

    #[test]
    fn usage_ratio() {
        let bucket = TokenBucket::new(100, Window::Minute, ts(0));
        assert!((bucket.usage_ratio(ts(0))).abs() < 0.01);

        bucket.record(80, ts(0));
        assert!((bucket.usage_ratio(ts(0)) - 0.8).abs() < 0.01);
    }

    #[test]
    fn predicted_exhaustion() {
        let bucket = TokenBucket::new(100, Window::Minute, ts(0));
        bucket.record(50, ts(5_000)); // 50 units in 5 seconds = 10/s

        let secs = bucket.predicted_exhaustion_secs(ts(5_000));
        // 50 remaining / 10 per sec = 5 seconds
        assert!((secs - 5.0).abs() < 1.0);
    }

    #[test]
    fn reset_restores_full_capacity() {
        let bucket = TokenBucket::new(100, Window::Minute, ts(0));
        bucket.record(100, ts(0));
        assert_eq!(bucket.remaining(ts(0)), 0);

        bucket.reset(ts(1_000));
        assert_eq!(bucket.remaining(ts(1_000)), 100);
    }
}
