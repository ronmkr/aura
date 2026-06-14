//! File descriptor limit adjustment (Decision-0064).

/// Helper to calculate the adjusted max_connections_per_task based on actual soft limit (Decision-0064).
pub(crate) fn calculate_adjusted_connections(
    actual_soft: usize,
    max_concurrent: usize,
    max_connections: usize,
) -> Option<usize> {
    let required_fds = (max_concurrent * max_connections * 2) + 512;
    if actual_soft < required_fds {
        let available_for_tasks = actual_soft.saturating_sub(512);
        let calculated_connections = available_for_tasks / (max_concurrent * 2);
        let new_max_connections = calculated_connections.max(2);
        if new_max_connections < max_connections {
            return Some(new_max_connections);
        }
    }
    None
}

/// Dynamic file descriptor limit adjustment based on configured download/connection concurrency (Decision-0064).
#[cfg(unix)]
pub(crate) fn adjust_file_descriptor_limit(config: &mut aura_core::Config) {
    let max_concurrent = config.bandwidth.max_concurrent_downloads;
    let max_connections = config.bandwidth.max_connections_per_task;
    let required_fds = (max_concurrent * max_connections * 2) + 512;

    // Get current limits
    let current_limits = match rlimit::getrlimit(rlimit::Resource::NOFILE) {
        Ok((soft, hard)) => (soft as usize, hard as usize),
        Err(e) => {
            tracing::warn!("Failed to query current file descriptor limits: {}", e);
            return;
        }
    };

    let (current_soft, current_hard) = current_limits;

    if current_soft >= required_fds {
        return;
    }

    // Try to raise the soft limit to what we need (up to the hard limit)
    let target_soft = required_fds.min(current_hard);
    if let Err(e) = rlimit::setrlimit(
        rlimit::Resource::NOFILE,
        target_soft as u64,
        current_hard as u64,
    ) {
        tracing::warn!(
            "Failed to set file descriptor limit to {}: {}",
            target_soft,
            e
        );
    }

    // Query again to check actual soft limit
    let actual_limits = match rlimit::getrlimit(rlimit::Resource::NOFILE) {
        Ok((soft, hard)) => (soft as usize, hard as usize),
        Err(_) => current_limits,
    };

    let (actual_soft, _) = actual_limits;

    if let Some(new_max_connections) =
        calculate_adjusted_connections(actual_soft, max_concurrent, max_connections)
    {
        tracing::warn!(
            "WARN: fd limit ({}) is insufficient for configured concurrency ({} required).\n\
             Suggestion: ulimit -n 4096, or add LimitNOFILE=4096 to your systemd unit.\n\
             Auto-reducing max_connections_per_task from {} to {} to fit within available fds.",
            actual_soft,
            required_fds,
            max_connections,
            new_max_connections
        );
        config.bandwidth.max_connections_per_task = new_max_connections;
    }
}

/// Dynamic file descriptor limit adjustment is a no-op on non-Unix platforms (Decision-0064).
#[cfg(not(unix))]
pub(crate) fn adjust_file_descriptor_limit(_config: &mut aura_core::Config) {
    // Windows or other non-Unix targets don't use RLIMIT_NOFILE
}
