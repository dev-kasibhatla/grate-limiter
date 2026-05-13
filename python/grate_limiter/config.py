"""Configuration types."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

from grate_limiter.clock import Clock


@dataclass
class ScoringWeights:
    """Weights for the composite scoring algorithm. Should sum to 1.0."""

    quota: float = 0.40
    health: float = 0.35
    priority: float = 0.20
    latency: float = 0.05


@dataclass
class HealthConfig:
    """Health engine configuration."""

    decay_half_life_seconds: float = 300.0
    penalty_429: float = 0.25
    penalty_403: float = 0.50
    penalty_5xx: float = 0.10
    penalty_timeout: float = 0.20
    boost_success: float = 0.02
    cooldown_trigger_count: int = 3
    cooldown_multiplier: float = 2.0
    max_cooldown_seconds: int = 600


@dataclass
class EngineConfig:
    """Top-level engine configuration."""

    scoring: ScoringWeights = field(default_factory=ScoringWeights)
    health: HealthConfig = field(default_factory=HealthConfig)
    minimum_health_score: float = 0.2
    default_cooldown_seconds: int = 60
    clock: Clock | None = None

    def with_clock(self, clock: Any) -> EngineConfig:
        self.clock = clock
        return self
