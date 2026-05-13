import { Timestamp } from "./clock.js";
import { Dimension, Window, windowAsNanos } from "./types.js";
import type { QuotaConfig } from "./models.js";

/** Internal interface for quota tracking strategies. */
export interface QuotaTracker {
  check(amount: number, now: Timestamp): boolean;
  record(amount: number, now: Timestamp): void;
  remaining(now: Timestamp): number;
  capacity(): number;
  usageRatio(now: Timestamp): number;
  burnRate(now: Timestamp): number;
  predictedExhaustionSecs(now: Timestamp): number;
  reset(now: Timestamp): void;
}

/** Token bucket quota strategy with continuous refill. */
export class TokenBucket implements QuotaTracker {
  private readonly _capacity: number;
  private readonly _windowNanos: number;
  private _tokens: number;
  private _lastRefill: number;
  private _consumedInWindow: number;
  private _windowStart: number;

  constructor(cap: number, window: Window, now: Timestamp) {
    this._capacity = cap;
    this._windowNanos = windowAsNanos(window);
    this._tokens = cap;
    this._lastRefill = now.nanos;
    this._consumedInWindow = 0;
    this._windowStart = now.nanos;
  }

  private _refill(now: Timestamp): number {
    const elapsed = now.nanos - this._lastRefill;
    if (elapsed <= 0) return this._tokens;

    const tokensToAdd = (elapsed * this._capacity) / this._windowNanos;
    if (tokensToAdd <= 0) return this._tokens;

    this._lastRefill = now.nanos;
    this._tokens = Math.min(this._tokens + tokensToAdd, this._capacity);

    if (now.nanos - this._windowStart >= this._windowNanos) {
      this._consumedInWindow = 0;
      this._windowStart = now.nanos;
    }

    return this._tokens;
  }

  check(amount: number, now: Timestamp): boolean {
    return this._refill(now) >= amount;
  }

  record(amount: number, now: Timestamp): void {
    this._refill(now);
    this._tokens = Math.max(0, this._tokens - amount);
    this._consumedInWindow += amount;
  }

  remaining(now: Timestamp): number {
    return Math.floor(this._refill(now));
  }

  capacity(): number {
    return this._capacity;
  }

  usageRatio(now: Timestamp): number {
    const cap = this._capacity;
    if (cap === 0) return 1.0;
    return 1.0 - this.remaining(now) / cap;
  }

  burnRate(now: Timestamp): number {
    const elapsedSecs = (now.nanos - this._windowStart) / 1_000_000_000;
    if (elapsedSecs < 0.001) return 0;
    return this._consumedInWindow / elapsedSecs;
  }

  predictedExhaustionSecs(now: Timestamp): number {
    const rate = this.burnRate(now);
    if (rate <= 0) return Infinity;
    return this.remaining(now) / rate;
  }

  reset(now: Timestamp): void {
    this._tokens = this._capacity;
    this._lastRefill = now.nanos;
    this._consumedInWindow = 0;
    this._windowStart = now.nanos;
  }
}

/** Sliding window counter quota strategy. */
export class SlidingWindowCounter implements QuotaTracker {
  private readonly _capacity: number;
  private readonly _windowNanos: number;
  private _currentCount: number;
  private _previousCount: number;
  private _windowStart: number;

  constructor(cap: number, window: Window, now: Timestamp) {
    this._capacity = cap;
    this._windowNanos = windowAsNanos(window);
    this._currentCount = 0;
    this._previousCount = 0;
    this._windowStart = now.nanos;
  }

  private _rotateAndCount(now: Timestamp): number {
    const elapsed = now.nanos - this._windowStart;

    if (elapsed >= 2 * this._windowNanos) {
      this._previousCount = 0;
      this._currentCount = 0;
      this._windowStart = now.nanos;
      return 0;
    }

    if (elapsed >= this._windowNanos) {
      const current = this._currentCount;
      this._previousCount = current;
      this._currentCount = 0;
      const newStart = this._windowStart + this._windowNanos;
      this._windowStart = newStart;

      const newElapsed = now.nanos - newStart;
      const fractionOfPrev = 1.0 - newElapsed / this._windowNanos;
      return Math.floor(current * fractionOfPrev);
    }

    const fractionOfPrev = 1.0 - elapsed / this._windowNanos;
    return Math.floor(this._previousCount * fractionOfPrev) + this._currentCount;
  }

  check(amount: number, now: Timestamp): boolean {
    return this._rotateAndCount(now) + amount <= this._capacity;
  }

  record(amount: number, now: Timestamp): void {
    this._rotateAndCount(now);
    this._currentCount += amount;
  }

  remaining(now: Timestamp): number {
    return Math.max(0, this._capacity - this._rotateAndCount(now));
  }

  capacity(): number {
    return this._capacity;
  }

  usageRatio(now: Timestamp): number {
    if (this._capacity === 0) return 1.0;
    return 1.0 - this.remaining(now) / this._capacity;
  }

  burnRate(now: Timestamp): number {
    const elapsedSecs = (now.nanos - this._windowStart) / 1_000_000_000;
    if (elapsedSecs < 0.001) return 0;
    return this._currentCount / elapsedSecs;
  }

  predictedExhaustionSecs(now: Timestamp): number {
    const rate = this.burnRate(now);
    if (rate <= 0) return Infinity;
    return this.remaining(now) / rate;
  }

  reset(now: Timestamp): void {
    this._currentCount = 0;
    this._previousCount = 0;
    this._windowStart = now.nanos;
  }
}

/** Fixed window quota strategy. */
export class FixedWindow implements QuotaTracker {
  private readonly _capacity: number;
  private readonly _windowNanos: number;
  private _count: number;
  private _windowStart: number;

  constructor(cap: number, window: Window, now: Timestamp) {
    this._capacity = cap;
    this._windowNanos = windowAsNanos(window);
    this._count = 0;
    this._windowStart = now.nanos;
  }

  private _maybeReset(now: Timestamp): void {
    const elapsed = now.nanos - this._windowStart;
    if (elapsed >= this._windowNanos) {
      this._count = 0;
      const windowsElapsed = Math.floor(elapsed / this._windowNanos);
      this._windowStart += windowsElapsed * this._windowNanos;
    }
  }

  check(amount: number, now: Timestamp): boolean {
    this._maybeReset(now);
    return this._count + amount <= this._capacity;
  }

  record(amount: number, now: Timestamp): void {
    this._maybeReset(now);
    this._count += amount;
  }

  remaining(now: Timestamp): number {
    this._maybeReset(now);
    return Math.max(0, this._capacity - this._count);
  }

  capacity(): number {
    return this._capacity;
  }

  usageRatio(now: Timestamp): number {
    if (this._capacity === 0) return 1.0;
    return 1.0 - this.remaining(now) / this._capacity;
  }

  burnRate(now: Timestamp): number {
    this._maybeReset(now);
    const elapsedSecs = (now.nanos - this._windowStart) / 1_000_000_000;
    if (elapsedSecs < 0.001) return 0;
    return this._count / elapsedSecs;
  }

  predictedExhaustionSecs(now: Timestamp): number {
    const rate = this.burnRate(now);
    if (rate <= 0) return Infinity;
    return this.remaining(now) / rate;
  }

  reset(now: Timestamp): void {
    this._count = 0;
    this._windowStart = now.nanos;
  }
}

/** Concurrency limiter — tracks in-flight requests. */
export class ConcurrencyLimiter implements QuotaTracker {
  private readonly _capacity: number;
  private _active: number;

  constructor(cap: number) {
    this._capacity = cap;
    this._active = 0;
  }

  release(amount: number): void {
    this._active = Math.max(0, this._active - amount);
  }

  check(amount: number, _now: Timestamp): boolean {
    return this._active + amount <= this._capacity;
  }

  record(amount: number, _now: Timestamp): void {
    this._active += amount;
  }

  remaining(_now: Timestamp): number {
    return Math.max(0, this._capacity - this._active);
  }

  capacity(): number {
    return this._capacity;
  }

  usageRatio(_now: Timestamp): number {
    if (this._capacity === 0) return 1.0;
    return 1.0 - this.remaining(_now) / this._capacity;
  }

  burnRate(_now: Timestamp): number {
    return 0;
  }

  predictedExhaustionSecs(_now: Timestamp): number {
    return Infinity;
  }

  reset(_now: Timestamp): void {
    this._active = 0;
  }
}

/** Create appropriate quota tracker for a given config. */
export function createTracker(config: QuotaConfig, now: Timestamp): QuotaTracker {
  if (config.dimension === Dimension.Concurrency) {
    return new ConcurrencyLimiter(config.limit);
  }
  const window = config.window ?? Window.Minute;
  return new TokenBucket(config.limit, window, now);
}
