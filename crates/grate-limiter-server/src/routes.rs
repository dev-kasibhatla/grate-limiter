use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use grate_limiter::{CapabilityConfig, Decision, Observation, ProviderConfig};

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health_check))
        .route("/providers", post(upsert_provider))
        .route("/capabilities", post(upsert_capability))
        .route("/select", post(select_provider))
        .route("/observe", post(observe))
        .route("/metrics", get(get_metrics))
}

async fn health_check() -> &'static str {
    "ok"
}

async fn upsert_provider(
    State(state): State<AppState>,
    Json(config): Json<ProviderConfig>,
) -> StatusCode {
    state.engine.upsert_provider(config);
    StatusCode::OK
}

async fn upsert_capability(
    State(state): State<AppState>,
    Json(config): Json<CapabilityConfig>,
) -> StatusCode {
    state.engine.upsert_capability(config);
    StatusCode::OK
}

#[derive(Deserialize)]
struct SelectRequest {
    capability: String,
}

async fn select_provider(
    State(state): State<AppState>,
    Json(req): Json<SelectRequest>,
) -> Result<Json<Decision>, (StatusCode, String)> {
    state
        .engine
        .select(&req.capability)
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
}

async fn observe(
    State(state): State<AppState>,
    Json(obs): Json<Observation>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .engine
        .observe(obs)
        .map(|_| StatusCode::OK)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
}

#[derive(Serialize)]
struct MetricsResponse {
    selects: u64,
    observations: u64,
    cooldowns_triggered: u64,
    no_provider_available: u64,
}

async fn get_metrics(State(state): State<AppState>) -> Json<MetricsResponse> {
    let m = state.engine.metrics();
    Json(MetricsResponse {
        selects: m.selects(),
        observations: m.observations(),
        cooldowns_triggered: m.cooldowns_triggered(),
        no_provider_available: m.no_provider_available(),
    })
}
