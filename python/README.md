# grate-limiter

[![PyPI version](https://img.shields.io/pypi/v/grate-limiter)](https://pypi.org/project/grate-limiter/)
[![Python versions](https://img.shields.io/pypi/pyversions/grate-limiter)](https://pypi.org/project/grate-limiter/)
[![CI](https://github.com/dev-kasibhatla/grate-limiter/actions/workflows/ci.yml/badge.svg)](https://github.com/dev-kasibhatla/grate-limiter/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

**Anticipatory rate-limit orchestration engine for multi-provider systems.**

Stop reacting to `429 Too Many Requests`. grate-limiter predicts quota exhaustion before it happens and routes requests to the best available provider — all in-process, with zero network overhead.

- **Anticipatory routing** — scores every provider on quota, health, priority, and latency before each request
- **Automatic failover** — cooldown tracking with EWMA health decay means degraded providers are bypassed automatically
- **Multiple quota dimensions** — requests-per-minute, tokens-per-minute, concurrency limits, all at once
- **Thread-safe** — uses `threading.Lock` internally; safe to call from multiple threads
- **Deterministic testing** — built-in `MockClock` lets you simulate time-based behavior in unit tests
- **Fully typed** — ships with `py.typed` marker; works with mypy strict mode

Part of a multi-language monorepo — identical algorithm and conformance tests across [Rust](https://crates.io/crates/grate-limiter), Python, and [TypeScript](https://www.npmjs.com/package/@dev-kasibhatla/grate-limiter).

---

## Installation

```bash
pip install grate-limiter
```

**Requirements:** Python 3.10+. No external runtime dependencies.

---

## Quick Start

```python
from grate_limiter import (
    GrateLimiter, EngineConfig,
    ProviderConfig, CapabilityConfig, CapabilityProvider,
    QuotaConfig, Observation, Usage, Outcome,
    Dimension, Window, StatusClass,
)

# Create the engine
engine = GrateLimiter(EngineConfig())

# Register providers with their rate-limit quotas
engine.upsert_provider(ProviderConfig(
    name="openai",
    quotas=[QuotaConfig(dimension=Dimension.REQUESTS, limit=5000, window=Window.MINUTE)],
    priority=10,
    cooldown_seconds=30,
))

engine.upsert_provider(ProviderConfig(
    name="anthropic",
    quotas=[QuotaConfig(dimension=Dimension.REQUESTS, limit=3000, window=Window.MINUTE)],
    priority=8,
    cooldown_seconds=30,
))

# Register a capability (logical operation served by multiple providers)
engine.upsert_capability(CapabilityConfig(
    name="chat-completion",
    providers=[
        CapabilityProvider(provider="openai", priority=10),
        CapabilityProvider(provider="anthropic", priority=8),
    ],
))

# Select the best provider for the next request
decision = engine.select("chat-completion")
print(f"Use: {decision.provider} (score: {decision.score:.2f})")
# → "Use: openai (score: 0.94)"

# After the request completes, report the outcome
engine.observe(Observation(
    provider="openai",
    capability="chat-completion",
    usage=Usage(requests=1, tokens=1200),
    outcome=Outcome(status=StatusClass.SUCCESS, latency_ms=830),
))
```

---

## Core Concepts

### Providers and Capabilities

A **provider** is a named upstream service (e.g. `"openai"`, `"anthropic"`) with associated rate-limit quotas. A **capability** is a logical operation (e.g. `"chat-completion"`, `"embeddings"`) that can be served by one or more providers.

```python
# Provider with multiple quota dimensions
engine.upsert_provider(ProviderConfig(
    name="openai-gpt4",
    quotas=[
        QuotaConfig(dimension=Dimension.REQUESTS, limit=500, window=Window.MINUTE),
        QuotaConfig(dimension=Dimension.TOKENS, limit=150_000, window=Window.MINUTE),
        QuotaConfig(dimension=Dimension.CONCURRENCY, limit=20),
    ],
    priority=10,
    cooldown_seconds=60,
))
```

### Scoring Algorithm

Every call to `select()` scores all eligible providers using a weighted formula:

```
score = quota_score  × 0.40
      + health_score × 0.35
      + priority_score × 0.20
      + latency_score  × 0.05
```

The provider with the highest score wins. Providers in cooldown or below minimum health are excluded entirely.

### Health Tracking

Health decays with each failure using an Exponential Weighted Moving Average (EWMA) and recovers gradually with successes. Providers that hit consecutive failures enter a **cooldown** period and are bypassed until it expires.

```python
# Observe a rate-limit response — health decays, cooldown may trigger
engine.observe(Observation(
    provider="openai",
    outcome=Outcome(status=StatusClass.RATE_LIMITED, latency_ms=200),
    usage=Usage(requests=1),
))

# Query provider state
in_cooldown = engine.provider_in_cooldown("openai")   # bool
health = engine.provider_health("openai")              # 0.0–1.0 or None
remaining = engine.remaining_quota("openai", Dimension.REQUESTS)  # int or None
```

### Quota Strategies

| Strategy | When to use |
|----------|-------------|
| `Dimension.REQUESTS` | Per-request rate limits (RPM / RPD) |
| `Dimension.TOKENS` | Token-based limits (TPM / TPD) |
| `Dimension.CONCURRENCY` | Max simultaneous in-flight requests |

---

## Deterministic Testing

Use `MockClock` to write fully deterministic tests — no real timers, no `time.sleep()`:

```python
import pytest
from grate_limiter import (
    GrateLimiter, EngineConfig, MockClock,
    ProviderConfig, CapabilityConfig, CapabilityProvider,
    QuotaConfig, Observation, Usage, Outcome,
    Dimension, Window, StatusClass, NoAvailableProviders,
)

def test_failover_after_rate_limit():
    clock = MockClock()
    engine = GrateLimiter(EngineConfig(clock=clock))

    engine.upsert_provider(ProviderConfig(
        name="primary",
        quotas=[QuotaConfig(dimension=Dimension.REQUESTS, limit=2, window=Window.MINUTE)],
        priority=10, cooldown_seconds=30,
    ))
    engine.upsert_provider(ProviderConfig(
        name="backup",
        quotas=[QuotaConfig(dimension=Dimension.REQUESTS, limit=100, window=Window.MINUTE)],
        priority=5, cooldown_seconds=30,
    ))
    engine.upsert_capability(CapabilityConfig(
        name="api",
        providers=[
            CapabilityProvider(provider="primary", priority=10),
            CapabilityProvider(provider="backup", priority=5),
        ],
    ))

    # Exhaust primary with rate-limited responses
    for _ in range(3):
        clock.advance_ms(1000)
        engine.observe(Observation(
            provider="primary",
            outcome=Outcome(status=StatusClass.RATE_LIMITED, latency_ms=50),
            usage=Usage(requests=1),
        ))

    # Should now route to backup
    decision = engine.select("api")
    assert decision.provider == "backup"

    # After cooldown expires, primary is eligible again
    clock.advance_secs(60)
    recovered = engine.select("api")
    assert recovered.provider == "primary"
```

---

## API Reference

### `GrateLimiter`

```python
class GrateLimiter:
    def __init__(self, config: EngineConfig | None = None) -> None

    # Register or update a provider and its quota configuration
    def upsert_provider(self, config: ProviderConfig) -> None

    # Register or update a capability and its provider mappings
    def upsert_capability(self, config: CapabilityConfig) -> None

    # Select the best provider for a capability.
    # Raises UnknownCapability if capability is not registered.
    # Raises NoAvailableProviders if all providers are in cooldown.
    def select(self, capability: str) -> Decision

    # Record the outcome of a completed request.
    # Raises UnknownProvider if provider is not registered.
    def observe(self, obs: Observation) -> None

    # Query provider state
    def provider_health(self, provider: str) -> float | None
    def provider_in_cooldown(self, provider: str) -> bool
    def remaining_quota(self, provider: str, dimension: Dimension) -> int | None
```

### `Decision`

```python
@dataclass
class Decision:
    provider: str               # Chosen provider name
    score: float                # Composite score (0.0–1.0)
    alternatives: list[Alternative]   # Other eligible providers, ranked
    breakdown: ScoreBreakdown         # Score components for observability
```

### `EngineConfig`

```python
@dataclass
class EngineConfig:
    clock: Clock | None = None           # Override for testing (use MockClock)
    scoring: ScoringWeights | None = None
    health: HealthConfig | None = None
```

---

## Advanced Configuration

```python
from grate_limiter import GrateLimiter, EngineConfig, ScoringWeights, HealthConfig

engine = GrateLimiter(EngineConfig(
    scoring=ScoringWeights(
        quota=0.50,     # Weight quota health more heavily
        health=0.30,
        priority=0.15,
        latency=0.05,
    ),
    health=HealthConfig(
        ewma_alpha=0.3,                  # Faster decay on failures
        cooldown_threshold=0.2,          # Enter cooldown below 20% health
        min_health_for_selection=0.1,    # Exclude below 10%
        max_cooldown_secs=300,           # Cap cooldown at 5 minutes
    ),
))
```

---

## Error Handling

```python
from grate_limiter import UnknownCapability, UnknownProvider, NoAvailableProviders

try:
    decision = engine.select("chat-completion")
    # use decision...
except NoAvailableProviders:
    # All providers are in cooldown or unhealthy
    # Implement circuit-breaker or return 503
    raise
except UnknownCapability:
    # Capability was never registered
    raise
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
- [TypeScript package](https://www.npmjs.com/package/@dev-kasibhatla/grate-limiter) — identical algorithm for browser and Node.js
- [GitHub repository](https://github.com/dev-kasibhatla/grate-limiter) — monorepo with all three implementations

---

## License

Apache-2.0 © [Aditya Kasibhatla](https://github.com/dev-kasibhatla)

