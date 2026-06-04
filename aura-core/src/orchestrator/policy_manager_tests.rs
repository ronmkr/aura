use super::*;

#[test]
fn test_error_classification() {
    let pm = PolicyManager::new();

    // Engine Errors
    assert_eq!(
        pm.classify("Captive portal detected"),
        ErrorSeverity::Engine
    );
    assert_eq!(
        pm.classify("Disk Full: 0 bytes free"),
        ErrorSeverity::Engine
    );
    assert_eq!(
        pm.classify("Permission denied writing file"),
        ErrorSeverity::Engine
    );
    assert_eq!(
        pm.classify("VPN tunnel closed unexpectedly"),
        ErrorSeverity::Engine
    );

    // Task Errors
    assert_eq!(
        pm.classify("HTTP response status: 404 Not Found"),
        ErrorSeverity::Task
    );
    assert_eq!(
        pm.classify("HTTP response status: 403 Forbidden"),
        ErrorSeverity::Task
    );
    assert_eq!(
        pm.classify("Checksum mismatch: expected abc got 123"),
        ErrorSeverity::Task
    );

    // Worker Errors
    assert_eq!(pm.classify("Connection timed out"), ErrorSeverity::Worker);
    assert_eq!(
        pm.classify("Connection reset by peer"),
        ErrorSeverity::Worker
    );
    assert_eq!(pm.classify("Generic DNS failure"), ErrorSeverity::Worker);
}

#[test]
fn test_retry_delay_calculation() {
    let pm = PolicyManager::new();

    assert_eq!(pm.get_retry_delay(1, 2), std::time::Duration::from_secs(2));
    assert_eq!(pm.get_retry_delay(3, 5), std::time::Duration::from_secs(15));
}
