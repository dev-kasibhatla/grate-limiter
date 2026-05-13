//! # grate-limiter
//!
//! Anticipatory rate-limit orchestration engine for multi-provider systems.
//!
//! **Predict limits before providers enforce them.**
//!
//! `grate-limiter` is not a retry library, proxy, or gateway. It is a predictive provider
//! orchestration engine that routes traffic intelligently across providers by continuously
//! learning their health, quota usage, and reliability — preventing rate-limit errors
//! before they happen.
//!
//! # Quick Start
//!
//! ```rust
//! use grate_limiter::{GrateLimiter, EngineConfig, ProviderConfig, CapabilityConfig};
//! use grate_limiter::{QuotaConfig, Dimension, Window, CapabilityProvider};
//! use grate_limiter::{Observation, Usage, Outcome, StatusClass};
//!
//! // Create the engine
//! let engine = GrateLimiter::new(EngineConfig::default());
//!
//! // Register providers with their quotas
//! engine.upsert_provider(ProviderConfig {
//!     name: "openai".into(),
//!     quotas: vec![
//!         QuotaConfig { dimension: Dimension::Requests, limit: 5000, window: Some(Window::Minute) },
//!     ],
//!     priority: 10,
//!     weight: 1.0,
//!     cooldown_seconds: 30,
//! });
//!
//! engine.upsert_provider(ProviderConfig {
//!     name: "anthropic".into(),
//!     quotas: vec![
//!         QuotaConfig { dimension: Dimension::Requests, limit: 3000, window: Some(Window::Minute) },
//!     ],
//!     priority: 8,
//!     weight: 1.0,
//!     cooldown_seconds: 30,
//! });
//!
//! // Register a capability with its providers
//! engine.upsert_capability(CapabilityConfig {
//!     name: "chat-completion".into(),
//!     providers: vec![
//!         CapabilityProvider { provider: "openai".into(), priority: 10 },
//!         CapabilityProvider { provider: "anthropic".into(), priority: 8 },
//!     ],
//! });
//!
//! // Select the best provider
//! let decision = engine.select("chat-completion").unwrap();
//! println!("Use: {} (score: {:.2})", decision.provider, decision.score);
//!
//! // Report what happened
//! engine.observe(Observation {
//!     provider: "openai".into(),
//!     capability: Some("chat-completion".into()),
//!     usage: Usage { requests: 1, tokens: Some(1200), ..Default::default() },
//!     outcome: Outcome { status: StatusClass::Success, latency_ms: 830 },
//! }).unwrap();
//! ```
//!
//! # Architecture
//!
//! The engine has four major subsystems:
//!
//! - **Quota Tracking**: Token bucket / sliding window / fixed window / concurrency strategies
//! - **Health Engine**: EWMA-based scoring with exponential decay and automatic cooldowns
//! - **Scoring Engine**: Weighted composite scoring with anticipatory exhaustion prediction
//! - **Decision Engine**: Deterministic, explainable provider selection
//!
//! # Deterministic Testing
//!
//! Use [`MockClock`] for fully deterministic, reproducible tests:
//!
//! ```rust
//! use grate_limiter::{GrateLimiter, EngineConfig, MockClock};
//! use std::sync::Arc;
//!
//! let clock = Arc::new(MockClock::new());
//! let config = EngineConfig::default().with_clock(clock.clone());
//! let engine = GrateLimiter::new(config);
//!
//! // Advance time deterministically
//! clock.advance_ms(1000);
//! ```

mod capability;
mod clock;
mod config;
mod decision;
mod engine;
mod error;
mod health;
mod metrics;
mod observation;
mod provider;
mod quota;
mod scoring;

// Public API re-exports
pub use capability::{CapabilityConfig, CapabilityProvider};
pub use clock::{Clock, MockClock, RealClock, Timestamp};
pub use config::EngineConfig;
pub use decision::{Decision, ScoreBreakdown};
pub use engine::GrateLimiter;
pub use error::{Error, Result};
pub use health::HealthConfig;
pub use observation::{Observation, Outcome, StatusClass, Usage};
pub use provider::ProviderConfig;
pub use quota::{Dimension, QuotaConfig, Window};
pub use scoring::{ScoringStrategy, ScoringWeights};
