use super::types::AppState;
use axum::{
    body::Body,
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use prometheus::{Gauge, Registry};
use std::sync::Arc;

pub struct DaemonMetrics {
    pub registry: Registry,
    pub task_count: Gauge,
    pub total_downloaded: Gauge,
    pub total_uploaded: Gauge,
    pub subtask_count: Gauge,
}

impl DaemonMetrics {
    pub fn new() -> Self {
        let registry = Registry::new();

        let task_count = Gauge::new("aura_tasks_active", "Number of active tasks").unwrap();
        registry.register(Box::new(task_count.clone())).unwrap();

        let total_downloaded = Gauge::new(
            "aura_bytes_downloaded_total",
            "Total bytes downloaded across all tasks",
        )
        .unwrap();
        registry
            .register(Box::new(total_downloaded.clone()))
            .unwrap();

        let total_uploaded = Gauge::new(
            "aura_bytes_uploaded_total",
            "Total bytes uploaded across all tasks",
        )
        .unwrap();
        registry.register(Box::new(total_uploaded.clone())).unwrap();

        let subtask_count = Gauge::new(
            "aura_subtasks_active",
            "Total number of active protocol workers",
        )
        .unwrap();
        registry.register(Box::new(subtask_count.clone())).unwrap();

        Self {
            registry,
            task_count,
            total_downloaded,
            total_uploaded,
            subtask_count,
        }
    }
}

impl Default for DaemonMetrics {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn metrics_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    use prometheus::{Encoder, TextEncoder};

    let encoder = TextEncoder::new();
    let metric_families = state.metrics.registry.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, encoder.format_type())
        .body(Body::from(buffer))
        .unwrap()
}

#[cfg(test)]
#[path = "metrics_tests.rs"]
mod tests;
