//! Scraping proxy rotation — distribute requests across multiple proxy providers.

use grate_limiter::*;
use std::sync::Arc;

fn main() {
    let clock = Arc::new(MockClock::new());
    let config = EngineConfig::default().with_clock(clock.clone());
    let engine = GrateLimiter::new(config);

    // Register proxy providers with daily quotas
    for (name, limit, priority) in [
        ("brightdata", 10000u64, 10u16),
        ("oxylabs", 8000, 8),
        ("smartproxy", 5000, 6),
    ] {
        engine.upsert_provider(ProviderConfig {
            name: name.into(),
            quotas: vec![
                QuotaConfig { dimension: Dimension::Requests, limit: 100, window: Some(Window::Minute) },
                QuotaConfig { dimension: Dimension::Bytes, limit: limit * 1000, window: Some(Window::Day) },
            ],
            priority,
            weight: 1.0,
            cooldown_seconds: 30,
        });
    }

    engine.upsert_capability(CapabilityConfig {
        name: "scrape".into(),
        providers: vec![
            CapabilityProvider { provider: "brightdata".into(), priority: 10 },
            CapabilityProvider { provider: "oxylabs".into(), priority: 8 },
            CapabilityProvider { provider: "smartproxy".into(), priority: 6 },
        ],
    });

    println!("=== Proxy Rotation Demo ===\n");

    let mut provider_counts = std::collections::HashMap::new();

    for i in 0..100 {
        clock.advance_ms(500);
        let d = engine.select("scrape").unwrap();
        *provider_counts.entry(d.provider.clone()).or_insert(0u32) += 1;

        // Simulate varying response sizes
        let bytes = 5000 + (i * 100);
        let status = if i > 60 && d.provider == "brightdata" {
            StatusClass::Forbidden // IP banned after heavy usage
        } else {
            StatusClass::Success
        };

        engine.observe(Observation {
            provider: d.provider,
            capability: Some("scrape".into()),
            usage: Usage {
                requests: 1,
                bytes: Some(bytes),
                ..Default::default()
            },
            outcome: Outcome { status, latency_ms: 300 },
        }).unwrap();
    }

    println!("Request distribution:");
    for (provider, count) in &provider_counts {
        println!("  {provider}: {count} requests");
    }

    println!("\nProvider health:");
    for p in ["brightdata", "oxylabs", "smartproxy"] {
        let h = engine.provider_health(p).unwrap();
        println!("  {p}: {h:.2}");
    }
}
