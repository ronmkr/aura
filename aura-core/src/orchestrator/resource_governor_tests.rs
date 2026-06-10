use crate::orchestrator::resource_governor::{MemoryGuard, ResourceGovernor};
use crate::TenantId;
use std::sync::Arc;

#[tokio::test]
async fn test_resource_governor_limit_enforcement() {
    let governor = Arc::new(ResourceGovernor::new(100, 0));
    let tenant = Some(TenantId("tenant1".to_string()));

    // 1. Acquire 60MB - OK
    assert!(governor.request_allocation(&tenant, 60, true));
    assert_eq!(governor.current_usage(), 60);

    // 2. Try to acquire another 50MB - Fails (total 110 > 100)
    assert!(!governor.request_allocation(&tenant, 50, true));

    // 3. Release 40MB
    governor.release_allocation(&tenant, 40);
    assert_eq!(governor.current_usage(), 20);

    // 4. Now 50MB should succeed
    assert!(governor.request_allocation(&tenant, 50, true));
    assert_eq!(governor.current_usage(), 70);
}

#[tokio::test]
async fn test_resource_governor_unlimited() {
    let governor = Arc::new(ResourceGovernor::new(0, 0));
    let tenant = Some(TenantId("tenant1".to_string()));

    // Should always return true
    assert!(governor.request_allocation(&tenant, 1_000_000, true));
    assert_eq!(governor.current_usage(), 0); // Usage tracking is skipped when limit is 0
}

#[tokio::test]
async fn test_memory_guard_raii() {
    let governor = Arc::new(ResourceGovernor::new(100, 0));
    let tenant = Some(TenantId("tenant1".to_string()));

    {
        assert!(governor.request_allocation(&tenant, 40, true));
        let _guard = MemoryGuard::new(governor.clone(), tenant.clone(), 40);
        assert_eq!(governor.current_usage(), 40);
    }

    // Guard dropped, memory should be released
    assert_eq!(governor.current_usage(), 0);
}

#[tokio::test]
async fn test_tenant_isolation() {
    let governor = Arc::new(ResourceGovernor::new(100, 0));
    let t1 = Some(TenantId("t1".to_string()));
    let t2 = Some(TenantId("t2".to_string()));

    governor.request_allocation(&t1, 50, true);
    governor.request_allocation(&t2, 30, true);

    assert_eq!(governor.tenant_usage(t1.as_ref().unwrap()), 50);
    assert_eq!(governor.tenant_usage(t2.as_ref().unwrap()), 30);
    assert_eq!(governor.current_usage(), 80);
}

#[tokio::test]
async fn test_resource_governor_safety_margin() {
    let governor = Arc::new(ResourceGovernor::new(100, 20));
    let tenant = Some(TenantId("tenant1".to_string()));

    // Effective limit for standard downloads is 80 (100 - 20)
    assert!(governor.request_allocation(&tenant, 70, false));
    assert!(!governor.request_allocation(&tenant, 20, false));

    // Metadata ignores safety margin
    assert!(governor.request_allocation(&tenant, 20, true));
}
