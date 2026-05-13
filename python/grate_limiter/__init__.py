"""grate-limiter: Anticipatory rate-limit orchestration engine."""

from grate_limiter.clock import Clock, MockClock, RealClock, Timestamp
from grate_limiter.config import EngineConfig, HealthConfig, ScoringWeights
from grate_limiter.decision import Alternative, Decision, ScoreBreakdown
from grate_limiter.engine import GrateLimiter
from grate_limiter.errors import (
    GrateLimiterError,
    NoAvailableProviders,
    UnknownCapability,
    UnknownProvider,
)
from grate_limiter.models import (
    CapabilityConfig,
    CapabilityProvider,
    Observation,
    Outcome,
    ProviderConfig,
    Usage,
)
from grate_limiter.types import Dimension, StatusClass, Window

__all__ = [
    "Alternative",
    "CapabilityConfig",
    "CapabilityProvider",
    "Clock",
    "Decision",
    "Dimension",
    "EngineConfig",
    "GrateLimiter",
    "GrateLimiterError",
    "HealthConfig",
    "MockClock",
    "NoAvailableProviders",
    "Observation",
    "Outcome",
    "ProviderConfig",
    "RealClock",
    "ScoreBreakdown",
    "ScoringWeights",
    "StatusClass",
    "Timestamp",
    "UnknownCapability",
    "UnknownProvider",
    "Usage",
    "Window",
]

__version__ = "0.1.0"
