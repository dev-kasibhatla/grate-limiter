use grate_limiter::*;
use proptest::prelude::*;
use std::sync::Arc;

fn engine_with_clock() -> (GrateLimiter, Arc<MockClock>) {
    let clock = Arc::new(MockClock::new());
    let config = EngineConfig::default().with_clock(clock.clone());
    let engine = GrateLimiter::new(config);
    (engine, clock)
}

proptest! {
    /// Health score must always remain in [0.0, 1.0] regardless of observation sequence.
    #[test]
    fn health_score_always_bounded(
        success_count in 0u32..50,
        failure_count in 0u32..50,
        interleave_seed in 0u64..1000,
    ) {
        let (engine, clock) = engine_with_clock();
        engine.upsert_provider(ProviderConfig {
            name: "test".into(),
            quotas: vec![QuotaConfig {
                dimension: Dimension::Requests,
                limit: 10000,
                window: Some(Window::Minute),
            }],
            priority: 5,
            weight: 1.0,
            cooldown_seconds: 30,
        });

        let mut ops: Vec<bool> = Vec::new();
        ops.extend(std::iter::repeat(true).take(success_count as usize));
        ops.extend(std::iter::repeat(false).take(failure_count as usize));

        // Deterministic shuffle using seed
        let mut seed = interleave_seed;
        for i in (1..ops.len()).rev() {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let j = (seed as usize) % (i + 1);
            ops.swap(i, j);
        }

        for (i, is_success) in ops.iter().enumerate() {
            clock.advance_ms(50);
            let status = if *is_success {
                StatusClass::Success
            } else {
                StatusClass::RateLimited
            };

            let _ = engine.observe(Observation {
                provider: "test".into(),
                capability: None,
                usage: Usage { requests: 1, ..Default::default() },
                outcome: Outcome { status, latency_ms: 100 },
            });
        }

        let health = engine.provider_health("test").unwrap();
        prop_assert!(health >= 0.0, "health={health} below 0");
        prop_assert!(health <= 1.0, "health={health} above 1");
    }

    /// Remaining quota should never exceed capacity.
    #[test]
    fn remaining_never_exceeds_capacity(
        limit in 10u64..10000,
        observations in 1u32..100,
    ) {
        let (engine, clock) = engine_with_clock();
        engine.upsert_provider(ProviderConfig {
            name: "test".into(),
            quotas: vec![QuotaConfig {
                dimension: Dimension::Requests,
                limit,
                window: Some(Window::Minute),
            }],
            priority: 5,
            weight: 1.0,
            cooldown_seconds: 30,
        });

        for _ in 0..observations {
            clock.advance_ms(10);
            let _ = engine.observe(Observation {
                provider: "test".into(),
                capability: None,
                usage: Usage { requests: 1, ..Default::default() },
                outcome: Outcome { status: StatusClass::Success, latency_ms: 50 },
            });
        }

        let remaining = engine.provider_quota_remaining("test", Dimension::Requests).unwrap();
        prop_assert!(remaining <= limit, "remaining={remaining} > capacity={limit}");
    }

    /// Select should always return a valid decision or an error — never panic.
    #[test]
    fn select_never_panics(
        provider_count in 1u8..10,
        observation_count in 0u32..50,
    ) {
        let (engine, clock) = engine_with_clock();

        let mut cap_providers = Vec::new();
        for i in 0..provider_count {
            let name = format!("p{i}");
            engine.upsert_provider(ProviderConfig {
                name: name.clone(),
                quotas: vec![QuotaConfig {
                    dimension: Dimension::Requests,
                    limit: 100,
                    window: Some(Window::Minute),
                }],
                priority: i as u16 + 1,
                weight: 1.0,
                cooldown_seconds: 10,
            });
            cap_providers.push(CapabilityProvider { provider: name, priority: i as u16 + 1 });
        }

        engine.upsert_capability(CapabilityConfig {
            name: "test".into(),
            providers: cap_providers,
        });

        for i in 0..observation_count {
            clock.advance_ms(50);
            let provider = format!("p{}", i as u8 % provider_count);
            let status = if i % 5 == 0 { StatusClass::RateLimited } else { StatusClass::Success };
            let _ = engine.observe(Observation {
                provider,
                capability: Some("test".into()),
                usage: Usage { requests: 1, ..Default::default() },
                outcome: Outcome { status, latency_ms: 100 },
            });
        }

        // This should never panic regardless of state
        let _result = engine.select("test");
    }

    /// Cooldowns should eventually expire.
    #[test]
    fn cooldowns_eventually_expire(
        failure_count in 3u32..20,
        wait_secs in 1u64..700,
    ) {
        let (engine, clock) = engine_with_clock();
        engine.upsert_provider(ProviderConfig {
            name: "test".into(),
            quotas: vec![QuotaConfig {
                dimension: Dimension::Requests,
                limit: 10000,
                window: Some(Window::Minute),
            }],
            priority: 5,
            weight: 1.0,
            cooldown_seconds: 30,
        });

        // Trigger cooldown
        for _ in 0..failure_count {
            clock.advance_ms(100);
            let _ = engine.observe(Observation {
                provider: "test".into(),
                capability: None,
                usage: Usage { requests: 1, ..Default::default() },
                outcome: Outcome { status: StatusClass::RateLimited, latency_ms: 50 },
            });
        }

        // Wait max_cooldown_seconds (600) + margin
        clock.advance_secs(601);
        let in_cooldown = engine.provider_in_cooldown("test").unwrap();
        prop_assert!(!in_cooldown, "Cooldown should have expired after 601s");
    }
}
