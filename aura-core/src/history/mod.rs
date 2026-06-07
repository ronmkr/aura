use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedTaskRecord {
    pub id: String,
    pub name: String,
    pub uris: Vec<String>,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub uploaded_bytes: u64,
    pub duration_secs: u64,
    pub checksum_verified: Option<bool>,
    pub phase: String,
    pub error: Option<String>,
    pub completed_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
thread_local! {
    pub static HISTORY_PATH_OVERRIDE: std::cell::RefCell<Option<std::path::PathBuf>> = const { std::cell::RefCell::new(None) };
}

pub struct HistoryManager;

impl HistoryManager {
    pub fn get_history_path(config: &crate::Config) -> std::path::PathBuf {
        #[cfg(test)]
        if let Some(path) = HISTORY_PATH_OVERRIDE.with(|p| p.borrow().clone()) {
            return path;
        }

        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        home.join(&config.general.aura_dir_name)
            .join(&config.general.history_file_name)
    }

    pub fn get_old_history_path(config: &crate::Config) -> std::path::PathBuf {
        #[cfg(test)]
        if let Some(path) = HISTORY_PATH_OVERRIDE.with(|p| p.borrow().clone()) {
            return path.with_extension("old.jsonl");
        }

        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let old_name = format!("{}.old", config.general.history_file_name);
        home.join(&config.general.aura_dir_name).join(old_name)
    }

    pub fn append_record(config: &crate::Config, record: CompletedTaskRecord) {
        let path = Self::get_history_path(config);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let file_res = OpenOptions::new().create(true).append(true).open(&path);

        match file_res {
            Ok(file) => {
                // Lock exclusively
                if let Err(e) = file.lock_exclusive() {
                    tracing::error!("Failed to lock history file: {}", e);
                    return;
                }

                let serialized = match serde_json::to_string(&record) {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = file.unlock();
                        tracing::error!("Failed to serialize history record: {}", e);
                        return;
                    }
                };

                let mut writer = file;
                if let Err(e) = writeln!(writer, "{}", serialized) {
                    tracing::error!("Failed to write history record: {}", e);
                }
                let _ = writer.unlock();

                // Check for rotation
                Self::check_rotation(config);
            }
            Err(e) => {
                tracing::error!("Failed to open history file for appending: {}", e);
            }
        }
    }

    pub fn read_records(config: &crate::Config) -> Vec<CompletedTaskRecord> {
        let path = Self::get_history_path(config);
        if !path.exists() {
            return Vec::new();
        }

        let file_res = File::open(&path);
        match file_res {
            Ok(file) => {
                // Lock shared for reading
                if let Err(e) = file.lock_shared() {
                    tracing::error!("Failed to acquire shared lock on history file: {}", e);
                    // Best effort read without lock if lock fails
                }

                let reader = BufReader::new(&file);
                let mut records = Vec::new();

                for line in reader.lines() {
                    let line = match line {
                        Ok(l) => l,
                        Err(_) => continue,
                    };
                    if line.trim().is_empty() {
                        continue;
                    }
                    match serde_json::from_str::<CompletedTaskRecord>(&line) {
                        Ok(rec) => records.push(rec),
                        Err(e) => {
                            tracing::warn!("Skipping malformed history line: {}", e);
                        }
                    }
                }

                let _ = file.unlock();
                records
            }
            Err(_) => Vec::new(),
        }
    }

    pub fn purge_history(config: &crate::Config) {
        let path = Self::get_history_path(config);
        let old_path = Self::get_old_history_path(config);
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(old_path);
    }

    pub fn remove_record_by_gid(config: &crate::Config, gid_str: &str) {
        let path = Self::get_history_path(config);
        if !path.exists() {
            return;
        }

        let file_res = OpenOptions::new().read(true).write(true).open(&path);

        if let Ok(file) = file_res {
            if let Err(e) = file.lock_exclusive() {
                tracing::error!("Failed to lock history file for removal: {}", e);
                return;
            }

            let reader = BufReader::new(&file);
            let mut kept_lines = Vec::new();
            for line in reader.lines() {
                let line = match line {
                    Ok(l) => l,
                    Err(_) => continue,
                };
                let matches_gid =
                    if let Ok(record) = serde_json::from_str::<CompletedTaskRecord>(&line) {
                        record.id == gid_str
                    } else {
                        false
                    };
                if !matches_gid {
                    kept_lines.push(line);
                }
            }

            // Truncate and write kept lines
            if let Ok(mut writer) = OpenOptions::new().write(true).truncate(true).open(&path) {
                for l in kept_lines {
                    let _ = writeln!(writer, "{}", l);
                }
            }

            let _ = file.unlock();
        }
    }

    fn check_rotation(config: &crate::Config) {
        let path = Self::get_history_path(config);
        let metadata = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => return,
        };

        let file_size_mb = metadata.len() as f64 / (1024.0 * 1024.0);

        // Read lines to check count
        let records = Self::read_records(config);

        if file_size_mb > config.limits.history_rotation_mb
            || records.len() > config.limits.history_rotation_records
        {
            // Trigger rotation
            let old_path = Self::get_old_history_path(config);

            // Keep most recent
            let split_index = records
                .len()
                .saturating_sub(config.limits.history_retention_records);
            let (old_records, new_records) = records.split_at(split_index);

            // Append old records to history.old.jsonl
            if let Ok(old_file) = OpenOptions::new().create(true).append(true).open(&old_path) {
                let _ = old_file.lock_exclusive();
                let mut writer = old_file;
                for rec in old_records {
                    if let Ok(serialized) = serde_json::to_string(rec) {
                        let _ = writeln!(writer, "{}", serialized);
                    }
                }
                let _ = writer.unlock();
            }

            // Rewrite history.jsonl with new records
            if let Ok(new_file) = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&path)
            {
                let _ = new_file.lock_exclusive();
                let mut writer = new_file;
                for rec in new_records {
                    if let Ok(serialized) = serde_json::to_string(rec) {
                        let _ = writeln!(writer, "{}", serialized);
                    }
                }
                let _ = writer.unlock();
            }
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
