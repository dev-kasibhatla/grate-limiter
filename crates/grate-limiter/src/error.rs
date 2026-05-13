use thiserror::Error;

/// All errors produced by grate-limiter.
#[derive(Debug, Error)]
pub enum Error {
    /// The requested capability does not exist.
    #[error("unknown capability: {0}")]
    UnknownCapability(String),

    /// The referenced provider does not exist.
    #[error("unknown provider: {0}")]
    UnknownProvider(String),

    /// No providers are available for the requested capability.
    #[error("no available providers for capability: {0}")]
    NoAvailableProviders(String),

    /// Provider referenced in capability is not registered.
    #[error("capability '{capability}' references unregistered provider '{provider}'")]
    ProviderNotRegistered {
        capability: String,
        provider: String,
    },

    /// Invalid configuration value.
    #[error("invalid config: {0}")]
    InvalidConfig(String),
}

/// Result type alias for grate-limiter operations.
pub type Result<T> = std::result::Result<T, Error>;
