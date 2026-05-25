use super::assets::{index_handler, static_handler};
use super::extension::handle_extension_add;
use super::jsonrpc::handle_jsonrpc;
use super::metrics::metrics_handler;
use super::types::AppState;
use super::websocket::handle_ws;
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(index_handler))
        .route("/metrics", get(metrics_handler))
        .route("/*file", get(static_handler))
        .route("/jsonrpc", post(handle_jsonrpc))
        .route("/ws", get(handle_ws))
        .route("/extension/add", post(handle_extension_add))
        .with_state(state)
}
