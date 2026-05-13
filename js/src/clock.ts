/** Monotonic timestamp in nanoseconds since engine creation. */
export class Timestamp {
  static readonly ZERO = new Timestamp(0);

  constructor(public readonly nanos: number) {}

  get asNanos(): number {
    return this.nanos;
  }

  get asMillis(): number {
    return Math.floor(this.nanos / 1_000_000);
  }

  get asSecsF64(): number {
    return this.nanos / 1_000_000_000;
  }

  durationSince(other: Timestamp): number {
    return Math.max(0, this.nanos - other.nanos);
  }

  addNanos(n: number): Timestamp {
    return new Timestamp(this.nanos + n);
  }

  addMillis(ms: number): Timestamp {
    return new Timestamp(this.nanos + ms * 1_000_000);
  }

  addSecs(secs: number): Timestamp {
    return new Timestamp(this.nanos + secs * 1_000_000_000);
  }

  lt(other: Timestamp): boolean {
    return this.nanos < other.nanos;
  }

  lte(other: Timestamp): boolean {
    return this.nanos <= other.nanos;
  }

  gt(other: Timestamp): boolean {
    return this.nanos > other.nanos;
  }

  gte(other: Timestamp): boolean {
    return this.nanos >= other.nanos;
  }

  eq(other: Timestamp): boolean {
    return this.nanos === other.nanos;
  }
}

/** Clock abstraction for monotonic time. */
export interface Clock {
  now(): Timestamp;
}

/** Real monotonic clock. Uses performance.now() where available, Date.now() as fallback. */
export class RealClock implements Clock {
  private readonly epoch: number;

  constructor() {
    this.epoch =
      typeof performance !== "undefined"
        ? performance.now()
        : Date.now();
  }

  now(): Timestamp {
    const elapsed =
      typeof performance !== "undefined"
        ? performance.now() - this.epoch
        : Date.now() - this.epoch;
    return new Timestamp(Math.floor(elapsed * 1_000_000));
  }
}

/** Mock clock for deterministic testing. Time only advances when explicitly told to. */
export class MockClock implements Clock {
  private _nanos: number;

  constructor(startNanos = 0) {
    this._nanos = startNanos;
  }

  static at(timestamp: Timestamp): MockClock {
    return new MockClock(timestamp.nanos);
  }

  now(): Timestamp {
    return new Timestamp(this._nanos);
  }

  advanceNanos(n: number): void {
    this._nanos += n;
  }

  advanceMs(ms: number): void {
    this._nanos += ms * 1_000_000;
  }

  advanceSecs(secs: number): void {
    this._nanos += secs * 1_000_000_000;
  }

  set(timestamp: Timestamp): void {
    this._nanos = timestamp.nanos;
  }
}
