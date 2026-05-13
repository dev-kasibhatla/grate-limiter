import { describe, expect, it } from "vitest";
import { MockClock } from "../src/clock.js";
import { GrateLimiter } from "../src/engine.js";
import { NoAvailableProviders, UnknownCapability, UnknownProvider } from "../src/errors.js";
import { Dimension, StatusClass, Window } from "../src/types.js";
import type { CapabilityConfig, Observation, ProviderConfig } from "../src/models.js";

function setupEngine(): { engine: GrateLimiter; clock: MockClock } {
  const clock = new MockClock();
  const engine = new GrateLimiter({ clock });

  engine.upsertProvider({
    name: "openai",
    quotas: [{ dimension: Dimension.Requests, limit: 100, window: Window.Minute }],
    priority: 10,
    weight: 1.0,
    cooldownSeconds: 30,
  });

  engine.upsertProvider({
    name: "anthropic",
    quotas: [{ dimension: Dimension.Requests, limit: 80, window: Window.Minute }],
    priority: 8,
    weight: 1.0,
    cooldownSeconds: 30,
  });

  engine.upsertCapability({
    name: "chat",
    providers: [
      { provider: "openai", priority: 10 },
      { provider: "anthropic", priority: 8 },
    ],
  });

  return { engine, clock };
}

describe("GrateLimiter", () => {
  it("select returns best provider", () => {
    const { engine } = setupEngine();
    const decision = engine.select("chat");
    expect(decision.provider).toBe("openai");
    expect(decision.score).toBeGreaterThan(0);
    expect(decision.alternatives).toHaveLength(1);
  });

  it("select unknown capability errors", () => {
    const { engine } = setupEngine();
    expect(() => engine.select("nonexistent")).toThrow(UnknownCapability);
  });

  it("observe updates health", () => {
    const { engine } = setupEngine();
    engine.observe({
      provider: "openai",
      capability: "chat",
      usage: { requests: 1 },
      outcome: { status: StatusClass.RateLimited, latencyMs: 100 },
    });
    const health = engine.providerHealth("openai");
    expect(health).not.toBeNull();
    expect(health!).toBeLessThan(1.0);
  });

  it("observe unknown provider errors", () => {
    const { engine } = setupEngine();
    expect(() =>
      engine.observe({
        provider: "nonexistent",
        usage: { requests: 0 },
        outcome: { status: StatusClass.Success, latencyMs: 100 },
      }),
    ).toThrow(UnknownProvider);
  });

  it("degraded provider loses to healthy", () => {
    const { engine, clock } = setupEngine();
    for (let i = 0; i < 3; i++) {
      clock.advanceMs(1000);
      engine.observe({
        provider: "openai",
        capability: "chat",
        usage: { requests: 1 },
        outcome: { status: StatusClass.RateLimited, latencyMs: 100 },
      });
    }
    const decision = engine.select("chat");
    expect(decision.provider).toBe("anthropic");
  });

  it("metrics increment", () => {
    const { engine } = setupEngine();
    engine.select("chat");
    engine.select("chat");
    expect(engine.metrics.selects).toBe(2);

    engine.observe({
      provider: "openai",
      usage: { requests: 1 },
      outcome: { status: StatusClass.Success, latencyMs: 50 },
    });
    expect(engine.metrics.observations).toBe(1);
  });

  it("provider quota tracking", () => {
    const { engine } = setupEngine();
    expect(engine.providerQuotaRemaining("openai", Dimension.Requests)).toBe(100);

    engine.observe({
      provider: "openai",
      usage: { requests: 30 },
      outcome: { status: StatusClass.Success, latencyMs: 100 },
    });
    expect(engine.providerQuotaRemaining("openai", Dimension.Requests)).toBe(70);
  });

  it("upsert provider preserves health", () => {
    const { engine } = setupEngine();
    engine.observe({
      provider: "openai",
      usage: { requests: 1 },
      outcome: { status: StatusClass.ServerError, latencyMs: 100 },
    });
    const healthBefore = engine.providerHealth("openai");

    engine.upsertProvider({
      name: "openai",
      quotas: [{ dimension: Dimension.Requests, limit: 200, window: Window.Minute }],
      priority: 10,
      weight: 1.0,
      cooldownSeconds: 30,
    });
    expect(engine.providerHealth("openai")).toBe(healthBefore);
  });

  it("all providers in cooldown", () => {
    const { engine, clock } = setupEngine();
    for (const prov of ["openai", "anthropic"]) {
      for (let i = 0; i < 3; i++) {
        clock.advanceMs(1000);
        engine.observe({
          provider: prov,
          usage: { requests: 1 },
          outcome: { status: StatusClass.RateLimited, latencyMs: 50 },
        });
      }
    }
    expect(() => engine.select("chat")).toThrow(NoAvailableProviders);
  });

  it("provider in cooldown", () => {
    const { engine, clock } = setupEngine();
    expect(engine.providerInCooldown("openai")).toBe(false);

    for (let i = 0; i < 3; i++) {
      clock.advanceMs(1000);
      engine.observe({
        provider: "openai",
        usage: { requests: 1 },
        outcome: { status: StatusClass.RateLimited, latencyMs: 50 },
      });
    }
    expect(engine.providerInCooldown("openai")).toBe(true);
  });

  it("cooldown expires", () => {
    const { engine, clock } = setupEngine();
    for (let i = 0; i < 3; i++) {
      clock.advanceMs(1000);
      engine.observe({
        provider: "openai",
        usage: { requests: 1 },
        outcome: { status: StatusClass.RateLimited, latencyMs: 50 },
      });
    }
    expect(engine.providerInCooldown("openai")).toBe(true);
    clock.advanceSecs(31);
    expect(engine.providerInCooldown("openai")).toBe(false);
  });

  it("returns null for unknown provider", () => {
    const { engine } = setupEngine();
    expect(engine.providerHealth("nonexistent")).toBeNull();
    expect(engine.providerInCooldown("nonexistent")).toBeNull();
    expect(engine.providerQuotaRemaining("nonexistent", Dimension.Requests)).toBeNull();
  });
});
