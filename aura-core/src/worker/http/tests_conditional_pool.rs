use super::{HttpWorker, HttpWorkerOptions};
use crate::Error;
use std::sync::Arc;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_client_pool_sharing_and_eviction() {
    use super::{ClientKey, ClientPool};

    let pool = ClientPool::new();

    let key1 = ClientKey {
        scheme: "http".to_string(),
        host: "localhost".to_string(),
        port: 8080,
    };
    let key2 = ClientKey {
        scheme: "http".to_string(),
        host: "localhost".to_string(),
        port: 8080,
    };
    let key3 = ClientKey {
        scheme: "http".to_string(),
        host: "localhost".to_string(),
        port: 8081,
    };

    // 1. Get first client
    let client1 = pool.get_or_create(key1.clone(), reqwest::Client::new);

    // 2. Get second client for same key, should be shared
    let client2 = pool.get_or_create(key2, reqwest::Client::new);
    assert!(Arc::ptr_eq(&client1, &client2));
    assert_eq!(pool.len(), 1);

    // 3. Get third client for different key, should be different
    let client3 = pool.get_or_create(key3.clone(), reqwest::Client::new);
    assert!(!Arc::ptr_eq(&client1, &client3));
    assert_eq!(pool.len(), 2);

    // 4. Drop references to client1 and client2
    drop(client1);
    drop(client2);

    // Now, key1 client is only held by the pool (strong count = 1).
    // key3 client is held by the pool and client3 (strong count = 2).

    // 5. Trigger next get_or_create to trigger eviction
    let key4 = ClientKey {
        scheme: "http".to_string(),
        host: "localhost".to_string(),
        port: 8082,
    };
    let _client4 = pool.get_or_create(key4, reqwest::Client::new);

    // key1 client should have been evicted.
    // key3 client should remain.
    // key4 client is added.
    // So length should be 2 (key3 and key4).
    assert_eq!(pool.len(), 2);

    // Drop client3 reference
    drop(client3);

    // Trigger another get_or_create to evict key3
    let key5 = ClientKey {
        scheme: "http".to_string(),
        host: "localhost".to_string(),
        port: 8083,
    };
    let _client5 = pool.get_or_create(key5, reqwest::Client::new);
    assert_eq!(pool.len(), 2); // key4 and key5
}

#[tokio::test]
async fn test_http_worker_conditional_get_304() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/conditional"))
        .and(header("if-none-match", "etag123"))
        .and(wiremock::matchers::header_exists("if-modified-since"))
        .respond_with(ResponseTemplate::new(304))
        .mount(&server)
        .await;

    let worker = HttpWorker::new(HttpWorkerOptions {
        uri: format!("{}/conditional", server.uri()),
        local_addr: None,
        user_agent: None,
        connect_timeout: None,
        proxy: None,
        referer: None,
        retry_count: 1,
        http_retry_delay_secs: 1,
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
        if_none_match: Some("etag123".to_string()),
        if_modified_since: Some("Wed, 21 Oct 2015 07:28:00 GMT".to_string()),
    });

    let result = worker.resolve_metadata().await;
    match result {
        Err(Error::NotModified) => {}
        other => panic!("Expected Err(Error::NotModified), got {:?}", other),
    }
}

#[tokio::test]
async fn test_http_worker_conditional_get_200() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/conditional"))
        .and(header("If-None-Match", "etag123"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(vec![0; 10])
                .insert_header("Content-Range", "bytes 0-9/10")
                .insert_header("ETag", "etag456")
                .insert_header("Last-Modified", "Wed, 21 Oct 2015 07:29:00 GMT"),
        )
        .mount(&server)
        .await;

    let worker = HttpWorker::new(HttpWorkerOptions {
        uri: format!("{}/conditional", server.uri()),
        local_addr: None,
        user_agent: None,
        connect_timeout: None,
        proxy: None,
        referer: None,
        retry_count: 1,
        http_retry_delay_secs: 1,
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
        if_none_match: Some("etag123".to_string()),
        if_modified_since: None,
    });

    let metadata = worker.resolve_metadata().await.expect("Should resolve");
    assert_eq!(metadata.etag, Some("etag456".to_string()));
    assert_eq!(
        metadata.last_modified,
        Some("Wed, 21 Oct 2015 07:29:00 GMT".to_string())
    );
}
