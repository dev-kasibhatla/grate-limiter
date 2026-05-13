"""Conformance test runner — loads JSON test vectors and validates behavioral parity."""

from __future__ import annotations

import json
import math
from pathlib import Path

import pytest

from grate_limiter import (
    CapabilityConfig,
    CapabilityProvider,
    EngineConfig,
    GrateLimiter,
    MockClock,
    Observation,
    Outcome,
    ProviderConfig,
    Usage,
)
from grate_limiter.config import HealthConfig, ScoringWeights
from grate_limiter.models import QuotaConfig
from grate_limiter.types import Dimension, StatusClass, Window

CONFORMANCE_DIR = Path(__file__).resolve().parent.parent.parent / "tests" / "conformance"

_DIMENSION_MAP = {
    "requests": Dimension.REQUESTS,
    "tokens": Dimension.TOKENS,
    "concurrency": Dimension.CONCURRENCY,
    "cost_usd": Dimension.COST_USD,
    "bytes": Dimension.BYTES,
}

_WINDOW_MAP = {
    "second": Window.SECOND,
    "minute": Window.MINUTE,
    "hour": Window.HOUR,
    "day": Window.DAY,
}

_STATUS_MAP = {
    "success": StatusClass.SUCCESS,
    "rate_limited": StatusClass.RATE_LIMITED,
    "forbidden": StatusClass.FORBIDDEN,
    "server_error": StatusClass.SERVER_ERROR,
    "timeout": StatusClass.TIMEOUT,
    "client_error": StatusClass.CLIENT_ERROR,
}


def _load_test_files() -> list[tuple[str, dict]]:  # type: ignore[type-arg]
    results = []
    for path in sorted(CONFORMANCE_DIR.glob("*.json")):
        with open(path) as f:
            data = json.load(f)
        results.append((data["name"], data))
    return results


def _build_engine(data: dict) -> tuple[GrateLimiter, MockClock]:  # type: ignore[type-arg]
    cfg = data.get("config", {})
    scoring_data = cfg.get("scoring", {})
    health_data = cfg.get("health", {})

    clock = MockClock()
    config = EngineConfig(
        scoring=ScoringWeights(
            quota=scoring_data.get("quota", 0.40),
            health=scoring_data.get("health", 0.35),
            priority=scoring_data.get("priority", 0.20),
            latency=scoring_data.get("latency", 0.05),
        ),
        health=HealthConfig(
            decay_half_life_seconds=health_data.get("decay_half_life_seconds", 300.0),
            penalty_429=health_data.get("penalty_429", 0.25),
            penalty_403=health_data.get("penalty_403", 0.50),
            penalty_5xx=health_data.get("penalty_5xx", 0.10),
            penalty_timeout=health_data.get("penalty_timeout", 0.20),
            boost_success=health_data.get("boost_success", 0.02),
            cooldown_trigger_count=health_data.get("cooldown_trigger_count", 3),
            cooldown_multiplier=health_data.get("cooldown_multiplier", 2.0),
            max_cooldown_seconds=health_data.get("max_cooldown_seconds", 600),
        ),
        minimum_health_score=cfg.get("minimum_health_score", 0.2),
        default_cooldown_seconds=cfg.get("default_cooldown_seconds", 60),
        clock=clock,
    )
    engine = GrateLimiter(config)

    for prov in data.get("providers", []):
        quotas = []
        for q in prov.get("quotas", []):
            window = _WINDOW_MAP.get(q["window"]) if "window" in q else None
            quotas.append(QuotaConfig(
                dimension=_DIMENSION_MAP[q["dimension"]],
                limit=q["limit"],
                window=window,
            ))
        engine.upsert_provider(ProviderConfig(
            name=prov["name"],
            quotas=quotas,
            priority=prov.get("priority", 10),
            weight=prov.get("weight", 1.0),
            cooldown_seconds=prov.get("cooldown_seconds", 60),
        ))

    for cap in data.get("capabilities", []):
        providers = [
            CapabilityProvider(provider=cp["provider"], priority=cp["priority"])
            for cp in cap.get("providers", [])
        ]
        engine.upsert_capability(CapabilityConfig(name=cap["name"], providers=providers))

    return engine, clock


def _run_steps(engine: GrateLimiter, clock: MockClock, steps: list[dict]) -> None:  # type: ignore[type-arg]
    for i, step in enumerate(steps):
        action = step["action"]
        desc = f"step {i}: {action}"

        if action == "advance_time_ms":
            clock.advance_ms(step["ms"])

        elif action == "observe":
            status = _STATUS_MAP[step["status"]]
            engine.observe(Observation(
                provider=step["provider"],
                capability=step.get("capability"),
                usage=Usage(
                    requests=step.get("requests", 0),
                    tokens=step.get("tokens"),
                    bytes=step.get("bytes"),
                ),
                outcome=Outcome(status=status, latency_ms=step.get("latency_ms", 0)),
            ))

        elif action == "select":
            decision = engine.select(step["capability"])
            if "expect_provider" in step:
                assert decision.provider == step["expect_provider"], (
                    f"{desc}: expected provider={step['expect_provider']}, "
                    f"got={decision.provider}"
                )
            if "expect_score_min" in step:
                assert decision.score >= step["expect_score_min"] - 0.001, (
                    f"{desc}: score={decision.score} < min={step['expect_score_min']}"
                )
            if "expect_score_max" in step:
                assert decision.score <= step["expect_score_max"] + 0.001, (
                    f"{desc}: score={decision.score} > max={step['expect_score_max']}"
                )
            if "expect_alternatives_count" in step:
                assert len(decision.alternatives) == step["expect_alternatives_count"], (
                    f"{desc}: alternatives count={len(decision.alternatives)} "
                    f"expected={step['expect_alternatives_count']}"
                )

        elif action == "check_health":
            health = engine.provider_health(step["provider"])
            assert health is not None, f"{desc}: provider not found"
            if "expect_min" in step:
                assert health >= step["expect_min"] - 0.001, (
                    f"{desc}: health={health} < min={step['expect_min']}"
                )
            if "expect_max" in step:
                assert health <= step["expect_max"] + 0.001, (
                    f"{desc}: health={health} > max={step['expect_max']}"
                )

        elif action == "check_remaining":
            dim = _DIMENSION_MAP[step["dimension"]]
            remaining = engine.provider_quota_remaining(step["provider"], dim)
            assert remaining is not None, f"{desc}: provider/dimension not found"
            assert remaining == step["expect"], (
                f"{desc}: remaining={remaining} expected={step['expect']}"
            )

        elif action == "check_in_cooldown":
            in_cooldown = engine.provider_in_cooldown(step["provider"])
            assert in_cooldown is not None, f"{desc}: provider not found"
            assert in_cooldown == step["expect"], (
                f"{desc}: in_cooldown={in_cooldown} expected={step['expect']}"
            )

        else:
            raise ValueError(f"Unknown action: {action}")


_TEST_CASES = _load_test_files()


@pytest.mark.parametrize("name,data", _TEST_CASES, ids=[t[0] for t in _TEST_CASES])
def test_conformance(name: str, data: dict) -> None:  # type: ignore[type-arg]
    engine, clock = _build_engine(data)
    _run_steps(engine, clock, data["steps"])
