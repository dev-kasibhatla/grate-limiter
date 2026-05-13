"""Health engine — EWMA-based health scoring with cooldowns."""

from __future__ import annotations

from grate_limiter.clock import Timestamp
from grate_limiter.config import HealthConfig

_LATENCY_ALPHA = 0.3


class HealthState:
    """Runtime health state for a single provider."""

    __slots__ = (
        "_score",
        "_consecutive_failures",
        "_current_cooldown_secs",
        "_cooldown_until",
        "_last_observation",
        "_total_observations",
        "_total_successes",
        "_ewma_latency_ms",
    )

    def __init__(self, now: Timestamp) -> None:
        self._score: float = 1.0
        self._consecutive_failures: int = 0
        self._current_cooldown_secs: int = 0
        self._cooldown_until: Timestamp | None = None
        self._last_observation: Timestamp = now
        self._total_observations: int = 0
        self._total_successes: int = 0
        self._ewma_latency_ms: float = 0.0

    @property
    def score(self) -> float:
        return self._score

    def is_in_cooldown(self, now: Timestamp) -> bool:
        if self._cooldown_until is None:
            return False
        return now < self._cooldown_until

    @property
    def latency_ms(self) -> float:
        return self._ewma_latency_ms

    def record_success(self, latency_ms: int, now: Timestamp, config: HealthConfig) -> None:
        self._apply_decay(now, config)
        self._score = min(self._score + config.boost_success, 1.0)
        self._consecutive_failures = 0
        self._total_observations += 1
        self._total_successes += 1
        self._update_latency(latency_ms)
        self._last_observation = now

    def record_rate_limited(
        self, now: Timestamp, config: HealthConfig, default_cooldown_secs: int
    ) -> None:
        self._apply_decay(now, config)
        self._score = max(self._score - config.penalty_429, 0.0)
        self._total_observations += 1
        self._record_failure(now, config, default_cooldown_secs)
        self._last_observation = now

    def record_forbidden(
        self, now: Timestamp, config: HealthConfig, default_cooldown_secs: int
    ) -> None:
        self._apply_decay(now, config)
        self._score = max(self._score - config.penalty_403, 0.0)
        self._total_observations += 1
        self._record_failure(now, config, default_cooldown_secs)
        self._last_observation = now

    def record_server_error(
        self, now: Timestamp, config: HealthConfig, default_cooldown_secs: int
    ) -> None:
        self._apply_decay(now, config)
        self._score = max(self._score - config.penalty_5xx, 0.0)
        self._total_observations += 1
        self._record_failure(now, config, default_cooldown_secs)
        self._last_observation = now

    def record_timeout(
        self, now: Timestamp, config: HealthConfig, default_cooldown_secs: int
    ) -> None:
        self._apply_decay(now, config)
        self._score = max(self._score - config.penalty_timeout, 0.0)
        self._total_observations += 1
        self._record_failure(now, config, default_cooldown_secs)
        self._last_observation = now

    def _apply_decay(self, now: Timestamp, config: HealthConfig) -> None:
        elapsed_secs = now.duration_since(self._last_observation) / 1_000_000_000.0
        if elapsed_secs <= 0.0 or config.decay_half_life_seconds <= 0.0:
            return
        decay_factor = 0.5 ** (elapsed_secs / config.decay_half_life_seconds)
        deficit = 1.0 - self._score
        self._score = 1.0 - deficit * decay_factor
        self._score = max(0.0, min(1.0, self._score))

    def _record_failure(
        self, now: Timestamp, config: HealthConfig, default_cooldown_secs: int
    ) -> None:
        self._consecutive_failures += 1
        if self._consecutive_failures >= config.cooldown_trigger_count:
            excess = self._consecutive_failures - config.cooldown_trigger_count
            multiplier = config.cooldown_multiplier ** excess
            cooldown_secs = int(default_cooldown_secs * multiplier)
            self._current_cooldown_secs = min(cooldown_secs, config.max_cooldown_seconds)
            self._cooldown_until = now.add_secs(self._current_cooldown_secs)

    def _update_latency(self, latency_ms: int) -> None:
        if self._total_observations <= 1:
            self._ewma_latency_ms = float(latency_ms)
        else:
            self._ewma_latency_ms = (
                _LATENCY_ALPHA * latency_ms + (1.0 - _LATENCY_ALPHA) * self._ewma_latency_ms
            )
