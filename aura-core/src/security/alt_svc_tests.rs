use super::*;

#[test]
fn test_parse_alt_svc_header_basic() {
    let parsed = parse_alt_svc_header("h3=\":443\"; ma=3600").unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].alt_protocol, "h3");
    assert_eq!(parsed[0].alt_host, None);
    assert_eq!(parsed[0].alt_port, 443);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Expiry should be roughly now + 3600
    assert!(parsed[0].expiry >= now + 3500);
}

#[test]
fn test_parse_alt_svc_header_multiple() {
    let parsed =
        parse_alt_svc_header("h3-29=\":8443\"; ma=600, h3=\"alt.example.com:443\"; ma=1200")
            .unwrap();
    assert_eq!(parsed.len(), 2);

    assert_eq!(parsed[0].alt_protocol, "h3-29");
    assert_eq!(parsed[0].alt_host, None);
    assert_eq!(parsed[0].alt_port, 8443);

    assert_eq!(parsed[1].alt_protocol, "h3");
    assert_eq!(parsed[1].alt_host, Some("alt.example.com".to_string()));
    assert_eq!(parsed[1].alt_port, 443);
}

#[test]
fn test_parse_alt_svc_header_clear() {
    let parsed = parse_alt_svc_header("clear").unwrap();
    assert_eq!(parsed.len(), 0);
}

#[test]
fn test_parse_alt_svc_header_invalid() {
    assert!(parse_alt_svc_header("invalid-data").is_none());
    assert!(parse_alt_svc_header("h3=invalid").is_none());
}

#[test]
fn test_rewrite_url_for_alt_svc() {
    let policy = AltSvcPolicy {
        host: "example.com".to_string(),
        alt_protocol: "h3".to_string(),
        alt_host: None,
        alt_port: 8443,
        expiry: 9999999999,
    };
    let rewritten =
        rewrite_url_for_alt_svc("https://example.com/download/file.bin", &policy).unwrap();
    assert_eq!(rewritten, "https://example.com:8443/download/file.bin");

    let policy_alt_host = AltSvcPolicy {
        host: "example.com".to_string(),
        alt_protocol: "h3".to_string(),
        alt_host: Some("alt.example.com".to_string()),
        alt_port: 443,
        expiry: 9999999999,
    };
    let rewritten_alt =
        rewrite_url_for_alt_svc("https://example.com/download/file.bin", &policy_alt_host).unwrap();
    assert_eq!(rewritten_alt, "https://alt.example.com/download/file.bin");
}

#[tokio::test]
async fn test_alt_svc_cache_workflow() {
    let cache = AltSvcCache::new();
    let domain = "api.example.com".to_string();

    // Initially no alt service
    assert!(cache.get_alt_svc(&domain).await.is_none());

    // Insert h3 policy
    cache
        .insert_policies(domain.clone(), "h3=\":443\"; ma=60")
        .await;
    let policy = cache.get_alt_svc(&domain).await.unwrap();
    assert_eq!(policy.alt_protocol, "h3");
    assert_eq!(policy.alt_port, 443);

    // Insert clear
    cache.insert_policies(domain.clone(), "clear").await;
    assert!(cache.get_alt_svc(&domain).await.is_none());
}
