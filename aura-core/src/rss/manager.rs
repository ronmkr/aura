use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeedSubscription {
    pub url: String,
    pub name: String,
    pub poll_interval: Option<u64>, // Polling frequency in minutes, default: 30
    pub filters: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct FeedsToml {
    #[serde(default)]
    pub feeds: Vec<FeedSubscription>,
}

pub struct RssManager {
    feeds_path: PathBuf,
    history_path: PathBuf,
}

impl Default for RssManager {
    fn default() -> Self {
        Self::new()
    }
}

impl RssManager {
    pub fn new() -> Self {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let aura_dir = home.join(".aura");
        let _ = fs::create_dir_all(&aura_dir);

        Self {
            feeds_path: aura_dir.join("feeds.toml"),
            history_path: aura_dir.join("feed_history.txt"),
        }
    }

    pub fn load_subscriptions(&self) -> Result<Vec<FeedSubscription>, String> {
        if !self.feeds_path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&self.feeds_path)
            .map_err(|e| format!("Failed to read feeds file: {}", e))?;
        let toml_data: FeedsToml =
            toml::from_str(&content).map_err(|e| format!("Failed to parse feeds TOML: {}", e))?;
        Ok(toml_data.feeds)
    }

    pub fn save_subscriptions(&self, feeds: Vec<FeedSubscription>) -> Result<(), String> {
        let toml_data = FeedsToml { feeds };
        let content = toml::to_string_pretty(&toml_data)
            .map_err(|e| format!("Failed to serialize feeds to TOML: {}", e))?;
        fs::write(&self.feeds_path, content)
            .map_err(|e| format!("Failed to write feeds file: {}", e))?;
        Ok(())
    }

    pub fn add_subscription(&self, sub: FeedSubscription) -> Result<(), String> {
        let mut subs = self.load_subscriptions()?;
        if subs.iter().any(|s| s.url == sub.url || s.name == sub.name) {
            return Err("Subscription with this URL or Name already exists".to_string());
        }
        subs.push(sub);
        self.save_subscriptions(subs)
    }

    pub fn remove_subscription(&self, name_or_url: &str) -> Result<(), String> {
        let mut subs = self.load_subscriptions()?;
        let original_len = subs.len();
        subs.retain(|s| s.name != name_or_url && s.url != name_or_url);
        if subs.len() == original_len {
            return Err("Subscription not found".to_string());
        }
        self.save_subscriptions(subs)
    }

    pub fn is_ingested(&self, guid: &str) -> bool {
        if !self.history_path.exists() {
            return false;
        }
        if let Ok(content) = fs::read_to_string(&self.history_path) {
            content.lines().any(|line| line.trim() == guid)
        } else {
            false
        }
    }

    pub fn mark_ingested(&self, guid: &str) -> Result<(), String> {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.history_path)
            .map_err(|e| format!("Failed to open history file: {}", e))?;
        writeln!(file, "{}", guid)
            .map_err(|e| format!("Failed to write to history file: {}", e))?;
        Ok(())
    }

    pub fn matches_filters(title: &str, filters: &Option<Vec<String>>) -> bool {
        if let Some(ref list) = filters {
            if list.is_empty() {
                return true;
            }
            for pattern in list {
                if let Ok(re) = regex::Regex::new(pattern) {
                    if re.is_match(title) {
                        return true;
                    }
                } else if title.contains(pattern) {
                    return true;
                }
            }
            false
        } else {
            true
        }
    }
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
