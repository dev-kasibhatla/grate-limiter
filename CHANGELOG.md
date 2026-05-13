# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2025-XX-XX

### Added

- Core anticipatory rate-limit engine
- Token bucket, sliding window, fixed window, and concurrency quota strategies
- Health engine with EWMA decay and automatic cooldowns
- Weighted composite scoring with anticipatory quota exhaustion prediction
- Deterministic simulation support via `MockClock`
- Optional HTTP server (`grate-limiter-server` crate)
- Full simulation framework (`grate-limiter-simulation` crate)
- Property-based tests via `proptest`
- Fuzz testing targets
- Criterion benchmarks for `select()` and `observe()`
- Examples: simple routing, AI provider balancing, hidden quota learning, SMS failover, scraping proxy rotation

### Performance

- `select()` target: <10µs p99
- `observe()` target: <5µs p99
- Zero allocations in hot paths

[Unreleased]: https://github.com/dev-kasibhatla/grate-limiter/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/dev-kasibhatla/grate-limiter/releases/tag/v0.1.0
