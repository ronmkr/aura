use crate::jsonrpc::authenticate;
use crate::types::AppState;
use aura_core::orchestrator::Engine;
use aura_core::Config;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use serde_json::json;
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
async fn test_authentication_logic() {
    use axum::http::HeaderMap;

    let secret = Some("my_super_secret_token".to_string());

    // 1. Correct X-Aura-Token header
    let mut headers = HeaderMap::new();
    headers.insert("X-Aura-Token", "my_super_secret_token".parse().unwrap());
    assert!(authenticate(&headers, &secret).is_ok());

    // 2. Correct Authorization header with Bearer
    let mut headers = HeaderMap::new();
    headers.insert(
        "Authorization",
        "Bearer my_super_secret_token".parse().unwrap(),
    );
    assert!(authenticate(&headers, &secret).is_ok());

    // 3. Incorrect X-Aura-Token
    let mut headers = HeaderMap::new();
    headers.insert("X-Aura-Token", "wrong_token".parse().unwrap());
    assert!(authenticate(&headers, &secret).is_err());

    // 4. Missing headers
    let headers = HeaderMap::new();
    assert!(authenticate(&headers, &secret).is_err());

    // 5. No secret configured - should always pass
    let headers = HeaderMap::new();
    assert!(authenticate(&headers, &None).is_ok());
}

#[tokio::test]
async fn test_jsonrpc_unauthorized_error() {
    let (app, _temp) = setup_test_app(Some("secret".to_string())).await;

    let req = Request::builder()
        .method("POST")
        .uri("/jsonrpc")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "jsonrpc": "2.0",
                "method": "aria2.getVersion",
                "id": 1
            }))
            .unwrap(),
        ))
        .unwrap();

    let res: Response = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_jsonrpc_get_version_success() {
    let (app, _temp) = setup_test_app(Some("secret".to_string())).await;

    let req = Request::builder()
        .method("POST")
        .uri("/jsonrpc")
        .header("content-type", "application/json")
        .header("X-Aura-Token", "secret")
        .body(Body::from(
            serde_json::to_string(&json!({
                "jsonrpc": "2.0",
                "method": "aria2.getVersion",
                "id": 42
            }))
            .unwrap(),
        ))
        .unwrap();

    let res: Response = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body_json["id"].as_i64(), Some(42));
    assert!(body_json["result"]["version"].is_string());
}

#[tokio::test]
async fn test_jsonrpc_get_session_info_success() {
    let (app, _temp) = setup_test_app(None).await;

    let req = Request::builder()
        .method("POST")
        .uri("/jsonrpc")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "jsonrpc": "2.0",
                "method": "aria2.getSessionInfo",
                "id": "abc"
            }))
            .unwrap(),
        ))
        .unwrap();

    let res: Response = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body_json["id"].as_str(), Some("abc"));
    assert!(body_json["result"]["sessionId"].is_string());
}

#[tokio::test]
async fn test_add_uri_validation_ssrf() {
    let (app, _temp) = setup_test_app(None).await;

    // Test file:// schema (must be rejected)
    let req = Request::builder()
        .method("POST")
        .uri("/jsonrpc")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "jsonrpc": "2.0",
                "method": "aria2.addUri",
                "params": [["file:///etc/passwd"]],
                "id": 1
            }))
            .unwrap(),
        ))
        .unwrap();

    let res: Response = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert!(!body_json["error"].is_null());
}
