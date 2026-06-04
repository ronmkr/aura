#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    Worker,
    Task,
    Engine,
}

#[derive(Debug, Clone)]
pub struct PolicyManager;

impl Default for PolicyManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PolicyManager {
    pub fn new() -> Self {
        Self
    }

    /// Categorizes failures into Worker, Task, and Engine error scopes.
    pub fn classify(&self, err: &str) -> ErrorSeverity {
        let err_lower = err.to_lowercase();
        if err_lower.contains("captive portal")
            || err_lower.contains("disk full")
            || err_lower.contains("no space left on device")
            || err_lower.contains("storage error")
            || err_lower.contains("permission denied")
            || err_lower.contains("vpn")
        {
            ErrorSeverity::Engine
        } else if err_lower.contains("404")
            || err_lower.contains("403")
            || err_lower.contains("401")
            || err_lower.contains("checksum mismatch")
            || err_lower.contains("integrity verification failed")
        {
            ErrorSeverity::Task
        } else {
            ErrorSeverity::Worker
        }
    }

    /// Computes the backoff delay for retrying degraded mirrors.
    pub fn get_retry_delay(&self, retry_count: u32, delay_base: u64) -> std::time::Duration {
        std::time::Duration::from_secs(retry_count as u64 * delay_base)
    }
}

#[cfg(test)]
#[path = "policy_manager_tests.rs"]
mod tests;
