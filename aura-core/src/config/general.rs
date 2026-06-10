use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub log_level: String,
    pub log_path: Option<String>,
    pub check_integrity: bool,
    pub event_poll_interval_ms: u64,
    pub daemon_mode: bool,
    pub aura_dir_name: String,
    pub history_file_name: String,
    pub theme: ThemeConfig,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            log_path: None,
            check_integrity: true,
            event_poll_interval_ms: 500,
            daemon_mode: false,
            aura_dir_name: ".aura".to_string(),
            history_file_name: "history.jsonl".to_string(),
            theme: ThemeConfig::galactic(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub primary: String,
    pub accent: String,
    pub highlight: String,
    pub background: String,
    pub foreground: String,
    pub success: String,
    pub error: String,
    pub warning: String,
}

impl ThemeConfig {
    pub fn galactic() -> Self {
        Self {
            primary: "#0000FF".to_string(),   // Galactic Blue
            accent: "#00FFFF".to_string(),    // Nebula Cyan
            highlight: "#FFFF00".to_string(), // Star Yellow
            background: "#000000".to_string(),
            foreground: "#FFFFFF".to_string(),
            success: "#00FF00".to_string(),
            error: "#FF0000".to_string(),
            warning: "#FFFF00".to_string(),
        }
    }

    pub fn matrix() -> Self {
        Self {
            primary: "#003B00".to_string(),
            accent: "#00FF41".to_string(),
            highlight: "#008F11".to_string(),
            background: "#000000".to_string(),
            foreground: "#00FF41".to_string(),
            success: "#00FF41".to_string(),
            error: "#FF0000".to_string(),
            warning: "#008F11".to_string(),
        }
    }
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self::galactic()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CredentialConfig {
    pub netrc_path: Option<String>,
    pub cookie_file: Option<String>,
}
