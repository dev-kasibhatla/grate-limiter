# Grate-Limiter

A lightweight anticipatory rate-limit orchestration engine for multi-provider systems.

Purpose:

* Prevent hitting third-party limits before they happen
* Route traffic intelligently across providers
* Continuously learn provider health and reliability
* Provide deterministic and debuggable decisions
* Remain extremely simple externally while highly optimized internally

---

# Core Philosophy

## External API

* Tiny
* Predictable
* Stateless APIs over stateful in-memory engine
* Easy to embed
* Easy to port to Rust/Python/JS

## Internal Engine

* Fast
* Lock-efficient
* Deterministic
* Time-window aware
* Quality aware
* Future extensible

---

# Terminology

| Old          | Improved    |
| ------------ | ----------- |
| Operation    | Capability  |
| Service      | Provider    |
| Rate Limit   | Quota       |
| Usage Report | Observation |
| Quality      | Health      |
| Priority     | Preference  |

Recommended terminology:

* Capability = what caller wants
* Provider = external system that can fulfill capability

Example:

```txt
Capability: image-generation

Providers:
- openai
- stability
- replicate
```

---

# High Level Architecture

```txt
                +----------------+
                | Client Apps    |
                +--------+-------+
                         |
                         v
              +----------+----------+
              | Grate-Limiter API   |
              +----------+----------+
                         |
         +---------------+----------------+
         |                                |
         v                                v
+-------------------+        +----------------------+
| Config Store      |        | Runtime State Engine |
| (in memory)       |        | counters + health    |
+-------------------+        +----------------------+
                                         |
                                         v
                           +--------------------------+
                           | Decision Engine          |
                           | provider scoring/ranking |
                           +--------------------------+
```

---

# Primary Responsibilities

## 1. Quota Tracking

Track all provider quotas:

* requests/minute
* requests/day
* tokens/minute
* bandwidth/day
* concurrency
* cost budgets
* burst windows

---

## 2. Anticipatory Routing

Instead of:

```txt
send -> fail with 429 -> retry elsewhere
```

Do:

```txt
predict nearing exhaustion -> avoid provider
```

This is the core innovation.

---

## 3. Provider Health Learning

Continuously learn:

* 429 frequency
* 403 bans
* 500 instability
* latency degradation
* timeout rate
* success ratio

---

## 4. Smart Selection

Return:

```json
{
  "provider": "openai",
  "confidence": 0.93,
  "reasoning": {
    "quota_score": 0.98,
    "health_score": 0.95,
    "priority_score": 0.8
  }
}
```

---

# Data Model

Keep this intentionally simple.

---

# Capability

```json
{
  "name": "chat-completion",
  "providers": [
    {
      "provider": "openai",
      "priority": 10
    },
    {
      "provider": "anthropic",
      "priority": 8
    }
  ]
}
```

Rules:

* capability names unique
* provider names unique globally
* same provider reused across capabilities
* provider state shared globally

---

# Provider

```json
{
  "name": "openai",
  "quotas": [
    {
      "dimension": "requests",
      "limit": 5000,
      "window": "minute"
    },
    {
      "dimension": "tokens",
      "limit": 90000,
      "window": "minute"
    }
  ],
  "settings": {
    "cooldown_seconds": 30,
    "weight": 1.0
  }
}
```

---

# Supported Quota Types

## Request Based

```json
{
  "dimension": "requests",
  "limit": 100,
  "window": "minute"
}
```

---

## Token Based

```json
{
  "dimension": "tokens",
  "limit": 100000,
  "window": "minute"
}
```

---

## Concurrency Based

```json
{
  "dimension": "concurrency",
  "limit": 20
}
```

---

## Cost Based

```json
{
  "dimension": "cost_usd",
  "limit": 100,
  "window": "day"
}
```

---

## Bandwidth Based

```json
{
  "dimension": "bytes",
  "limit": 1000000000,
  "window": "day"
}
```

---

# Observation API

Users report usage and outcomes.

This is critical.

---

# Request Observation

```json
{
  "provider": "openai",
  "capability": "chat-completion",
  "usage": {
    "requests": 1,
    "tokens": 1200
  },
  "response": {
    "status_code": 200,
    "latency_ms": 830
  }
}
```

---

# Why This Matters

The system learns:

* effective limits
* hidden throttling
* instability
* degradation patterns
* bans
* latency spikes

---

# Health Model

Health score:

```txt
0.0 -> unusable
1.0 -> perfect
```

Built from weighted metrics.

---

# Suggested Health Signals

| Signal        | Impact            |
| ------------- | ----------------- |
| 200 success   | positive          |
| 429           | major negative    |
| 403           | severe negative   |
| 5xx           | negative          |
| timeout       | negative          |
| latency spike | moderate negative |

---

# Example Health Weights

| Event   | Penalty |
| ------- | ------- |
| 429     | -0.25   |
| 403     | -0.5    |
| timeout | -0.2    |
| 500     | -0.1    |

Decay penalties over time.

A provider should recover naturally.

---

# Runtime State

Stored entirely in memory.

---

# Internal Provider State

```rust
struct ProviderRuntime {
    health_score: f32,
    quotas: Vec<QuotaRuntime>,
    rolling_stats: RollingStats,
    cooldown_until: Option<Instant>,
}
```

---

# Quota Runtime

```rust
struct QuotaRuntime {
    used: AtomicU64,
    window_start: Instant,
}
```

---

# Sliding Window Strategy

Do NOT use naive reset-at-minute-boundary logic.

Use:

* rolling windows
* token buckets
* leaky buckets

Recommended:

* token bucket internally
* fixed-window compatibility externally

---

# Provider Selection Algorithm

Core differentiator.

---

# Scoring Components

| Component           | Weight |
| ------------------- | ------ |
| Remaining quota     | 40%    |
| Health score        | 35%    |
| Capability priority | 20%    |
| Latency             | 5%     |

---

# Final Score

```txt
score =
quota_score * 0.40 +
health_score * 0.35 +
priority_score * 0.20 +
latency_score * 0.05
```

---

# Quota Score

Should be anticipatory.

Not:

```txt
remaining > 0
```

Instead:

```txt
remaining percentage
+
recent burn rate
+
predicted exhaustion
```

Example:

* provider at 80% usage
* but accelerating rapidly
* reduce score early

---

# Predicted Exhaustion

Estimate:

```txt
time_until_exhaustion
```

Using:

```txt
current_rate_of_consumption
```

If projected exhaustion < threshold:

* aggressively deprioritize

This is the anticipatory core.

---

# Example Selection Flow

## Providers

| Provider  | Health | Remaining | Priority |
| --------- | ------ | --------- | -------- |
| openai    | 0.95   | 10%       | 10       |
| anthropic | 0.90   | 70%       | 8        |

Despite higher priority:

* openai may lose because exhaustion is imminent.

---

# API Surface

---

# Create Provider

```http
POST /providers
```

---

# Upsert Provider

```http
PUT /providers/{name}
```

Partial updates allowed.

---

# Patch Provider Quotas

```http
PATCH /providers/{name}/quotas
```

Granular quota upserts.

---

# Create Capability

```http
POST /capabilities
```

---

# Upsert Capability Provider

```http
PUT /capabilities/{capability}/providers/{provider}
```

---

# Select Provider

```http
POST /select
```

Request:

```json
{
  "capability": "chat-completion"
}
```

Response:

```json
{
  "provider": "openai",
  "score": 0.92,
  "alternatives": [
    {
      "provider": "anthropic",
      "score": 0.89
    }
  ]
}
```

---

# Report Observation

```http
POST /observe
```

---

# Get Runtime Status

```http
GET /providers/{name}/runtime
```

Useful for debugging.

---

# Granular Upsert Philosophy

Must support:

* updating only one quota
* updating only priority
* updating only cooldown
* adding dimensions later

No large overwrite payloads required.

---

# Suggested Internal Storage

Rust:

```txt
DashMap
Arc
AtomicU64
RwLock
```

Avoid:

* global mutexes
* giant locks

---

# Recommended Algorithms

| Problem           | Solution                 |
| ----------------- | ------------------------ |
| rolling rate      | EWMA                     |
| quota tracking    | token bucket             |
| health scoring    | weighted decay           |
| latency smoothing | moving average           |
| provider ranking  | weighted composite score |

---

# Important Edge Cases

## Hidden Provider Throttling

Some providers claim:

```txt
1000 rpm
```

But silently throttle earlier.

Learn actual safe throughput dynamically.

---

## Burst Traffic

Need:

* burst allowance
* sustained allowance

---

## Retry Storms

When providers degrade:

* avoid oscillation
* avoid synchronized retries

Use:

* cooldowns
* score hysteresis

---

# Hysteresis

Avoid provider flapping.

Without hysteresis:

```txt
openai -> anthropic -> openai -> anthropic
```

Use:

* minimum stickiness duration
* switching penalties

---

# Cooldowns

After repeated:

* 429
* 403
* timeout

Temporarily suppress provider.

Example:

```txt
3 consecutive 429s
-> cooldown 60s
```

---

# Recommended Default Config

```json
{
  "health_decay_half_life_seconds": 300,
  "429_cooldown_seconds": 60,
  "timeout_cooldown_seconds": 30,
  "selection_cache_ms": 100,
  "minimum_health_score": 0.2
}
```

---

# Suggested Future Features

## Multi-Region Awareness

Providers by region.

---

## Cost Optimization

Prefer cheaper providers dynamically.

---

## Tenant Isolation

Separate quotas per tenant.

---

## Distributed Runtime

Redis/Kafka backed state sync.

---

## Adaptive Learning

Auto-discover practical provider limits.

---

## Circuit Breakers

Temporary provider disablement.

---

# Rust Implementation Guidance

## Crates

| Purpose       | Crate     |
| ------------- | --------- |
| web server    | `axum`    |
| serialization | `serde`   |
| concurrency   | `dashmap` |
| metrics       | `metrics` |
| time          | `chrono`  |
| async runtime | `tokio`   |

---

# Recommended Module Structure

```txt
src/
 ├── api/
 ├── config/
 ├── runtime/
 ├── scoring/
 ├── quotas/
 ├── health/
 ├── storage/
 ├── simulation/
 └── tests/
```

---

# Testing Requirements

This project absolutely needs:

* deterministic simulations
* traffic replay tests
* burst tests
* concurrency tests
* provider failure chaos tests

---

# Critical Test Cases

## Quota Near Exhaustion

Should reroute before 429.

---

## Provider Recovery

Provider health should recover gradually.

---

## Multiple Quotas

Requests may pass rpm but fail token quota.

All dimensions must be checked.

---

## Concurrent Reporting

No counter corruption.

---

# Recommended MVP Scope

## Include

* in-memory runtime
* token bucket quotas
* weighted provider selection
* health scoring
* cooldowns
* anticipatory exhaustion
* REST API

## Exclude Initially

* distributed state
* persistence
* ML
* dashboards
* auto-discovery

---

# Positioning

This is not:

* a retry library
* a proxy
* a gateway

It is:

* a predictive provider orchestration engine

Closest conceptual categories:

* smart rate limiter
* provider scheduler
* adaptive quota router
* anticipatory service balancer

---

# Example Real Use Cases

## AI APIs

OpenAI / Anthropic / Gemini balancing

---

## Web Scraping

Multiple proxy vendors

---

## SMS Delivery

Twilio / MessageBird / Nexmo

---

## CAPTCHA Solving

Multi-provider orchestration

---

## Search APIs

SerpAPI / BrightData / custom search

---

# Suggested Tagline

```txt
Predict limits before providers enforce them.
```

Alternative:

```txt
Adaptive provider orchestration for quota-bound systems.
```
