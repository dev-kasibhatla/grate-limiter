import { describe, expect, it } from "vitest";
import { MockClock, RealClock, Timestamp } from "../src/clock.js";

describe("Timestamp", () => {
  it("ZERO is zero", () => {
    expect(Timestamp.ZERO.asNanos).toBe(0);
    expect(Timestamp.ZERO.asMillis).toBe(0);
  });

  it("conversions", () => {
    const ts = new Timestamp(5_000_000_000);
    expect(ts.asMillis).toBe(5000);
    expect(Math.abs(ts.asSecsF64 - 5.0)).toBeLessThan(0.001);
  });

  it("durationSince", () => {
    const a = new Timestamp(10_000_000);
    const b = new Timestamp(3_000_000);
    expect(a.durationSince(b)).toBe(7_000_000);
    expect(b.durationSince(a)).toBe(0);
  });

  it("add methods", () => {
    const ts = new Timestamp(0);
    expect(ts.addMillis(100).asMillis).toBe(100);
    expect(ts.addSecs(2).asMillis).toBe(2000);
    expect(ts.addNanos(1_000_000).asMillis).toBe(1);
  });

  it("comparison", () => {
    const a = new Timestamp(100);
    const b = new Timestamp(200);
    expect(a.lt(b)).toBe(true);
    expect(b.gt(a)).toBe(true);
    expect(a.lte(a)).toBe(true);
    expect(a.eq(new Timestamp(100))).toBe(true);
  });
});

describe("RealClock", () => {
  it("monotonic", () => {
    const clock = new RealClock();
    const t1 = clock.now();
    const t2 = clock.now();
    expect(t2.gte(t1)).toBe(true);
  });
});

describe("MockClock", () => {
  it("starts at zero", () => {
    const clock = new MockClock();
    expect(clock.now().asMillis).toBe(0);
  });

  it("advance ms", () => {
    const clock = new MockClock();
    clock.advanceMs(5000);
    expect(clock.now().asMillis).toBe(5000);
  });

  it("advance secs", () => {
    const clock = new MockClock();
    clock.advanceSecs(3);
    expect(clock.now().asMillis).toBe(3000);
  });

  it("at", () => {
    const clock = MockClock.at(new Timestamp(10_000_000_000));
    expect(clock.now().asMillis).toBe(10000);
  });

  it("set", () => {
    const clock = new MockClock();
    clock.set(new Timestamp(42_000_000));
    expect(clock.now().asMillis).toBe(42);
  });
});
