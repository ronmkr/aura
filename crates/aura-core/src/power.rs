//! power: Cross-platform power management and system sleep prevention.

use nosleep::{NoSleep, NoSleepType};
use tracing::{info, warn};

/// Manages system power assertions to prevent sleep during active downloads.
pub struct PowerManager {
    no_sleep: Option<NoSleep>,
    assertion: Option<Result<(), nosleep::Error>>,
    is_active: bool,
}

impl PowerManager {
    /// Creates a new PowerManager.
    pub fn new() -> Self {
        let no_sleep = match NoSleep::new() {
            Ok(ns) => Some(ns),
            Err(e) => {
                warn!(
                    "Failed to initialize PowerManager: {}. Sleep prevention will be disabled.",
                    e
                );
                None
            }
        };

        Self {
            no_sleep,
            assertion: None,
            is_active: false,
        }
    }

    /// Updates the sleep prevention state based on whether downloads are active.
    pub fn set_active(&mut self, active: bool) {
        if active && !self.is_active {
            self.require_awake();
        } else if !active && self.is_active {
            self.allow_sleep();
        }
        self.is_active = active;
    }

    fn require_awake(&mut self) {
        if let Some(ref mut no_sleep) = self.no_sleep {
            if self.assertion.is_none() {
                info!("Preventing system sleep due to active downloads");
                let res = no_sleep.start(NoSleepType::PreventUserIdleSystemSleep);
                if let Err(ref e) = res {
                    warn!("Failed to start sleep prevention: {}", e);
                }
                self.assertion = Some(res);
            }
        }
    }

    fn allow_sleep(&mut self) {
        if let Some(ref mut no_sleep) = self.no_sleep {
            if self.assertion.is_some() {
                info!("Releasing system sleep prevention");
                if let Err(e) = no_sleep.stop() {
                    warn!("Failed to stop sleep prevention: {}", e);
                }
                self.assertion = None;
            }
        }
    }
}

impl Default for PowerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_manager_state() {
        let mut manager = PowerManager::new();
        assert!(!manager.is_active);
        assert!(manager.assertion.is_none());

        manager.set_active(true);
        assert!(manager.is_active);

        // If supported, assertion should be Some
        if manager.no_sleep.is_some() {
            assert!(manager.assertion.is_some());
        }

        manager.set_active(true); // Redundant call
        assert!(manager.is_active);

        manager.set_active(false);
        assert!(!manager.is_active);
        assert!(manager.assertion.is_none());
    }
}
