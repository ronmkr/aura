use aura_core::orchestrator::Engine;
use aura_core::task::TaskType;
use aura_core::TaskId;
use aura_daemon::{create_router, AppState};
use futures_util::StreamExt;
use serde_json::Value;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;

#[tokio::test]
async fn test_ws_telemetry() {
    // 1. Setup Engine
    let mut config = aura_core::Config::default();
    config.network.listen_port = 0; // Prevent port conflicts in parallel tests
    let (engine, orchestrator, storage) = Engine::new(config).await.unwrap();
    let engine = Arc::new(engine);

    tokio::spawn(async move {
        let _ = orchestrator.run().await;
    });
    tokio::spawn(async move {
        let _ = storage.run().await;
    });

    // 2. Setup Server
    let state = Arc::new(AppState {
        engine: engine.clone(),
        rpc_secret: Some("test-secret".to_string()),
    });

    let app = create_router(state);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // 3. Connect via WebSocket
    let ws_url = format!("ws://{}/ws?token=test-secret", addr);
    let (ws_stream, _) = connect_async(ws_url).await.expect("Failed to connect");
    let (_, mut read) = ws_stream.split();

    // 4. Trigger an event
    let id = TaskId(12345);
    engine
        .add_task_with_sources(
            id,
            "test-task".to_string(),
            vec![("http://example.com".to_string(), TaskType::Http)],
        )
        .await
        .unwrap();

    // 5. Verify event received over WS
    if let Some(Ok(msg)) = read.next().await {
        if let TungsteniteMessage::Text(text) = msg {
            let val: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(val["method"], "aura.onEvent");
            assert_eq!(val["params"]["TaskAdded"], 12345);
        } else {
            panic!("Expected text message, got {:?}", msg);
        }
    } else {
        panic!("Expected message from WS");
    }
}
