#![no_main]
use libfuzzer_sys::fuzz_target;
use grate_limiter::*;
use std::sync::Arc;

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }

    let clock = Arc::new(MockClock::new());
    let config = EngineConfig::default().with_clock(clock.clone());
    let engine = GrateLimiter::new(config);

    // Register a fixed set of providers
    for name in ["p0", "p1", "p2"] {
        engine.upsert_provider(ProviderConfig {
            name: name.into(),
            quotas: vec![QuotaConfig {
                dimension: Dimension::Requests,
                limit: 100,
                window: Some(Window::Minute),
            }],
            priority: 5,
            weight: 1.0,
            cooldown_seconds: 10,
        });
    }

    engine.upsert_capability(CapabilityConfig {
        name: "cap".into(),
        providers: vec![
            CapabilityProvider { provider: "p0".into(), priority: 10 },
            CapabilityProvider { provider: "p1".into(), priority: 8 },
            CapabilityProvider { provider: "p2".into(), priority: 6 },
        ],
    });

    // Use fuzz data to drive operations
    for chunk in data.chunks(2) {
        let op = chunk[0] % 4;
        let val = chunk.get(1).copied().unwrap_or(0);

        match op {
            0 => {
                // Select
                let _ = engine.select("cap");
            }
            1 => {
                // Observe success
                let provider = match val % 3 {
                    0 => "p0",
                    1 => "p1",
                    _ => "p2",
                };
                let _ = engine.observe(Observation {
                    provider: provider.into(),
                    capability: Some("cap".into()),
                    usage: Usage { requests: 1, ..Default::default() },
                    outcome: Outcome {
                        status: StatusClass::Success,
                        latency_ms: val as u64 * 10,
                    },
                });
            }
            2 => {
                // Observe failure
                let provider = match val % 3 {
                    0 => "p0",
                    1 => "p1",
                    _ => "p2",
                };
                let status = match val % 4 {
                    0 => StatusClass::RateLimited,
                    1 => StatusClass::Forbidden,
                    2 => StatusClass::ServerError,
                    _ => StatusClass::Timeout,
                };
                let _ = engine.observe(Observation {
                    provider: provider.into(),
                    capability: Some("cap".into()),
                    usage: Usage { requests: 1, ..Default::default() },
                    outcome: Outcome { status, latency_ms: 100 },
                });
            }
            3 => {
                // Advance time
                clock.advance_ms(val as u64 * 100);
            }
            _ => {}
        }

        // Invariant checks after every operation
        for p in ["p0", "p1", "p2"] {
            if let Some(h) = engine.provider_health(p) {
                assert!(h >= 0.0 && h <= 1.0, "Health out of bounds: {h}");
            }
        }
    }
});
