"""Tests for the main engine."""

from grate_limiter import (
    CapabilityConfig,
    CapabilityProvider,
    EngineConfig,
    GrateLimiter,
    MockClock,
    NoAvailableProviders,
    Observation,
    Outcome,
    ProviderConfig,
    UnknownCapability,
    UnknownProvider,
    Usage,
)
from grate_limiter.models import QuotaConfig
from grate_limiter.types import Dimension, StatusClass, Window


def setup_engine() -> tuple[GrateLimiter, MockClock]:
    clock = MockClock()
    config = EngineConfig().with_clock(clock)
    engine = GrateLimiter(config)

    engine.upsert_provider(ProviderConfig(
        name="openai",
        quotas=[QuotaConfig(dimension=Dimension.REQUESTS, limit=100, window=Window.MINUTE)],
        priority=10,
        weight=1.0,
        cooldown_seconds=30,
    ))
    engine.upsert_provider(ProviderConfig(
        name="anthropic",
        quotas=[QuotaConfig(dimension=Dimension.REQUESTS, limit=80, window=Window.MINUTE)],
        priority=8,
        weight=1.0,
        cooldown_seconds=30,
    ))
    engine.upsert_capability(CapabilityConfig(
        name="chat",
        providers=[
            CapabilityProvider(provider="openai", priority=10),
            CapabilityProvider(provider="anthropic", priority=8),
        ],
    ))
    return engine, clock


class TestEngine:
    def test_select_returns_best_provider(self) -> None:
        engine, _ = setup_engine()
        decision = engine.select("chat")
        assert decision.provider == "openai"
        assert decision.score > 0.0
        assert len(decision.alternatives) == 1

    def test_select_unknown_capability_errors(self) -> None:
        engine, _ = setup_engine()
        try:
            engine.select("nonexistent")
            assert False, "Should have raised"
        except UnknownCapability:
            pass

    def test_observe_updates_health(self) -> None:
        engine, _ = setup_engine()
        engine.observe(Observation(
            provider="openai",
            capability="chat",
            usage=Usage(requests=1),
            outcome=Outcome(status=StatusClass.RATE_LIMITED, latency_ms=100),
        ))
        health = engine.provider_health("openai")
        assert health is not None and health < 1.0

    def test_observe_unknown_provider_errors(self) -> None:
        engine, _ = setup_engine()
        try:
            engine.observe(Observation(
                provider="nonexistent",
                usage=Usage(),
                outcome=Outcome(status=StatusClass.SUCCESS, latency_ms=100),
            ))
            assert False, "Should have raised"
        except UnknownProvider:
            pass

    def test_degraded_provider_loses_to_healthy(self) -> None:
        engine, clock = setup_engine()
        for _ in range(3):
            clock.advance_ms(1000)
            engine.observe(Observation(
                provider="openai",
                capability="chat",
                usage=Usage(requests=1),
                outcome=Outcome(status=StatusClass.RATE_LIMITED, latency_ms=100),
            ))
        decision = engine.select("chat")
        assert decision.provider == "anthropic"

    def test_metrics_increment(self) -> None:
        engine, _ = setup_engine()
        engine.select("chat")
        engine.select("chat")
        assert engine.metrics.selects == 2

        engine.observe(Observation(
            provider="openai",
            usage=Usage(requests=1),
            outcome=Outcome(status=StatusClass.SUCCESS, latency_ms=50),
        ))
        assert engine.metrics.observations == 1

    def test_provider_quota_tracking(self) -> None:
        engine, _ = setup_engine()
        assert engine.provider_quota_remaining("openai", Dimension.REQUESTS) == 100

        engine.observe(Observation(
            provider="openai",
            usage=Usage(requests=30),
            outcome=Outcome(status=StatusClass.SUCCESS, latency_ms=100),
        ))
        remaining = engine.provider_quota_remaining("openai", Dimension.REQUESTS)
        assert remaining == 70

    def test_upsert_provider_preserves_health(self) -> None:
        engine, _ = setup_engine()
        engine.observe(Observation(
            provider="openai",
            usage=Usage(requests=1),
            outcome=Outcome(status=StatusClass.SERVER_ERROR, latency_ms=100),
        ))
        health_before = engine.provider_health("openai")

        # Re-upsert provider
        engine.upsert_provider(ProviderConfig(
            name="openai",
            quotas=[QuotaConfig(dimension=Dimension.REQUESTS, limit=200, window=Window.MINUTE)],
            priority=10,
            weight=1.0,
            cooldown_seconds=30,
        ))
        health_after = engine.provider_health("openai")
        assert health_before == health_after

    def test_all_providers_in_cooldown(self) -> None:
        engine, clock = setup_engine()
        for prov in ["openai", "anthropic"]:
            for _ in range(3):
                clock.advance_ms(1000)
                engine.observe(Observation(
                    provider=prov,
                    usage=Usage(requests=1),
                    outcome=Outcome(status=StatusClass.RATE_LIMITED, latency_ms=50),
                ))
        try:
            engine.select("chat")
            assert False, "Should have raised"
        except NoAvailableProviders:
            pass

    def test_provider_in_cooldown(self) -> None:
        engine, clock = setup_engine()
        assert engine.provider_in_cooldown("openai") is False

        for _ in range(3):
            clock.advance_ms(1000)
            engine.observe(Observation(
                provider="openai",
                usage=Usage(requests=1),
                outcome=Outcome(status=StatusClass.RATE_LIMITED, latency_ms=50),
            ))
        assert engine.provider_in_cooldown("openai") is True

    def test_cooldown_expires(self) -> None:
        engine, clock = setup_engine()
        for _ in range(3):
            clock.advance_ms(1000)
            engine.observe(Observation(
                provider="openai",
                usage=Usage(requests=1),
                outcome=Outcome(status=StatusClass.RATE_LIMITED, latency_ms=50),
            ))
        assert engine.provider_in_cooldown("openai") is True
        clock.advance_secs(31)
        assert engine.provider_in_cooldown("openai") is False

    def test_provider_health_returns_none_for_unknown(self) -> None:
        engine, _ = setup_engine()
        assert engine.provider_health("nonexistent") is None
        assert engine.provider_in_cooldown("nonexistent") is None
        assert engine.provider_quota_remaining("nonexistent", Dimension.REQUESTS) is None
