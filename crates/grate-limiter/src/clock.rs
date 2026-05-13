use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Monotonic timestamp in nanoseconds since engine creation.
///
/// Uses monotonic time internally — immune to NTP drift, daylight saving, and clock rewinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(pub(crate) u64);

impl Timestamp {
    /// Creates a timestamp at the zero epoch (engine start).
    pub const ZERO: Timestamp = Timestamp(0);

    /// Returns the timestamp value in nanoseconds.
    pub fn as_nanos(&self) -> u64 {
        self.0
    }

    /// Returns the timestamp value in milliseconds.
    pub fn as_millis(&self) -> u64 {
        self.0 / 1_000_000
    }

    /// Returns the timestamp value in seconds (f64).
    pub fn as_secs_f64(&self) -> f64 {
        self.0 as f64 / 1_000_000_000.0
    }

    /// Duration since another timestamp in nanoseconds.
    /// Returns 0 if `other` is after `self`.
    pub fn duration_since(&self, other: Timestamp) -> u64 {
        self.0.saturating_sub(other.0)
    }

    /// Add nanoseconds to this timestamp.
    pub fn add_nanos(&self, nanos: u64) -> Timestamp {
        Timestamp(self.0.saturating_add(nanos))
    }

    /// Add milliseconds to this timestamp.
    pub fn add_millis(&self, millis: u64) -> Timestamp {
        Timestamp(self.0.saturating_add(millis * 1_000_000))
    }

    /// Add seconds to this timestamp.
    pub fn add_secs(&self, secs: u64) -> Timestamp {
        Timestamp(self.0.saturating_add(secs * 1_000_000_000))
    }
}

/// Clock abstraction for monotonic time.
///
/// Use [`RealClock`] in production and [`MockClock`] for deterministic testing.
pub trait Clock: Send + Sync {
    /// Returns the current monotonic timestamp.
    fn now(&self) -> Timestamp;
}

/// Real monotonic clock backed by [`std::time::Instant`].
pub struct RealClock {
    epoch: Instant,
}

impl RealClock {
    pub fn new() -> Self {
        Self {
            epoch: Instant::now(),
        }
    }
}

impl Default for RealClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for RealClock {
    fn now(&self) -> Timestamp {
        let elapsed = self.epoch.elapsed();
        Timestamp(elapsed.as_nanos() as u64)
    }
}

/// Mock clock for deterministic testing.
///
/// Time only advances when explicitly told to, enabling fully reproducible test scenarios.
///
/// # Example
///
/// ```rust
/// use grate_limiter::{MockClock, Clock};
/// use std::sync::Arc;
///
/// let clock = Arc::new(MockClock::new());
/// assert_eq!(clock.now().as_millis(), 0);
///
/// clock.advance_ms(5000);
/// assert_eq!(clock.now().as_millis(), 5000);
/// ```
pub struct MockClock {
    nanos: AtomicU64,
}

impl MockClock {
    pub fn new() -> Self {
        Self {
            nanos: AtomicU64::new(0),
        }
    }

    /// Create a mock clock starting at a specific timestamp.
    pub fn at(timestamp: Timestamp) -> Self {
        Self {
            nanos: AtomicU64::new(timestamp.0),
        }
    }

    /// Advance time by the given number of nanoseconds.
    pub fn advance_nanos(&self, nanos: u64) {
        self.nanos.fetch_add(nanos, Ordering::Release);
    }

    /// Advance time by the given number of milliseconds.
    pub fn advance_ms(&self, ms: u64) {
        self.advance_nanos(ms * 1_000_000);
    }

    /// Advance time by the given number of seconds.
    pub fn advance_secs(&self, secs: u64) {
        self.advance_nanos(secs * 1_000_000_000);
    }

    /// Set time to a specific timestamp.
    pub fn set(&self, timestamp: Timestamp) {
        self.nanos.store(timestamp.0, Ordering::Release);
    }
}

impl Default for MockClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for MockClock {
    fn now(&self) -> Timestamp {
        Timestamp(self.nanos.load(Ordering::Acquire))
    }
}

/// Default clock factory — returns a real clock.
pub(crate) fn default_clock() -> Arc<dyn Clock> {
    Arc::new(RealClock::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_arithmetic() {
        let t = Timestamp::ZERO;
        assert_eq!(t.as_nanos(), 0);
        assert_eq!(t.as_millis(), 0);

        let t2 = t.add_millis(1500);
        assert_eq!(t2.as_millis(), 1500);
        assert_eq!(t2.as_nanos(), 1_500_000_000);

        let t3 = t2.add_secs(2);
        assert_eq!(t3.as_millis(), 3500);

        assert_eq!(t3.duration_since(t2), 2_000_000_000);
        assert_eq!(t2.duration_since(t3), 0); // saturating
    }

    #[test]
    fn mock_clock_advances() {
        let clock = MockClock::new();
        assert_eq!(clock.now(), Timestamp::ZERO);

        clock.advance_ms(100);
        assert_eq!(clock.now().as_millis(), 100);

        clock.advance_secs(1);
        assert_eq!(clock.now().as_millis(), 1100);
    }

    #[test]
    fn mock_clock_set() {
        let clock = MockClock::new();
        clock.set(Timestamp(5_000_000_000));
        assert_eq!(clock.now().as_secs_f64(), 5.0);
    }

    #[test]
    fn real_clock_monotonic() {
        let clock = RealClock::new();
        let t1 = clock.now();
        let t2 = clock.now();
        assert!(t2 >= t1);
    }

    #[test]
    fn timestamp_ordering() {
        let a = Timestamp(100);
        let b = Timestamp(200);
        assert!(a < b);
        assert!(b > a);
        assert_eq!(a, Timestamp(100));
    }
}
