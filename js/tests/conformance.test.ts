import { readFileSync, readdirSync } from "fs";
import { join, resolve } from "path";
import { describe, expect, it } from "vitest";
import { MockClock } from "../src/clock.js";
import { GrateLimiter } from "../src/engine.js";
import { Dimension, StatusClass, Window } from "../src/types.js";
import type { EngineConfig } from "../src/config.js";

const CONFORMANCE_DIR = resolve(__dirname, "../../tests/conformance");

const DIMENSION_MAP: Record<string, Dimension> = {
  requests: Dimension.Requests,
  tokens: Dimension.Tokens,
  concurrency: Dimension.Concurrency,
  cost_usd: Dimension.CostUsd,
  bytes: Dimension.Bytes,
};

const WINDOW_MAP: Record<string, Window> = {
  second: Window.Second,
  minute: Window.Minute,
  hour: Window.Hour,
  day: Window.Day,
};

const STATUS_MAP: Record<string, StatusClass> = {
  success: StatusClass.Success,
  rate_limited: StatusClass.RateLimited,
  forbidden: StatusClass.Forbidden,
  server_error: StatusClass.ServerError,
  timeout: StatusClass.Timeout,
  client_error: StatusClass.ClientError,
};

interface TestData {
  name: string;
  description: string;
  config: Record<string, unknown>;
  providers: Array<Record<string, unknown>>;
  capabilities: Array<Record<string, unknown>>;
  steps: Array<Record<string, unknown>>;
}

function loadTestFiles(): Array<{ name: string; data: TestData }> {
  const files = readdirSync(CONFORMANCE_DIR)
    .filter((f) => f.endsWith(".json"))
    .sort();
  return files.map((f) => {
    const data = JSON.parse(readFileSync(join(CONFORMANCE_DIR, f), "utf-8")) as TestData;
    return { name: data.name, data };
  });
}

function buildEngine(data: TestData): { engine: GrateLimiter; clock: MockClock } {
  const cfg = data.config ?? {};
  const scoringData = (cfg.scoring ?? {}) as Record<string, number>;
  const healthData = (cfg.health ?? {}) as Record<string, number>;

  const clock = new MockClock();
  const engineConfig: EngineConfig = {
    scoring: {
      quota: scoringData.quota ?? 0.4,
      health: scoringData.health ?? 0.35,
      priority: scoringData.priority ?? 0.2,
      latency: scoringData.latency ?? 0.05,
    },
    health: {
      decayHalfLifeSeconds: healthData.decay_half_life_seconds ?? 300,
      penalty429: healthData.penalty_429 ?? 0.25,
      penalty403: healthData.penalty_403 ?? 0.5,
      penalty5xx: healthData.penalty_5xx ?? 0.1,
      penaltyTimeout: healthData.penalty_timeout ?? 0.2,
      boostSuccess: healthData.boost_success ?? 0.02,
      cooldownTriggerCount: healthData.cooldown_trigger_count ?? 3,
      cooldownMultiplier: healthData.cooldown_multiplier ?? 2.0,
      maxCooldownSeconds: healthData.max_cooldown_seconds ?? 600,
    },
    minimumHealthScore: (cfg.minimum_health_score as number) ?? 0.2,
    defaultCooldownSeconds: (cfg.default_cooldown_seconds as number) ?? 60,
    clock,
  };

  const engine = new GrateLimiter(engineConfig);

  for (const prov of data.providers ?? []) {
    const quotas = ((prov.quotas as Array<Record<string, unknown>>) ?? []).map((q) => ({
      dimension: DIMENSION_MAP[q.dimension as string]!,
      limit: q.limit as number,
      window: q.window ? WINDOW_MAP[q.window as string] : undefined,
    }));
    engine.upsertProvider({
      name: prov.name as string,
      quotas,
      priority: (prov.priority as number) ?? 10,
      weight: (prov.weight as number) ?? 1.0,
      cooldownSeconds: (prov.cooldown_seconds as number) ?? 60,
    });
  }

  for (const cap of data.capabilities ?? []) {
    const providers = ((cap.providers as Array<Record<string, unknown>>) ?? []).map((cp) => ({
      provider: cp.provider as string,
      priority: cp.priority as number,
    }));
    engine.upsertCapability({
      name: cap.name as string,
      providers,
    });
  }

  return { engine, clock };
}

function runSteps(engine: GrateLimiter, clock: MockClock, steps: Array<Record<string, unknown>>): void {
  for (let i = 0; i < steps.length; i++) {
    const step = steps[i]!;
    const action = step.action as string;
    const desc = `step ${i}: ${action}`;

    if (action === "advance_time_ms") {
      clock.advanceMs(step.ms as number);
    } else if (action === "observe") {
      const status = STATUS_MAP[step.status as string]!;
      engine.observe({
        provider: step.provider as string,
        capability: step.capability as string | undefined,
        usage: {
          requests: (step.requests as number) ?? 0,
          tokens: step.tokens as number | undefined,
          bytes: step.bytes as number | undefined,
        },
        outcome: { status, latencyMs: (step.latency_ms as number) ?? 0 },
      });
    } else if (action === "select") {
      const decision = engine.select(step.capability as string);
      if (step.expect_provider !== undefined) {
        expect(decision.provider, desc).toBe(step.expect_provider);
      }
      if (step.expect_score_min !== undefined) {
        expect(decision.score, `${desc}: score too low`).toBeGreaterThanOrEqual(
          (step.expect_score_min as number) - 0.001,
        );
      }
      if (step.expect_score_max !== undefined) {
        expect(decision.score, `${desc}: score too high`).toBeLessThanOrEqual(
          (step.expect_score_max as number) + 0.001,
        );
      }
      if (step.expect_alternatives_count !== undefined) {
        expect(decision.alternatives.length, desc).toBe(step.expect_alternatives_count);
      }
    } else if (action === "check_health") {
      const health = engine.providerHealth(step.provider as string);
      expect(health, desc).not.toBeNull();
      if (step.expect_min !== undefined) {
        expect(health!, `${desc}: health too low`).toBeGreaterThanOrEqual(
          (step.expect_min as number) - 0.001,
        );
      }
      if (step.expect_max !== undefined) {
        expect(health!, `${desc}: health too high`).toBeLessThanOrEqual(
          (step.expect_max as number) + 0.001,
        );
      }
    } else if (action === "check_remaining") {
      const dim = DIMENSION_MAP[step.dimension as string]!;
      const remaining = engine.providerQuotaRemaining(step.provider as string, dim);
      expect(remaining, desc).not.toBeNull();
      expect(remaining!, desc).toBe(step.expect);
    } else if (action === "check_in_cooldown") {
      const inCooldown = engine.providerInCooldown(step.provider as string);
      expect(inCooldown, desc).not.toBeNull();
      expect(inCooldown!, desc).toBe(step.expect);
    } else {
      throw new Error(`Unknown action: ${action}`);
    }
  }
}

const testCases = loadTestFiles();

describe("Conformance Tests", () => {
  for (const { name, data } of testCases) {
    it(name, () => {
      const { engine, clock } = buildEngine(data);
      runSteps(engine, clock, data.steps);
    });
  }
});
