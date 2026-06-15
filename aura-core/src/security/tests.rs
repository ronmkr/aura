use super::*;

#[test]
fn test_parse_hsts_header() {
    assert_eq!(
        parse_hsts_header("max-age=31536000"),
        Some((31536000, false))
    );
    assert_eq!(
        parse_hsts_header("max-age=600; includeSubDomains"),
        Some((600, true))
    );
    assert_eq!(
        parse_hsts_header("includeSubDomains; max-age=1200"),
        Some((1200, true))
    );
    assert_eq!(parse_hsts_header("invalid-header"), None);
}

#[tokio::test]
async fn test_hsts_cache_upgrades() {
    let temp_dir = tempfile::tempdir().unwrap();
    let sandbox_path = temp_dir.path().to_str().unwrap().to_string();
    let mut config = crate::Config::default();
    config.storage.sandbox_root = Some(sandbox_path);
    let config_swap = Arc::new(arc_swap::ArcSwap::from_pointee(config));
    let cache = HstsCache::new(config_swap);
    let domain = "secure-example.com".to_string();

    // Initially no policy, so should not upgrade
    assert!(!cache.should_upgrade(&domain).await);

    // Add HSTS policy with 60s max-age
    cache.insert_policy(domain.clone(), 60, false).await;
    assert!(cache.should_upgrade(&domain).await);

    // Subdomain should not upgrade since include_subdomains is false
    assert!(!cache.should_upgrade("sub.secure-example.com").await);

    // Expiration check (simulated with 0 max-age)
    cache.insert_policy(domain.clone(), 0, false).await;
    // Small delay just to be completely sure expiry is not in exact same second
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(!cache.should_upgrade(&domain).await);
}

#[tokio::test]
async fn test_hsts_subdomain_matching() {
    let temp_dir = tempfile::tempdir().unwrap();
    let sandbox_path = temp_dir.path().to_str().unwrap().to_string();
    let mut config = crate::Config::default();
    config.storage.sandbox_root = Some(sandbox_path);
    let config_swap = Arc::new(arc_swap::ArcSwap::from_pointee(config));
    let cache = HstsCache::new(config_swap);
    let domain = "parent.com".to_string();

    cache.insert_policy(domain.clone(), 300, true).await;
    assert!(cache.should_upgrade(&domain).await);
    // Subdomains should also be upgraded
    assert!(cache.should_upgrade("sub.parent.com").await);
    assert!(cache.should_upgrade("deep.sub.parent.com").await);

    // Mismatched domain should not upgrade
    assert!(!cache.should_upgrade("otherparent.com").await);
}
