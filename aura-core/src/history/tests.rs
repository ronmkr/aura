use super::*;
use tempfile::tempdir;

static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn test_history_append_and_read() {
    let _lock = TEST_MUTEX.lock().unwrap();

    let tmp_dir = tempdir().unwrap();
    let history_file = tmp_dir.path().join("history.jsonl");

    // Set the path override
    HISTORY_PATH_OVERRIDE.with(|p| *p.borrow_mut() = Some(history_file));

    let config = crate::Config::default();

    // Purge first
    HistoryManager::purge_history(&config);

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

    HistoryManager::append_record(&config, rec1.clone());

    let records = HistoryManager::read_records(&config);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id, "123");
    assert_eq!(records[0].name, "test1");

    // Cleanup
    HistoryManager::purge_history(&config);

    // Clear override
    HISTORY_PATH_OVERRIDE.with(|p| *p.borrow_mut() = None);
}
