# Contributing to grate-limiter

Thank you for your interest in contributing! This document provides guidelines for contributing to grate-limiter.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/grate-limiter.git`
3. Create a branch: `git checkout -b feature/your-feature`
4. Make your changes
5. Run the test suite: `cargo test --workspace`
6. Submit a pull request

## Development Setup

### Prerequisites

- Rust stable (latest)
- `cargo-fuzz` (optional, for fuzz testing)
- `cargo-tarpaulin` (optional, for coverage)

### Building

```bash
cargo build --workspace
```

### Testing

```bash
# All tests
cargo test --workspace

# Property tests
cargo test --test property_tests -p grate-limiter

# Benchmarks
cargo bench -p grate-limiter

# Examples
cargo run --example simple_routing -p grate-limiter
```

### Code Quality

```bash
# Format
cargo fmt --all

# Lint
cargo clippy --workspace --all-targets -- -D warnings

# Documentation
cargo doc --workspace --no-deps
```

## Code Style

- Follow `rustfmt` defaults (configured in `rustfmt.toml`)
- Use `clippy` with `-D warnings`
- Write doc comments for all public items
- Prefer descriptive variable names over abbreviations
- Keep functions focused and small

## Architecture

### Crate Structure

- `crates/grate-limiter/` — Core library (the main crate most users depend on)
- `crates/grate-limiter-server/` — HTTP server wrapping the core
- `crates/grate-limiter-simulation/` — Simulation & chaos testing

### Key Modules

- `engine.rs` — Main `GrateLimiter` struct and public API
- `quota/` — Quota tracking strategies (token bucket, sliding window, etc.)
- `health.rs` — EWMA-based provider health scoring
- `scoring.rs` — Pluggable scoring strategies
- `clock.rs` — Time abstraction for deterministic testing

### Design Principles

1. **Correctness over features** — Tests and determinism first
2. **No hidden complexity** — No background threads, no implicit state
3. **Monotonic time only** — Use `Clock` trait, never wall clock
4. **Portability** — Design for future Python/JS reimplementation

## Testing Guidelines

- Every module should have unit tests
- Use `MockClock` for all time-dependent tests
- Property tests for invariants (health bounded, quota non-negative, etc.)
- Integration tests for multi-component workflows
- Benchmarks for performance-sensitive paths

### Test Naming

```rust
#[test]
fn select_returns_highest_scored_provider() { ... }

#[test]
fn health_decays_toward_full_over_time() { ... }
```

## Pull Request Process

1. Ensure all tests pass: `cargo test --workspace`
2. Ensure no clippy warnings: `cargo clippy --workspace -- -D warnings`
3. Ensure code is formatted: `cargo fmt --all -- --check`
4. Update CHANGELOG.md with your changes
5. Write a clear PR description

## Versioning

We use [Semantic Versioning](https://semver.org/):

- **Patch**: Bug fixes only
- **Minor**: Backward-compatible features
- **Major**: Scoring changes, API changes, algorithm changes affecting decisions

## License

By contributing, you agree that your contributions will be licensed under the Apache License 2.0.
