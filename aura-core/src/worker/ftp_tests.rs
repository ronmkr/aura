use super::*;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;

#[tokio::test]
async fn test_ftp_worker_retry_on_connection_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let port = addr.port();

    let connections_count = Arc::new(AtomicU32::new(0));
    let count_clone = Arc::clone(&connections_count);

    tokio::spawn(async move {
        while let Ok((_stream, _)) = listener.accept().await {
            count_clone.fetch_add(1, Ordering::SeqCst);
        }
    });

    let worker = FtpWorker::new(FtpWorkerOptions {
        uri: format!("ftp://127.0.0.1:{}/test_file.bin", port),
        local_addr: None,
        retry_count: 3,
        http_retry_delay_secs: 0,
        happy_eyeballs_stagger_ms: 250,
        credential_provider: None,
        resource_governor: None,
        tenant_id: None,
    });

    let result = worker.resolve_metadata().await;
    assert!(result.is_err());
    assert_eq!(connections_count.load(Ordering::SeqCst), 4);
}
