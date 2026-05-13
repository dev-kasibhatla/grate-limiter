//! SMS failover — route between multiple SMS providers with priority failover.

use grate_limiter::*;

fn main() {
    let engine = GrateLimiter::new(EngineConfig::default());

    engine.upsert_provider(ProviderConfig {
        name: "twilio".into(),
        quotas: vec![QuotaConfig {
            dimension: Dimension::Requests,
            limit: 100,
            window: Some(Window::Second),
        }],
        priority: 10,
        weight: 1.0,
        cooldown_seconds: 60,
    });

    engine.upsert_provider(ProviderConfig {
        name: "messagebird".into(),
        quotas: vec![QuotaConfig {
            dimension: Dimension::Requests,
            limit: 50,
            window: Some(Window::Second),
        }],
        priority: 7,
        weight: 1.0,
        cooldown_seconds: 60,
    });

    engine.upsert_provider(ProviderConfig {
        name: "nexmo".into(),
        quotas: vec![QuotaConfig {
            dimension: Dimension::Requests,
            limit: 30,
            window: Some(Window::Second),
        }],
        priority: 5,
        weight: 1.0,
        cooldown_seconds: 60,
    });

    engine.upsert_capability(CapabilityConfig {
        name: "send-sms".into(),
        providers: vec![
            CapabilityProvider {
                provider: "twilio".into(),
                priority: 10,
            },
            CapabilityProvider {
                provider: "messagebird".into(),
                priority: 7,
            },
            CapabilityProvider {
                provider: "nexmo".into(),
                priority: 5,
            },
        ],
    });

    println!("=== SMS Failover Demo ===\n");

    for i in 0..20 {
        match engine.select("send-sms") {
            Ok(d) => {
                println!("[{:2}] SMS via {} (score: {:.3})", i, d.provider, d.score);
                engine
                    .observe(Observation {
                        provider: d.provider,
                        capability: Some("send-sms".into()),
                        usage: Usage {
                            requests: 1,
                            ..Default::default()
                        },
                        outcome: Outcome {
                            status: StatusClass::Success,
                            latency_ms: 200,
                        },
                    })
                    .unwrap();
            }
            Err(e) => println!("[{i:2}] No provider: {e}"),
        }
    }
}
