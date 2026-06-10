use crate::task::TaskType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "value", rename_all = "lowercase")]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ConflictPolicy {
    #[default]
    AutoRename,
    Overwrite,
    Skip,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceMappingConfig {
    pub rules: Vec<MappingRule>,
    pub default_conflict_policy: ConflictPolicy,
}
