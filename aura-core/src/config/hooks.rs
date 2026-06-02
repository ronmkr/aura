use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct HookConfig {
    pub on_download_start: Option<String>,
    pub on_download_complete: Option<String>,
    pub on_download_error: Option<String>,
    pub on_download_pause: Option<String>,
}
