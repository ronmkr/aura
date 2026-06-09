use super::HttpWorker;
use crate::orchestrator::resource_governor::ResourceGovernor;
use crate::worker::builder::WorkerOptions;
use crate::worker::{ProtocolWorker, Segment};
use std::sync::Arc;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_http_worker_resource_governor() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![0; 50]))
        .mount(&server)
        .await;

    let governor = Arc::new(ResourceGovernor::new(100, 20));
    let tenant = Some(crate::TenantId("tenant1".to_string()));

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
        alt_svc_cache: None,
        resource_governor: Some(governor.clone()),
        tenant_id: tenant.clone(),
        client_pool: None,
        if_none_match: None,
        if_modified_since: None,
        tcp_keepalive_secs: None,
    });

    let throttler = std::sync::Arc::new(crate::throttler::Throttler::new(0, 0, 100));

    let piece = worker
        .fetch_segment(
            crate::TaskId(1),
            Segment {
                offset: 0,
                length: 50,
            },
            None,
            None,
            throttler,
        )
        .await
        .unwrap();

    assert_eq!(piece.data.len(), 50);
    assert_eq!(governor.current_usage(), 0);
}

#[tokio::test]
async fn test_http_worker_resource_governor_backpressure() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![0; 50]))
        .mount(&server)
        .await;

    let governor = Arc::new(ResourceGovernor::new(60, 20));
    let tenant = Some(crate::TenantId("tenant1".to_string()));

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
        alt_svc_cache: None,
        resource_governor: Some(governor.clone()),
        tenant_id: tenant.clone(),
        client_pool: None,
        if_none_match: None,
        if_modified_since: None,
        tcp_keepalive_secs: None,
    });

    let throttler = std::sync::Arc::new(crate::throttler::Throttler::new(0, 0, 100));

    let res = tokio::time::timeout(
        std::time::Duration::from_millis(300),
        worker.fetch_segment(
            crate::TaskId(1),
            Segment {
                offset: 0,
                length: 50,
            },
            None,
            None,
            throttler,
        ),
    )
    .await;

    assert!(res.is_err());
}
