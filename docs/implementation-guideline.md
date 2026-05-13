# Grate-Limiter Rust Library Specification

## Goal

Build a production-grade Rust crate for anticipatory provider orchestration and quota-aware routing.

The library must:

* be deterministic
* extremely well tested
* benchmarkable
* easy to embed
* memory efficient
* concurrency safe
* portable to Python/JS later
* measurable scientifically

This is infrastructure software.
Correctness and predictability matter more than feature count.

---

# Crate Positioning

## Crate Name

Suggested:

* `grate-limiter`
* `gratelimiter`
* `grate`

Recommended:

```txt
grate-limiter
```

---

# Core Design Goals

| Goal                | Priority |
| ------------------- | -------- |
| Correctness         | Critical |
| Predictability      | Critical |
| Performance         | Critical |
| Simplicity          | Critical |
| Extensibility       | High     |
| Minimal allocations | High     |
| Lock efficiency     | High     |
| Deterministic tests | Critical |

---

# Public Library Philosophy

Surface API should feel:

* tiny
* obvious
* boring
* stable

Internal engine can be sophisticated.

---

# Recommended Public API

## Main Engine

```rust
let mut grate = GrateLimiter::new(config);
```

---

## Register Provider

```rust
grate.upsert_provider(provider);
```

---

## Register Capability

```rust
grate.upsert_capability(capability);
```

---

## Select Provider

```rust
let decision = grate.select("chat-completion")?;
```

---

## Report Observation

```rust
grate.observe(observation)?;
```

---

# Important Library Properties

## 1. No Background Threads By Default

Avoid hidden runtime complexity.

Everything should operate:

* synchronously
* deterministically
* predictably

Optional async support later.

---

## 2. Monotonic Time

Never use wall clock internally.

Use:

```rust
std::time::Instant
```

Prevents:

* NTP drift
* daylight saving bugs
* time rewinds

---

## 3. Fully Deterministic Simulation Mode

Critical feature.

Need:

```rust
MockClock
```

Example:

```rust
let clock = MockClock::new();
```

Enables:

* deterministic quota tests
* reproducible failures
* replay testing

This is mandatory.

---

# Recommended Internal Architecture

```txt
src/
├── api/
├── capability/
├── provider/
├── quotas/
├── scoring/
├── health/
├── runtime/
├── simulation/
├── benchmarks/
├── testing/
├── metrics/
├── utils/
└── lib.rs
```

---

# Core Traits

## Quota Trait

```rust
pub trait QuotaStrategy {
    fn allow(&self, amount: u64) -> bool;
    fn remaining(&self) -> u64;
    fn observe(&self, amount: u64);
}
```

---

# Initial Quota Implementations

| Type              | Required |
| ----------------- | -------- |
| Token Bucket      | Yes      |
| Sliding Window    | Yes      |
| Fixed Window      | Yes      |
| Concurrency Limit | Yes      |

Token bucket should be default.

---

# Provider Selection Engine

## Design Requirement

Selection must be:

* deterministic
* explainable
* stable under pressure

---

# Decision Object

```rust
pub struct Decision {
    pub provider: String,
    pub score: f32,
    pub reasoning: ScoreBreakdown,
}
```

---

# Score Breakdown

```rust
pub struct ScoreBreakdown {
    pub quota_score: f32,
    pub health_score: f32,
    pub latency_score: f32,
    pub priority_score: f32,
}
```

---

# Scoring Engine Requirements

Must support:

* pluggable scoring
* weighted scoring
* future ML integration
* custom provider scoring hooks

---

# Suggested Trait

```rust
pub trait ScoringStrategy {
    fn score(&self, ctx: &ProviderContext) -> f32;
}
```

---

# Health Engine

## Required Metrics

| Metric        | Required |
| ------------- | -------- |
| success rate  | Yes      |
| 429 frequency | Yes      |
| 403 frequency | Yes      |
| 5xx rate      | Yes      |
| timeout rate  | Yes      |
| p95 latency   | Yes      |

---

# Required Internal Mechanisms

## Exponential Decay

Old failures should matter less.

Must use:

* EWMA
* weighted decay

Never raw cumulative counts.

---

# Cooldown System

Must support:

* consecutive failure cooldowns
* exponential cooldown growth
* recovery ramp-up

Example:

```txt
3x 429 -> 30s
6x 429 -> 2m
10x 429 -> 10m
```

---

# Critical Performance Requirements

| Metric                     | Target  |
| -------------------------- | ------- |
| select()                   | <10µs   |
| observe()                  | <5µs    |
| allocations during select  | 0       |
| allocations during observe | 0       |
| lock contention            | minimal |

---

# Concurrency Design

## Avoid

```txt
Mutex<HashMap>
```

---

## Prefer

```txt
DashMap
AtomicU64
Arc
RwLock
```

---

# Memory Layout Optimizations

Very important.

## Avoid

* String cloning
* dynamic dispatch in hot paths
* heap allocations in scoring

---

## Prefer

* interned identifiers
* compact enums
* stack allocations
* contiguous provider arrays

---

# Recommended Identifier Strategy

Convert:

```txt
"openai"
```

into:

```rust
ProviderId(u32)
```

Internally.

Huge performance win.

---

# Capability Lookup Optimization

Do NOT:

```txt
capability -> Vec<String>
```

Use:

```rust
CapabilityId -> SmallVec<[ProviderId; 4]>
```

Most capabilities likely have few providers.

---

# Benchmark Suite

This is extremely important.

Benchmarks are part of product identity.

---

# Benchmark Categories

## 1. Throughput Benchmarks

Measure:

* selects/sec
* observations/sec

---

## 2. Contention Benchmarks

Multiple concurrent:

* readers
* writers
* mixed workloads

---

## 3. Quota Prediction Accuracy

Most important benchmark.

Measure:

```txt
How often did the engine fail to prevent a rate limit?
```

---

# Core Accuracy Metric

## Rate Limit Miss Rate

Definition:

```txt
Requests that hit real provider limit
despite engine predicting safe usage
```

Metric:

```txt
miss_rate = unexpected_429s / total_requests
```

This is a flagship metric.

---

# Additional Accuracy Metrics

| Metric               | Meaning                               |
| -------------------- | ------------------------------------- |
| false safe           | engine said safe but hit limit        |
| false reject         | engine avoided provider unnecessarily |
| provider oscillation | excessive switching                   |
| recovery latency     | time to trust recovered provider      |
| prediction lead time | how early exhaustion predicted        |

---

# Required Simulation Framework

Mandatory.

---

# Traffic Simulator

Need ability to simulate:

* providers
* quotas
* hidden throttling
* random failures
* burst traffic
* latency degradation

---

# Example

```rust
Simulation::new()
    .provider(openai_sim)
    .provider(anthropic_sim)
    .traffic_pattern(burst_pattern)
    .run();
```

---

# Provider Simulator Features

## Simulated Behaviors

| Behavior               | Required |
| ---------------------- | -------- |
| hard quotas            | Yes      |
| hidden quotas          | Yes      |
| random 500s            | Yes      |
| burst penalties        | Yes      |
| latency spikes         | Yes      |
| progressive throttling | Yes      |

---

# Realistic Hidden Throttling

Critical.

Example:

```txt
Documented:
1000 RPM

Actual:
soft throttling begins at 850 RPM
```

Need simulator support.

---

# Chaos Testing

Mandatory.

---

# Chaos Scenarios

| Scenario                 | Required |
| ------------------------ | -------- |
| provider disappears      | Yes      |
| provider latency spikes  | Yes      |
| clock jumps              | Yes      |
| concurrent floods        | Yes      |
| all providers degrade    | Yes      |
| partial quota corruption | Yes      |

---

# Fuzz Testing

Mandatory.

Use:

```txt
cargo-fuzz
```

Fuzz:

* config parsing
* quota math
* scoring edge cases
* concurrent observation ordering

---

# Property Testing

Mandatory.

Use:

```txt
proptest
```

---

# Critical Properties

## Quotas Never Negative

---

## Remaining Never Exceeds Capacity

---

## Cooldowns Eventually Expire

---

## Health Score Always Bounded

```txt
0.0 <= health <= 1.0
```

---

## Deterministic Replay

Same event stream must produce:

* identical routing
* identical scores

---

# Load Testing

Must include dedicated suite.

---

# Required Load Profiles

| Profile           | Description                    |
| ----------------- | ------------------------------ |
| steady            | stable RPS                     |
| bursty            | sudden spikes                  |
| cascading failure | providers degrade sequentially |
| thundering herd   | synchronized retries           |
| quota exhaustion  | sustained overload             |

---

# Large Scale Benchmark Targets

Must test:

* 10 providers
* 100 providers
* 1000 providers
* 10k capabilities

---

# Required Benchmark Outputs

Every release must publish:

| Metric            | Required |
| ----------------- | -------- |
| select latency    | Yes      |
| observe latency   | Yes      |
| throughput        | Yes      |
| memory usage      | Yes      |
| lock contention   | Yes      |
| miss rate         | Yes      |
| false reject rate | Yes      |
| oscillation rate  | Yes      |

---

# Benchmark Publishing

Each release should automatically generate:

```txt
benchmarks/
 ├── v0.1.0.md
 ├── v0.1.1.md
 └── latest.md
```

---

# CI/CD Requirements

Mandatory.

---

# CI Pipeline

## On Every PR

Run:

* unit tests
* integration tests
* property tests
* fuzz smoke tests
* formatting
* clippy
* docs build

---

# On Main Branch

Run:

* full simulations
* load tests
* benchmark suite

---

# On Release

Publish:

* crate
* benchmark reports
* test reports
* changelog
* flamegraphs
* memory profiles

---

# Required GitHub Actions

| Action        | Required |
| ------------- | -------- |
| cargo test    | Yes      |
| cargo bench   | Yes      |
| cargo fmt     | Yes      |
| cargo clippy  | Yes      |
| cargo audit   | Yes      |
| cargo deny    | Yes      |
| cargo nextest | Yes      |

---

# Benchmark Tooling

Recommended:

* `criterion`
* `iai-callgrind`
* `pprof-rs`

---

# Required Generated Artifacts

Per release:

* benchmark markdown
* SVG graphs
* flamegraphs
* memory snapshots

---

# Versioning Strategy

Use:

```txt
SemVer
```

---

# Stability Rules

## Patch

* bug fixes only

## Minor

* backward compatible features

## Major

* scoring changes
* API changes
* algorithm changes affecting decisions

---

# Changelog Requirements

Every release must include:

| Section         | Required |
| --------------- | -------- |
| Added           | Yes      |
| Changed         | Yes      |
| Fixed           | Yes      |
| Performance     | Yes      |
| Benchmark delta | Yes      |
| Accuracy delta  | Yes      |

---

# Example Benchmark Changelog

```txt
v0.4.0

Performance:
- select() latency improved 18%
- memory reduced 12%

Accuracy:
- unexpected 429 rate reduced from 1.8% -> 0.7%
- false reject rate improved 22%
```

---

# Documentation Requirements

Docs must include:

* architecture
* scoring explanation
* quota math
* simulations
* tuning guide
* failure modes

---

# Required Example Programs

| Example                 | Required |
| ----------------------- | -------- |
| simple routing          | Yes      |
| AI provider balancing   | Yes      |
| scraping proxy rotation | Yes      |
| SMS failover            | Yes      |
| hidden quota learning   | Yes      |

---

# Observability

Must expose:

* counters
* gauges
* tracing hooks

---

# Recommended Metrics

| Metric             | Type    |
| ------------------ | ------- |
| provider_selected  | counter |
| unexpected_429     | counter |
| cooldown_triggered | counter |
| quota_exhausted    | counter |
| provider_switches  | counter |

---

# Future Compatibility Design

Prepare for:

* distributed state
* Redis backend
* persistent snapshots
* WASM builds
* Python bindings
* JS bindings

---

# Python/JS Portability Constraints

Avoid Rust-only API weirdness.

Prefer:

* plain structs
* serializable configs
* deterministic behavior

---

# Suggested Release Quality Bar

A release cannot publish unless:

* all tests pass
* fuzz suite clean
* no benchmark regression >10%
* miss rate below threshold
* memory within target

---

# Suggested Quality Targets

| Metric              | Goal  |
| ------------------- | ----- |
| unexpected 429s     | <0.5% |
| select p99          | <50µs |
| observe p99         | <20µs |
| memory/provider     | <4KB  |
| routing determinism | 100%  |

---

# Suggested Repository Structure

```txt
grate-limiter/
├── crates/
│   ├── core/
│   ├── simulation/
│   ├── benchmarks/
│   └── bindings/
├── examples/
├── benchmarks/
├── docs/
├── scripts/
├── fuzz/
├── tests/
└── .github/
```

---

# Strong Recommendation

Treat simulation and benchmarking as first-class product features.

The differentiator is not:

```txt
rate limiting
```

The differentiator is:

```txt
predictive accuracy under real-world failure conditions
```

That is what should define the project.
