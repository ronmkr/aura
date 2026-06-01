use crate::{Error, Result, TaskId};
use std::path::{Path, PathBuf};

pub fn get_part_path(base_path: &Path) -> Result<PathBuf> {
    let mut part_path = crate::storage::sys::harden_path(base_path);
    let mut filename = part_path
        .file_name()
        .ok_or_else(|| Error::Task(TaskId(0), "Invalid filename".to_string()))?
        .to_os_string();
    filename.push(".part");
    part_path.set_file_name(filename);
    Ok(part_path)
}

pub async fn sync_parent_dir(path: &Path) {
    if let Some(parent) = path.parent() {
        let parent_clone = parent.to_path_buf();
        let _ = tokio::task::spawn_blocking(move || {
            if let Ok(dir) = std::fs::File::open(&parent_clone) {
                let _ = dir.sync_all();
            }
        })
        .await;
    }
}
