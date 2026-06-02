use crate::config::{ConflictPolicy, MappingCondition, ResourceMappingConfig};
use crate::task::MetaTask;
use std::path::{Path, PathBuf};

pub struct MappingEngine {
    config: ResourceMappingConfig,
}

impl MappingEngine {
    pub fn new(config: ResourceMappingConfig) -> Self {
        Self { config }
    }

    pub fn resolve_path(&self, task: &MetaTask, base_dir: &Path) -> PathBuf {
        let mut final_path = base_dir.join(&task.name);

        for rule in &self.config.rules {
            if self.matches(task, &rule.condition) {
                let mapped = self.apply_template(&rule.target, task);
                let sanitized: PathBuf = Path::new(&mapped)
                    .components()
                    .filter(|c| matches!(c, std::path::Component::Normal(_)))
                    .collect();
                final_path = base_dir.join(sanitized);
                break;
            }
        }

        self.resolve_conflict(final_path)
    }

    fn resolve_conflict(&self, mut path: PathBuf) -> PathBuf {
        if !path.exists() {
            return path;
        }

        match self.config.default_conflict_policy {
            ConflictPolicy::Overwrite => path,
            ConflictPolicy::Skip => {
                // Return original path, higher layers will decide how to handle "Skip"
                // (e.g. by checking if it exists before opening)
                path
            }
            ConflictPolicy::AutoRename => {
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("file")
                    .to_string();
                let extension = path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();

                let mut counter = 1;
                while path.exists() {
                    let new_name = if extension.is_empty() {
                        format!("{}.{}", stem, counter)
                    } else {
                        format!("{}.{}.{}", stem, counter, extension)
                    };
                    path.set_file_name(new_name);
                    counter += 1;
                }
                path
            }
        }
    }

    fn matches(&self, task: &MetaTask, condition: &MappingCondition) -> bool {
        match condition {
            MappingCondition::Extension(ext) => Path::new(&task.name)
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case(ext))
                .unwrap_or(false),
            MappingCondition::Domain(domain) => task.subtasks.iter().any(|sub| {
                if let Ok(url) = url::Url::parse(&sub.uri) {
                    url.domain().map(|d| d.contains(domain)).unwrap_or(false)
                } else {
                    false
                }
            }),
            MappingCondition::Protocol(ttype) => {
                task.subtasks.iter().any(|sub| sub.task_type == *ttype)
            }
            MappingCondition::Regex(pattern) => {
                if let Ok(re) = regex::Regex::new(pattern) {
                    re.is_match(&task.name)
                } else {
                    false
                }
            }
        }
    }

    fn apply_template(&self, template: &str, task: &MetaTask) -> String {
        let mut result = template.replace("{name}", &task.name);
        result = result.replace("{id}", &task.id.0.to_string());

        if let Some(ext) = Path::new(&task.name).extension().and_then(|s| s.to_str()) {
            result = result.replace("{ext}", ext);
        } else {
            result = result.replace("{ext}", "");
        }

        // Add domain/host/protocol placeholders from first subtask
        if let Some(sub) = task.subtasks.first() {
            if let Ok(url) = url::Url::parse(&sub.uri) {
                if let Some(host) = url.host_str() {
                    result = result.replace("{host}", host);
                }
                if let Some(domain) = url.domain() {
                    result = result.replace("{domain}", domain);
                }
                result = result.replace("{protocol}", url.scheme());
            }
        }

        // Fallback for missing placeholders
        result = result.replace("{host}", "unknown_host");
        result = result.replace("{domain}", "unknown_domain");
        result = result.replace("{protocol}", "unknown_protocol");

        // Date placeholders
        let now = chrono::Local::now();
        result = result.replace("{year}", &now.format("%Y").to_string());
        result = result.replace("{month}", &now.format("%m").to_string());
        result = result.replace("{day}", &now.format("%d").to_string());

        result
    }
}

#[cfg(test)]
#[path = "mapping_tests.rs"]
mod tests;
