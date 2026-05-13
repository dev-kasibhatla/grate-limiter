use std::sync::Arc;

use dashmap::DashMap;
use parking_lot::RwLock;
use smallvec::SmallVec;

use crate::capability::{CapabilityConfig, CapabilityProvider};
use crate::clock::{Clock, Timestamp};
use crate::config::EngineConfig;
use crate::decision::{Alternative, Decision, ScoreBreakdown};
use crate::error::{Error, Result};
use crate::health::HealthState;
use crate::metrics::Metrics;
use crate::observation::{Observation, StatusClass};
use crate::provider::ProviderConfig;
use crate::quota::{self, Dimension, QuotaConfig};
use crate::scoring::{ProviderScoreContext, ScoringStrategy, WeightedScorer};

/// The main grate-limiter engine.
///
/// Thread-safe and cheaply cloneable. All instances share the same internal state.
///
/// # Example
///
/// ```rust
/// use grate_limiter::{GrateLimiter, EngineConfig};
///
/// let engine = GrateLimiter::new(EngineConfig::default());
/// // Register providers and capabilities, then use engine.select() and engine.observe()
/// ```
#[derive(Clone)]
pub struct GrateLimiter {
    inner: Arc<Inner>,
}

struct Inner {
    /// Per-provider runtime state. Key = provider name.
    providers: DashMap<String, ProviderRuntime>,
    /// Capability definitions. Key = capability name.
    capabilities: RwLock<DashMap<String, CapabilityDef>>,
    /// Scoring strategy.
    scorer: Box<dyn ScoringStrategy>,
    /// Engine configuration.
    config: EngineConfig,
    /// Monotonic clock.
    clock: Arc<dyn Clock>,
    /// Observable metrics.
    metrics: Metrics,
}

/// Internal runtime state for a provider.
struct ProviderRuntime {
    config: ProviderConfig,
    health: RwLock<HealthState>,
    quota_trackers: Vec<(QuotaConfig, Box<dyn crate::quota::QuotaTracker>)>,
}

/// Internal capability definition.
struct CapabilityDef {
    providers: SmallVec<[CapabilityProvider; 4]>,
}

impl GrateLimiter {
    /// Create a new engine with the given configuration.
    pub fn new(config: EngineConfig) -> Self {
        let clock = config.clock();
        let scorer = Box::new(WeightedScorer::new(config.scoring.clone()));

        Self {
            inner: Arc::new(Inner {
                providers: DashMap::new(),
                capabilities: RwLock::new(DashMap::new()),
                scorer,
                config,
                clock,
                metrics: Metrics::new(),
            }),
        }
    }

    /// Register or update a provider and its quotas.
    ///
    /// If the provider already exists, its configuration and quota trackers are replaced.
    /// Health state is preserved across upserts.
    pub fn upsert_provider(&self, config: ProviderConfig) {
        let now = self.inner.clock.now();
        let trackers: Vec<_> = config
            .quotas
            .iter()
            .map(|qc| (qc.clone(), quota::create_tracker(qc, now)))
            .collect();

        if let Some(mut existing) = self.inner.providers.get_mut(&config.name) {
            // Preserve health, update config and trackers
            existing.config = config;
            existing.quota_trackers = trackers;
        } else {
            self.inner.providers.insert(
                config.name.clone(),
                ProviderRuntime {
                    config,
                    health: RwLock::new(HealthState::new(now)),
                    quota_trackers: trackers,
                },
            );
        }
    }

    /// Register or update a capability and its provider mappings.
    pub fn upsert_capability(&self, config: CapabilityConfig) {
        let caps = self.inner.capabilities.read();
        caps.insert(
            config.name.clone(),
            CapabilityDef {
                providers: SmallVec::from_vec(config.providers),
            },
        );
    }

    /// Select the best provider for a capability.
    ///
    /// Returns a [`Decision`] with the selected provider, its score, reasoning breakdown,
    /// and ranked alternatives.
    ///
    /// # Errors
    ///
    /// - [`Error::UnknownCapability`] if the capability is not registered.
    /// - [`Error::NoAvailableProviders`] if all providers are in cooldown or below minimum health.
    pub fn select(&self, capability: &str) -> Result<Decision> {
        self.inner
            .metrics
            .selects
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let now = self.inner.clock.now();

        // Look up capability
        let caps = self.inner.capabilities.read();
        let cap_def = caps
            .get(capability)
            .ok_or_else(|| Error::UnknownCapability(capability.to_string()))?;

        let cap_providers = &cap_def.providers;
        if cap_providers.is_empty() {
            return Err(Error::NoAvailableProviders(capability.to_string()));
        }

        // Find max priority and max latency for normalization
        let max_priority = cap_providers.iter().map(|p| p.priority).max().unwrap_or(1);

        let mut max_latency_ms: f64 = 0.0;
        for cp in cap_providers.iter() {
            if let Some(pr) = self.inner.providers.get(&cp.provider) {
                let health = pr.health.read();
                if health.latency_ms() > max_latency_ms {
                    max_latency_ms = health.latency_ms();
                }
            }
        }
        if max_latency_ms <= 0.0 {
            max_latency_ms = 1.0;
        }

        // Score all providers
        let mut candidates: SmallVec<[(String, f32, ScoreBreakdown); 4]> = SmallVec::new();

        for cp in cap_providers.iter() {
            let Some(pr) = self.inner.providers.get(&cp.provider) else {
                continue;
            };

            let health = pr.health.read();

            // Skip providers in cooldown
            if health.is_in_cooldown(now) {
                continue;
            }

            // Skip providers below minimum health
            if health.score() < self.inner.config.minimum_health_score {
                continue;
            }

            // Calculate worst quota state across all dimensions
            let (quota_remaining_ratio, predicted_exhaustion, burn_rate) =
                self.worst_quota_state(&pr.quota_trackers, now);

            let ctx = ProviderScoreContext {
                quota_remaining_ratio,
                predicted_exhaustion_secs: predicted_exhaustion,
                burn_rate,
                health_score: health.score(),
                priority: cp.priority,
                max_priority,
                latency_ms: health.latency_ms(),
                max_latency_ms,
            };

            let score = self.inner.scorer.score(&ctx);
            let breakdown = ScoreBreakdown {
                quota_score: ctx.quota_remaining_ratio as f32,
                health_score: ctx.health_score,
                priority_score: cp.priority as f32 / max_priority as f32,
                latency_score: if max_latency_ms > 0.0 {
                    (1.0 - (ctx.latency_ms / max_latency_ms) as f32).max(0.0)
                } else {
                    1.0
                },
            };

            candidates.push((cp.provider.clone(), score, breakdown));
        }

        drop(cap_def);
        drop(caps);

        if candidates.is_empty() {
            self.inner
                .metrics
                .no_provider_available
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Err(Error::NoAvailableProviders(capability.to_string()));
        }

        // Sort by score descending
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let (provider, score, reasoning) = candidates.remove(0);
        let alternatives = candidates
            .into_iter()
            .map(|(p, s, _)| Alternative {
                provider: p,
                score: s,
            })
            .collect();

        Ok(Decision {
            provider,
            score,
            reasoning,
            alternatives,
        })
    }

    /// Report an observation after a provider interaction.
    ///
    /// Updates quota counters and health state for the provider.
    ///
    /// # Errors
    ///
    /// - [`Error::UnknownProvider`] if the provider is not registered.
    pub fn observe(&self, obs: Observation) -> Result<()> {
        self.inner
            .metrics
            .observations
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let now = self.inner.clock.now();

        let pr = self
            .inner
            .providers
            .get(&obs.provider)
            .ok_or_else(|| Error::UnknownProvider(obs.provider.clone()))?;

        // Update quota trackers
        for (qc, tracker) in &pr.quota_trackers {
            let amount = match qc.dimension {
                Dimension::Requests => obs.usage.requests,
                Dimension::Tokens => obs.usage.tokens.unwrap_or(0),
                Dimension::Bytes => obs.usage.bytes.unwrap_or(0),
                Dimension::CostUsd => obs.usage.cost_micro_usd.unwrap_or(0),
                Dimension::Concurrency => obs.usage.requests, // track as concurrency slot
            };
            if amount > 0 {
                tracker.record(amount, now);
            }
        }

        // Update health
        let cooldown_secs = pr.config.cooldown_seconds;
        let health_config = &self.inner.config.health;
        let mut health = pr.health.write();
        let was_in_cooldown = health.is_in_cooldown(now);

        match obs.outcome.status {
            StatusClass::Success | StatusClass::ClientError => {
                health.record_success(obs.outcome.latency_ms, now, health_config);
            }
            StatusClass::RateLimited => {
                health.record_rate_limited(now, health_config, cooldown_secs);
            }
            StatusClass::Forbidden => {
                health.record_forbidden(now, health_config, cooldown_secs);
            }
            StatusClass::ServerError => {
                health.record_server_error(now, health_config, cooldown_secs);
            }
            StatusClass::Timeout => {
                health.record_timeout(now, health_config, cooldown_secs);
            }
        }

        // Track cooldown triggers
        if !was_in_cooldown && health.is_in_cooldown(now) {
            self.inner
                .metrics
                .cooldowns_triggered
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        Ok(())
    }

    /// Access engine metrics.
    pub fn metrics(&self) -> &Metrics {
        &self.inner.metrics
    }

    /// Get the current health score for a provider.
    pub fn provider_health(&self, provider: &str) -> Option<f32> {
        self.inner
            .providers
            .get(provider)
            .map(|pr| pr.health.read().score())
    }

    /// Check if a provider is currently in cooldown.
    pub fn provider_in_cooldown(&self, provider: &str) -> Option<bool> {
        let now = self.inner.clock.now();
        self.inner
            .providers
            .get(provider)
            .map(|pr| pr.health.read().is_in_cooldown(now))
    }

    /// Get the remaining quota for a specific dimension on a provider.
    pub fn provider_quota_remaining(&self, provider: &str, dimension: Dimension) -> Option<u64> {
        let now = self.inner.clock.now();
        self.inner.providers.get(provider).and_then(|pr| {
            pr.quota_trackers
                .iter()
                .find(|(qc, _)| qc.dimension == dimension)
                .map(|(_, tracker)| tracker.remaining(now))
        })
    }

    /// Calculate the worst (most constrained) quota state across all dimensions.
    fn worst_quota_state(
        &self,
        trackers: &[(QuotaConfig, Box<dyn crate::quota::QuotaTracker>)],
        now: Timestamp,
    ) -> (f64, f64, f64) {
        if trackers.is_empty() {
            return (1.0, f64::INFINITY, 0.0);
        }

        let mut worst_remaining = 1.0_f64;
        let mut worst_exhaustion = f64::INFINITY;
        let mut max_burn_rate = 0.0_f64;

        for (_, tracker) in trackers {
            let remaining = 1.0 - tracker.usage_ratio(now);
            let exhaustion = tracker.predicted_exhaustion_secs(now);
            let burn = tracker.burn_rate(now);

            if remaining < worst_remaining {
                worst_remaining = remaining;
            }
            if exhaustion < worst_exhaustion {
                worst_exhaustion = exhaustion;
            }
            if burn > max_burn_rate {
                max_burn_rate = burn;
            }
        }

        (worst_remaining, worst_exhaustion, max_burn_rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::MockClock;
    use crate::observation::{Outcome, Usage};
    use crate::quota::Window;

    fn setup_engine() -> (GrateLimiter, Arc<MockClock>) {
        let clock = Arc::new(MockClock::new());
        let config = EngineConfig::default().with_clock(clock.clone());
        let engine = GrateLimiter::new(config);

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

        (engine, clock)
    }

    #[test]
    fn select_returns_best_provider() {
        let (engine, _clock) = setup_engine();
        let decision = engine.select("chat").unwrap();
        // openai has higher priority and both are fully healthy
        assert_eq!(decision.provider, "openai");
        assert!(decision.score > 0.0);
        assert_eq!(decision.alternatives.len(), 1);
    }

    #[test]
    fn select_unknown_capability_errors() {
        let (engine, _clock) = setup_engine();
        let result = engine.select("nonexistent");
        assert!(matches!(result, Err(Error::UnknownCapability(_))));
    }

    #[test]
    fn observe_updates_health() {
        let (engine, _clock) = setup_engine();

        // Report a 429 for openai
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
                    latency_ms: 100,
                },
            })
            .unwrap();

        let health = engine.provider_health("openai").unwrap();
        assert!(health < 1.0);
    }

    #[test]
    fn observe_unknown_provider_errors() {
        let (engine, _clock) = setup_engine();
        let result = engine.observe(Observation {
            provider: "nonexistent".into(),
            capability: None,
            usage: Usage::default(),
            outcome: Outcome {
                status: StatusClass::Success,
                latency_ms: 100,
            },
        });
        assert!(matches!(result, Err(Error::UnknownProvider(_))));
    }

    #[test]
    fn degraded_provider_loses_to_healthy() {
        let (engine, clock) = setup_engine();

        // Degrade openai with repeated 429s
        for i in 0..3 {
            clock.advance_ms(1000);
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
                        latency_ms: 100,
                    },
                })
                .unwrap();
        }

        // openai should now be in cooldown, so anthropic wins
        let decision = engine.select("chat").unwrap();
        assert_eq!(decision.provider, "anthropic");
    }

    #[test]
    fn metrics_increment() {
        let (engine, _clock) = setup_engine();

        engine.select("chat").unwrap();
        engine.select("chat").unwrap();
        assert_eq!(engine.metrics().selects(), 2);

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
                    latency_ms: 50,
                },
            })
            .unwrap();
        assert_eq!(engine.metrics().observations(), 1);
    }

    #[test]
    fn provider_quota_tracking() {
        let (engine, _clock) = setup_engine();

        assert_eq!(
            engine.provider_quota_remaining("openai", Dimension::Requests),
            Some(100)
        );

        engine
            .observe(Observation {
                provider: "openai".into(),
                capability: None,
                usage: Usage {
                    requests: 30,
                    ..Default::default()
                },
                outcome: Outcome {
                    status: StatusClass::Success,
                    latency_ms: 100,
                },
            })
            .unwrap();

        let remaining = engine
            .provider_quota_remaining("openai", Dimension::Requests)
            .unwrap();
        assert_eq!(remaining, 70);
    }

    #[test]
    fn upsert_provider_preserves_health() {
        let (engine, clock) = setup_engine();

        // Damage openai health
        engine
            .observe(Observation {
                provider: "openai".into(),
                capability: None,
                usage: Usage {
                    requests: 1,
                    ..Default::default()
                },
                outcome: Outcome {
                    status: StatusClass::ServerError,
                    latency_ms: 100,
                },
            })
            .unwrap();

        let health_before = engine.provider_health("openai").unwrap();

        // Re-upsert provider
        engine.upsert_provider(ProviderConfig {
            name: "openai".into(),
            quotas: vec![QuotaConfig {
                dimension: Dimension::Requests,
                limit: 200, // new limit
                window: Some(Window::Minute),
            }],
            priority: 10,
            weight: 1.0,
            cooldown_seconds: 30,
        });

        // Health should be preserved
        let health_after = engine.provider_health("openai").unwrap();
        assert!((health_before - health_after).abs() < 0.01);

        // But quota should be reset to new limit
        assert_eq!(
            engine.provider_quota_remaining("openai", Dimension::Requests),
            Some(200)
        );
    }

    #[test]
    fn engine_is_clone_and_send() {
        let (engine, _) = setup_engine();
        let engine2 = engine.clone();

        // Spawn a thread to prove Send + Sync
        let handle = std::thread::spawn(move || {
            engine2.select("chat").unwrap()
        });
        let decision = handle.join().unwrap();
        assert!(!decision.provider.is_empty());
    }

    #[test]
    fn anticipatory_routing_under_pressure() {
        let (engine, clock) = setup_engine();

        // Consume 90% of openai's quota rapidly
        for _ in 0..90 {
            engine
                .observe(Observation {
                    provider: "openai".into(),
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
        clock.advance_ms(5000); // 5 seconds elapsed — very fast burn rate

        // With 10% remaining and high burn rate, anthropic should win
        // despite lower priority
        let decision = engine.select("chat").unwrap();
        // At this point openai has 10% remaining with rapid burn,
        // anthropic has 100% remaining. Anthropic should score higher.
        assert_eq!(
            decision.provider, "anthropic",
            "Anticipatory routing should prefer anthropic when openai is nearly exhausted"
        );
    }

    #[test]
    fn cooldown_expires_and_provider_recovers() {
        let (engine, clock) = setup_engine();

        // Trigger cooldown on openai
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
                        latency_ms: 100,
                    },
                })
                .unwrap();
        }

        assert_eq!(engine.provider_in_cooldown("openai"), Some(true));

        // Advance past cooldown (30s)
        clock.advance_secs(31);

        assert_eq!(engine.provider_in_cooldown("openai"), Some(false));

        // openai should be selectable again (though health is degraded)
        let decision = engine.select("chat").unwrap();
        // Either provider could win depending on health recovery
        assert!(!decision.provider.is_empty());
    }
}
