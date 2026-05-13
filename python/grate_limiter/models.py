"""Data models for providers, capabilities, observations."""

from __future__ import annotations

from dataclasses import dataclass, field

from grate_limiter.types import Dimension, StatusClass, Window


@dataclass
class QuotaConfig:
    """Configuration for a single quota dimension on a provider."""

    dimension: Dimension
    limit: int
    window: Window | None = None


@dataclass
class ProviderConfig:
    """Configuration for a provider."""

    name: str
    quotas: list[QuotaConfig] = field(default_factory=list)
    priority: int = 10
    weight: float = 1.0
    cooldown_seconds: int = 60


@dataclass
class CapabilityProvider:
    """A provider registered under a capability with its priority."""

    provider: str
    priority: int


@dataclass
class CapabilityConfig:
    """Configuration for a capability."""

    name: str
    providers: list[CapabilityProvider] = field(default_factory=list)


@dataclass
class Usage:
    """Resource usage for a single interaction."""

    requests: int = 0
    tokens: int | None = None
    bytes: int | None = None
    cost_micro_usd: int | None = None


@dataclass
class Outcome:
    """Outcome of a provider interaction."""

    status: StatusClass = StatusClass.SUCCESS
    latency_ms: int = 0


@dataclass
class Observation:
    """An observation reported after a provider interaction."""

    provider: str
    capability: str | None = None
    usage: Usage = field(default_factory=Usage)
    outcome: Outcome = field(default_factory=Outcome)
