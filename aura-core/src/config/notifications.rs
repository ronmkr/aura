use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NotificationConfig {
    pub enabled: bool,
    pub notify_on_complete: bool,
    pub notify_on_error: bool,
    pub app_name: String,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            notify_on_complete: true,
            notify_on_error: true,
            app_name: "Aura".to_string(),
        }
    }
}
