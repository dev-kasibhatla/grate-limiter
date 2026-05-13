import type { ScoringWeights } from "./config.js";

/** Context provided to the scoring strategy for a single provider. */
export interface ProviderScoreContext {
  quotaRemainingRatio: number;
  predictedExhaustionSecs: number;
  burnRate: number;
  healthScore: number;
  priority: number;
  maxPriority: number;
  latencyMs: number;
  maxLatencyMs: number;
}

/** Default weighted composite scorer. */
export class WeightedScorer {
  constructor(private readonly weights: ScoringWeights) {}

  score(ctx: ProviderScoreContext): number {
    const qs = WeightedScorer._quotaScore(ctx);
    const hs = ctx.healthScore;
    const ps = WeightedScorer._priorityScore(ctx);
    const ls = WeightedScorer._latencyScore(ctx);

    const final =
      qs * this.weights.quota +
      hs * this.weights.health +
      ps * this.weights.priority +
      ls * this.weights.latency;

    return Math.max(0.0, Math.min(1.0, final));
  }

  private static _quotaScore(ctx: ProviderScoreContext): number {
    const base = ctx.quotaRemainingRatio;

    let exhaustionPenalty: number;
    if (ctx.predictedExhaustionSecs < 10) exhaustionPenalty = 0.8;
    else if (ctx.predictedExhaustionSecs < 30) exhaustionPenalty = 0.5;
    else if (ctx.predictedExhaustionSecs < 60) exhaustionPenalty = 0.3;
    else if (ctx.predictedExhaustionSecs < 120) exhaustionPenalty = 0.1;
    else exhaustionPenalty = 0;

    const burnPenalty = ctx.burnRate > 0 && ctx.quotaRemainingRatio < 0.5 ? 0.1 : 0;

    return Math.max(0, base - exhaustionPenalty - burnPenalty);
  }

  private static _priorityScore(ctx: ProviderScoreContext): number {
    if (ctx.maxPriority === 0) return 0.5;
    return ctx.priority / ctx.maxPriority;
  }

  private static _latencyScore(ctx: ProviderScoreContext): number {
    if (ctx.maxLatencyMs <= 0 || ctx.latencyMs <= 0) return 1.0;
    return Math.max(0, 1.0 - ctx.latencyMs / ctx.maxLatencyMs);
  }
}
