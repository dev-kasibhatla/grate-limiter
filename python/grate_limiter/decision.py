"""Decision types returned by the engine."""

from __future__ import annotations

from dataclasses import dataclass, field


@dataclass
class ScoreBreakdown:
    """Detailed breakdown of how a provider was scored."""

    quota_score: float = 0.0
    health_score: float = 0.0
    priority_score: float = 0.0
    latency_score: float = 0.0


@dataclass
class Alternative:
    """An alternative provider candidate with its score."""

    provider: str
    score: float


@dataclass
class Decision:
    """The result of a provider selection decision."""

    provider: str
    score: float
    reasoning: ScoreBreakdown
    alternatives: list[Alternative] = field(default_factory=list)
