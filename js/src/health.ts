import { Timestamp } from "./clock.js";
import type { HealthConfig } from "./config.js";

const LATENCY_ALPHA = 0.3;

/** Runtime health state for a single provider. */
export class HealthState {
  private _score = 1.0;
  private _consecutiveFailures = 0;
  private _currentCooldownSecs = 0;
  private _cooldownUntil: Timestamp | null = null;
  private _lastObservation: Timestamp;
  private _totalObservations = 0;
  private _totalSuccesses = 0;
  private _ewmaLatencyMs = 0;

  constructor(now: Timestamp) {
    this._lastObservation = now;
  }

  get score(): number {
    return this._score;
  }

  isInCooldown(now: Timestamp): boolean {
    if (this._cooldownUntil === null) return false;
    return now.lt(this._cooldownUntil);
  }

  get latencyMs(): number {
    return this._ewmaLatencyMs;
  }

  recordSuccess(latencyMs: number, now: Timestamp, config: HealthConfig): void {
    this._applyDecay(now, config);
    this._score = Math.min(this._score + config.boostSuccess, 1.0);
    this._consecutiveFailures = 0;
    this._totalObservations++;
    this._totalSuccesses++;
    this._updateLatency(latencyMs);
    this._lastObservation = now;
  }

  recordRateLimited(now: Timestamp, config: HealthConfig, defaultCooldownSecs: number): void {
    this._applyDecay(now, config);
    this._score = Math.max(this._score - config.penalty429, 0.0);
    this._totalObservations++;
    this._recordFailure(now, config, defaultCooldownSecs);
    this._lastObservation = now;
  }

  recordForbidden(now: Timestamp, config: HealthConfig, defaultCooldownSecs: number): void {
    this._applyDecay(now, config);
    this._score = Math.max(this._score - config.penalty403, 0.0);
    this._totalObservations++;
    this._recordFailure(now, config, defaultCooldownSecs);
    this._lastObservation = now;
  }

  recordServerError(now: Timestamp, config: HealthConfig, defaultCooldownSecs: number): void {
    this._applyDecay(now, config);
    this._score = Math.max(this._score - config.penalty5xx, 0.0);
    this._totalObservations++;
    this._recordFailure(now, config, defaultCooldownSecs);
    this._lastObservation = now;
  }

  recordTimeout(now: Timestamp, config: HealthConfig, defaultCooldownSecs: number): void {
    this._applyDecay(now, config);
    this._score = Math.max(this._score - config.penaltyTimeout, 0.0);
    this._totalObservations++;
    this._recordFailure(now, config, defaultCooldownSecs);
    this._lastObservation = now;
  }

  private _applyDecay(now: Timestamp, config: HealthConfig): void {
    const elapsedSecs = now.durationSince(this._lastObservation) / 1_000_000_000;
    if (elapsedSecs <= 0 || config.decayHalfLifeSeconds <= 0) return;

    const decayFactor = Math.pow(0.5, elapsedSecs / config.decayHalfLifeSeconds);
    const deficit = 1.0 - this._score;
    this._score = 1.0 - deficit * decayFactor;
    this._score = Math.max(0.0, Math.min(1.0, this._score));
  }

  private _recordFailure(now: Timestamp, config: HealthConfig, defaultCooldownSecs: number): void {
    this._consecutiveFailures++;
    if (this._consecutiveFailures >= config.cooldownTriggerCount) {
      const excess = this._consecutiveFailures - config.cooldownTriggerCount;
      const multiplier = Math.pow(config.cooldownMultiplier, excess);
      const cooldownSecs = Math.floor(defaultCooldownSecs * multiplier);
      this._currentCooldownSecs = Math.min(cooldownSecs, config.maxCooldownSeconds);
      this._cooldownUntil = now.addSecs(this._currentCooldownSecs);
    }
  }

  private _updateLatency(latencyMs: number): void {
    if (this._totalObservations <= 1) {
      this._ewmaLatencyMs = latencyMs;
    } else {
      this._ewmaLatencyMs = LATENCY_ALPHA * latencyMs + (1 - LATENCY_ALPHA) * this._ewmaLatencyMs;
    }
  }
}
