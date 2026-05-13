import { describe, expect, it } from "vitest";
import { Timestamp } from "../src/clock.js";
import { defaultHealthConfig, type HealthConfig } from "../src/config.js";
import { HealthState } from "../src/health.js";

function ts(ms: number): Timestamp {
  return new Timestamp(ms * 1_000_000);
}

describe("HealthState", () => {
  it("initial health is perfect", () => {
    const h = new HealthState(ts(0));
    expect(h.score).toBe(1.0);
    expect(h.isInCooldown(ts(0))).toBe(false);
  });

  it("success maintains health", () => {
    const config = defaultHealthConfig();
    const h = new HealthState(ts(0));
    h.recordSuccess(100, ts(1_000), config);
    expect(h.score).toBeGreaterThanOrEqual(1.0);
  });

  it("rate limit reduces health", () => {
    const config = defaultHealthConfig();
    const h = new HealthState(ts(0));
    h.recordRateLimited(ts(1_000), config, 60);
    expect(h.score).toBeLessThan(1.0);
    expect(Math.abs(h.score - (1.0 - config.penalty429))).toBeLessThan(0.01);
  });

  it("health decays toward full", () => {
    const config: HealthConfig = { ...defaultHealthConfig(), decayHalfLifeSeconds: 10.0 };
    const h = new HealthState(ts(0));
    h.recordRateLimited(ts(0), config, 60);
    const afterPenalty = h.score;
    h.recordSuccess(100, ts(10_000), config);
    expect(h.score).toBeGreaterThan(afterPenalty);
  });

  it("consecutive failures trigger cooldown", () => {
    const config: HealthConfig = { ...defaultHealthConfig(), cooldownTriggerCount: 3 };
    const h = new HealthState(ts(0));

    h.recordRateLimited(ts(1_000), config, 30);
    expect(h.isInCooldown(ts(1_000))).toBe(false);

    h.recordRateLimited(ts(2_000), config, 30);
    expect(h.isInCooldown(ts(2_000))).toBe(false);

    h.recordRateLimited(ts(3_000), config, 30);
    expect(h.isInCooldown(ts(3_000))).toBe(true);
    expect(h.isInCooldown(ts(32_000))).toBe(true);
    expect(h.isInCooldown(ts(34_000))).toBe(false);
  });

  it("cooldown grows exponentially", () => {
    const config: HealthConfig = {
      ...defaultHealthConfig(),
      cooldownTriggerCount: 2,
      cooldownMultiplier: 2.0,
      maxCooldownSeconds: 600,
    };
    const h = new HealthState(ts(0));

    h.recordRateLimited(ts(1_000), config, 30);
    h.recordRateLimited(ts(2_000), config, 30);
    expect(h.isInCooldown(ts(2_000))).toBe(true);

    h.recordRateLimited(ts(33_000), config, 30);
    expect(h.isInCooldown(ts(92_000))).toBe(true);
  });

  it("health score bounded", () => {
    const config = defaultHealthConfig();
    const h = new HealthState(ts(0));

    for (let i = 0; i < 20; i++) {
      h.recordRateLimited(ts(i * 1_000), config, 60);
    }
    expect(h.score).toBeGreaterThanOrEqual(0.0);

    for (let i = 20; i < 40; i++) {
      h.recordSuccess(100, ts(i * 1_000), config);
    }
    expect(h.score).toBeLessThanOrEqual(1.0);
  });

  it("ewma latency smooths", () => {
    const config = defaultHealthConfig();
    const h = new HealthState(ts(0));

    h.recordSuccess(100, ts(1_000), config);
    expect(Math.abs(h.latencyMs - 100.0)).toBeLessThan(0.01);

    h.recordSuccess(200, ts(2_000), config);
    expect(Math.abs(h.latencyMs - 130.0)).toBeLessThan(1.0);
  });

  it("forbidden applies heavy penalty", () => {
    const config = defaultHealthConfig();
    const h = new HealthState(ts(0));
    h.recordForbidden(ts(1_000), config, 60);
    expect(Math.abs(h.score - (1.0 - config.penalty403))).toBeLessThan(0.01);
  });

  it("server error penalty", () => {
    const config = defaultHealthConfig();
    const h = new HealthState(ts(0));
    h.recordServerError(ts(1_000), config, 60);
    expect(Math.abs(h.score - (1.0 - config.penalty5xx))).toBeLessThan(0.01);
  });

  it("timeout penalty", () => {
    const config = defaultHealthConfig();
    const h = new HealthState(ts(0));
    h.recordTimeout(ts(1_000), config, 60);
    expect(Math.abs(h.score - (1.0 - config.penaltyTimeout))).toBeLessThan(0.01);
  });
});
