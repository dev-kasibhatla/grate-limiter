//! # grate-limiter-server
//!
//! HTTP server for the grate-limiter anticipatory rate-limit engine.
//!
//! Provides a REST API wrapping the core `grate-limiter` library, enabling use from
//! any language or service via HTTP.

mod routes;
mod state;

pub use state::AppState;

use axum::Router;
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;

/// Build the axum router with all routes.
pub fn app(state: AppState) -> Router {
    Router::new()
        .merge(routes::routes())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Start the HTTP server on the given address.
pub async fn serve(state: AppState, addr: SocketAddr) -> std::io::Result<()> {
    let app = app(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("grate-limiter-server listening on {addr}");
    axum::serve(listener, app).await
}
