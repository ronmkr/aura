use super::HttpWorker;
use crate::worker::builder::WorkerOptions;
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

    let worker = HttpWorker::new(WorkerOptions {
        uri: format!("{}/start", server.uri()),
        local_addr: None,
        user_agent: None,
        connect_timeout: None,
        proxy: None,
        referer: None,
        retry_count: 5,
        retry_delay_secs: 2,
        max_redirects: 20,
        happy_eyeballs_stagger_ms: 250,
        http_buffer_capacity: 16384,
        http_concurrent_requests: 32,
        credential_provider: None,
        dns_resolver: None,
        hsts_cache: None,
        alt_svc_cache: None,
        resource_governor: None,
        tenant_id: None,
        client_pool: None,
        if_none_match: None,
        if_modified_since: None,
        tcp_keepalive_secs: None,
    });
    let metadata = worker
        .resolve_metadata()
        .await
        .expect("Should resolve metadata with redirects");

    let worker_final = HttpWorker::new(WorkerOptions {
        uri: metadata.final_uri,
        local_addr: None,
        user_agent: None,
        connect_timeout: None,
        proxy: None,
        referer: Some(format!("{}/start", server.uri())),
        retry_count: 5,
        retry_delay_secs: 2,
        max_redirects: 20,
        happy_eyeballs_stagger_ms: 250,
        http_buffer_capacity: 16384,
        http_concurrent_requests: 32,
        credential_provider: None,
        dns_resolver: None,
        hsts_cache: None,
        alt_svc_cache: None,
        resource_governor: None,
        tenant_id: None,
        client_pool: None,
        if_none_match: None,
        if_modified_since: None,
        tcp_keepalive_secs: None,
    });
    let throttler = std::sync::Arc::new(crate::throttler::Throttler::new(0, 0, 100));
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

    let worker = HttpWorker::new(WorkerOptions {
        uri: format!("{}/a", server.uri()),
        local_addr: None,
        user_agent: None,
        connect_timeout: None,
        proxy: None,
        referer: None,
        retry_count: 5,
        retry_delay_secs: 2,
        max_redirects: 20,
        happy_eyeballs_stagger_ms: 250,
        http_buffer_capacity: 16384,
        http_concurrent_requests: 32,
        credential_provider: None,
        dns_resolver: None,
        hsts_cache: None,
        alt_svc_cache: None,
        resource_governor: None,
        tenant_id: None,
        client_pool: None,
        if_none_match: None,
        if_modified_since: None,
        tcp_keepalive_secs: None,
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

    let worker = HttpWorker::new(WorkerOptions {
        uri: format!("{}/file", server.uri()),
        local_addr: None,
        user_agent: None,
        connect_timeout: None,
        proxy: None,
        referer: None,
        retry_count: 5,
        retry_delay_secs: 2,
        max_redirects: 20,
        happy_eyeballs_stagger_ms: 250,
        http_buffer_capacity: 16384,
        http_concurrent_requests: 32,
        credential_provider: None,
        dns_resolver: Some(resolver_arc),
        hsts_cache: None,
        alt_svc_cache: None,
        resource_governor: None,
        tenant_id: None,
        client_pool: None,
        if_none_match: None,
        if_modified_since: None,
        tcp_keepalive_secs: None,
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

    let worker = HttpWorker::new(WorkerOptions {
        uri: format!("{}/retry", server.uri()),
        local_addr: None,
        user_agent: None,
        connect_timeout: None,
        proxy: None,
        referer: None,
        retry_count: 3,      // Max retries
        retry_delay_secs: 1, // 1s base delay
        max_redirects: 20,
        happy_eyeballs_stagger_ms: 250,
        http_buffer_capacity: 16384,
        http_concurrent_requests: 32,
        credential_provider: None,
        dns_resolver: None,
        hsts_cache: None,
        alt_svc_cache: None,
        resource_governor: None,
        tenant_id: None,
        client_pool: None,
        if_none_match: None,
        if_modified_since: None,
        tcp_keepalive_secs: None,
    });

    let throttler = std::sync::Arc::new(crate::throttler::Throttler::new(0, 0, 100));
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
    let temp_dir = tempfile::tempdir().unwrap();
    let sandbox_path = temp_dir.path().to_str().unwrap().to_string();
    let mut config = crate::Config::default();
    config.storage.sandbox_root = Some(sandbox_path);
    let config_swap = std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(config));
    let hsts_cache = crate::security::HstsCache::new(config_swap);
    let host = "example.com".to_string();
    hsts_cache.insert_policy(host, 300, true).await;

    // Create a worker with an insecure http URL
    let http_uri = "http://example.com/file".to_string();
    let worker = HttpWorker::new(WorkerOptions {
        uri: http_uri,
        local_addr: None,
        user_agent: None,
        connect_timeout: None,
        proxy: None,
        referer: None,
        retry_count: 3,
        retry_delay_secs: 1,
        max_redirects: 20,
        happy_eyeballs_stagger_ms: 250,
        http_buffer_capacity: 16384,
        http_concurrent_requests: 32,
        credential_provider: None,
        dns_resolver: None,
        hsts_cache: Some(hsts_cache),
        alt_svc_cache: None,
        resource_governor: None,
        tenant_id: None,
        client_pool: None,
        if_none_match: None,
        if_modified_since: None,
        tcp_keepalive_secs: None,
    });

    let upgraded = worker.upgrade_url(&worker.options.uri).await;
    assert_eq!(upgraded, "https://example.com/file");
}

#[tokio::test]
async fn test_http_worker_alt_svc_header_caching() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/file"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("alt-svc", "h3=\":8443\"; ma=600")
                .set_body_string("data"),
        )
        .mount(&server)
        .await;

    let temp_dir = tempfile::tempdir().unwrap();
    let sandbox_path = temp_dir.path().to_str().unwrap().to_string();
    let mut config = crate::Config::default();
    config.storage.sandbox_root = Some(sandbox_path);
    let config_swap = std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(config));
    let alt_svc_cache = crate::security::AltSvcCache::new(config_swap);
    let worker = HttpWorker::new(WorkerOptions {
        uri: format!("{}/file", server.uri()),
        local_addr: None,
        user_agent: None,
        connect_timeout: None,
        proxy: None,
        referer: None,
        retry_count: 1,
        retry_delay_secs: 1,
        max_redirects: 20,
        happy_eyeballs_stagger_ms: 250,
        http_buffer_capacity: 16384,
        http_concurrent_requests: 32,
        credential_provider: None,
        dns_resolver: None,
        hsts_cache: None,
        alt_svc_cache: Some(alt_svc_cache.clone()),
        resource_governor: None,
        tenant_id: None,
        client_pool: None,
        if_none_match: None,
        if_modified_since: None,
        tcp_keepalive_secs: None,
    });

    let result = worker.resolve_metadata().await;
    assert!(result.is_ok());

    let parsed_uri = url::Url::parse(&server.uri()).unwrap();
    let host = parsed_uri.host_str().unwrap();

    let policy = alt_svc_cache.get_alt_svc(host).await.unwrap();
    assert_eq!(policy.alt_protocol, "h3");
    assert_eq!(policy.alt_port, 8443);
}
