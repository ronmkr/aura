use super::assets::{index_handler, static_handler};
use super::extension::handle_extension_add;
use super::jsonrpc::{authenticate, handle_jsonrpc};
use super::metrics::metrics_handler;
use super::types::AppState;
use super::websocket::handle_ws;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use std::sync::Arc;

/// Unauthenticated liveness probe — required for Docker/K8s health checks (Decision-0051).
async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}

/// Metrics scrape endpoint — requires auth (Issue #254).
/// Accepts `X-Aura-Token` or `Authorization: Bearer <token>`.
async fn authenticated_metrics_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(err) = authenticate(&headers, &state.rpc_secret) {
        return err.into_response();
    }
    metrics_handler(State(state)).await.into_response()
}

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(index_handler))
        .route("/health", get(health_handler))
        .route("/metrics", get(authenticated_metrics_handler))
        .route("/{*file}", get(static_handler))
        .route("/jsonrpc", post(handle_jsonrpc))
        .route(
            "/ws",
            get(handle_ws).route_layer(axum::middleware::from_fn_with_state(
                Arc::clone(&state),
                crate::websocket::ws_auth_middleware,
            )),
        )
        .route("/extension/add", post(handle_extension_add))
        .with_state(state)
}
