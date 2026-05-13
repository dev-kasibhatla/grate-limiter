import { describe, expect, it } from "vitest";
import { defaultScoringWeights } from "../src/config.js";
import type { ProviderScoreContext } from "../src/scoring.js";
import { WeightedScorer } from "../src/scoring.js";

function defaultCtx(): ProviderScoreContext {
  return {
    quotaRemainingRatio: 1.0,
    predictedExhaustionSecs: Infinity,
    burnRate: 0,
    healthScore: 1.0,
    priority: 10,
    maxPriority: 10,
    latencyMs: 100,
    maxLatencyMs: 200,
  };
}

describe("WeightedScorer", () => {
  it("perfect provider scores high", () => {
    const scorer = new WeightedScorer(defaultScoringWeights());
    const score = scorer.score(defaultCtx());
    expect(score).toBeGreaterThan(0.9);
  });

  it("exhausted provider scores low", () => {
    const scorer = new WeightedScorer(defaultScoringWeights());
    const ctx: ProviderScoreContext = {
      ...defaultCtx(),
      quotaRemainingRatio: 0.05,
      predictedExhaustionSecs: 5.0,
      healthScore: 0.5,
    };
    expect(scorer.score(ctx)).toBeLessThan(0.5);
  });

  it("unhealthy provider scores low", () => {
    const scorer = new WeightedScorer(defaultScoringWeights());
    const ctx = { ...defaultCtx(), healthScore: 0.2 };
    expect(scorer.score(ctx)).toBeLessThan(0.8);
  });

  it("low priority scores lower", () => {
    const scorer = new WeightedScorer(defaultScoringWeights());
    const high = scorer.score(defaultCtx());
    const low = scorer.score({ ...defaultCtx(), priority: 2 });
    expect(high).toBeGreaterThan(low);
  });

  it("anticipatory penalty kicks in", () => {
    const scorer = new WeightedScorer(defaultScoringWeights());
    const fastBurn: ProviderScoreContext = {
      ...defaultCtx(),
      quotaRemainingRatio: 0.3,
      predictedExhaustionSecs: 20.0,
      burnRate: 50,
    };
    const slowBurn: ProviderScoreContext = {
      ...defaultCtx(),
      quotaRemainingRatio: 0.3,
      predictedExhaustionSecs: 300.0,
      burnRate: 1.0,
    };
    expect(scorer.score(slowBurn)).toBeGreaterThan(scorer.score(fastBurn));
  });

  it("score always bounded", () => {
    const scorer = new WeightedScorer(defaultScoringWeights());
    const worst: ProviderScoreContext = {
      quotaRemainingRatio: 0,
      predictedExhaustionSecs: 0,
      burnRate: 1000,
      healthScore: 0,
      priority: 0,
      maxPriority: 10,
      latencyMs: 1000,
      maxLatencyMs: 1000,
    };
    const score = scorer.score(worst);
    expect(score).toBeGreaterThanOrEqual(0);
    expect(score).toBeLessThanOrEqual(1);
  });
});
