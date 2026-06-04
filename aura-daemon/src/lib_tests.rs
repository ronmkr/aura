use super::fd_limit::calculate_adjusted_connections;

#[test]
fn test_fd_limit_calculation_matches_config() {
    // Required fds = (max_concurrent * max_connections * 2) + 512
    // If soft limit is sufficient (e.g. 2048), no adjustment is returned
    let res = calculate_adjusted_connections(2048, 5, 128);
    assert_eq!(res, None);

    // If soft limit is insufficient (e.g. 1024), we should reduce connections
    // Required = (5 * 128 * 2) + 512 = 1280 + 512 = 1792.
    // 1024 is less than 1792.
    // Available = 1024 - 512 = 512.
    // Calc connections = 512 / (5 * 2) = 512 / 10 = 51.
    let res = calculate_adjusted_connections(1024, 5, 128);
    assert_eq!(res, Some(51));
}

#[test]
fn test_startup_warns_on_insufficient_hard_limit() {
    // If limit is extremely low (e.g. 256), connections are reduced to minimum of 2
    // Required = (5 * 128 * 2) + 512 = 1792.
    // 256 is less than 1792.
    // Available = 0 (since 256 <= 512).
    // Calc connections = 0. We clamp to .max(2) = 2.
    let res = calculate_adjusted_connections(256, 5, 128);
    assert_eq!(res, Some(2));
}
