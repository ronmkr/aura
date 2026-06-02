use super::*;
use std::time::Duration;

#[test]
fn test_power_manager_state() {
    let mut manager = PowerManager::new();

    // We can't easily inspect the internal state of the thread,
    // but we can verify that sending commands doesn't panic.
    manager.set_active(true);
    thread::sleep(Duration::from_millis(50));

    manager.set_active(false);
    thread::sleep(Duration::from_millis(50));

    manager.set_active(true);
    // Dropping manager should stop the thread and release assertions
}
