use serde::{Deserialize, Serialize};

/// An observation reported by the caller after a provider interaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    /// Which provider was used.
    pub provider: String,
    /// Which capability was invoked (optional, for metrics).
    pub capability: Option<String>,
    /// Resource usage for this interaction.
    pub usage: Usage,
    /// Outcome of the interaction.
    pub outcome: Outcome,
}

/// Resource usage for a single interaction.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Number of requests consumed (typically 1).
    pub requests: u64,
    /// Tokens consumed (e.g., LLM tokens).
    pub tokens: Option<u64>,
    /// Bytes transferred.
    pub bytes: Option<u64>,
    /// Cost in USD (micro-dollars for precision).
    pub cost_micro_usd: Option<u64>,
}

/// Outcome of a provider interaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Outcome {
    /// Classified status of the response.
    pub status: StatusClass,
    /// Response latency in milliseconds.
    pub latency_ms: u64,
}

/// Classified response status for health tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusClass {
    /// 2xx success.
    Success,
    /// 429 rate limited.
    RateLimited,
    /// 403 forbidden / banned.
    Forbidden,
    /// 5xx server error.
    ServerError,
    /// Request timed out.
    Timeout,
    /// Other client error (4xx excluding 429/403).
    ClientError,
}
