use super::*;

#[test]
fn test_resource_governor_limit_enforcement() {
    let governor = Arc::new(ResourceGovernor::new(100));
    let tenant = Some(TenantId("tenant1".to_string()));

    assert!(governor.request_allocation(&tenant, 60));
    assert_eq!(governor.current_usage(), 60);
    assert_eq!(governor.tenant_usage(tenant.as_ref().unwrap()), 60);

    assert!(!governor.request_allocation(&tenant, 50));
    assert_eq!(governor.current_usage(), 60);

    governor.release_allocation(&tenant, 30);
    assert_eq!(governor.current_usage(), 30);
    assert_eq!(governor.tenant_usage(tenant.as_ref().unwrap()), 30);

    assert!(governor.request_allocation(&tenant, 50));
    assert_eq!(governor.current_usage(), 80);
}

#[test]
fn test_resource_governor_unlimited() {
    let governor = Arc::new(ResourceGovernor::new(0));
    let tenant = Some(TenantId("tenant1".to_string()));

    assert!(governor.request_allocation(&tenant, 1000));
    assert_eq!(governor.current_usage(), 0);
}

#[test]
fn test_memory_guard_raii() {
    let governor = Arc::new(ResourceGovernor::new(100));
    let tenant = Some(TenantId("tenant1".to_string()));

    {
        assert!(governor.request_allocation(&tenant, 40));
        let _guard = MemoryGuard::new(governor.clone(), tenant.clone(), 40);
        assert_eq!(governor.current_usage(), 40);
    }

    assert_eq!(governor.current_usage(), 0);
}

#[test]
fn test_tenant_isolation() {
    let governor = Arc::new(ResourceGovernor::new(100));
    let t1 = Some(TenantId("t1".to_string()));
    let t2 = Some(TenantId("t2".to_string()));

    assert!(governor.request_allocation(&t1, 40));
    assert!(governor.request_allocation(&t2, 30));

    assert_eq!(governor.tenant_usage(t1.as_ref().unwrap()), 40);
    assert_eq!(governor.tenant_usage(t2.as_ref().unwrap()), 30);
    assert_eq!(governor.current_usage(), 70);

    governor.release_allocation(&t1, 20);
    assert_eq!(governor.tenant_usage(t1.as_ref().unwrap()), 20);
    assert_eq!(governor.tenant_usage(t2.as_ref().unwrap()), 30);
    assert_eq!(governor.current_usage(), 50);
}
