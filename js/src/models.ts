import { Dimension, StatusClass, Window } from "./types.js";

/** Configuration for a single quota dimension on a provider. */
export interface QuotaConfig {
  dimension: Dimension;
  limit: number;
  window?: Window;
}

/** Configuration for a provider. */
export interface ProviderConfig {
  name: string;
  quotas: QuotaConfig[];
  priority: number;
  weight?: number;
  cooldownSeconds: number;
}

/** A provider registered under a capability with its priority. */
export interface CapabilityProvider {
  provider: string;
  priority: number;
}

/** Configuration for a capability. */
export interface CapabilityConfig {
  name: string;
  providers: CapabilityProvider[];
}

/** Resource usage for a single interaction. */
export interface Usage {
  requests: number;
  tokens?: number;
  bytes?: number;
  costMicroUsd?: number;
}

/** Outcome of a provider interaction. */
export interface Outcome {
  status: StatusClass;
  latencyMs: number;
}

/** An observation reported after a provider interaction. */
export interface Observation {
  provider: string;
  capability?: string;
  usage: Usage;
  outcome: Outcome;
}

/** Detailed breakdown of how a provider was scored. */
export interface ScoreBreakdown {
  quotaScore: number;
  healthScore: number;
  priorityScore: number;
  latencyScore: number;
}

/** An alternative provider candidate with its score. */
export interface Alternative {
  provider: string;
  score: number;
}

/** The result of a provider selection decision. */
export interface Decision {
  provider: string;
  score: number;
  reasoning: ScoreBreakdown;
  alternatives: Alternative[];
}
