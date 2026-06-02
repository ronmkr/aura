use super::{HttpWorker, HttpWorkerOptions};
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

    let worker = HttpWorker::new(HttpWorkerOptions {
        uri: format!("{}/start", server.uri()),
        local_addr: None,
        user_agent: None,
        connect_timeout: None,
        proxy: None,
        referer: None,
        retry_count: 5,
        retry_delay_secs: 2,
        credential_provider: None,
        dns_resolver: None,
        hsts_cache: None,
    });
    let metadata = worker
        .resolve_metadata()
        .await
        .expect("Should resolve metadata with redirects");

    let worker_final = HttpWorker::new(HttpWorkerOptions {
        uri: metadata.final_uri,
        local_addr: None,
        user_agent: None,
        connect_timeout: None,
        proxy: None,
        referer: Some(format!("{}/start", server.uri())),
        retry_count: 5,
        retry_delay_secs: 2,
        credential_provider: None,
        dns_resolver: None,
        hsts_cache: None,
    });
    let throttler = std::sync::Arc::new(crate::throttler::Throttler::new(0, 0));
    let result = worker_final
        .fetch_segment(
            TaskId(1),
            Segment {
                offset: 0,
                length: 11,
            },
            None,
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

    let worker = HttpWorker::new(HttpWorkerOptions {
        uri: format!("{}/a", server.uri()),
        local_addr: None,
        user_agent: None,
        connect_timeout: None,
        proxy: None,
        referer: None,
        retry_count: 5,
        retry_delay_secs: 2,
        credential_provider: None,
        dns_resolver: None,
        hsts_cache: None,
    });
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

    let worker = HttpWorker::new(HttpWorkerOptions {
        uri: format!("{}/file", server.uri()),
        local_addr: None,
        user_agent: None,
        connect_timeout: None,
        proxy: None,
        referer: None,
        retry_count: 5,
        retry_delay_secs: 2,
        credential_provider: None,
        dns_resolver: Some(resolver_arc),
        hsts_cache: None,
    });

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

    let worker = HttpWorker::new(HttpWorkerOptions {
        uri: format!("{}/retry", server.uri()),
        local_addr: None,
        user_agent: None,
        connect_timeout: None,
        proxy: None,
        referer: None,
        retry_count: 3,      // Max retries
        retry_delay_secs: 1, // 1s base delay
        credential_provider: None,
        dns_resolver: None,
        hsts_cache: None,
    });

    let throttler = std::sync::Arc::new(crate::throttler::Throttler::new(0, 0));
    let result = worker
        .fetch_segment(
            TaskId(1),
            Segment {
                offset: 0,
                length: 10,
            },
            None,
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
async fn test_http_worker_hsts_upgrade() {
    let hsts_cache = crate::security::HstsCache::new();
    let host = "example.com".to_string();
    hsts_cache.insert_policy(host, 300, true).await;

    // Create a worker with an insecure http URL
    let http_uri = "http://example.com/file".to_string();
    let worker = HttpWorker::new(HttpWorkerOptions {
        uri: http_uri,
        local_addr: None,
        user_agent: None,
        connect_timeout: None,
        proxy: None,
        referer: None,
        retry_count: 3,
        retry_delay_secs: 1,
        credential_provider: None,
        dns_resolver: None,
        hsts_cache: Some(hsts_cache),
    });

    let upgraded = worker.upgrade_url(&worker.options.uri).await;
    assert_eq!(upgraded, "https://example.com/file");
}

#[path = "captive_portal_tests.rs"]
mod captive_portal_tests;
