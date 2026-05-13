"""Tests for scoring engine."""

import math

from grate_limiter.config import ScoringWeights
from grate_limiter.scoring import ProviderScoreContext, WeightedScorer


def default_ctx() -> ProviderScoreContext:
    return ProviderScoreContext(
        quota_remaining_ratio=1.0,
        predicted_exhaustion_secs=math.inf,
        burn_rate=0.0,
        health_score=1.0,
        priority=10,
        max_priority=10,
        latency_ms=100.0,
        max_latency_ms=200.0,
    )


class TestWeightedScorer:
    def test_perfect_provider_scores_high(self) -> None:
        scorer = WeightedScorer(ScoringWeights())
        score = scorer.score(default_ctx())
        assert score > 0.9, f"score={score}"

    def test_exhausted_provider_scores_low(self) -> None:
        scorer = WeightedScorer(ScoringWeights())
        ctx = default_ctx()
        ctx.quota_remaining_ratio = 0.05
        ctx.predicted_exhaustion_secs = 5.0
        ctx.health_score = 0.5
        score = scorer.score(ctx)
        assert score < 0.5, f"score={score}"

    def test_unhealthy_provider_scores_low(self) -> None:
        scorer = WeightedScorer(ScoringWeights())
        ctx = default_ctx()
        ctx.health_score = 0.2
        score = scorer.score(ctx)
        assert score < 0.8, f"score={score}"

    def test_low_priority_scores_lower(self) -> None:
        scorer = WeightedScorer(ScoringWeights())
        high = scorer.score(default_ctx())
        ctx = default_ctx()
        ctx.priority = 2
        low = scorer.score(ctx)
        assert high > low

    def test_anticipatory_penalty_kicks_in(self) -> None:
        scorer = WeightedScorer(ScoringWeights())

        fast_burn = default_ctx()
        fast_burn.quota_remaining_ratio = 0.3
        fast_burn.predicted_exhaustion_secs = 20.0
        fast_burn.burn_rate = 50.0

        slow_burn = default_ctx()
        slow_burn.quota_remaining_ratio = 0.3
        slow_burn.predicted_exhaustion_secs = 300.0
        slow_burn.burn_rate = 1.0

        fast_score = scorer.score(fast_burn)
        slow_score = scorer.score(slow_burn)
        assert slow_score > fast_score, f"slow={slow_score} fast={fast_score}"

    def test_score_always_bounded(self) -> None:
        scorer = WeightedScorer(ScoringWeights())

        # Worst case
        ctx = ProviderScoreContext(
            quota_remaining_ratio=0.0,
            predicted_exhaustion_secs=0.0,
            burn_rate=1000.0,
            health_score=0.0,
            priority=0,
            max_priority=10,
            latency_ms=1000.0,
            max_latency_ms=1000.0,
        )
        score = scorer.score(ctx)
        assert 0.0 <= score <= 1.0

        # Best case
        ctx2 = default_ctx()
        score2 = scorer.score(ctx2)
        assert 0.0 <= score2 <= 1.0
