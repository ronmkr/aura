use super::{HttpWorker, HttpWorkerOptions};
use crate::Error;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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

    let worker = HttpWorker::new(HttpWorkerOptions {
        uri: format!("{}/landing", server.uri()),
        local_addr: None,
        user_agent: None,
        connect_timeout: None,
        proxy: None,
        referer: None,
        retry_count: 3,
        retry_delay_secs: 1,
        credential_provider: None,
        dns_resolver: None,
        hsts_cache: None,
    });

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

    let worker = HttpWorker::new(HttpWorkerOptions {
        uri: format!("{}/landing", server.uri()),
        local_addr: None,
        user_agent: None,
        connect_timeout: None,
        proxy: None,
        referer: None,
        retry_count: 3,
        retry_delay_secs: 1,
        credential_provider: None,
        dns_resolver: None,
        hsts_cache: None,
    });

    let result = worker.resolve_metadata().await;
    assert!(result.is_err());
    match result {
        Err(Error::Protocol(msg)) => assert!(msg.contains("Direct link resolution failed")),
        _ => panic!("Expected Protocol error for HTML landing page failure"),
    }
}

#[tokio::test]
async fn test_http_worker_captive_portal_detection() {
    let server = MockServer::start().await;

    // A mock captive portal landing page response containing signature keywords
    Mock::given(method("GET"))
        .and(path("/wifi-login/download.zip"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/html")
                .set_body_bytes(
                    "<html><body>Please log in to our coffee shop WiFi portal.</body></html>"
                        .as_bytes()
                        .to_vec(),
                ),
        )
        .mount(&server)
        .await;

    // Try downloading an asset (e.g. .zip)
    let worker = HttpWorker::new(HttpWorkerOptions {
        uri: format!("{}/wifi-login/download.zip", server.uri()),
        local_addr: None,
        user_agent: None,
        connect_timeout: None,
        proxy: None,
        referer: None,
        retry_count: 3,
        retry_delay_secs: 1,
        credential_provider: None,
        dns_resolver: None,
        hsts_cache: None,
    });

    let result = worker.resolve_metadata().await;
    assert!(
        result.is_err(),
        "Expected error, but got success: {:?}",
        result
    );
    match result {
        Err(Error::CaptivePortal(msg)) => {
            assert!(msg.contains("Captive portal landing page detected"));
        }
        _ => panic!("Expected Error::CaptivePortal, got {:?}", result),
    }
}
