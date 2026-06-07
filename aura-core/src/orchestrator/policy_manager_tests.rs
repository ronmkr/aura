use crate::orchestrator::policy_manager::{ErrorSeverity, PolicyManager};

#[test]
fn test_error_classification() {
    let pm = PolicyManager::new();

    // Engine level errors
    assert_eq!(
        pm.classify("Storage error: Disk full"),
        ErrorSeverity::Engine
    );
    assert_eq!(pm.classify("VPN tunnel dropped"), ErrorSeverity::Engine);

    // Task level errors
    assert_eq!(
        pm.classify("Protocol error: status 404"),
        ErrorSeverity::Task
    );
    assert_eq!(
        pm.classify("Integrity verification failed"),
        ErrorSeverity::Task
    );

    // Worker level errors
    assert_eq!(pm.classify("Connection timed out"), ErrorSeverity::Worker);
}

#[test]
fn test_retry_delay_calculation() {
    let pm = PolicyManager::new();

    // Linear backoff in current implementation: retry_count * delay_base
    assert_eq!(pm.get_retry_delay(1, 2), std::time::Duration::from_secs(2));
    assert_eq!(pm.get_retry_delay(2, 2), std::time::Duration::from_secs(4));
    assert_eq!(pm.get_retry_delay(3, 2), std::time::Duration::from_secs(6));
    assert_eq!(pm.get_retry_delay(5, 2), std::time::Duration::from_secs(10));
}
