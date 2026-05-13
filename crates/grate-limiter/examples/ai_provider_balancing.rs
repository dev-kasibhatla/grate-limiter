//! AI provider balancing example — route between OpenAI, Anthropic, and Gemini.

use grate_limiter::*;
use std::sync::Arc;

fn main() {
    let clock = Arc::new(MockClock::new());
    let config = EngineConfig::default().with_clock(clock.clone());
    let engine = GrateLimiter::new(config);

    // Register AI providers with realistic quotas
    engine.upsert_provider(ProviderConfig {
        name: "openai".into(),
        quotas: vec![
            QuotaConfig {
                dimension: Dimension::Requests,
                limit: 500,
                window: Some(Window::Minute),
            },
            QuotaConfig {
                dimension: Dimension::Tokens,
                limit: 90000,
                window: Some(Window::Minute),
            },
        ],
        priority: 10,
        weight: 1.0,
        cooldown_seconds: 30,
    });

    engine.upsert_provider(ProviderConfig {
        name: "anthropic".into(),
        quotas: vec![
            QuotaConfig {
                dimension: Dimension::Requests,
                limit: 300,
                window: Some(Window::Minute),
            },
            QuotaConfig {
                dimension: Dimension::Tokens,
                limit: 80000,
                window: Some(Window::Minute),
            },
        ],
        priority: 8,
        weight: 1.0,
        cooldown_seconds: 30,
    });

    engine.upsert_provider(ProviderConfig {
        name: "gemini".into(),
        quotas: vec![QuotaConfig {
            dimension: Dimension::Requests,
            limit: 200,
            window: Some(Window::Minute),
        }],
        priority: 6,
        weight: 1.0,
        cooldown_seconds: 30,
    });

    engine.upsert_capability(CapabilityConfig {
        name: "chat-completion".into(),
        providers: vec![
            CapabilityProvider {
                provider: "openai".into(),
                priority: 10,
            },
            CapabilityProvider {
                provider: "anthropic".into(),
                priority: 8,
            },
            CapabilityProvider {
                provider: "gemini".into(),
                priority: 6,
            },
        ],
    });

    println!("=== AI Provider Balancing Demo ===\n");

    // Simulate 50 requests with varying outcomes
    for i in 0..50 {
        clock.advance_ms(200);

        let decision = engine.select("chat-completion").unwrap();
        println!(
            "[{:3}] -> {} (score: {:.3})",
            i, decision.provider, decision.score
        );

        // Simulate: openai starts returning 429s after 20 requests
        let status = if decision.provider == "openai" && i > 20 {
            StatusClass::RateLimited
        } else {
            StatusClass::Success
        };

        engine
            .observe(Observation {
                provider: decision.provider,
                capability: Some("chat-completion".into()),
                usage: Usage {
                    requests: 1,
                    tokens: Some(500),
                    ..Default::default()
                },
                outcome: Outcome {
                    status,
                    latency_ms: 200,
                },
            })
            .unwrap();
    }

    println!("\n=== Final State ===");
    for provider in ["openai", "anthropic", "gemini"] {
        let health = engine.provider_health(provider).unwrap_or(0.0);
        let cooldown = engine.provider_in_cooldown(provider).unwrap_or(false);
        let remaining = engine
            .provider_quota_remaining(provider, Dimension::Requests)
            .unwrap_or(0);
        println!(
            "{provider:>10}: health={health:.2} cooldown={cooldown} remaining_requests={remaining}"
        );
    }

    println!("\n=== Metrics ===");
    let m = engine.metrics();
    println!("Selects: {}", m.selects());
    println!("Observations: {}", m.observations());
    println!("Cooldowns triggered: {}", m.cooldowns_triggered());
}
