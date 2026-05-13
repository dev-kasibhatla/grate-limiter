# grate-limiter

[![CI](https://github.com/dev-kasibhatla/grate-limiter/actions/workflows/ci.yml/badge.svg)](https://github.com/dev-kasibhatla/grate-limiter/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/grate-limiter.svg)](https://crates.io/crates/grate-limiter)
[![PyPI](https://img.shields.io/pypi/v/grate-limiter.svg)](https://pypi.org/project/grate-limiter/)
[![npm](https://img.shields.io/npm/v/@dev-kasibhatla/grate-limiter.svg)](https://www.npmjs.com/package/@dev-kasibhatla/grate-limiter)
[![docs.rs](https://docs.rs/grate-limiter/badge.svg)](https://docs.rs/grate-limiter)
[![codecov](https://codecov.io/gh/dev-kasibhatla/grate-limiter/branch/main/graph/badge.svg)](https://codecov.io/gh/dev-kasibhatla/grate-limiter)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

**Predict limits before providers enforce them.**

`grate-limiter` is an anticipatory rate-limit orchestration engine for multi-provider systems. It routes traffic intelligently across providers by continuously learning their health, quota usage, and reliability — preventing rate-limit errors before they happen.

## What it is

- A **predictive provider orchestration engine**
- An **adaptive quota router** with anticipatory exhaustion detection
- A **provider health tracker** with EWMA-based scoring

## What it is NOT

- Not a retry library
- Not a proxy or gateway
- Not a simple rate limiter

## Features

- **Anticipatory routing** — predicts quota exhaustion before it happens using burn rate analysis
- **Multiple quota strategies** — token bucket, sliding window, fixed window, concurrency limiter
- **Health engine** — EWMA-based scoring with exponential decay and automatic cooldowns
- **Weighted composite scoring** — configurable weights for quota, health, priority, and latency
- **Deterministic testing** — `MockClock` for fully reproducible test scenarios
- **Zero-allocation hot paths** — designed for `<10µs` select latency
- **Thread-safe** — lock-free atomic operations on the hot path
- **Explainable decisions** — full score breakdown with every selection

## Quick Start

<details open>
<summary><strong>Rust</strong></summary>

Add to your `Cargo.toml`:

```toml
[dependencies]
grate-limiter = "0.1"
```

```rust
use grate_limiter::*;

// Create the engine
let engine = GrateLimiter::new(EngineConfig::default());

// Register providers with their quotas
engine.upsert_provider(ProviderConfig {
    name: "openai".into(),
    quotas: vec![
        QuotaConfig { dimension: Dimension::Requests, limit: 5000, window: Some(Window::Minute) },
        QuotaConfig { dimension: Dimension::Tokens, limit: 90000, window: Some(Window::Minute) },
    ],
    priority: 10,
    weight: 1.0,
    cooldown_seconds: 30,
});

engine.upsert_provider(ProviderConfig {
    name: "anthropic".into(),
    quotas: vec![
        QuotaConfig { dimension: Dimension::Requests, limit: 3000, window: Some(Window::Minute) },
    ],
    priority: 8,
    weight: 1.0,
    cooldown_seconds: 30,
});

// Register a capability
engine.upsert_capability(CapabilityConfig {
    name: "chat-completion".into(),
    providers: vec![
        CapabilityProvider { provider: "openai".into(), priority: 10 },
        CapabilityProvider { provider: "anthropic".into(), priority: 8 },
    ],
});

// Select the best provider
let decision = engine.select("chat-completion").unwrap();
println!("Use: {} (score: {:.2})", decision.provider, decision.score);

// Report what happened
engine.observe(Observation {
    provider: "openai".into(),
    capability: Some("chat-completion".into()),
    usage: Usage { requests: 1, tokens: Some(1200), ..Default::default() },
    outcome: Outcome { status: StatusClass::Success, latency_ms: 830 },
}).unwrap();
```

</details>

<details>
<summary><strong>Python</strong></summary>

```bash
pip install grate-limiter
```

```python
from grate_limiter import *

engine = GrateLimiter(EngineConfig())

engine.upsert_provider(ProviderConfig(
    name="openai",
    quotas=[QuotaConfig(dimension=Dimension.REQUESTS, limit=5000, window=Window.MINUTE)],
    priority=10, cooldown_seconds=30,
))
engine.upsert_provider(ProviderConfig(
    name="anthropic",
    quotas=[QuotaConfig(dimension=Dimension.REQUESTS, limit=3000, window=Window.MINUTE)],
    priority=8, cooldown_seconds=30,
))

engine.upsert_capability(CapabilityConfig(
    name="chat-completion",
    providers=[
        CapabilityProvider(provider="openai", priority=10),
        CapabilityProvider(provider="anthropic", priority=8),
    ],
))

decision = engine.select("chat-completion")
print(f"Use: {decision.provider} (score: {decision.score:.2f})")

engine.observe(Observation(
    provider="openai", capability="chat-completion",
    usage=Usage(requests=1, tokens=1200),
    outcome=Outcome(status=StatusClass.SUCCESS, latency_ms=830),
))
```

</details>

<details>
<summary><strong>JavaScript / TypeScript</strong></summary>

```bash
npm install @dev-kasibhatla/grate-limiter
```

```typescript
import { GrateLimiter, Dimension, Window, StatusClass } from '@dev-kasibhatla/grate-limiter';

const engine = new GrateLimiter();

engine.upsertProvider({
  name: 'openai',
  quotas: [{ dimension: Dimension.Requests, limit: 5000, window: Window.Minute }],
  priority: 10, cooldownSeconds: 30,
});
engine.upsertProvider({
  name: 'anthropic',
  quotas: [{ dimension: Dimension.Requests, limit: 3000, window: Window.Minute }],
  priority: 8, cooldownSeconds: 30,
});

engine.upsertCapability({
  name: 'chat-completion',
  providers: [
    { provider: 'openai', priority: 10 },
    { provider: 'anthropic', priority: 8 },
  ],
});

const decision = engine.select('chat-completion');
console.log(`Use: ${decision.provider} (score: ${decision.score.toFixed(2)})`);

engine.observe({
  provider: 'openai', capability: 'chat-completion',
  usage: { requests: 1, tokens: 1200 },
  outcome: { status: StatusClass.Success, latencyMs: 830 },
});
```

</details>

## How It Works

### Scoring Algorithm

Each provider is scored using a weighted composite of four factors:

| Component | Weight | Description |
|-----------|--------|-------------|
| Quota | 40% | Remaining capacity + anticipatory exhaustion penalty |
| Health | 35% | EWMA-based health score from observed outcomes |
| Priority | 20% | Capability-level provider preference |
| Latency | 5% | Normalized response time |

### Anticipatory Routing

Instead of the traditional approach:

```
send → fail with 429 → retry elsewhere
```

grate-limiter does:

```
predict nearing exhaustion → avoid provider before failure
```

The engine tracks burn rate and predicts time-to-exhaustion. Providers with imminent exhaustion are aggressively deprioritized even if they still have remaining capacity.

### Health Engine

Provider health is tracked via EWMA (Exponentially Weighted Moving Average):

- **Successes** boost health slightly
- **429s** apply a major penalty (-0.25)
- **403s** apply a severe penalty (-0.50)
- **5xx/timeouts** apply moderate penalties
- Health **decays toward 1.0** over time (recovery)
- Consecutive failures trigger **exponential cooldowns**

## HTTP Server

For use from non-Rust services, the `grate-limiter-server` crate provides a REST API:

```bash
cargo install grate-limiter-server
grate-limiter-server  # Starts on :3000
```

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/providers` | POST | Register/update a provider |
| `/capabilities` | POST | Register/update a capability |
| `/select` | POST | Select the best provider |
| `/observe` | POST | Report an observation |
| `/metrics` | GET | Engine metrics |
| `/health` | GET | Health check |

## Packages

| Package | Registry | Description |
|---------|----------|-------------|
| [`grate-limiter`](crates/grate-limiter/) | [crates.io](https://crates.io/crates/grate-limiter) | Core Rust library |
| [`grate-limiter-server`](crates/grate-limiter-server/) | [crates.io](https://crates.io/crates/grate-limiter-server) | HTTP server |
| [`grate-limiter-simulation`](crates/grate-limiter-simulation/) | [crates.io](https://crates.io/crates/grate-limiter-simulation) | Simulation framework |
| [`grate-limiter`](python/) | [PyPI](https://pypi.org/project/grate-limiter/) | Python port |
| [`@dev-kasibhatla/grate-limiter`](js/) | [npm](https://www.npmjs.com/package/@dev-kasibhatla/grate-limiter) | TypeScript port |

## Examples

```bash
cargo run --example simple_routing -p grate-limiter
cargo run --example ai_provider_balancing -p grate-limiter
cargo run --example hidden_quota_learning -p grate-limiter
cargo run --example sms_failover -p grate-limiter
cargo run --example scraping_proxy_rotation -p grate-limiter
```

## Real-World Use Cases

- **AI APIs** — OpenAI / Anthropic / Gemini load balancing
- **Web scraping** — Multi-proxy vendor rotation
- **SMS delivery** — Twilio / MessageBird / Nexmo failover
- **CAPTCHA solving** — Multi-provider orchestration
- **Search APIs** — SerpAPI / BrightData balancing

## Deterministic Testing

Use `MockClock` for reproducible tests:

```rust
use grate_limiter::{GrateLimiter, EngineConfig, MockClock, Clock};
use std::sync::Arc;

let clock = Arc::new(MockClock::new());
let config = EngineConfig::default().with_clock(clock.clone());
let engine = GrateLimiter::new(config);

// Time only advances when you say so
clock.advance_ms(5000);
clock.advance_secs(60);
```

## Benchmarks

Run benchmarks locally:

```bash
cargo bench -p grate-limiter
```

Performance targets:

| Metric | Target |
|--------|--------|
| `select()` p99 | <50µs |
| `observe()` p99 | <20µs |
| Memory per provider | <4KB |
| Routing determinism | 100% |

## Roadmap

- [x] Core anticipatory engine
- [x] Token bucket / sliding window / fixed window / concurrency quotas
- [x] EWMA health scoring with cooldowns
- [x] HTTP server
- [x] Simulation framework
- [x] Property-based testing
- [x] Python port (native) — [PyPI](https://pypi.org/project/grate-limiter/)
- [x] JavaScript/TypeScript port (native) — [npm](https://www.npmjs.com/package/@dev-kasibhatla/grate-limiter)
- [x] Cross-language conformance tests
- [ ] Distributed state (Redis backend)
- [ ] Persistent snapshots
- [ ] WASM builds
- [ ] Adaptive hidden-quota learning

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.
