use crate::orchestrator::Engine;
use tracing::warn;

pub async fn ingest_watch_file(
    engine: &Engine,
    path: &std::path::Path,
) -> std::result::Result<(), String> {
    let file_name = path
        .file_name()
        .and_then(|f| f.to_str())
        .ok_or_else(|| "Invalid file name".to_string())?;

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let metadata =
        std::fs::metadata(path).map_err(|e| format!("Failed to read metadata: {}", e))?;
    if metadata.len() == 0 {
        return Err("File is empty".to_string());
    }

    let task_id = crate::TaskId::random();

    let ttype = match ext.as_str() {
        "torrent" => crate::task::TaskType::BitTorrent,
        _ => crate::task::TaskType::Http,
    };

    let abs_path =
        std::fs::canonicalize(path).map_err(|e| format!("Failed to canonicalize path: {}", e))?;
    let uri = abs_path.to_string_lossy().to_string();
    let sources = vec![(uri, ttype)];

    let args = crate::orchestrator::command::AddTaskArgs {
        id: task_id,
        tenant_id: None,
        name: file_name.to_string(),
        sources,
        checksum: None,
        priority: 3,
        streaming_mode: false,
        depends_on: Vec::new(),
        follow_on: None,
    };

    match engine.add_task_with_options(args).await {
        Ok(_) => {
            {
                let mut guard = engine.last_ingested_file.lock().await;
                *guard = Some(file_name.to_string());
            }
            Ok(())
        }
        Err(crate::Error::DuplicateTask(_)) => {
            warn!("Watch folder: task already exists for {:?}", file_name);
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}
