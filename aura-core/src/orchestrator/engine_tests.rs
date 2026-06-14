use super::*;
use std::fs;

#[tokio::test]
async fn test_ingest_watch_file_non_existent() {
    let mut config = crate::AuraConfig::default();
    let temp_dir = tempfile::tempdir().unwrap();
    config.storage.download_dir = temp_dir.path().to_string_lossy().into_owned();
    let (engine, _orch, _store) = Engine::new(config).await.unwrap();

    let path = std::path::Path::new("non_existent_file.torrent");
    let result = crate::orchestrator::watch::ingest_watch_file(&engine, path).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("metadata"));
}

#[tokio::test]
async fn test_ingest_watch_file_empty() {
    let mut config = crate::AuraConfig::default();
    let temp_dir = tempfile::tempdir().unwrap();
    config.storage.download_dir = temp_dir.path().to_string_lossy().into_owned();
    let (engine, _orch, _store) = Engine::new(config).await.unwrap();

    let temp_dir_files = tempfile::tempdir().unwrap();
    let path = temp_dir_files.path().join("empty.torrent");
    fs::write(&path, "").unwrap();

    let result = crate::orchestrator::watch::ingest_watch_file(&engine, &path).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("empty"));
}
