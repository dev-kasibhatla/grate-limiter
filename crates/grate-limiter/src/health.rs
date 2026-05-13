use serde::{Deserialize, Serialize};

use crate::clock::Timestamp;

/// Health engine configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthConfig {
    /// Half-life for EWMA decay in seconds. Older observations matter less.
    pub decay_half_life_seconds: f64,
    /// Penalty applied to health score on a 429 response.
    pub penalty_429: f32,
    /// Penalty applied to health score on a 403 response.
    pub penalty_403: f32,
    /// Penalty applied to health score on a 5xx response.
    pub penalty_5xx: f32,
    /// Penalty applied to health score on a timeout.
    pub penalty_timeout: f32,
    /// Boost applied on successful response.
    pub boost_success: f32,
    /// Number of consecutive failures before triggering cooldown.
    pub cooldown_trigger_count: u32,
    /// Multiplier for exponential cooldown growth.
    pub cooldown_multiplier: f64,
    /// Maximum cooldown duration in seconds.
    pub max_cooldown_seconds: u64,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            decay_half_life_seconds: 300.0,
            penalty_429: 0.25,
            penalty_403: 0.50,
            penalty_5xx: 0.10,
            penalty_timeout: 0.20,
            boost_success: 0.02,
            cooldown_trigger_count: 3,
            cooldown_multiplier: 2.0,
            max_cooldown_seconds: 600,
        }
    }
}

/// Runtime health state for a single provider.
pub(crate) struct HealthState {
    /// Current health score [0.0, 1.0].
    score: f32,
    /// Consecutive failure count (for cooldown triggering).
    consecutive_failures: u32,
    /// Current cooldown duration in seconds (grows exponentially).
    current_cooldown_secs: u64,
    /// Timestamp when cooldown expires (if active).
    cooldown_until: Option<Timestamp>,
    /// Last observation timestamp (for EWMA decay).
    last_observation: Timestamp,
    /// Total observations.
    total_observations: u64,
    /// Total successes.
    total_successes: u64,
    /// EWMA latency in milliseconds.
    ewma_latency_ms: f64,
}

impl HealthState {
    pub(crate) fn new(now: Timestamp) -> Self {
        Self {
            score: 1.0,
            consecutive_failures: 0,
            current_cooldown_secs: 0,
            cooldown_until: None,
            last_observation: now,
            total_observations: 0,
            total_successes: 0,
            ewma_latency_ms: 0.0,
        }
    }

    /// Current health score.
    pub(crate) fn score(&self) -> f32 {
        self.score
    }

    /// Whether the provider is currently in cooldown.
    pub(crate) fn is_in_cooldown(&self, now: Timestamp) -> bool {
        match self.cooldown_until {
            Some(until) => now < until,
            None => false,
        }
    }

    /// EWMA latency in milliseconds.
    pub(crate) fn latency_ms(&self) -> f64 {
        self.ewma_latency_ms
    }

    /// Apply a successful observation.
    pub(crate) fn record_success(&mut self, latency_ms: u64, now: Timestamp, config: &HealthConfig) {
        self.apply_decay(now, config);
        self.score = (self.score + config.boost_success).min(1.0);
        self.consecutive_failures = 0;
        self.total_observations += 1;
        self.total_successes += 1;
        self.update_latency(latency_ms, config);
        self.last_observation = now;
    }

    /// Apply a 429 (rate limited) observation.
    pub(crate) fn record_rate_limited(
        &mut self,
        now: Timestamp,
        config: &HealthConfig,
        default_cooldown_secs: u64,
    ) {
        self.apply_decay(now, config);
        self.score = (self.score - config.penalty_429).max(0.0);
        self.total_observations += 1;
        self.record_failure(now, config, default_cooldown_secs);
        self.last_observation = now;
    }

    /// Apply a 403 (forbidden) observation.
    pub(crate) fn record_forbidden(
        &mut self,
        now: Timestamp,
        config: &HealthConfig,
        default_cooldown_secs: u64,
    ) {
        self.apply_decay(now, config);
        self.score = (self.score - config.penalty_403).max(0.0);
        self.total_observations += 1;
        self.record_failure(now, config, default_cooldown_secs);
        self.last_observation = now;
    }

    /// Apply a 5xx (server error) observation.
    pub(crate) fn record_server_error(
        &mut self,
        now: Timestamp,
        config: &HealthConfig,
        default_cooldown_secs: u64,
    ) {
        self.apply_decay(now, config);
        self.score = (self.score - config.penalty_5xx).max(0.0);
        self.total_observations += 1;
        self.record_failure(now, config, default_cooldown_secs);
        self.last_observation = now;
    }

    /// Apply a timeout observation.
    pub(crate) fn record_timeout(
        &mut self,
        now: Timestamp,
        config: &HealthConfig,
        default_cooldown_secs: u64,
    ) {
        self.apply_decay(now, config);
        self.score = (self.score - config.penalty_timeout).max(0.0);
        self.total_observations += 1;
        self.record_failure(now, config, default_cooldown_secs);
        self.last_observation = now;
    }

    /// Apply EWMA decay — old penalties fade, health recovers naturally.
    fn apply_decay(&mut self, now: Timestamp, config: &HealthConfig) {
        let elapsed_secs =
            now.duration_since(self.last_observation) as f64 / 1_000_000_000.0;
        if elapsed_secs <= 0.0 || config.decay_half_life_seconds <= 0.0 {
            return;
        }

        // EWMA decay: score moves toward 1.0 over time
        let decay_factor = (0.5_f64).powf(elapsed_secs / config.decay_half_life_seconds);
        // score = 1.0 - (1.0 - score) * decay_factor
        let deficit = 1.0 - self.score;
        self.score = 1.0 - deficit * decay_factor as f32;
        self.score = self.score.clamp(0.0, 1.0);
    }

    /// Record a failure and potentially trigger cooldown.
    fn record_failure(
        &mut self,
        now: Timestamp,
        config: &HealthConfig,
        default_cooldown_secs: u64,
    ) {
        self.consecutive_failures += 1;

        if self.consecutive_failures >= config.cooldown_trigger_count {
            // Calculate cooldown duration with exponential growth
            let excess = self.consecutive_failures - config.cooldown_trigger_count;
            let multiplier = config.cooldown_multiplier.powi(excess as i32);
            let cooldown_secs = (default_cooldown_secs as f64 * multiplier) as u64;
            self.current_cooldown_secs = cooldown_secs.min(config.max_cooldown_seconds);
            self.cooldown_until = Some(now.add_secs(self.current_cooldown_secs));
        }
    }

    /// Update EWMA latency with a new sample.
    fn update_latency(&mut self, latency_ms: u64, _config: &HealthConfig) {
        const ALPHA: f64 = 0.3; // EWMA smoothing factor
        if self.total_observations <= 1 {
            self.ewma_latency_ms = latency_ms as f64;
        } else {
            self.ewma_latency_ms =
                ALPHA * latency_ms as f64 + (1.0 - ALPHA) * self.ewma_latency_ms;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(ms: u64) -> Timestamp {
        Timestamp(ms * 1_000_000)
    }

    #[test]
    fn initial_health_is_perfect() {
        let h = HealthState::new(ts(0));
        assert_eq!(h.score(), 1.0);
        assert!(!h.is_in_cooldown(ts(0)));
    }

    #[test]
    fn success_maintains_health() {
        let config = HealthConfig::default();
        let mut h = HealthState::new(ts(0));
        h.record_success(100, ts(1_000), &config);
        assert!(h.score() >= 1.0); // boost applied, clamped to 1.0
    }

    #[test]
    fn rate_limit_reduces_health() {
        let config = HealthConfig::default();
        let mut h = HealthState::new(ts(0));
        h.record_rate_limited(ts(1_000), &config, 60);
        assert!(h.score() < 1.0);
        assert!((h.score() - (1.0 - config.penalty_429)).abs() < 0.01);
    }

    #[test]
    fn health_decays_toward_full() {
        let config = HealthConfig {
            decay_half_life_seconds: 10.0, // fast decay for testing
            ..Default::default()
        };
        let mut h = HealthState::new(ts(0));
        h.record_rate_limited(ts(0), &config, 60);
        let after_penalty = h.score();

        // After 10s (one half-life), deficit should halve
        h.record_success(100, ts(10_000), &config);
        assert!(h.score() > after_penalty);
    }

    #[test]
    fn consecutive_failures_trigger_cooldown() {
        let config = HealthConfig {
            cooldown_trigger_count: 3,
            ..Default::default()
        };
        let mut h = HealthState::new(ts(0));

        h.record_rate_limited(ts(1_000), &config, 30);
        assert!(!h.is_in_cooldown(ts(1_000)));

        h.record_rate_limited(ts(2_000), &config, 30);
        assert!(!h.is_in_cooldown(ts(2_000)));

        h.record_rate_limited(ts(3_000), &config, 30);
        assert!(h.is_in_cooldown(ts(3_000)));
        assert!(h.is_in_cooldown(ts(32_000))); // still in cooldown at 32s

        // After cooldown expires
        assert!(!h.is_in_cooldown(ts(34_000)));
    }

    #[test]
    fn cooldown_grows_exponentially() {
        let config = HealthConfig {
            cooldown_trigger_count: 2,
            cooldown_multiplier: 2.0,
            max_cooldown_seconds: 600,
            ..Default::default()
        };
        let mut h = HealthState::new(ts(0));

        // First cooldown at consecutive_failures=2: base duration
        h.record_rate_limited(ts(1_000), &config, 30);
        h.record_rate_limited(ts(2_000), &config, 30);
        assert!(h.is_in_cooldown(ts(2_000)));

        // Third failure: cooldown should double
        h.record_rate_limited(ts(33_000), &config, 30);
        // excess=1, multiplier=2^1=2, cooldown=60s
        assert!(h.is_in_cooldown(ts(92_000))); // 33s + 60s = 93s
    }

    #[test]
    fn health_score_bounded() {
        let config = HealthConfig::default();
        let mut h = HealthState::new(ts(0));

        // Many failures
        for i in 0..20 {
            h.record_rate_limited(ts(i * 1_000), &config, 60);
        }
        assert!(h.score() >= 0.0);

        // Many successes
        for i in 20..40 {
            h.record_success(100, ts(i * 1_000), &config);
        }
        assert!(h.score() <= 1.0);
    }

    #[test]
    fn ewma_latency_smooths() {
        let config = HealthConfig::default();
        let mut h = HealthState::new(ts(0));

        h.record_success(100, ts(1_000), &config);
        assert_eq!(h.latency_ms(), 100.0);

        h.record_success(200, ts(2_000), &config);
        // EWMA: 0.3 * 200 + 0.7 * 100 = 130
        assert!((h.latency_ms() - 130.0).abs() < 1.0);
    }

    #[test]
    fn forbidden_is_severe() {
        let config = HealthConfig::default();
        let mut h = HealthState::new(ts(0));
        h.record_forbidden(ts(1_000), &config, 60);
        assert!((h.score() - (1.0 - config.penalty_403)).abs() < 0.01);
    }
}
