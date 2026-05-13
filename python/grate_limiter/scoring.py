"""Scoring engine — weighted composite scoring with anticipatory penalties."""

from __future__ import annotations

from dataclasses import dataclass

from grate_limiter.config import ScoringWeights


@dataclass
class ProviderScoreContext:
    """Context provided to the scoring strategy for a single provider."""

    quota_remaining_ratio: float
    predicted_exhaustion_secs: float
    burn_rate: float
    health_score: float
    priority: int
    max_priority: int
    latency_ms: float
    max_latency_ms: float


class WeightedScorer:
    """Default weighted composite scorer."""

    def __init__(self, weights: ScoringWeights) -> None:
        self.weights = weights

    def score(self, ctx: ProviderScoreContext) -> float:
        qs = self._quota_score(ctx)
        hs = ctx.health_score
        ps = self._priority_score(ctx)
        ls = self._latency_score(ctx)

        final = (
            qs * self.weights.quota
            + hs * self.weights.health
            + ps * self.weights.priority
            + ls * self.weights.latency
        )
        return max(0.0, min(1.0, final))

    @staticmethod
    def _quota_score(ctx: ProviderScoreContext) -> float:
        base = ctx.quota_remaining_ratio

        if ctx.predicted_exhaustion_secs < 10.0:
            exhaustion_penalty = 0.8
        elif ctx.predicted_exhaustion_secs < 30.0:
            exhaustion_penalty = 0.5
        elif ctx.predicted_exhaustion_secs < 60.0:
            exhaustion_penalty = 0.3
        elif ctx.predicted_exhaustion_secs < 120.0:
            exhaustion_penalty = 0.1
        else:
            exhaustion_penalty = 0.0

        burn_penalty = 0.1 if (ctx.burn_rate > 0.0 and ctx.quota_remaining_ratio < 0.5) else 0.0

        return max(0.0, base - exhaustion_penalty - burn_penalty)

    @staticmethod
    def _priority_score(ctx: ProviderScoreContext) -> float:
        if ctx.max_priority == 0:
            return 0.5
        return ctx.priority / ctx.max_priority

    @staticmethod
    def _latency_score(ctx: ProviderScoreContext) -> float:
        if ctx.max_latency_ms <= 0.0 or ctx.latency_ms <= 0.0:
            return 1.0
        return max(0.0, 1.0 - (ctx.latency_ms / ctx.max_latency_ms))
