"""Tests for health engine."""

from grate_limiter.clock import Timestamp
from grate_limiter.config import HealthConfig
from grate_limiter.health import HealthState


def ts(ms: int) -> Timestamp:
    return Timestamp(ms * 1_000_000)


class TestHealthState:
    def test_initial_health_is_perfect(self) -> None:
        h = HealthState(ts(0))
        assert h.score == 1.0
        assert not h.is_in_cooldown(ts(0))

    def test_success_maintains_health(self) -> None:
        config = HealthConfig()
        h = HealthState(ts(0))
        h.record_success(100, ts(1_000), config)
        assert h.score >= 1.0  # boost applied, clamped to 1.0

    def test_rate_limit_reduces_health(self) -> None:
        config = HealthConfig()
        h = HealthState(ts(0))
        h.record_rate_limited(ts(1_000), config, 60)
        assert h.score < 1.0
        assert abs(h.score - (1.0 - config.penalty_429)) < 0.01

    def test_health_decays_toward_full(self) -> None:
        config = HealthConfig(decay_half_life_seconds=10.0)
        h = HealthState(ts(0))
        h.record_rate_limited(ts(0), config, 60)
        after_penalty = h.score
        h.record_success(100, ts(10_000), config)
        assert h.score > after_penalty

    def test_consecutive_failures_trigger_cooldown(self) -> None:
        config = HealthConfig(cooldown_trigger_count=3)
        h = HealthState(ts(0))

        h.record_rate_limited(ts(1_000), config, 30)
        assert not h.is_in_cooldown(ts(1_000))

        h.record_rate_limited(ts(2_000), config, 30)
        assert not h.is_in_cooldown(ts(2_000))

        h.record_rate_limited(ts(3_000), config, 30)
        assert h.is_in_cooldown(ts(3_000))
        assert h.is_in_cooldown(ts(32_000))  # still in cooldown at 32s
        assert not h.is_in_cooldown(ts(34_000))  # after cooldown expires

    def test_cooldown_grows_exponentially(self) -> None:
        config = HealthConfig(
            cooldown_trigger_count=2,
            cooldown_multiplier=2.0,
            max_cooldown_seconds=600,
        )
        h = HealthState(ts(0))

        h.record_rate_limited(ts(1_000), config, 30)
        h.record_rate_limited(ts(2_000), config, 30)
        assert h.is_in_cooldown(ts(2_000))

        # Third failure: cooldown should double
        h.record_rate_limited(ts(33_000), config, 30)
        assert h.is_in_cooldown(ts(92_000))  # 33s + 60s = 93s

    def test_health_score_bounded(self) -> None:
        config = HealthConfig()
        h = HealthState(ts(0))

        for i in range(20):
            h.record_rate_limited(ts(i * 1_000), config, 60)
        assert h.score >= 0.0

        for i in range(20, 40):
            h.record_success(100, ts(i * 1_000), config)
        assert h.score <= 1.0

    def test_ewma_latency_smooths(self) -> None:
        config = HealthConfig()
        h = HealthState(ts(0))

        h.record_success(100, ts(1_000), config)
        assert abs(h.latency_ms - 100.0) < 0.01

        h.record_success(200, ts(2_000), config)
        # EWMA: 0.3 * 200 + 0.7 * 100 = 130
        assert abs(h.latency_ms - 130.0) < 1.0

    def test_forbidden_applies_heavy_penalty(self) -> None:
        config = HealthConfig()
        h = HealthState(ts(0))
        h.record_forbidden(ts(1_000), config, 60)
        assert abs(h.score - (1.0 - config.penalty_403)) < 0.01

    def test_server_error_penalty(self) -> None:
        config = HealthConfig()
        h = HealthState(ts(0))
        h.record_server_error(ts(1_000), config, 60)
        assert abs(h.score - (1.0 - config.penalty_5xx)) < 0.01

    def test_timeout_penalty(self) -> None:
        config = HealthConfig()
        h = HealthState(ts(0))
        h.record_timeout(ts(1_000), config, 60)
        assert abs(h.score - (1.0 - config.penalty_timeout)) < 0.01
