use grate_limiter::*;
use std::sync::Arc;

fn setup() -> (GrateLimiter, Arc<MockClock>) {
    let clock = Arc::new(MockClock::new());
    let config = EngineConfig::default().with_clock(clock.clone());
    let engine = GrateLimiter::new(config);
    (engine, clock)
}

fn register_ai_providers(engine: &GrateLimiter) {
    engine.upsert_provider(ProviderConfig {
        name: "openai".into(),
        quotas: vec![
            QuotaConfig {
                dimension: Dimension::Requests,
                limit: 100,
                window: Some(Window::Minute),
            },
            QuotaConfig {
                dimension: Dimension::Tokens,
                limit: 50000,
                window: Some(Window::Minute),
            },
        ],
        priority: 10,
        weight: 1.0,
        cooldown_seconds: 30,
    });

    engine.upsert_provider(ProviderConfig {
        name: "anthropic".into(),
        quotas: vec![QuotaConfig {
            dimension: Dimension::Requests,
            limit: 80,
            window: Some(Window::Minute),
        }],
        priority: 8,
        weight: 1.0,
        cooldown_seconds: 30,
    });

    engine.upsert_provider(ProviderConfig {
        name: "gemini".into(),
        quotas: vec![QuotaConfig {
            dimension: Dimension::Requests,
            limit: 60,
            window: Some(Window::Minute),
        }],
        priority: 6,
        weight: 1.0,
        cooldown_seconds: 30,
    });

    engine.upsert_capability(CapabilityConfig {
        name: "chat".into(),
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
}

// --- Integration Tests ---

#[test]
fn full_lifecycle_select_observe_cycle() {
    let (engine, clock) = setup();
    register_ai_providers(&engine);

    // First selection should pick the highest priority provider
    let d1 = engine.select("chat").unwrap();
    assert_eq!(d1.provider, "openai");
    assert_eq!(d1.alternatives.len(), 2);

    // Observe success
    engine
        .observe(Observation {
            provider: "openai".into(),
            capability: Some("chat".into()),
            usage: Usage {
                requests: 1,
                tokens: Some(500),
                ..Default::default()
            },
            outcome: Outcome {
                status: StatusClass::Success,
                latency_ms: 200,
            },
        })
        .unwrap();

    // Metrics should update
    assert_eq!(engine.metrics().selects(), 1);
    assert_eq!(engine.metrics().observations(), 1);
}

#[test]
fn cascading_provider_failover() {
    let (engine, clock) = setup();
    register_ai_providers(&engine);

    // Degrade openai into cooldown
    for _ in 0..3 {
        clock.advance_ms(100);
        engine
            .observe(Observation {
                provider: "openai".into(),
                capability: Some("chat".into()),
                usage: Usage {
                    requests: 1,
                    ..Default::default()
                },
                outcome: Outcome {
                    status: StatusClass::RateLimited,
                    latency_ms: 50,
                },
            })
            .unwrap();
    }

    // Should failover to anthropic
    let d = engine.select("chat").unwrap();
    assert_eq!(d.provider, "anthropic");

    // Degrade anthropic into cooldown
    for _ in 0..3 {
        clock.advance_ms(100);
        engine
            .observe(Observation {
                provider: "anthropic".into(),
                capability: Some("chat".into()),
                usage: Usage {
                    requests: 1,
                    ..Default::default()
                },
                outcome: Outcome {
                    status: StatusClass::RateLimited,
                    latency_ms: 50,
                },
            })
            .unwrap();
    }

    // Should failover to gemini
    let d = engine.select("chat").unwrap();
    assert_eq!(d.provider, "gemini");

    // Degrade gemini too
    for _ in 0..3 {
        clock.advance_ms(100);
        engine
            .observe(Observation {
                provider: "gemini".into(),
                capability: Some("chat".into()),
                usage: Usage {
                    requests: 1,
                    ..Default::default()
                },
                outcome: Outcome {
                    status: StatusClass::RateLimited,
                    latency_ms: 50,
                },
            })
            .unwrap();
    }

    // All providers in cooldown — should error
    let result = engine.select("chat");
    assert!(matches!(result, Err(Error::NoAvailableProviders(_))));
}

#[test]
fn provider_recovery_after_cooldown() {
    let (engine, clock) = setup();
    register_ai_providers(&engine);

    // Put openai in cooldown
    for _ in 0..3 {
        clock.advance_ms(100);
        engine
            .observe(Observation {
                provider: "openai".into(),
                capability: None,
                usage: Usage {
                    requests: 1,
                    ..Default::default()
                },
                outcome: Outcome {
                    status: StatusClass::RateLimited,
                    latency_ms: 50,
                },
            })
            .unwrap();
    }

    assert_eq!(engine.provider_in_cooldown("openai"), Some(true));

    // Wait for cooldown to expire
    clock.advance_secs(31);
    assert_eq!(engine.provider_in_cooldown("openai"), Some(false));

    // Feed successes so health recovers
    for _ in 0..20 {
        clock.advance_ms(500);
        engine
            .observe(Observation {
                provider: "openai".into(),
                capability: None,
                usage: Usage {
                    requests: 1,
                    ..Default::default()
                },
                outcome: Outcome {
                    status: StatusClass::Success,
                    latency_ms: 100,
                },
            })
            .unwrap();
    }

    let health = engine.provider_health("openai").unwrap();
    assert!(health > 0.6, "health should recover, got {health}");
}

#[test]
fn multi_dimensional_quota_tracking() {
    let (engine, clock) = setup();
    register_ai_providers(&engine);

    // Consume most of openai's token quota (but not requests)
    for _ in 0..10 {
        engine
            .observe(Observation {
                provider: "openai".into(),
                capability: Some("chat".into()),
                usage: Usage {
                    requests: 1,
                    tokens: Some(4500), // 45000 tokens total
                    ..Default::default()
                },
                outcome: Outcome {
                    status: StatusClass::Success,
                    latency_ms: 200,
                },
            })
            .unwrap();
    }
    clock.advance_ms(5000);

    // Request quota: 90 remaining, Token quota: ~5000 remaining
    // Token quota is the bottleneck — should deprioritize openai
    let d = engine.select("chat").unwrap();
    // With token quota nearly exhausted and high burn rate, anthropic should win
    assert_eq!(
        d.provider, "anthropic",
        "Token quota exhaustion should trigger failover"
    );
}

#[test]
fn concurrent_select_observe() {
    let (engine, clock) = setup();
    register_ai_providers(&engine);

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let engine = engine.clone();
            std::thread::spawn(move || {
                for _ in 0..100 {
                    let d = engine.select("chat").unwrap();
                    engine
                        .observe(Observation {
                            provider: d.provider,
                            capability: Some("chat".into()),
                            usage: Usage {
                                requests: 1,
                                ..Default::default()
                            },
                            outcome: Outcome {
                                status: StatusClass::Success,
                                latency_ms: 50,
                            },
                        })
                        .unwrap();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // All operations should have completed without panics
    assert_eq!(engine.metrics().selects(), 1000);
    assert_eq!(engine.metrics().observations(), 1000);
}

#[test]
fn dynamic_provider_registration() {
    let (engine, clock) = setup();

    // Start with just openai
    engine.upsert_provider(ProviderConfig {
        name: "openai".into(),
        quotas: vec![QuotaConfig {
            dimension: Dimension::Requests,
            limit: 100,
            window: Some(Window::Minute),
        }],
        priority: 10,
        weight: 1.0,
        cooldown_seconds: 30,
    });

    engine.upsert_capability(CapabilityConfig {
        name: "chat".into(),
        providers: vec![CapabilityProvider {
            provider: "openai".into(),
            priority: 10,
        }],
    });

    let d = engine.select("chat").unwrap();
    assert_eq!(d.provider, "openai");
    assert!(d.alternatives.is_empty());

    // Add anthropic dynamically
    engine.upsert_provider(ProviderConfig {
        name: "anthropic".into(),
        quotas: vec![QuotaConfig {
            dimension: Dimension::Requests,
            limit: 80,
            window: Some(Window::Minute),
        }],
        priority: 8,
        weight: 1.0,
        cooldown_seconds: 30,
    });

    // Update capability to include anthropic
    engine.upsert_capability(CapabilityConfig {
        name: "chat".into(),
        providers: vec![
            CapabilityProvider {
                provider: "openai".into(),
                priority: 10,
            },
            CapabilityProvider {
                provider: "anthropic".into(),
                priority: 8,
            },
        ],
    });

    let d = engine.select("chat").unwrap();
    assert_eq!(d.alternatives.len(), 1);
}

#[test]
fn deterministic_replay() {
    // Same sequence of operations should produce identical results
    let run = |_| {
        let clock = Arc::new(MockClock::new());
        let config = EngineConfig::default().with_clock(clock.clone());
        let engine = GrateLimiter::new(config);
        register_ai_providers(&engine);

        let mut decisions = Vec::new();

        for i in 0..20 {
            clock.advance_ms(100);
            let d = engine.select("chat").unwrap();
            decisions.push((d.provider.clone(), format!("{:.4}", d.score)));

            engine
                .observe(Observation {
                    provider: d.provider,
                    capability: Some("chat".into()),
                    usage: Usage {
                        requests: 1,
                        tokens: Some(100),
                        ..Default::default()
                    },
                    outcome: Outcome {
                        status: if i % 7 == 0 {
                            StatusClass::RateLimited
                        } else {
                            StatusClass::Success
                        },
                        latency_ms: 100 + (i * 10),
                    },
                })
                .unwrap();
        }

        decisions
    };

    let run1 = run(1);
    let run2 = run(2);
    assert_eq!(run1, run2, "Deterministic replay failed");
}
