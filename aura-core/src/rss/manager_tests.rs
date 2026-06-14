use super::*;

#[test]
fn test_matches_filters() {
    // Empty filters match everything
    assert!(RssManager::matches_filters(
        "Aura v1.0.0 Stable Release",
        None,
        None,
        &None,
        &None,
        None
    ));
    assert!(RssManager::matches_filters(
        "Aura v1.0.0 Stable Release",
        None,
        None,
        &Some(vec![]),
        &None,
        None
    ));

    // Matching title filters
    let filters = Some(vec!["Stable".to_string(), r"v\d+\.\d+\.\d+".to_string()]);
    assert!(RssManager::matches_filters(
        "Aura v1.0.0 Stable Release",
        None,
        None,
        &filters,
        &None,
        None
    ));
    assert!(!RssManager::matches_filters(
        "Aura Patch Release",
        None,
        None,
        &filters,
        &None,
        None
    ));
    assert!(!RssManager::matches_filters(
        "Aura Beta Release",
        None,
        None,
        &filters,
        &None,
        None
    ));

    // Regex specific match
    let regex_filters = Some(vec![r"^Aura.*Stable$".to_string()]);
    assert!(RssManager::matches_filters(
        "Aura Stable",
        None,
        None,
        &regex_filters,
        &None,
        None
    ));
    assert!(!RssManager::matches_filters(
        "Aura Stable Release",
        None,
        None,
        &regex_filters,
        &None,
        None
    ));

    // Category filtering
    let categories = Some(vec!["anime".to_string(), "movie".to_string()]);
    assert!(RssManager::matches_filters(
        "Aura v1.0.0",
        Some("anime"),
        None,
        &None,
        &categories,
        None
    ));
    assert!(!RssManager::matches_filters(
        "Aura v1.0.0",
        Some("software"),
        None,
        &None,
        &categories,
        None
    ));
    assert!(!RssManager::matches_filters(
        "Aura v1.0.0",
        None,
        None,
        &None,
        &categories,
        None
    ));

    // Max size filtering
    assert!(RssManager::matches_filters(
        "Aura v1.0.0",
        None,
        Some(1000),
        &None,
        &None,
        Some(2000)
    ));
    assert!(!RssManager::matches_filters(
        "Aura v1.0.0",
        None,
        Some(3000),
        &None,
        &None,
        Some(2000)
    ));
}

#[test]
fn test_manager_subscriptions() {
    let temp_dir = tempfile::tempdir().unwrap();
    let feeds_path = temp_dir.path().join("feeds.toml");
    let history_path = temp_dir.path().join("feed_history.txt");

    let manager = RssManager {
        feeds_path: feeds_path.clone(),
        history_path: history_path.clone(),
    };

    // Load empty subscriptions
    let subs = manager.load_subscriptions().unwrap();
    assert!(subs.is_empty());

    // Add subscription
    let sub = FeedSubscription {
        url: "https://example.com/rss".to_string(),
        name: "Example Feed".to_string(),
        poll_interval: Some(15),
        filters: Some(vec!["Aura".to_string()]),
        categories: None,
        max_size: None,
    };
    manager.add_subscription(sub.clone()).unwrap();

    // Verify duplicate error
    assert!(manager.add_subscription(sub).is_err());

    // Load and check
    let subs = manager.load_subscriptions().unwrap();
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].name, "Example Feed");
    assert_eq!(subs[0].url, "https://example.com/rss");
    assert_eq!(subs[0].poll_interval, Some(15));

    // Test mark and check ingested
    assert!(!manager.is_ingested("guid-123"));
    manager.mark_ingested("guid-123").unwrap();
    assert!(manager.is_ingested("guid-123"));

    // Remove subscription
    manager.remove_subscription("Example Feed").unwrap();
    let subs = manager.load_subscriptions().unwrap();
    assert!(subs.is_empty());
}
