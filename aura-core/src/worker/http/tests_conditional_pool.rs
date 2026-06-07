use super::*;

#[test]
fn test_client_pool_deduplication() {
    let pool = ClientPool::new();

    let key1 = ClientKey {
        host: "example.com".to_string(),
        port: Some(8080),
        interface: None,
    };
    let key2 = ClientKey {
        host: "example.com".to_string(),
        port: Some(8080),
        interface: None,
    };
    let key3 = ClientKey {
        host: "example.com".to_string(),
        port: Some(8081),
        interface: None,
    };

    let client1 = pool.get_or_create(key1.clone(), reqwest::Client::new);
    let client2 = pool.get_or_create(key2, reqwest::Client::new);
    let client3 = pool.get_or_create(key3, reqwest::Client::new);

    assert!(Arc::ptr_eq(&client1, &client2));
    assert!(!Arc::ptr_eq(&client1, &client3));
    assert_eq!(pool.len(), 2);
}

#[test]
fn test_client_pool_caching() {
    let pool = ClientPool::new();

    let key = ClientKey {
        host: "example.com".to_string(),
        port: Some(8082),
        interface: None,
    };

    let client1 = pool.get_or_create(key.clone(), reqwest::Client::new);
    let client2 = pool.get_or_create(key, reqwest::Client::new);

    assert!(Arc::ptr_eq(&client1, &client2));
    assert_eq!(pool.len(), 1);
}

#[tokio::test]
async fn test_http_worker_with_client_pool() {
    let pool = ClientPool::new();
    let options = HttpWorkerOptions {
        uri: "http://example.com:8083".to_string(),
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
        client_pool: Some(pool.clone()),
        if_none_match: None,
        if_modified_since: None,
    };

    let worker1 = HttpWorker::new(options.clone());
    let worker2 = HttpWorker::new(options);

    assert!(Arc::ptr_eq(&worker1.client, &worker2.client));
    assert_eq!(pool.len(), 1);
}
