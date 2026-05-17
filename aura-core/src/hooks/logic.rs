use crate::config::HookConfig;
use crate::orchestrator::Event;
use crate::TaskId;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, error, info};

pub struct HookManager {
    config: HookConfig,
}

impl HookManager {
    pub fn new(config: HookConfig) -> Self {
        Self { config }
    }

    pub fn update_config(&mut self, config: HookConfig) {
        self.config = config;
    }

    pub async fn handle_event(&self, event: &Event) {
        match event {
            Event::TaskAdded(id) => {
                if let Some(ref script) = self.config.on_download_start {
                    self.execute_hook(script.clone(), *id, "start").await;
                }
            }
            Event::TaskCompleted(id) => {
                if let Some(ref script) = self.config.on_download_complete {
                    self.execute_hook(script.clone(), *id, "complete").await;
                }
            }
            Event::TaskError { id, message } => {
                if let Some(ref script) = self.config.on_download_error {
                    // Pass error message as third argument
                    self.execute_hook_with_arg(script.clone(), *id, "error", message)
                        .await;
                }
            }
            Event::TaskPaused(id) => {
                if let Some(ref script) = self.config.on_download_pause {
                    self.execute_hook(script.clone(), *id, "pause").await;
                }
            }
            _ => {}
        }
    }

    async fn execute_hook(&self, script: String, task_id: TaskId, event_name: &str) {
        self.execute_hook_with_arg(script, task_id, event_name, "")
            .await
    }

    async fn execute_hook_with_arg(
        &self,
        script: String,
        task_id: TaskId,
        event_name: &str,
        extra_arg: &str,
    ) {
        let event_name = event_name.to_string();
        let extra_arg = extra_arg.to_string();
        tokio::spawn(async move {
            debug!(
                "Executing hook script: {} for task {} ({})",
                script, task_id.0, event_name
            );

            let mut cmd = Command::new("sh");
            cmd.arg("-c");

            // Pass task_id, event_name, and extra_arg as environment variables or arguments
            // We'll pass them as arguments to the script
            let script_cmd = format!("{} {} {} \"{}\"", script, task_id.0, event_name, extra_arg);
            cmd.arg(&script_cmd);

            cmd.stdout(Stdio::null());
            cmd.stderr(Stdio::piped());

            match cmd.output().await {
                Ok(output) => {
                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        error!(
                            "Hook script '{}' failed with status: {}. Stderr: {}",
                            script, output.status, stderr
                        );
                    } else {
                        info!(
                            "Hook script '{}' executed successfully for task {}",
                            script, task_id.0
                        );
                    }
                }
                Err(e) => {
                    error!("Failed to execute hook script '{}': {}", script, e);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TaskId;

    #[tokio::test]
    async fn test_hook_manager_config_update() {
        let config1 = HookConfig {
            on_download_start: Some("echo start1".into()),
            on_download_complete: None,
            on_download_error: None,
            on_download_pause: None,
        };
        let mut manager = HookManager::new(config1);
        
        let config2 = HookConfig {
            on_download_start: Some("echo start2".into()),
            on_download_complete: Some("echo complete2".into()),
            on_download_error: None,
            on_download_pause: None,
        };
        manager.update_config(config2);
        
        assert_eq!(manager.config.on_download_start.as_deref(), Some("echo start2"));
        assert_eq!(manager.config.on_download_complete.as_deref(), Some("echo complete2"));
    }

    #[tokio::test]
    async fn test_handle_event_no_hooks() {
        let config = HookConfig {
            on_download_start: None,
            on_download_complete: None,
            on_download_error: None,
            on_download_pause: None,
        };
        let manager = HookManager::new(config);
        
        // This shouldn't panic or do anything since no hooks are defined
        manager.handle_event(&Event::TaskAdded(TaskId(1))).await;
    }
}

