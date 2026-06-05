use super::*;

#[test]
fn test_resource_governor_limit_enforcement() {
    let governor = Arc::new(ResourceGovernor::new(100, 0));
    let tenant = Some(TenantId("tenant1".to_string()));

    assert!(governor.request_allocation(&tenant, 60, false));
    assert_eq!(governor.current_usage(), 60);
    assert_eq!(governor.tenant_usage(tenant.as_ref().unwrap()), 60);

    assert!(!governor.request_allocation(&tenant, 50, false));
    assert_eq!(governor.current_usage(), 60);

    governor.release_allocation(&tenant, 30);
    assert_eq!(governor.current_usage(), 30);
    assert_eq!(governor.tenant_usage(tenant.as_ref().unwrap()), 30);

    assert!(governor.request_allocation(&tenant, 50, false));
    assert_eq!(governor.current_usage(), 80);
}

#[test]
fn test_resource_governor_unlimited() {
    let governor = Arc::new(ResourceGovernor::new(0, 0));
    let tenant = Some(TenantId("tenant1".to_string()));

    assert!(governor.request_allocation(&tenant, 1000, false));
    assert_eq!(governor.current_usage(), 0);
}

#[test]
fn test_memory_guard_raii() {
    let governor = Arc::new(ResourceGovernor::new(100, 0));
    let tenant = Some(TenantId("tenant1".to_string()));

    {
        assert!(governor.request_allocation(&tenant, 40, false));
        let _guard = MemoryGuard::new(governor.clone(), tenant.clone(), 40);
        assert_eq!(governor.current_usage(), 40);
    }

    assert_eq!(governor.current_usage(), 0);
}

#[test]
fn test_tenant_isolation() {
    let governor = Arc::new(ResourceGovernor::new(100, 0));
    let t1 = Some(TenantId("t1".to_string()));
    let t2 = Some(TenantId("t2".to_string()));

    assert!(governor.request_allocation(&t1, 40, false));
    assert!(governor.request_allocation(&t2, 30, false));

    assert_eq!(governor.tenant_usage(t1.as_ref().unwrap()), 40);
    assert_eq!(governor.tenant_usage(t2.as_ref().unwrap()), 30);
    assert_eq!(governor.current_usage(), 70);

    governor.release_allocation(&t1, 20);
    assert_eq!(governor.tenant_usage(t1.as_ref().unwrap()), 20);
    assert_eq!(governor.tenant_usage(t2.as_ref().unwrap()), 30);
    assert_eq!(governor.current_usage(), 50);
}

#[test]
fn test_resource_governor_safety_margin() {
    let governor = Arc::new(ResourceGovernor::new(100, 20));
    let tenant = Some(TenantId("tenant1".to_string()));

    // Data request of 70 should succeed (70 <= 100 - 20)
    assert!(governor.request_allocation(&tenant, 70, false));
    assert_eq!(governor.current_usage(), 70);

    // Data request of 15 should fail (70 + 15 = 85 > 80)
    assert!(!governor.request_allocation(&tenant, 15, false));

    // Metadata request of 15 should succeed (70 + 15 = 85 <= 100)
    assert!(governor.request_allocation(&tenant, 15, true));
    assert_eq!(governor.current_usage(), 85);
}

#[test]
fn test_resource_governor_fair_share() {
    let governor = Arc::new(ResourceGovernor::new(100, 0));
    let t1 = Some(TenantId("t1".to_string()));
    let t2 = Some(TenantId("t2".to_string()));

    // T1 allocates 40 (only active tenant, so fair_share is 100)
    assert!(governor.request_allocation(&t1, 40, false));

    // T2 allocates 30 (T2 is now active, so N=2, fair_share is 50. T2 usage is 30 <= 50, so allowed)
    assert!(governor.request_allocation(&t2, 30, false));

    // T1 requests 20 more (T1 total would be 60. But T2 is active, so T1 limit is 50. 60 > 50, so rejected)
    assert!(!governor.request_allocation(&t1, 20, false));

    // Metadata requests are exempt from fair share
    assert!(governor.request_allocation(&t1, 20, true));
}
