import type { Clock } from "./clock.js";

/** Weights for the composite scoring algorithm. Should sum to 1.0. */
export interface ScoringWeights {
  quota: number;
  health: number;
  priority: number;
  latency: number;
}

export function defaultScoringWeights(): ScoringWeights {
  return { quota: 0.4, health: 0.35, priority: 0.2, latency: 0.05 };
}

/** Health engine configuration. */
export interface HealthConfig {
  decayHalfLifeSeconds: number;
  penalty429: number;
  penalty403: number;
  penalty5xx: number;
  penaltyTimeout: number;
  boostSuccess: number;
  cooldownTriggerCount: number;
  cooldownMultiplier: number;
  maxCooldownSeconds: number;
}

export function defaultHealthConfig(): HealthConfig {
  return {
    decayHalfLifeSeconds: 300.0,
    penalty429: 0.25,
    penalty403: 0.5,
    penalty5xx: 0.1,
    penaltyTimeout: 0.2,
    boostSuccess: 0.02,
    cooldownTriggerCount: 3,
    cooldownMultiplier: 2.0,
    maxCooldownSeconds: 600,
  };
}

/** Top-level engine configuration. */
export interface EngineConfig {
  scoring?: Partial<ScoringWeights>;
  health?: Partial<HealthConfig>;
  minimumHealthScore?: number;
  defaultCooldownSeconds?: number;
  clock?: Clock;
}

export interface ResolvedEngineConfig {
  scoring: ScoringWeights;
  health: HealthConfig;
  minimumHealthScore: number;
  defaultCooldownSeconds: number;
  clock?: Clock;
}

export function resolveEngineConfig(cfg?: EngineConfig): ResolvedEngineConfig {
  return {
    scoring: { ...defaultScoringWeights(), ...cfg?.scoring },
    health: { ...defaultHealthConfig(), ...cfg?.health },
    minimumHealthScore: cfg?.minimumHealthScore ?? 0.2,
    defaultCooldownSeconds: cfg?.defaultCooldownSeconds ?? 60,
    clock: cfg?.clock,
  };
}
