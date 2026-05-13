use serde::{Deserialize, Serialize};

/// Quota dimension — what resource is being tracked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    /// Number of requests.
    Requests,
    /// Number of tokens (e.g., LLM tokens).
    Tokens,
    /// Concurrent active requests.
    Concurrency,
    /// Cost in USD.
    CostUsd,
    /// Bytes transferred.
    Bytes,
}

/// Time window for quota reset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Window {
    /// Per-second window.
    Second,
    /// Per-minute window.
    Minute,
    /// Per-hour window.
    Hour,
    /// Per-day window.
    Day,
}

impl Window {
    /// Duration of this window in nanoseconds.
    pub fn as_nanos(&self) -> u64 {
        match self {
            Window::Second => 1_000_000_000,
            Window::Minute => 60_000_000_000,
            Window::Hour => 3_600_000_000_000,
            Window::Day => 86_400_000_000_000,
        }
    }

    /// Duration of this window in seconds.
    pub fn as_secs(&self) -> u64 {
        match self {
            Window::Second => 1,
            Window::Minute => 60,
            Window::Hour => 3_600,
            Window::Day => 86_400,
        }
    }
}

/// Configuration for a single quota dimension on a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaConfig {
    /// What resource is being limited.
    pub dimension: Dimension,
    /// Maximum allowed usage within the window.
    pub limit: u64,
    /// Time window for the limit. `None` for concurrency (no window).
    pub window: Option<Window>,
}

mod concurrency;
mod fixed_window;
mod sliding_window;
mod strategy;
mod token_bucket;

pub(crate) use concurrency::ConcurrencyLimiter;
pub(crate) use strategy::QuotaTracker;
pub(crate) use token_bucket::TokenBucket;

use crate::clock::Timestamp;

/// Create appropriate quota tracker for a given config.
pub(crate) fn create_tracker(config: &QuotaConfig, now: Timestamp) -> Box<dyn QuotaTracker> {
    match config.dimension {
        Dimension::Concurrency => Box::new(ConcurrencyLimiter::new(config.limit)),
        _ => match config.window {
            Some(window) => Box::new(TokenBucket::new(config.limit, window, now)),
            None => Box::new(TokenBucket::new(config.limit, Window::Minute, now)),
        },
    }
}
