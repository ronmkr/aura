use crate::orchestrator::mapping::{
    MappingCondition, MappingEngine, MappingRule, ResourceMappingConfig,
};
use crate::task::MetaTask;
use crate::TaskId;
use std::path::{Path, PathBuf};

#[test]
fn test_mapping_extension() {
    let config = ResourceMappingConfig {
        rules: vec![MappingRule {
            condition: MappingCondition::Extension("mp4".to_string()),
            target: "videos/{name}".to_string(),
        }],
        ..Default::default()
    };
    let engine = MappingEngine::new(config);
    let task = MetaTask::new(TaskId(1), "movie.mp4".to_string(), 1000);
    let base = Path::new("/downloads");

    let path = engine.resolve_path(&task, base);
    assert_eq!(path, PathBuf::from("/downloads/videos/movie.mp4"));
}

#[test]
fn test_mapping_regex() {
    let config = ResourceMappingConfig {
        rules: vec![MappingRule {
            condition: MappingCondition::Regex(".*[0-9]+.*".to_string()),
            target: "episodes/{name}".to_string(),
        }],
        ..Default::default()
    };
    let engine = MappingEngine::new(config);
    let task = MetaTask::new(TaskId(1), "show_s01e01.mkv".to_string(), 1000);
    let base = Path::new("/downloads");

    let path = engine.resolve_path(&task, base);
    assert_eq!(path, PathBuf::from("/downloads/episodes/show_s01e01.mkv"));
}

#[test]
fn test_mapping_placeholders() {
    let config = ResourceMappingConfig {
        rules: vec![MappingRule {
            condition: MappingCondition::Extension("zip".to_string()),
            target: "archives/{id}_{ext}/{name}".to_string(),
        }],
        ..Default::default()
    };
    let engine = MappingEngine::new(config);
    let task = MetaTask::new(TaskId(123), "data.zip".to_string(), 1000);
    let base = Path::new("/downloads");

    let path = engine.resolve_path(&task, base);
    assert_eq!(path, PathBuf::from("/downloads/archives/123_zip/data.zip"));
}

#[test]
fn test_mapping_traversal_prevention() {
    let config = ResourceMappingConfig {
        rules: vec![MappingRule {
            condition: MappingCondition::Extension("mp4".to_string()),
            target: "../../../escaped/{name}".to_string(),
        }],
        ..Default::default()
    };
    let engine = MappingEngine::new(config);
    let task = MetaTask::new(TaskId(1), "movie.mp4".to_string(), 1000);
    let base = Path::new("/downloads");

    let path = engine.resolve_path(&task, base);
    assert_eq!(path, PathBuf::from("/downloads/escaped/movie.mp4"));
}

#[test]
fn test_mapping_new_placeholders() {
    let config = ResourceMappingConfig {
        rules: vec![MappingRule {
            condition: MappingCondition::Extension("iso".to_string()),
            target: "{protocol}/{domain}/{year}/{name}".to_string(),
        }],
        default_conflict_policy: crate::orchestrator::mapping::ConflictPolicy::AutoRename,
    };
    let engine = MappingEngine::new(config);
    let mut task = MetaTask::new(TaskId(1), "ubuntu.iso".to_string(), 1000);
    task.add_subtask(
        "https://releases.ubuntu.com/22.04/ubuntu.iso".to_string(),
        crate::task::TaskType::Http,
    );
    let base = Path::new("/downloads");

    let path = engine.resolve_path(&task, base);
    let now = chrono::Local::now();
    let year = now.format("%Y").to_string();
    assert_eq!(
        path,
        PathBuf::from(format!(
            "/downloads/https/releases.ubuntu.com/{}/ubuntu.iso",
            year
        ))
    );
}

#[test]
fn test_mapping_conflict_autorename() {
    let temp_dir = tempfile::tempdir().unwrap();
    let base = temp_dir.path();

    // Create an existing file
    let existing_file = base.join("data.zip");
    std::fs::write(&existing_file, "existing").unwrap();

    let config = ResourceMappingConfig {
        rules: Vec::new(),
        default_conflict_policy: crate::orchestrator::mapping::ConflictPolicy::AutoRename,
    };
    let engine = MappingEngine::new(config);
    let task = MetaTask::new(TaskId(1), "data.zip".to_string(), 1000);

    let path = engine.resolve_path(&task, base);
    assert_eq!(path, base.join("data.1.zip"));

    // Create another one to test sequential renaming
    std::fs::write(&path, "existing 2").unwrap();
    let path2 = engine.resolve_path(&task, base);
    assert_eq!(path2, base.join("data.2.zip"));
}
