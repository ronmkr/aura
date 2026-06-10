use crate::config::Config;
use arc_swap::ArcSwap;
use notify_rust::Notification;
use std::sync::Arc;

pub struct NotificationService {
    config: Arc<ArcSwap<Config>>,
}

impl NotificationService {
    pub fn new(config: Arc<ArcSwap<Config>>) -> Self {
        Self { config }
    }

    pub fn notify_complete(&self, task_name: &str) {
        let cfg = self.config.load();
        if !cfg.notifications.enabled || !cfg.notifications.notify_on_complete {
            return;
        }

        let _ = Notification::new()
            .summary(&format!("Download Complete: {}", task_name))
            .body(&format!(
                "The task '{}' has finished successfully.",
                task_name
            ))
            .appname(&cfg.notifications.app_name)
            .icon("download")
            .show();
    }

    pub fn notify_error(&self, task_name: &str, error: &str) {
        let cfg = self.config.load();
        if !cfg.notifications.enabled || !cfg.notifications.notify_on_error {
            return;
        }

        let _ = Notification::new()
            .summary(&format!("Download Error: {}", task_name))
            .body(&format!("Task '{}' failed: {}", task_name, error))
            .appname(&cfg.notifications.app_name)
            .icon("error")
            .show();
    }
}
