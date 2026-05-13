//! # grate-limiter-simulation
//!
//! Simulation and chaos testing framework for grate-limiter.
//!
//! Provides tools for deterministic testing of rate-limit routing under realistic
//! conditions including: provider failures, hidden throttling, traffic bursts,
//! latency spikes, and cascading degradation.

mod chaos;
mod provider_sim;
mod traffic;

pub use chaos::{ChaosConfig, ChaosEvent};
pub use provider_sim::{SimulatedBehavior, SimulatedProvider};
pub use traffic::{LoadProfile, TrafficGenerator, TrafficPattern};

use grate_limiter::{
    EngineConfig, GrateLimiter, MockClock, Observation, Outcome, StatusClass, Usage,
};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::sync::Arc;

/// A complete simulation run configuration.
pub struct Simulation {
    pub providers: Vec<SimulatedProvider>,
    pub traffic: TrafficPattern,
    pub chaos: Option<ChaosConfig>,
    pub seed: u64,
    pub duration_secs: u64,
}

/// Results from a simulation run.
#[derive(Debug, Clone)]
pub struct SimulationResults {
    /// Total requests sent.
    pub total_requests: u64,
    /// Requests that hit an unexpected 429.
    pub unexpected_429s: u64,
    /// Miss rate: unexpected_429s / total_requests.
    pub miss_rate: f64,
    /// False rejections (engine avoided a provider that was actually fine).
    pub false_rejects: u64,
    /// Provider switches (oscillation count).
    pub provider_switches: u64,
    /// Average latency in ms.
    pub avg_latency_ms: f64,
}

impl Simulation {
    /// Run the simulation and return results.
    pub fn run(&self) -> SimulationResults {
        let clock = Arc::new(MockClock::new());
        let config = EngineConfig::default().with_clock(clock.clone());
        let engine = GrateLimiter::new(config);

        let mut rng = ChaCha8Rng::seed_from_u64(self.seed);

        // Register providers
        for sim_provider in &self.providers {
            engine.upsert_provider(sim_provider.to_provider_config());
        }

        // Register a default capability with all providers
        let cap_providers: Vec<_> = self
            .providers
            .iter()
            .enumerate()
            .map(|(i, p)| grate_limiter::CapabilityProvider {
                provider: p.name.clone(),
                priority: (self.providers.len() - i) as u16,
            })
            .collect();

        engine.upsert_capability(grate_limiter::CapabilityConfig {
            name: "default".into(),
            providers: cap_providers,
        });

        let mut results = SimulationResults {
            total_requests: 0,
            unexpected_429s: 0,
            miss_rate: 0.0,
            false_rejects: 0,
            provider_switches: 0,
            avg_latency_ms: 0.0,
        };

        let mut last_provider: Option<String> = None;
        let mut total_latency: f64 = 0.0;
        let step_ms = 100; // 100ms per step

        for step in 0..(self.duration_secs * 1000 / step_ms) {
            clock.advance_ms(step_ms);

            let requests_this_step = self.traffic.requests_at(step);
            for _ in 0..requests_this_step {
                results.total_requests += 1;

                let decision = match engine.select("default") {
                    Ok(d) => d,
                    Err(_) => {
                        results.false_rejects += 1;
                        continue;
                    }
                };

                // Track oscillation
                if let Some(ref last) = last_provider {
                    if *last != decision.provider {
                        results.provider_switches += 1;
                    }
                }
                last_provider = Some(decision.provider.clone());

                // Simulate provider response
                let sim_provider = self
                    .providers
                    .iter()
                    .find(|p| p.name == decision.provider)
                    .unwrap();

                let (status, latency) =
                    sim_provider.simulate_response(results.total_requests, &mut rng);
                total_latency += latency as f64;

                if status == StatusClass::RateLimited {
                    results.unexpected_429s += 1;
                }

                let _ = engine.observe(Observation {
                    provider: decision.provider,
                    capability: Some("default".into()),
                    usage: Usage {
                        requests: 1,
                        ..Default::default()
                    },
                    outcome: Outcome {
                        status,
                        latency_ms: latency,
                    },
                });
            }
        }

        if results.total_requests > 0 {
            results.miss_rate = results.unexpected_429s as f64 / results.total_requests as f64;
            results.avg_latency_ms = total_latency / results.total_requests as f64;
        }

        results
    }
}
