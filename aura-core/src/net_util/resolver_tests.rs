use super::*;

#[tokio::test]
async fn test_create_resolver_simple_system() {
    let config = ResolverConfig::Simple("system".to_string());
    let result = create_resolver(&config).await;
    assert!(
        result.is_ok(),
        "Failed to create system resolver: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn test_create_resolver_simple_cloudflare() {
    let config = ResolverConfig::Simple("cloudflare".to_string());
    let result = create_resolver(&config).await;
    assert!(
        result.is_ok(),
        "Failed to create cloudflare resolver: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn test_create_resolver_simple_google() {
    let config = ResolverConfig::Simple("google".to_string());
    let result = create_resolver(&config).await;
    assert!(
        result.is_ok(),
        "Failed to create google resolver: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn test_create_resolver_simple_custom_ip() {
    let config = ResolverConfig::Simple("127.0.0.1".to_string());
    let result = create_resolver(&config).await;
    assert!(
        result.is_ok(),
        "Failed to create custom IP resolver: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn test_create_resolver_doh_with_ips() {
    let config = ResolverConfig::Structured(StructuredResolverConfig::Doh {
        url: "https://cloudflare-dns.com/dns-query".to_string(),
        ips: Some(vec!["1.1.1.1".to_string()]),
    });
    let result = create_resolver(&config).await;
    assert!(
        result.is_ok(),
        "Failed to create DoH resolver with IPs: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn test_create_resolver_doh_bootstrap() {
    let config = ResolverConfig::Structured(StructuredResolverConfig::Doh {
        url: "https://localhost/dns-query".to_string(),
        ips: None,
    });
    let result = create_resolver(&config).await;
    assert!(
        result.is_ok(),
        "Failed to create DoH resolver with bootstrap: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn test_create_resolver_dot_with_ip() {
    let config = ResolverConfig::Structured(StructuredResolverConfig::Dot {
        server: "127.0.0.1".to_string(),
        port: Some(853),
        tls_name: "localhost".to_string(),
    });
    let result = create_resolver(&config).await;
    assert!(
        result.is_ok(),
        "Failed to create DoT resolver with IP: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn test_create_resolver_dot_bootstrap() {
    let config = ResolverConfig::Structured(StructuredResolverConfig::Dot {
        server: "localhost".to_string(),
        port: Some(853),
        tls_name: "localhost".to_string(),
    });
    let result = create_resolver(&config).await;
    assert!(
        result.is_ok(),
        "Failed to create DoT resolver with bootstrap: {:?}",
        result.err()
    );
}
