use crate::task::{MetaTask, TaskType};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MappingCondition {
    Extension(String),
    Domain(String),
    Protocol(TaskType),
    Regex(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MappingRule {
    pub condition: MappingCondition,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceMappingConfig {
    pub rules: Vec<MappingRule>,
}

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

        final_path
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
