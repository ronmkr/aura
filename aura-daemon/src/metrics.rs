use super::types::AppState;
use axum::{
    body::Body,
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use std::sync::Arc;

pub async fn metrics_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    use prometheus::{Encoder, Gauge, Registry, TextEncoder};

    let registry = Registry::new();
    let active_tasks = state.engine.tell_active().await.unwrap_or_default();

    let task_count = Gauge::new("aura_tasks_active", "Number of active tasks").unwrap();
    registry.register(Box::new(task_count.clone())).unwrap();
    task_count.set(active_tasks.len() as f64);

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

    let mut dl = 0.0;
    let mut ul = 0.0;
    let mut st = 0.0;

    for task in active_tasks {
        dl += task.completed_length as f64;
        ul += task.uploaded_length as f64;
        st += task.subtasks.iter().filter(|s| s.active).count() as f64;
    }

    total_downloaded.set(dl);
    total_uploaded.set(ul);
    subtask_count.set(st);

    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, encoder.format_type())
        .body(Body::from(buffer))
        .unwrap()
}
