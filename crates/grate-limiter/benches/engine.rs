use criterion::{black_box, criterion_group, criterion_main, Criterion};
use grate_limiter::{
    CapabilityConfig, CapabilityProvider, Dimension, EngineConfig, GrateLimiter, MockClock,
    Observation, Outcome, ProviderConfig, QuotaConfig, StatusClass, Usage, Window,
};
use std::sync::Arc;

fn setup_engine(provider_count: usize) -> (GrateLimiter, Arc<MockClock>) {
    let clock = Arc::new(MockClock::new());
    let config = EngineConfig::default().with_clock(clock.clone());
    let engine = GrateLimiter::new(config);

    let mut cap_providers = Vec::with_capacity(provider_count);
    for i in 0..provider_count {
        let name = format!("provider-{i}");
        engine.upsert_provider(ProviderConfig {
            name: name.clone(),
            quotas: vec![QuotaConfig {
                dimension: Dimension::Requests,
                limit: 1000,
                window: Some(Window::Minute),
            }],
            priority: (provider_count - i) as u16,
            weight: 1.0,
            cooldown_seconds: 30,
        });
        cap_providers.push(CapabilityProvider {
            provider: name,
            priority: (provider_count - i) as u16,
        });
    }

    engine.upsert_capability(CapabilityConfig {
        name: "bench-cap".into(),
        providers: cap_providers,
    });

    (engine, clock)
}

fn bench_select(c: &mut Criterion) {
    let mut group = c.benchmark_group("select");

    for count in [2, 10, 50, 100] {
        let (engine, _clock) = setup_engine(count);
        group.bench_function(format!("{count}_providers"), |b| {
            b.iter(|| black_box(engine.select("bench-cap")))
        });
    }

    group.finish();
}

fn bench_observe(c: &mut Criterion) {
    let mut group = c.benchmark_group("observe");

    let (engine, _clock) = setup_engine(10);
    group.bench_function("single_observation", |b| {
        b.iter(|| {
            black_box(
                engine.observe(Observation {
                    provider: "provider-0".into(),
                    capability: Some("bench-cap".into()),
                    usage: Usage {
                        requests: 1,
                        tokens: Some(100),
                        ..Default::default()
                    },
                    outcome: Outcome {
                        status: StatusClass::Success,
                        latency_ms: 100,
                    },
                }),
            )
        })
    });

    group.finish();
}

fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed");

    let (engine, clock) = setup_engine(10);
    group.bench_function("select_observe_cycle", |b| {
        b.iter(|| {
            let decision = engine.select("bench-cap").unwrap();
            clock.advance_ms(1);
            engine
                .observe(Observation {
                    provider: decision.provider,
                    capability: Some("bench-cap".into()),
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
        })
    });

    group.finish();
}

criterion_group!(benches, bench_select, bench_observe, bench_mixed_workload);
criterion_main!(benches);
