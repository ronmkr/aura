//! power: Cross-platform power management and system sleep prevention.

use nosleep::{NoSleep, NoSleepType};
use std::sync::mpsc;
use std::thread;
use tracing::{info, warn};

/// Manages system power assertions to prevent sleep during active downloads.
///
/// This struct is thread-safe (Send + Sync) because it isolates the OS-specific
/// non-thread-safe components (like D-Bus connections on Linux) into a
/// dedicated OS thread.
pub struct PowerManager {
    tx: mpsc::Sender<bool>,
}

impl PowerManager {
    /// Creates a new PowerManager and spawns its management thread.
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let mut no_sleep = match NoSleep::new() {
                Ok(ns) => Some(ns),
                Err(e) => {
                    warn!(
                        "Failed to initialize PowerManager: {}. Sleep prevention will be disabled.",
                        e
                    );
                    None
                }
            };

            let mut assertion: Option<Result<(), nosleep::Error>> = None;
            let mut is_active = false;

            while let Ok(active) = rx.recv() {
                if active && !is_active {
                    if let Some(ref mut ns) = no_sleep {
                        info!("Preventing system sleep due to active downloads");
                        let res = ns.start(NoSleepType::PreventUserIdleSystemSleep);
                        if let Err(ref e) = res {
                            warn!("Failed to start sleep prevention: {}", e);
                        }
                        assertion = Some(res);
                    }
                } else if !active && is_active {
                    if let Some(ref mut ns) = no_sleep {
                        if assertion.is_some() {
                            info!("Releasing system sleep prevention");
                            if let Err(e) = ns.stop() {
                                warn!("Failed to stop sleep prevention: {}", e);
                            }
                            assertion = None;
                        }
                    }
                }
                is_active = active;
            }

            // Cleanup on thread exit (when PowerManager is dropped)
            if is_active {
                if let Some(ref mut ns) = no_sleep {
                    let _ = ns.stop();
                }
            }
        });

        Self { tx }
    }

    /// Updates the sleep prevention state based on whether downloads are active.
    pub fn set_active(&mut self, active: bool) {
        let _ = self.tx.send(active);
    }
}

impl Default for PowerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod tests;
