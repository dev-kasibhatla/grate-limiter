import { describe, expect, it } from "vitest";
import { Timestamp } from "../src/clock.js";
import {
  ConcurrencyLimiter,
  FixedWindow,
  SlidingWindowCounter,
  TokenBucket,
} from "../src/quota.js";
import { Window } from "../src/types.js";

function ts(ms: number): Timestamp {
  return new Timestamp(ms * 1_000_000);
}

describe("TokenBucket", () => {
  it("new bucket is full", () => {
    const bucket = new TokenBucket(100, Window.Minute, ts(0));
    expect(bucket.remaining(ts(0))).toBe(100);
    expect(bucket.check(100, ts(0))).toBe(true);
    expect(bucket.check(101, ts(0))).toBe(false);
  });

  it("consume reduces remaining", () => {
    const bucket = new TokenBucket(100, Window.Minute, ts(0));
    bucket.record(30, ts(0));
    expect(bucket.remaining(ts(0))).toBe(70);
  });

  it("tokens refill over time", () => {
    const bucket = new TokenBucket(60, Window.Minute, ts(0));
    bucket.record(60, ts(0));
    expect(bucket.remaining(ts(0))).toBe(0);
    expect(bucket.remaining(ts(30_000))).toBe(30);
    expect(bucket.remaining(ts(60_000))).toBe(60);
  });

  it("never exceeds capacity", () => {
    const bucket = new TokenBucket(100, Window.Minute, ts(0));
    expect(bucket.remaining(ts(120_000))).toBe(100);
  });

  it("burn rate tracks consumption", () => {
    const bucket = new TokenBucket(100, Window.Minute, ts(0));
    bucket.record(10, ts(1_000));
    bucket.record(10, ts(2_000));
    const rate = bucket.burnRate(ts(5_000));
    expect(Math.abs(rate - 4.0)).toBeLessThan(0.5);
  });

  it("usage ratio", () => {
    const bucket = new TokenBucket(100, Window.Minute, ts(0));
    expect(Math.abs(bucket.usageRatio(ts(0)))).toBeLessThan(0.01);
    bucket.record(80, ts(0));
    expect(Math.abs(bucket.usageRatio(ts(0)) - 0.8)).toBeLessThan(0.01);
  });

  it("predicted exhaustion", () => {
    const bucket = new TokenBucket(100, Window.Minute, ts(0));
    bucket.record(50, ts(5_000));
    const secs = bucket.predictedExhaustionSecs(ts(5_000));
    expect(Math.abs(secs - 5.0)).toBeLessThan(1.0);
  });

  it("reset restores full capacity", () => {
    const bucket = new TokenBucket(100, Window.Minute, ts(0));
    bucket.record(100, ts(0));
    expect(bucket.remaining(ts(0))).toBe(0);
    bucket.reset(ts(1_000));
    expect(bucket.remaining(ts(1_000))).toBe(100);
  });
});

describe("SlidingWindowCounter", () => {
  it("new window has full capacity", () => {
    const sw = new SlidingWindowCounter(100, Window.Minute, ts(0));
    expect(sw.remaining(ts(0))).toBe(100);
  });

  it("record reduces remaining", () => {
    const sw = new SlidingWindowCounter(100, Window.Minute, ts(0));
    sw.record(40, ts(0));
    expect(sw.remaining(ts(0))).toBe(60);
  });

  it("window rotation interpolates", () => {
    const sw = new SlidingWindowCounter(100, Window.Minute, ts(0));
    sw.record(80, ts(0));
    const remaining = sw.remaining(ts(90_000));
    expect(remaining).toBeGreaterThanOrEqual(55);
    expect(remaining).toBeLessThanOrEqual(65);
  });

  it("full window resets", () => {
    const sw = new SlidingWindowCounter(100, Window.Minute, ts(0));
    sw.record(100, ts(0));
    expect(sw.remaining(ts(0))).toBe(0);
    expect(sw.remaining(ts(120_001))).toBe(100);
  });
});

describe("FixedWindow", () => {
  it("new window full capacity", () => {
    const fw = new FixedWindow(100, Window.Minute, ts(0));
    expect(fw.remaining(ts(0))).toBe(100);
  });

  it("record reduces remaining", () => {
    const fw = new FixedWindow(100, Window.Minute, ts(0));
    fw.record(60, ts(0));
    expect(fw.remaining(ts(0))).toBe(40);
  });

  it("window resets after expiry", () => {
    const fw = new FixedWindow(100, Window.Minute, ts(0));
    fw.record(100, ts(0));
    expect(fw.remaining(ts(0))).toBe(0);
    expect(fw.remaining(ts(60_000))).toBe(100);
  });

  it("check respects capacity", () => {
    const fw = new FixedWindow(100, Window.Minute, ts(0));
    expect(fw.check(100, ts(0))).toBe(true);
    expect(fw.check(101, ts(0))).toBe(false);
    fw.record(90, ts(0));
    expect(fw.check(10, ts(0))).toBe(true);
    expect(fw.check(11, ts(0))).toBe(false);
  });
});

describe("ConcurrencyLimiter", () => {
  it("new has full capacity", () => {
    const cl = new ConcurrencyLimiter(10);
    expect(cl.remaining(ts(0))).toBe(10);
  });

  it("record occupies slots", () => {
    const cl = new ConcurrencyLimiter(10);
    cl.record(3, ts(0));
    expect(cl.remaining(ts(0))).toBe(7);
  });

  it("release frees slots", () => {
    const cl = new ConcurrencyLimiter(10);
    cl.record(5, ts(0));
    expect(cl.remaining(ts(0))).toBe(5);
    cl.release(3);
    expect(cl.remaining(ts(0))).toBe(8);
  });

  it("check respects capacity", () => {
    const cl = new ConcurrencyLimiter(5);
    expect(cl.check(5, ts(0))).toBe(true);
    expect(cl.check(6, ts(0))).toBe(false);
    cl.record(3, ts(0));
    expect(cl.check(2, ts(0))).toBe(true);
    expect(cl.check(3, ts(0))).toBe(false);
  });

  it("time does not affect concurrency", () => {
    const cl = new ConcurrencyLimiter(10);
    cl.record(10, ts(0));
    expect(cl.remaining(ts(0))).toBe(0);
    expect(cl.remaining(ts(60_000))).toBe(0);
  });
});
