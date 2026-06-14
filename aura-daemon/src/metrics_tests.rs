use super::*;
use axum::{extract::State, response::IntoResponse};

#[tokio::test]
async fn test_metrics_registration() {
    let dm = DaemonMetrics::new();
    let families = dm.registry.gather();

    // Verify all 4 metrics are registered
    let mut names: Vec<String> = families.iter().map(|f| f.name().to_string()).collect();
    names.sort();

    assert_eq!(
        names,
        vec![
            "aura_bytes_downloaded_total".to_string(),
            "aura_bytes_uploaded_total".to_string(),
            "aura_subtasks_active".to_string(),
            "aura_tasks_active".to_string(),
        ]
    );
}

#[tokio::test]
async fn test_metrics_handler() {
    // Bootstrap minimal engine
    let mut config = aura_core::Config::default();
    config.network.listen_port = 0; // prevent conflict
    let (engine, _orch, _store) = aura_core::orchestrator::Engine::new(config).await.unwrap();
    let engine = Arc::new(engine);

    let metrics = Arc::new(DaemonMetrics::new());
    // Manually set some metric values to check output formatting
    metrics.task_count.set(3.0);
    metrics.total_downloaded.set(1024.0);
    metrics.total_uploaded.set(512.0);
    metrics.subtask_count.set(5.0);

    let state = Arc::new(AppState {
        engine,
        rpc_secret: None,
        metrics,
        rss_refresh_tx: None,
    });

    let response = metrics_handler(State(state)).await.into_response();
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    let headers = response.headers();
    assert!(headers
        .get(axum::http::header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap()
        .contains("text/plain"));

    // Extract body bytes
    let body_bytes = axum::body::to_bytes(response.into_body(), 10000)
        .await
        .unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

    assert!(body_str.contains("aura_tasks_active 3"));
    assert!(body_str.contains("aura_bytes_downloaded_total 1024"));
    assert!(body_str.contains("aura_bytes_uploaded_total 512"));
    assert!(body_str.contains("aura_subtasks_active 5"));
}
