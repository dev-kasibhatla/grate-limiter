use grate_limiter_server::{AppState, serve};
use grate_limiter::EngineConfig;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let state = AppState::new(EngineConfig::default());
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));

    serve(state, addr).await
}
