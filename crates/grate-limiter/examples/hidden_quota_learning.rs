//! Hidden quota learning — the engine adapts when providers throttle below documented limits.

use grate_limiter::*;
use std::sync::Arc;

fn main() {
    let clock = Arc::new(MockClock::new());
    let config = EngineConfig::default().with_clock(clock.clone());
    let engine = GrateLimiter::new(config);

    // Provider claims 1000 RPM but actually soft-throttles at 700
    engine.upsert_provider(ProviderConfig {
        name: "sneaky-api".into(),
        quotas: vec![QuotaConfig {
            dimension: Dimension::Requests,
            limit: 1000, // documented limit
            window: Some(Window::Minute),
        }],
        priority: 10,
        weight: 1.0,
        cooldown_seconds: 15,
    });

    engine.upsert_provider(ProviderConfig {
        name: "honest-api".into(),
        quotas: vec![QuotaConfig {
            dimension: Dimension::Requests,
            limit: 500,
            window: Some(Window::Minute),
        }],
        priority: 7,
        weight: 1.0,
        cooldown_seconds: 15,
    });

    engine.upsert_capability(CapabilityConfig {
        name: "search".into(),
        providers: vec![
            CapabilityProvider {
                provider: "sneaky-api".into(),
                priority: 10,
            },
            CapabilityProvider {
                provider: "honest-api".into(),
                priority: 7,
            },
        ],
    });

    println!("=== Hidden Quota Learning Demo ===\n");
    println!("sneaky-api claims 1000 RPM but throttles at ~700\n");

    let mut sneaky_requests = 0u64;

    for i in 0..100 {
        clock.advance_ms(100);
        let decision = engine.select("search").unwrap();

        // Simulate hidden throttling: sneaky-api returns 429 after ~70 requests
        let (status, latency) = if decision.provider == "sneaky-api" {
            sneaky_requests += 1;
            if sneaky_requests > 70 {
                (StatusClass::RateLimited, 20u64)
            } else {
                (StatusClass::Success, 50)
            }
        } else {
            (StatusClass::Success, 80)
        };

        if i % 10 == 0 {
            let health = engine.provider_health("sneaky-api").unwrap_or(0.0);
            println!(
                "[{:3}] {} (score: {:.3}) sneaky-health={:.2} sneaky-reqs={sneaky_requests}",
                i, decision.provider, decision.score, health
            );
        }

        engine
            .observe(Observation {
                provider: decision.provider,
                capability: Some("search".into()),
                usage: Usage {
                    requests: 1,
                    ..Default::default()
                },
                outcome: Outcome {
                    status,
                    latency_ms: latency,
                },
            })
            .unwrap();
    }

    println!("\n=== Result ===");
    println!("Engine learned sneaky-api's actual limits and routed to honest-api");
    let s_health = engine.provider_health("sneaky-api").unwrap();
    let h_health = engine.provider_health("honest-api").unwrap();
    println!("sneaky-api health: {s_health:.2}");
    println!("honest-api health: {h_health:.2}");
}
