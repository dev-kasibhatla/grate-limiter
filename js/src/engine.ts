import type { Clock } from "./clock.js";
import { RealClock } from "./clock.js";
import type { EngineConfig, ResolvedEngineConfig } from "./config.js";
import { resolveEngineConfig } from "./config.js";
import { NoAvailableProviders, UnknownCapability, UnknownProvider } from "./errors.js";
import { HealthState } from "./health.js";
import { Metrics } from "./metrics.js";
import type {
  Alternative,
  CapabilityConfig,
  CapabilityProvider,
  Decision,
  Observation,
  ProviderConfig,
  QuotaConfig,
  ScoreBreakdown,
} from "./models.js";
import { createTracker } from "./quota.js";
import type { QuotaTracker } from "./quota.js";
import type { ProviderScoreContext } from "./scoring.js";
import { WeightedScorer } from "./scoring.js";
import { Dimension, StatusClass } from "./types.js";

interface ProviderRuntime {
  config: ProviderConfig;
  health: HealthState;
  quotaTrackers: Array<{ config: QuotaConfig; tracker: QuotaTracker }>;
}

interface CapabilityDef {
  providers: CapabilityProvider[];
}

/**
 * The main grate-limiter engine.
 *
 * @example
 * ```ts
 * import { GrateLimiter } from '@dev-kasibhatla/grate-limiter';
 * const engine = new GrateLimiter();
 * ```
 */
export class GrateLimiter {
  private readonly _config: ResolvedEngineConfig;
  private readonly _clock: Clock;
  private readonly _scorer: WeightedScorer;
  private readonly _providers = new Map<string, ProviderRuntime>();
  private readonly _capabilities = new Map<string, CapabilityDef>();
  private readonly _metrics = new Metrics();

  constructor(config?: EngineConfig) {
    this._config = resolveEngineConfig(config);
    this._clock = this._config.clock ?? new RealClock();
    this._scorer = new WeightedScorer(this._config.scoring);
  }

  /** Register or update a provider and its quotas. */
  upsertProvider(config: ProviderConfig): void {
    const now = this._clock.now();
    const trackers = config.quotas.map((qc) => ({
      config: qc,
      tracker: createTracker(qc, now),
    }));

    const existing = this._providers.get(config.name);
    if (existing) {
      existing.config = config;
      existing.quotaTrackers = trackers;
    } else {
      this._providers.set(config.name, {
        config,
        health: new HealthState(now),
        quotaTrackers: trackers,
      });
    }
  }

  /** Register or update a capability and its provider mappings. */
  upsertCapability(config: CapabilityConfig): void {
    this._capabilities.set(config.name, {
      providers: [...config.providers],
    });
  }

  /**
   * Select the best provider for a capability.
   * @throws {UnknownCapability} If the capability is not registered.
   * @throws {NoAvailableProviders} If all providers are in cooldown or below minimum health.
   */
  select(capability: string): Decision {
    this._metrics.incSelects();
    const now = this._clock.now();

    const capDef = this._capabilities.get(capability);
    if (!capDef) throw new UnknownCapability(capability);

    const capProviders = capDef.providers;
    if (capProviders.length === 0) throw new NoAvailableProviders(capability);

    const maxPriority = Math.max(...capProviders.map((p) => p.priority));

    let maxLatencyMs = 0;
    for (const cp of capProviders) {
      const pr = this._providers.get(cp.provider);
      if (pr && pr.health.latencyMs > maxLatencyMs) {
        maxLatencyMs = pr.health.latencyMs;
      }
    }
    if (maxLatencyMs <= 0) maxLatencyMs = 1;

    const candidates: Array<{ provider: string; score: number; breakdown: ScoreBreakdown }> = [];

    for (const cp of capProviders) {
      const pr = this._providers.get(cp.provider);
      if (!pr) continue;

      if (pr.health.isInCooldown(now)) continue;
      if (pr.health.score < this._config.minimumHealthScore) continue;

      const [quotaRemainingRatio, predictedExhaustion, burnRate] = this._worstQuotaState(
        pr.quotaTrackers,
        now,
      );

      const ctx: ProviderScoreContext = {
        quotaRemainingRatio,
        predictedExhaustionSecs: predictedExhaustion,
        burnRate,
        healthScore: pr.health.score,
        priority: cp.priority,
        maxPriority,
        latencyMs: pr.health.latencyMs,
        maxLatencyMs,
      };

      const score = this._scorer.score(ctx);
      const breakdown: ScoreBreakdown = {
        quotaScore: ctx.quotaRemainingRatio,
        healthScore: ctx.healthScore,
        priorityScore: maxPriority > 0 ? cp.priority / maxPriority : 0.5,
        latencyScore: maxLatencyMs > 0 ? Math.max(0, 1 - ctx.latencyMs / maxLatencyMs) : 1,
      };

      candidates.push({ provider: cp.provider, score, breakdown });
    }

    if (candidates.length === 0) {
      this._metrics.incNoProvider();
      throw new NoAvailableProviders(capability);
    }

    candidates.sort((a, b) => b.score - a.score);

    const best = candidates[0]!;
    const alternatives: Alternative[] = candidates.slice(1).map((c) => ({
      provider: c.provider,
      score: c.score,
    }));

    return {
      provider: best.provider,
      score: best.score,
      reasoning: best.breakdown,
      alternatives,
    };
  }

  /**
   * Report an observation after a provider interaction.
   * @throws {UnknownProvider} If the provider is not registered.
   */
  observe(obs: Observation): void {
    this._metrics.incObservations();
    const now = this._clock.now();

    const pr = this._providers.get(obs.provider);
    if (!pr) throw new UnknownProvider(obs.provider);

    // Update quota trackers
    for (const { config: qc, tracker } of pr.quotaTrackers) {
      const amount = this._usageForDimension(qc.dimension, obs);
      if (amount > 0) tracker.record(amount, now);
    }

    // Update health
    const cooldownSecs = pr.config.cooldownSeconds;
    const healthConfig = this._config.health;
    const wasInCooldown = pr.health.isInCooldown(now);

    switch (obs.outcome.status) {
      case StatusClass.Success:
      case StatusClass.ClientError:
        pr.health.recordSuccess(obs.outcome.latencyMs, now, healthConfig);
        break;
      case StatusClass.RateLimited:
        pr.health.recordRateLimited(now, healthConfig, cooldownSecs);
        break;
      case StatusClass.Forbidden:
        pr.health.recordForbidden(now, healthConfig, cooldownSecs);
        break;
      case StatusClass.ServerError:
        pr.health.recordServerError(now, healthConfig, cooldownSecs);
        break;
      case StatusClass.Timeout:
        pr.health.recordTimeout(now, healthConfig, cooldownSecs);
        break;
    }

    if (!wasInCooldown && pr.health.isInCooldown(now)) {
      this._metrics.incCooldowns();
    }
  }

  get metrics(): Metrics {
    return this._metrics;
  }

  /** Get the current health score for a provider. */
  providerHealth(provider: string): number | null {
    const pr = this._providers.get(provider);
    return pr ? pr.health.score : null;
  }

  /** Check if a provider is currently in cooldown. */
  providerInCooldown(provider: string): boolean | null {
    const pr = this._providers.get(provider);
    if (!pr) return null;
    return pr.health.isInCooldown(this._clock.now());
  }

  /** Get the remaining quota for a specific dimension on a provider. */
  providerQuotaRemaining(provider: string, dimension: Dimension): number | null {
    const pr = this._providers.get(provider);
    if (!pr) return null;
    const now = this._clock.now();
    for (const { config: qc, tracker } of pr.quotaTrackers) {
      if (qc.dimension === dimension) return tracker.remaining(now);
    }
    return null;
  }

  private _worstQuotaState(
    trackers: Array<{ config: QuotaConfig; tracker: QuotaTracker }>,
    now: import("./clock.js").Timestamp,
  ): [number, number, number] {
    if (trackers.length === 0) return [1.0, Infinity, 0];

    let worstRemaining = 1.0;
    let worstExhaustion = Infinity;
    let maxBurnRate = 0;

    for (const { tracker } of trackers) {
      const remaining = 1.0 - tracker.usageRatio(now);
      const exhaustion = tracker.predictedExhaustionSecs(now);
      const burn = tracker.burnRate(now);

      if (remaining < worstRemaining) worstRemaining = remaining;
      if (exhaustion < worstExhaustion) worstExhaustion = exhaustion;
      if (burn > maxBurnRate) maxBurnRate = burn;
    }

    return [worstRemaining, worstExhaustion, maxBurnRate];
  }

  private _usageForDimension(dimension: Dimension, obs: Observation): number {
    switch (dimension) {
      case Dimension.Requests:
        return obs.usage.requests;
      case Dimension.Tokens:
        return obs.usage.tokens ?? 0;
      case Dimension.Bytes:
        return obs.usage.bytes ?? 0;
      case Dimension.CostUsd:
        return obs.usage.costMicroUsd ?? 0;
      case Dimension.Concurrency:
        return obs.usage.requests;
    }
  }
}
