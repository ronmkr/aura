use crate::types::AppState;
use aura_core::orchestrator::Engine;
use aura_core::Config;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use std::sync::Arc;
use tower::ServiceExt;

async fn setup_test_app(secret: Option<String>) -> (axum::Router, tempfile::TempDir) {
    let mut config = Config::default();
    config.network.dht_port = 0;
    let temp_dir = tempfile::tempdir().unwrap();
    config.storage.download_dir = temp_dir.path().to_string_lossy().into_owned();

    let (engine, _orchestrator, _storage) = Engine::new(config).await.unwrap();
    let metrics = Arc::new(crate::metrics::DaemonMetrics::new());
    let state = Arc::new(AppState {
        engine: Arc::new(engine),
        rpc_secret: secret,
        metrics,
    });
    (crate::router::create_router(state), temp_dir)
}

#[tokio::test]
async fn test_ws_authentication() {
    // 1. Test missing token when secret is configured -> UNAUTHORIZED
    let (app, _temp) = setup_test_app(Some("secret123".to_string())).await;
    let req = Request::builder()
        .method("GET")
        .uri("/ws")
        .body(Body::empty())
        .unwrap();
    let res: Response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 2. Test valid token via query parameter -> BAD_REQUEST (not upgraded, but authenticated)
    let req = Request::builder()
        .method("GET")
        .uri("/ws?token=secret123")
        .body(Body::empty())
        .unwrap();
    let res: Response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // 3. Test valid token via X-Aura-Token header -> BAD_REQUEST
    let req = Request::builder()
        .method("GET")
        .uri("/ws")
        .header("X-Aura-Token", "secret123")
        .body(Body::empty())
        .unwrap();
    let res: Response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // 4. Test valid token via Authorization Bearer header -> BAD_REQUEST
    let req = Request::builder()
        .method("GET")
        .uri("/ws")
        .header("Authorization", "Bearer secret123")
        .body(Body::empty())
        .unwrap();
    let res: Response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // 5. Test no secret configured -> BAD_REQUEST (always passes auth)
    let (app_no_auth, _temp) = setup_test_app(None).await;
    let req = Request::builder()
        .method("GET")
        .uri("/ws")
        .body(Body::empty())
        .unwrap();
    let res: Response = app_no_auth.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}
