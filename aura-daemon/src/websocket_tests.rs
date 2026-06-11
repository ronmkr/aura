use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use tower::ServiceExt;

use crate::test_helpers::setup_test_app;

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
