use super::*;
use tempfile::tempdir;

static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn test_history_append_and_read() {
    let _lock = TEST_MUTEX.lock().unwrap();

    let orig_home = std::env::var_os("HOME");
    let orig_userprofile = std::env::var_os("USERPROFILE");

    let tmp_dir = tempdir().unwrap();
    std::env::set_var("HOME", tmp_dir.path());
    std::env::set_var("USERPROFILE", tmp_dir.path());

    // Purge first
    HistoryManager::purge_history();

    let rec1 = CompletedTaskRecord {
        id: "123".to_string(),
        name: "test1".to_string(),
        uris: vec!["http://example.com/1".to_string()],
        total_bytes: 100,
        downloaded_bytes: 100,
        uploaded_bytes: 10,
        duration_secs: 5,
        checksum_verified: Some(true),
        phase: "Complete".to_string(),
        error: None,
        completed_at: chrono::Utc::now(),
    };

    HistoryManager::append_record(rec1.clone());

    let records = HistoryManager::read_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id, "123");
    assert_eq!(records[0].name, "test1");

    // Cleanup
    HistoryManager::purge_history();

    // Restore
    if let Some(h) = orig_home {
        std::env::set_var("HOME", h);
    } else {
        std::env::remove_var("HOME");
    }
    if let Some(u) = orig_userprofile {
        std::env::set_var("USERPROFILE", u);
    } else {
        std::env::remove_var("USERPROFILE");
    }
}
