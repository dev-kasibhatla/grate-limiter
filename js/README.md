# grate-limiter

[![npm version](https://img.shields.io/npm/v/@dev-kasibhatla/grate-limiter)](https://www.npmjs.com/package/@dev-kasibhatla/grate-limiter)
[![CI](https://github.com/dev-kasibhatla/grate-limiter/actions/workflows/ci.yml/badge.svg)](https://github.com/dev-kasibhatla/grate-limiter/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.x-blue)](https://www.typescriptlang.org/)

**Anticipatory rate-limit orchestration engine for multi-provider systems.**

Stop reacting to `429 Too Many Requests`. grate-limiter predicts quota exhaustion before it happens and routes requests to the best available provider — all within a single in-process call, with zero network overhead.

- **Zero dependencies** — pure TypeScript, no external runtime packages
- **Browser + Node.js** — ESM and CommonJS builds with full TypeScript declarations
- **Anticipatory routing** — scores every provider on quota, health, priority, and latency before each request
- **Automatic failover** — cooldown tracking with EWMA health decay means degraded providers are bypassed automatically
- **Deterministic testing** — built-in `MockClock` lets you simulate time-based behavior without `setTimeout`
- **Thread-safe equivalent** — single-threaded JS model; state mutations go through a single `observe()` path

Part of a multi-language monorepo — identical algorithm and conformance tests across [Rust](https://crates.io/crates/grate-limiter), [Python](https://pypi.org/project/grate-limiter/), and TypeScript.

---

## Installation

```bash
npm install @dev-kasibhatla/grate-limiter
```

```bash
yarn add @dev-kasibhatla/grate-limiter
```

```bash
pnpm add @dev-kasibhatla/grate-limiter
```

**Requirements:** Node.js 18+ or any modern browser. No build step required for browser ESM.

---

## Quick Start

```typescript
import {
  GrateLimiter,
  EngineConfig,
  ProviderConfig,
  CapabilityConfig,
  CapabilityProvider,
  QuotaConfig,
  Observation,
  Usage,
  Outcome,
  Dimension,
  Window,
  StatusClass,
} from "@dev-kasibhatla/grate-limiter";

// Create the engine
const engine = new GrateLimiter();

// Register providers with their rate-limit quotas
engine.upsertProvider({
  name: "openai",
  quotas: [{ dimension: Dimension.REQUESTS, limit: 5000, window: Window.MINUTE }],
  priority: 10,
  cooldownSeconds: 30,
});

engine.upsertProvider({
  name: "anthropic",
  quotas: [{ dimension: Dimension.REQUESTS, limit: 3000, window: Window.MINUTE }],
  priority: 8,
  cooldownSeconds: 30,
});

// Register a capability (logical operation that can be served by multiple providers)
engine.upsertCapability({
  name: "chat-completion",
  providers: [
    { provider: "openai", priority: 10 },
    { provider: "anthropic", priority: 8 },
  ],
});

// Select the best provider for the next request
const decision = engine.select("chat-completion");
console.log(`Use: ${decision.provider} (score: ${decision.score.toFixed(2)})`);
// → "Use: openai (score: 0.94)"

// After the request completes, report the outcome
engine.observe({
  provider: "openai",
  capability: "chat-completion",
  usage: { requests: 1, tokens: 1200 },
  outcome: { status: StatusClass.SUCCESS, latencyMs: 830 },
});
```

---

## Core Concepts

### Providers and Capabilities

A **provider** is a named upstream service (e.g. `"openai"`, `"anthropic"`) with associated rate-limit quotas. A **capability** is a logical operation (e.g. `"chat-completion"`, `"embeddings"`) that can be served by one or more providers.

```typescript
// Provider with multiple quota dimensions
engine.upsertProvider({
  name: "openai-gpt4",
  quotas: [
    { dimension: Dimension.REQUESTS, limit: 500, window: Window.MINUTE },
    { dimension: Dimension.TOKENS, limit: 150_000, window: Window.MINUTE },
    { dimension: Dimension.CONCURRENCY, limit: 20 },
  ],
  priority: 10,
  cooldownSeconds: 60,
});
```

### Scoring Algorithm

Every call to `select()` scores all eligible providers using a weighted formula:

```
score = quota_score × 0.40
      + health_score × 0.35
      + priority_score × 0.20
      + latency_score × 0.05
```

The provider with the highest score wins. Providers in cooldown or below minimum health are excluded entirely.

### Health Tracking

Health decays with each failure using an Exponential Weighted Moving Average (EWMA) and recovers gradually with successes. Providers that hit consecutive failures enter a **cooldown** period and are bypassed until it expires.

```typescript
// Observe a rate-limit response — health will decay, cooldown may trigger
engine.observe({
  provider: "openai",
  outcome: { status: StatusClass.RATE_LIMITED, latencyMs: 200 },
  usage: { requests: 1 },
});

// Check if a provider is currently in cooldown
const inCooldown = engine.providerInCooldown("openai");
const health = engine.providerHealth("openai"); // 0.0–1.0
```

### Quota Strategies

| Strategy | When to use |
|----------|-------------|
| `Dimension.REQUESTS` | Per-request rate limits (RPM / RPD) |
| `Dimension.TOKENS` | Token-based limits (TPM / TPD) |
| `Dimension.CONCURRENCY` | Max simultaneous in-flight requests |

---

## Deterministic Testing

Use `MockClock` to write fully deterministic tests — no real timers, no flakiness:

```typescript
import { GrateLimiter, MockClock, EngineConfig } from "@dev-kasibhatla/grate-limiter";
import { describe, it, expect } from "vitest";

describe("rate limit failover", () => {
  it("routes to backup after primary hits limit", () => {
    const clock = new MockClock();
    const engine = new GrateLimiter({ clock });

    engine.upsertProvider({
      name: "primary",
      quotas: [{ dimension: Dimension.REQUESTS, limit: 2, window: Window.MINUTE }],
      priority: 10,
      cooldownSeconds: 30,
    });
    engine.upsertProvider({
      name: "backup",
      quotas: [{ dimension: Dimension.REQUESTS, limit: 100, window: Window.MINUTE }],
      priority: 5,
      cooldownSeconds: 30,
    });
    engine.upsertCapability({
      name: "api",
      providers: [{ provider: "primary", priority: 10 }, { provider: "backup", priority: 5 }],
    });

    // Exhaust primary with rate-limited responses
    for (let i = 0; i < 3; i++) {
      clock.advanceMs(1000);
      engine.observe({
        provider: "primary",
        outcome: { status: StatusClass.RATE_LIMITED, latencyMs: 50 },
        usage: { requests: 1 },
      });
    }

    // Should now route to backup
    const decision = engine.select("api");
    expect(decision.provider).toBe("backup");

    // After cooldown expires, primary is eligible again
    clock.advanceSecs(60);
    const recovered = engine.select("api");
    expect(recovered.provider).toBe("primary");
  });
});
```

---

## API Reference

### `GrateLimiter`

```typescript
class GrateLimiter {
  constructor(config?: EngineConfig)

  // Register or update a provider and its quota configuration
  upsertProvider(config: ProviderConfig): void

  // Register or update a capability and its provider mappings
  upsertCapability(config: CapabilityConfig): void

  // Select the best provider for a capability
  // Throws NoAvailableProviders if all providers are in cooldown
  // Throws UnknownCapability if capability is not registered
  select(capability: string): Decision

  // Record the outcome of a completed request
  // Throws UnknownProvider if provider is not registered
  observe(obs: Observation): void

  // Query provider state
  providerHealth(provider: string): number | null
  providerInCooldown(provider: string): boolean
  remainingQuota(provider: string, dimension: Dimension): number | null

  // Get aggregate metrics
  metrics(): MetricsSnapshot
}
```

### `Decision`

```typescript
interface Decision {
  provider: string       // Chosen provider name
  score: number          // Composite score (0.0–1.0)
  alternatives: Alternative[]  // Other eligible providers, ranked
  breakdown: ScoreBreakdown    // Score components for observability
}
```

### `EngineConfig`

```typescript
interface EngineConfig {
  clock?: Clock               // Override for testing (use MockClock)
  scoring?: ScoringWeights    // Adjust score component weights
  health?: HealthConfig       // Tune EWMA decay and cooldown thresholds
}
```

### `MockClock`

```typescript
class MockClock {
  advanceMs(ms: number): void
  advanceSecs(secs: number): void
  now(): Timestamp
}
```

---

## CommonJS Usage

```javascript
const { GrateLimiter, Dimension, Window, StatusClass } = require("@dev-kasibhatla/grate-limiter");

const engine = new GrateLimiter();
engine.upsertProvider({
  name: "provider-a",
  quotas: [{ dimension: Dimension.REQUESTS, limit: 1000, window: Window.MINUTE }],
  priority: 10,
  cooldownSeconds: 30,
});
```

---

## Error Handling

```typescript
import { UnknownCapability, UnknownProvider, NoAvailableProviders } from "@dev-kasibhatla/grate-limiter";

try {
  const decision = engine.select("chat-completion");
  // use decision...
} catch (e) {
  if (e instanceof NoAvailableProviders) {
    // All providers are in cooldown or unhealthy
    // Implement circuit-breaker or return 503
  } else if (e instanceof UnknownCapability) {
    // Capability was never registered
  }
}
```

---

## Advanced Configuration

```typescript
import { GrateLimiter, defaultScoringWeights, defaultHealthConfig } from "@dev-kasibhatla/grate-limiter";

const engine = new GrateLimiter({
  scoring: {
    ...defaultScoringWeights(),
    quota: 0.50,    // Weight quota health more heavily
    health: 0.30,
    priority: 0.15,
    latency: 0.05,
  },
  health: {
    ...defaultHealthConfig(),
    ewmaAlpha: 0.3,                   // Faster decay on failures
    cooldownThreshold: 0.2,           // Enter cooldown below 20% health
    minHealthForSelection: 0.1,       // Exclude below 10%
    maxCooldownSecs: 300,             // Cap cooldown at 5 minutes
  },
});
```

---

## Contributing

Issues and pull requests are welcome at **[github.com/dev-kasibhatla/grate-limiter](https://github.com/dev-kasibhatla/grate-limiter)**.

- [Open an issue](https://github.com/dev-kasibhatla/grate-limiter/issues/new)
- [Browse existing issues](https://github.com/dev-kasibhatla/grate-limiter/issues)
- [Read the implementation spec](https://github.com/dev-kasibhatla/grate-limiter/blob/master/docs/product-spec.md)

---

## Related

- [Rust crate](https://crates.io/crates/grate-limiter) — the original, highest-performance implementation
- [Python package](https://pypi.org/project/grate-limiter/) — identical algorithm for Python services
- [GitHub repository](https://github.com/dev-kasibhatla/grate-limiter) — monorepo with all three implementations

---

## License

Apache-2.0 © [Aditya Kasibhatla](https://github.com/dev-kasibhatla)
