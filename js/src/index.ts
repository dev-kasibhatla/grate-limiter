// Core
export { Timestamp, RealClock, MockClock } from "./clock.js";
export type { Clock } from "./clock.js";
export {
  defaultScoringWeights,
  defaultHealthConfig,
} from "./config.js";
export type {
  ScoringWeights,
  HealthConfig,
  EngineConfig,
} from "./config.js";
export { Dimension, Window, StatusClass } from "./types.js";
export {
  GrateLimiterError,
  UnknownCapability,
  UnknownProvider,
  NoAvailableProviders,
} from "./errors.js";

// Models
export type {
  QuotaConfig,
  ProviderConfig,
  CapabilityProvider,
  CapabilityConfig,
  Usage,
  Outcome,
  Observation,
  ScoreBreakdown,
  Alternative,
  Decision,
} from "./models.js";

// Engine
export { GrateLimiter } from "./engine.js";

// Metrics
export { Metrics } from "./metrics.js";

// Quota (for advanced usage)
export {
  TokenBucket,
  SlidingWindowCounter,
  FixedWindow,
  ConcurrencyLimiter,
} from "./quota.js";
export type { QuotaTracker } from "./quota.js";

// Scoring (for advanced usage)
export { WeightedScorer } from "./scoring.js";
export type { ProviderScoreContext } from "./scoring.js";

// Health (for advanced usage)
export { HealthState } from "./health.js";
