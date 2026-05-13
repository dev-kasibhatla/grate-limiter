# Cross-Language Conformance Tests

JSON test vectors that all implementations (Rust, Python, JS/TS) must pass identically.

## Format

Each `.json` file defines a test scenario:

```json
{
  "name": "test_name",
  "description": "What this test verifies",
  "config": { ... },
  "providers": [ ... ],
  "capabilities": [ ... ],
  "steps": [ ... ]
}
```

### Step types

| Action | Description |
|--------|-------------|
| `advance_time_ms` | Advance the mock clock by N milliseconds |
| `observe` | Report an observation |
| `select` | Select a provider and assert the result |
| `check_health` | Assert a provider's health score is within range |
| `check_remaining` | Assert remaining quota for a dimension |
| `check_in_cooldown` | Assert whether a provider is in cooldown |

### Floating-point tolerance

Health scores and composite scores use `expect_min` / `expect_max` ranges to
accommodate minor floating-point differences across languages. Typical tolerance
is ±0.05.

## Running

- **Rust**: `cargo test --test conformance_tests -p grate-limiter`
- **Python**: `cd python && pytest tests/test_conformance.py -v`
- **JS/TS**: `cd js && npx vitest run tests/conformance.test.ts`
