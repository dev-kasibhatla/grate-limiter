//! Simple routing example — the most basic usage of grate-limiter.

use grate_limiter::*;

fn main() {
    // Create the engine with default configuration
    let engine = GrateLimiter::new(EngineConfig::default());

    // Register two providers
    engine.upsert_provider(ProviderConfig {
        name: "primary".into(),
        quotas: vec![QuotaConfig {
            dimension: Dimension::Requests,
            limit: 100,
            window: Some(Window::Minute),
        }],
        priority: 10,
        weight: 1.0,
        cooldown_seconds: 30,
    });

    engine.upsert_provider(ProviderConfig {
        name: "fallback".into(),
        quotas: vec![QuotaConfig {
            dimension: Dimension::Requests,
            limit: 50,
            window: Some(Window::Minute),
        }],
        priority: 5,
        weight: 1.0,
        cooldown_seconds: 30,
    });

    // Register a capability
    engine.upsert_capability(CapabilityConfig {
        name: "api-call".into(),
        providers: vec![
            CapabilityProvider {
                provider: "primary".into(),
                priority: 10,
            },
            CapabilityProvider {
                provider: "fallback".into(),
                priority: 5,
            },
        ],
    });

    // Select the best provider
    let decision = engine.select("api-call").unwrap();
    println!(
        "Selected: {} (score: {:.2})",
        decision.provider, decision.score
    );
    println!("Reasoning: {:?}", decision.reasoning);

    // Report the outcome
    engine
        .observe(Observation {
            provider: decision.provider,
            capability: Some("api-call".into()),
            usage: Usage {
                requests: 1,
                ..Default::default()
            },
            outcome: Outcome {
                status: StatusClass::Success,
                latency_ms: 150,
            },
        })
        .unwrap();

    println!(
        "Metrics: selects={}, observations={}",
        engine.metrics().selects(),
        engine.metrics().observations()
    );
}
