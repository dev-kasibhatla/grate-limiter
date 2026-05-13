use grate_limiter::{EngineConfig, GrateLimiter};

/// Shared application state for the HTTP server.
#[derive(Clone)]
pub struct AppState {
    pub engine: GrateLimiter,
}

impl AppState {
    /// Create a new application state with the given engine configuration.
    pub fn new(config: EngineConfig) -> Self {
        Self {
            engine: GrateLimiter::new(config),
        }
    }

    /// Create from an existing engine instance.
    pub fn from_engine(engine: GrateLimiter) -> Self {
        Self { engine }
    }
}
