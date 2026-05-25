use super::HttpWorker;
use crate::worker::{ProtocolWorker, Segment};
use crate::Error;
use crate::TaskId;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_http_worker_referer_propagation() {
    let server = MockServer::start().await;

    // 1. Initial request redirects to 2
    Mock::given(method("GET"))
        .and(path("/start"))
        .respond_with(ResponseTemplate::new(302).insert_header("Location", "/final"))
        .mount(&server)
        .await;

    // 2. Second request must have Referer: /start
    Mock::given(method("GET"))
        .and(path("/final"))
        .and(header("Referer", &format!("{}/start", server.uri())))
        .respond_with(ResponseTemplate::new(200).set_body_string("binary_data"))
        .mount(&server)
        .await;

    let worker = HttpWorker::new(
        format!("{}/start", server.uri()),
        None,
        None,
        None,
        None,
        None,
        None,
        5,
        2,
        None,
        None,
        None,
    );
    let metadata = worker
        .resolve_metadata()
        .await
        .expect("Should resolve metadata with redirects");

    let worker_final = HttpWorker::new(
        metadata.final_uri,
        None,
        None,
        None,
        None,
        Some(format!("{}/start", server.uri())),
        None,
        5,
        2,
        None,
        None,
        None,
    );
    let throttler = std::sync::Arc::new(crate::throttler::Throttler::new(0, 0));
    let result = worker_final
        .fetch_segment(
            TaskId(1),
            Segment {
                offset: 0,
                length: 11,
            },
            None,
            throttler,
        )
        .await;

    assert!(result.is_ok(), "Worker should succeed with resolved URI");
}

#[tokio::test]
async fn test_http_worker_redirect_loop() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/a"))
        .respond_with(ResponseTemplate::new(302).insert_header("Location", "/b"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/b"))
        .respond_with(ResponseTemplate::new(302).insert_header("Location", "/a"))
        .mount(&server)
        .await;

    let worker = HttpWorker::new(
        format!("{}/a", server.uri()),
        None,
        None,
        None,
        None,
        None,
        None,
        5,
        2,
        None,
        None,
        None,
    );
    let result = worker.resolve_metadata().await;
    match result {
        Err(Error::Protocol(msg)) => assert!(msg.to_lowercase().contains("redirect")),
        _ => panic!("Expected redirect loop error"),
    }
}

#[tokio::test]
async fn test_http_worker_custom_dns() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![0; 10]))
        .mount(&server)
        .await;

    let dns_config = crate::config::ResolverConfig::Simple("system".to_string());
    let resolver = crate::net_util::create_resolver(&dns_config).await.unwrap();
    let resolver_arc = std::sync::Arc::new(resolver);

    let worker = HttpWorker::new(
        format!("{}/file", server.uri()),
        None,
        None,
        None,
        None,
        None,
        None,
        5,
        2,
        None,
        Some(resolver_arc),
        None,
    );

    let metadata = worker.resolve_metadata().await.expect("Should resolve");
    assert_eq!(metadata.total_length, Some(10));
}

#[tokio::test]
async fn test_http_worker_retry_on_503() {
    let server = MockServer::start().await;
    let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));

    Mock::given(method("GET"))
        .respond_with(move |_req: &wiremock::Request| {
            let prev = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if prev < 2 {
                ResponseTemplate::new(503)
            } else {
                ResponseTemplate::new(200)
                    .set_body_bytes(vec![1u8; 10])
                    .insert_header("Content-Range", "bytes 0-9/10")
            }
        })
        .mount(&server)
        .await;

    let worker = HttpWorker::new(
        format!("{}/retry", server.uri()),
        None,
        None,
        None,
        None,
        None,
        None,
        3, // Max retries
        1, // 1s base delay
        None,
        None,
        None,
    );

    let throttler = std::sync::Arc::new(crate::throttler::Throttler::new(0, 0));
    let result = worker
        .fetch_segment(
            TaskId(1),
            Segment {
                offset: 0,
                length: 10,
            },
            None,
            throttler,
        )
        .await;

    if let Err(ref e) = result {
        panic!("Retry test failed with error: {}", e);
    }
    assert!(result.is_ok());
    assert_eq!(result.unwrap().data.len(), 10);
}

#[tokio::test]
async fn test_http_worker_html_landing_page_resolution_success() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/landing"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/html; charset=UTF-8")
                .set_body_bytes(
                    "<html><body>Download here: <a href='/download/file.zip'>link</a></body></html>"
                        .as_bytes()
                        .to_vec(),
                ),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/download/file.zip"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/octet-stream")
                .set_body_bytes(vec![0u8; 100]),
        )
        .mount(&server)
        .await;

    let worker = HttpWorker::new(
        format!("{}/landing", server.uri()),
        None,
        None,
        None,
        None,
        None,
        None,
        3,
        1,
        None,
        None,
        None,
    );

    let result = worker.resolve_metadata().await;
    assert!(
        result.is_ok(),
        "Should successfully resolve intermediate landing page: {:?}",
        result.err()
    );
    let meta = result.unwrap();
    assert!(meta.final_uri.contains("/download/file.zip"));
    assert_eq!(meta.total_length, Some(100));
}

#[tokio::test]
async fn test_http_worker_html_landing_page_resolution_failure() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/landing"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/html; charset=UTF-8")
                .set_body_bytes(
                    "<html><body>Welcome to landing page! No direct links here.</body></html>"
                        .as_bytes()
                        .to_vec(),
                ),
        )
        .mount(&server)
        .await;

    let worker = HttpWorker::new(
        format!("{}/landing", server.uri()),
        None,
        None,
        None,
        None,
        None,
        None,
        3,
        1,
        None,
        None,
        None,
    );

    let result = worker.resolve_metadata().await;
    assert!(result.is_err());
    match result {
        Err(Error::Protocol(msg)) => assert!(msg.contains("Direct link resolution failed")),
        _ => panic!("Expected Protocol error for HTML landing page failure"),
    }
}

#[tokio::test]
async fn test_http_worker_hsts_upgrade() {
    let hsts_cache = crate::security::HstsCache::new();
    let host = "example.com".to_string();
    hsts_cache.insert_policy(host, 300, true).await;

    // Create a worker with an insecure http URL
    let http_uri = "http://example.com/file".to_string();
    let worker = HttpWorker::new(
        http_uri,
        None,
        None,
        None,
        None,
        None,
        None,
        3,
        1,
        None,
        None,
        Some(hsts_cache),
    );

    let upgraded = worker.upgrade_url(&worker.uri).await;
    assert_eq!(upgraded, "https://example.com/file");
}
