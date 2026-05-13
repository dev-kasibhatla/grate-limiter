"""The main grate-limiter engine."""

from __future__ import annotations

import math
import threading

from grate_limiter.clock import Clock, RealClock, Timestamp
from grate_limiter.config import EngineConfig
from grate_limiter.decision import Alternative, Decision, ScoreBreakdown
from grate_limiter.errors import NoAvailableProviders, UnknownCapability, UnknownProvider
from grate_limiter.health import HealthState
from grate_limiter.metrics import Metrics
from grate_limiter.models import (
    CapabilityConfig,
    CapabilityProvider,
    Observation,
    ProviderConfig,
    QuotaConfig,
)
from grate_limiter.quota import QuotaTracker, create_tracker
from grate_limiter.scoring import ProviderScoreContext, WeightedScorer
from grate_limiter.types import Dimension, StatusClass


class _ProviderRuntime:
    __slots__ = ("config", "health", "quota_trackers", "lock")

    def __init__(
        self,
        config: ProviderConfig,
        health: HealthState,
        trackers: list[tuple[QuotaConfig, QuotaTracker]],
    ) -> None:
        self.config = config
        self.health = health
        self.quota_trackers = trackers
        self.lock = threading.Lock()


class _CapabilityDef:
    __slots__ = ("providers",)

    def __init__(self, providers: list[CapabilityProvider]) -> None:
        self.providers = providers


class GrateLimiter:
    """The main grate-limiter engine. Thread-safe.

    Example::

        from grate_limiter import GrateLimiter, EngineConfig
        engine = GrateLimiter(EngineConfig())
    """

    def __init__(self, config: EngineConfig | None = None) -> None:
        if config is None:
            config = EngineConfig()
        self._config = config
        self._clock: Clock = config.clock if config.clock is not None else RealClock()
        self._scorer = WeightedScorer(config.scoring)
        self._providers: dict[str, _ProviderRuntime] = {}
        self._capabilities: dict[str, _CapabilityDef] = {}
        self._metrics = Metrics()
        self._prov_lock = threading.Lock()
        self._cap_lock = threading.Lock()

    def upsert_provider(self, config: ProviderConfig) -> None:
        """Register or update a provider and its quotas."""
        now = self._clock.now()
        trackers: list[tuple[QuotaConfig, QuotaTracker]] = [
            (qc, create_tracker(qc, now)) for qc in config.quotas
        ]

        with self._prov_lock:
            existing = self._providers.get(config.name)
            if existing is not None:
                with existing.lock:
                    existing.config = config
                    existing.quota_trackers = trackers
            else:
                self._providers[config.name] = _ProviderRuntime(
                    config=config,
                    health=HealthState(now),
                    trackers=trackers,
                )

    def upsert_capability(self, config: CapabilityConfig) -> None:
        """Register or update a capability and its provider mappings."""
        with self._cap_lock:
            self._capabilities[config.name] = _CapabilityDef(
                providers=list(config.providers),
            )

    def select(self, capability: str) -> Decision:
        """Select the best provider for a capability.

        Raises:
            UnknownCapability: If the capability is not registered.
            NoAvailableProviders: If all providers are in cooldown or below minimum health.
        """
        self._metrics._inc_selects()
        now = self._clock.now()

        with self._cap_lock:
            cap_def = self._capabilities.get(capability)
            if cap_def is None:
                raise UnknownCapability(capability)
            cap_providers = list(cap_def.providers)

        if not cap_providers:
            raise NoAvailableProviders(capability)

        max_priority = max(cp.priority for cp in cap_providers)

        # Find max latency for normalization
        max_latency_ms = 0.0
        with self._prov_lock:
            for cp in cap_providers:
                pr = self._providers.get(cp.provider)
                if pr is not None:
                    with pr.lock:
                        lat = pr.health.latency_ms
                    if lat > max_latency_ms:
                        max_latency_ms = lat
        if max_latency_ms <= 0.0:
            max_latency_ms = 1.0

        # Score all providers
        candidates: list[tuple[str, float, ScoreBreakdown]] = []

        with self._prov_lock:
            providers_snapshot = dict(self._providers)

        for cp in cap_providers:
            pr = providers_snapshot.get(cp.provider)
            if pr is None:
                continue

            with pr.lock:
                if pr.health.is_in_cooldown(now):
                    continue
                if pr.health.score < self._config.minimum_health_score:
                    continue

                quota_remaining_ratio, predicted_exhaustion, burn_rate = self._worst_quota_state(
                    pr.quota_trackers, now
                )

                ctx = ProviderScoreContext(
                    quota_remaining_ratio=quota_remaining_ratio,
                    predicted_exhaustion_secs=predicted_exhaustion,
                    burn_rate=burn_rate,
                    health_score=pr.health.score,
                    priority=cp.priority,
                    max_priority=max_priority,
                    latency_ms=pr.health.latency_ms,
                    max_latency_ms=max_latency_ms,
                )

            score = self._scorer.score(ctx)
            breakdown = ScoreBreakdown(
                quota_score=ctx.quota_remaining_ratio,
                health_score=ctx.health_score,
                priority_score=cp.priority / max_priority if max_priority > 0 else 0.5,
                latency_score=max(0.0, 1.0 - (ctx.latency_ms / max_latency_ms))
                if max_latency_ms > 0.0
                else 1.0,
            )
            candidates.append((cp.provider, score, breakdown))

        if not candidates:
            self._metrics._inc_no_provider()
            raise NoAvailableProviders(capability)

        # Sort by score descending
        candidates.sort(key=lambda c: c[1], reverse=True)

        provider, score, reasoning = candidates[0]
        alternatives = [Alternative(provider=p, score=s) for p, s, _ in candidates[1:]]

        return Decision(
            provider=provider,
            score=score,
            reasoning=reasoning,
            alternatives=alternatives,
        )

    def observe(self, obs: Observation) -> None:
        """Report an observation after a provider interaction.

        Raises:
            UnknownProvider: If the provider is not registered.
        """
        self._metrics._inc_observations()
        now = self._clock.now()

        with self._prov_lock:
            pr = self._providers.get(obs.provider)
        if pr is None:
            raise UnknownProvider(obs.provider)

        with pr.lock:
            # Update quota trackers
            for qc, tracker in pr.quota_trackers:
                amount = self._usage_for_dimension(qc.dimension, obs)
                if amount > 0:
                    tracker.record(amount, now)

            # Update health
            cooldown_secs = pr.config.cooldown_seconds
            health_config = self._config.health
            was_in_cooldown = pr.health.is_in_cooldown(now)

            if obs.outcome.status in (StatusClass.SUCCESS, StatusClass.CLIENT_ERROR):
                pr.health.record_success(obs.outcome.latency_ms, now, health_config)
            elif obs.outcome.status == StatusClass.RATE_LIMITED:
                pr.health.record_rate_limited(now, health_config, cooldown_secs)
            elif obs.outcome.status == StatusClass.FORBIDDEN:
                pr.health.record_forbidden(now, health_config, cooldown_secs)
            elif obs.outcome.status == StatusClass.SERVER_ERROR:
                pr.health.record_server_error(now, health_config, cooldown_secs)
            elif obs.outcome.status == StatusClass.TIMEOUT:
                pr.health.record_timeout(now, health_config, cooldown_secs)

            if not was_in_cooldown and pr.health.is_in_cooldown(now):
                self._metrics._inc_cooldowns()

    @property
    def metrics(self) -> Metrics:
        return self._metrics

    def provider_health(self, provider: str) -> float | None:
        """Get the current health score for a provider."""
        with self._prov_lock:
            pr = self._providers.get(provider)
        if pr is None:
            return None
        with pr.lock:
            return pr.health.score

    def provider_in_cooldown(self, provider: str) -> bool | None:
        """Check if a provider is currently in cooldown."""
        now = self._clock.now()
        with self._prov_lock:
            pr = self._providers.get(provider)
        if pr is None:
            return None
        with pr.lock:
            return pr.health.is_in_cooldown(now)

    def provider_quota_remaining(self, provider: str, dimension: Dimension) -> int | None:
        """Get the remaining quota for a specific dimension on a provider."""
        now = self._clock.now()
        with self._prov_lock:
            pr = self._providers.get(provider)
        if pr is None:
            return None
        with pr.lock:
            for qc, tracker in pr.quota_trackers:
                if qc.dimension == dimension:
                    return tracker.remaining(now)
        return None

    @staticmethod
    def _worst_quota_state(
        trackers: list[tuple[QuotaConfig, QuotaTracker]], now: Timestamp
    ) -> tuple[float, float, float]:
        if not trackers:
            return (1.0, math.inf, 0.0)

        worst_remaining = 1.0
        worst_exhaustion = math.inf
        max_burn_rate = 0.0

        for _qc, tracker in trackers:
            remaining = 1.0 - tracker.usage_ratio(now)
            exhaustion = tracker.predicted_exhaustion_secs(now)
            burn = tracker.burn_rate(now)

            if remaining < worst_remaining:
                worst_remaining = remaining
            if exhaustion < worst_exhaustion:
                worst_exhaustion = exhaustion
            if burn > max_burn_rate:
                max_burn_rate = burn

        return (worst_remaining, worst_exhaustion, max_burn_rate)

    @staticmethod
    def _usage_for_dimension(dimension: Dimension, obs: Observation) -> int:
        if dimension == Dimension.REQUESTS:
            return obs.usage.requests
        if dimension == Dimension.TOKENS:
            return obs.usage.tokens or 0
        if dimension == Dimension.BYTES:
            return obs.usage.bytes or 0
        if dimension == Dimension.COST_USD:
            return obs.usage.cost_micro_usd or 0
        if dimension == Dimension.CONCURRENCY:
            return obs.usage.requests
        return 0
