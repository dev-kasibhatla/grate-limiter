# grate-limiter (Python)

Anticipatory rate-limit orchestration engine for multi-provider systems.

**Predict limits before providers enforce them.**

This is the Python port of [grate-limiter](https://github.com/dev-kasibhatla/grate-limiter). For detailed documentation, see the main repository.

## Installation

```bash
pip install grate-limiter
```

## Quick Start

```python
from grate_limiter import *

# Create the engine
engine = GrateLimiter(EngineConfig())

# Register providers with their quotas
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

# Register a capability
engine.upsert_capability(CapabilityConfig(
    name="chat-completion",
    providers=[
        CapabilityProvider(provider="openai", priority=10),
        CapabilityProvider(provider="anthropic", priority=8),
    ],
))

# Select the best provider
decision = engine.select("chat-completion")
print(f"Use: {decision.provider} (score: {decision.score:.2f})")

# Report what happened
engine.observe(Observation(
    provider="openai",
    capability="chat-completion",
    usage=Usage(requests=1, tokens=1200),
    outcome=Outcome(status=StatusClass.SUCCESS, latency_ms=830),
))
```

## Deterministic Testing

```python
from grate_limiter import GrateLimiter, EngineConfig, MockClock

clock = MockClock()
config = EngineConfig().with_clock(clock)
engine = GrateLimiter(config)

# Time only advances when you say so
clock.advance_ms(5000)
clock.advance_secs(60)
```

## License

Apache-2.0
