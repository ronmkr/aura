use super::*;
use chrono::{TimeZone, Utc};

#[test]
fn test_empty_schedule() {
    let config = BandwidthConfig {
        global_download_limit: 100,
        global_upload_limit: 50,
        schedule: Vec::new(),
        ..Default::default()
    };
    let (dl, ul, active) = BandwidthScheduler::effective_limits(&config, Utc::now());
    assert_eq!(dl, 100);
    assert_eq!(ul, 50);
    assert!(active.is_none());
}

#[test]
fn test_matching_schedule() {
    let entry = BandwidthSchedule {
        from: "09:00".to_string(),
        to: "17:00".to_string(),
        download_limit: 1000,
        upload_limit: 500,
        days: vec!["Mon".to_string(), "Tue".to_string()],
        timezone: Some("UTC".to_string()),
    };
    let config = BandwidthConfig {
        global_download_limit: 100,
        global_upload_limit: 50,
        schedule: vec![entry],
        ..Default::default()
    };

    // Monday 10:00 UTC -> Matches
    let test_time = Utc.with_ymd_and_hms(2026, 6, 1, 10, 0, 0).unwrap();
    let (dl, ul, active) = BandwidthScheduler::effective_limits(&config, test_time);
    assert_eq!(dl, 1000);
    assert_eq!(ul, 500);
    assert!(active.is_some());

    // Monday 18:00 UTC -> Outside time range
    let test_time = Utc.with_ymd_and_hms(2026, 6, 1, 18, 0, 0).unwrap();
    let (dl, _, active) = BandwidthScheduler::effective_limits(&config, test_time);
    assert_eq!(dl, 100);
    assert!(active.is_none());

    // Wednesday 10:00 UTC -> Outside day range
    let test_time = Utc.with_ymd_and_hms(2026, 6, 3, 10, 0, 0).unwrap();
    let (dl, _, active) = BandwidthScheduler::effective_limits(&config, test_time);
    assert_eq!(dl, 100);
    assert!(active.is_none());
}

#[test]
fn test_midnight_spanning_window() {
    let entry = BandwidthSchedule {
        from: "22:00".to_string(),
        to: "06:00".to_string(),
        download_limit: 5000,
        upload_limit: 2500,
        days: Vec::new(),
        timezone: Some("UTC".to_string()),
    };
    let config = BandwidthConfig {
        global_download_limit: 100,
        global_upload_limit: 50,
        schedule: vec![entry],
        ..Default::default()
    };

    // 23:00 UTC -> Matches
    let test_time = Utc.with_ymd_and_hms(2026, 6, 1, 23, 0, 0).unwrap();
    let (dl, _, active) = BandwidthScheduler::effective_limits(&config, test_time);
    assert_eq!(dl, 5000);
    assert!(active.is_some());

    // 02:00 UTC -> Matches
    let test_time = Utc.with_ymd_and_hms(2026, 6, 1, 2, 0, 0).unwrap();
    let (dl, _, active) = BandwidthScheduler::effective_limits(&config, test_time);
    assert_eq!(dl, 5000);
    assert!(active.is_some());

    // 12:00 UTC -> Outside
    let test_time = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
    let (dl, _, active) = BandwidthScheduler::effective_limits(&config, test_time);
    assert_eq!(dl, 100);
    assert!(active.is_none());
}

#[test]
fn test_overlapping_windows_last_wins() {
    let entry1 = BandwidthSchedule {
        from: "09:00".to_string(),
        to: "17:00".to_string(),
        download_limit: 1000,
        upload_limit: 500,
        days: Vec::new(),
        timezone: Some("UTC".to_string()),
    };
    let entry2 = BandwidthSchedule {
        from: "12:00".to_string(),
        to: "14:00".to_string(),
        download_limit: 2000,
        upload_limit: 1000,
        days: Vec::new(),
        timezone: Some("UTC".to_string()),
    };
    let config = BandwidthConfig {
        global_download_limit: 100,
        global_upload_limit: 50,
        schedule: vec![entry1, entry2],
        ..Default::default()
    };

    // 13:00 UTC -> Matches both, entry2 wins because it is listed last
    let test_time = Utc.with_ymd_and_hms(2026, 6, 1, 13, 0, 0).unwrap();
    let (dl, _, active) = BandwidthScheduler::effective_limits(&config, test_time);
    assert_eq!(dl, 2000);
    assert_eq!(active.unwrap().download_limit, 2000);
}

#[test]
fn test_specificity_priority() {
    let entry1 = BandwidthSchedule {
        from: "00:00".to_string(),
        to: "23:59".to_string(),
        download_limit: 1000,
        upload_limit: 500,
        days: Vec::new(), // General (specificity = 0)
        timezone: Some("UTC".to_string()),
    };
    let entry2 = BandwidthSchedule {
        from: "00:00".to_string(),
        to: "23:59".to_string(),
        download_limit: 2000,
        upload_limit: 1000,
        days: vec!["Mon".to_string(), "Tue".to_string()], // Specific (specificity = 6)
        timezone: Some("UTC".to_string()),
    };
    let config = BandwidthConfig {
        global_download_limit: 100,
        global_upload_limit: 50,
        schedule: vec![entry1, entry2],
        ..Default::default()
    };

    // Monday -> Matches both, entry2 wins because it's more specific (days filter is populated)
    let test_time = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
    let (dl, _, active) = BandwidthScheduler::effective_limits(&config, test_time);
    assert_eq!(dl, 2000);
    assert_eq!(active.unwrap().download_limit, 2000);
}

#[test]
fn test_next_transition() {
    let entry = BandwidthSchedule {
        from: "09:00".to_string(),
        to: "17:00".to_string(),
        download_limit: 1000,
        upload_limit: 500,
        days: Vec::new(),
        timezone: Some("UTC".to_string()),
    };
    let config = BandwidthConfig {
        schedule: vec![entry],
        ..Default::default()
    };

    // Monday 08:00 UTC -> Next transition is 09:00 UTC today
    let test_time = Utc.with_ymd_and_hms(2026, 6, 1, 8, 0, 0).unwrap();
    let next = BandwidthScheduler::next_transition(&config, test_time);
    assert_eq!(
        next.unwrap(),
        Utc.with_ymd_and_hms(2026, 6, 1, 9, 0, 0).unwrap()
    );

    // Monday 10:00 UTC -> Next transition is 17:00 UTC today
    let test_time = Utc.with_ymd_and_hms(2026, 6, 1, 10, 0, 0).unwrap();
    let next = BandwidthScheduler::next_transition(&config, test_time);
    assert_eq!(
        next.unwrap(),
        Utc.with_ymd_and_hms(2026, 6, 1, 17, 0, 0).unwrap()
    );

    // Monday 18:00 UTC -> Next transition is 09:00 UTC tomorrow
    let test_time = Utc.with_ymd_and_hms(2026, 6, 1, 18, 0, 0).unwrap();
    let next = BandwidthScheduler::next_transition(&config, test_time);
    assert_eq!(
        next.unwrap(),
        Utc.with_ymd_and_hms(2026, 6, 2, 9, 0, 0).unwrap()
    );
}
