use crate::config::HookConfig;
use crate::orchestrator::Event;
use crate::TaskId;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

#[derive(Debug, Error)]
pub enum HookError {
    #[error("IO error when invoking script: {0}")]
    Io(#[from] std::io::Error),

    #[error("Hook execution failed with status: {status}. Stderr: {stderr}")]
    ExecutionFailed {
        status: std::process::ExitStatus,
        stderr: String,
    },

    #[error("Failed to receive event due to broadcast lag/overflow: {0}")]
    Lagged(u64),

    #[error("Hook execution timed out after {duration_secs} seconds")]
    Timeout { duration_secs: u64 },
}

pub trait ConfigProvider: Send + Sync + 'static {
    fn get_hooks(&self) -> HookConfig;
}

impl ConfigProvider for HookConfig {
    fn get_hooks(&self) -> HookConfig {
        self.clone()
    }
}

impl ConfigProvider for Arc<arc_swap::ArcSwap<crate::Config>> {
    fn get_hooks(&self) -> HookConfig {
        self.load().hooks.clone()
    }
}

#[async_trait::async_trait]
pub trait CommandExecutor: Send + Sync + 'static {
    async fn execute(
        &self,
        script: String,
        task_id: TaskId,
        event_name: &str,
        extra_arg: &str,
    ) -> Result<(), HookError>;
}

#[derive(Default)]
pub struct ShellExecutor;

impl ShellExecutor {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl CommandExecutor for ShellExecutor {
    async fn execute(
        &self,
        script: String,
        task_id: TaskId,
        event_name: &str,
        extra_arg: &str,
    ) -> Result<(), HookError> {
        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c");

        let script_cmd = format!("{} {} {} \"{}\"", script, task_id.0, event_name, extra_arg);
        cmd.arg(&script_cmd);
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::piped());

        let output = cmd.output().await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            Err(HookError::ExecutionFailed {
                status: output.status,
                stderr,
            })
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone)]
pub struct HookOptions {
    pub timeout_seconds: Option<u64>,
    pub max_concurrent_hooks: usize,
}

impl Default for HookOptions {
    fn default() -> Self {
        Self {
            timeout_seconds: Some(30),
            max_concurrent_hooks: 16,
        }
    }
}

pub struct HookServiceHandle {
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
    join_handle: tokio::task::JoinHandle<()>,
}

impl HookServiceHandle {
    pub async fn shutdown(self) -> Result<(), tokio::task::JoinError> {
        let _ = self.shutdown_tx.send(());
        self.join_handle.await
    }
}

pub struct HookManager;

impl HookManager {
    /// Boots the hook-execution daemon in a single one-line call.
    pub fn boot<C, E>(
        mut event_rx: broadcast::Receiver<Event>,
        config_provider: C,
        executor: E,
        options: HookOptions,
    ) -> HookServiceHandle
    where
        C: ConfigProvider,
        E: CommandExecutor,
    {
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(options.max_concurrent_hooks));

        let join_handle = tokio::spawn(async move {
            let executor = Arc::new(executor);
            let config_provider = Arc::new(config_provider);

            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => {
                        debug!("HookManager service received shutdown signal, exiting.");
                        break;
                    }
                    recv_res = event_rx.recv() => {
                        match recv_res {
                            Ok(event) => {
                                let hooks = config_provider.get_hooks();

                                // Determine if we should run a script
                                let (script_opt, task_id, event_name, extra_arg) = match event {
                                    Event::TaskAdded(id) => {
                                        (hooks.on_download_start.clone(), id, "start", "".to_string())
                                    }
                                    Event::TaskCompleted(id) => {
                                        (hooks.on_download_complete.clone(), id, "complete", "".to_string())
                                    }
                                    Event::TaskPaused(id) => {
                                        (hooks.on_download_pause.clone(), id, "pause", "".to_string())
                                    }
                                    Event::TaskError { id, message } => {
                                        (hooks.on_download_error.clone(), id, "error", message)
                                    }
                                    _ => (None, TaskId(0), "", "".to_string()),
                                };

                                if let Some(script) = script_opt {
                                    let executor_clone = executor.clone();
                                    let sem_clone = semaphore.clone();
                                    let timeout_secs = options.timeout_seconds;

                                    tokio::spawn(async move {
                                        // Concurrency limit check
                                        let _permit = match sem_clone.acquire().await {
                                            Ok(permit) => permit,
                                            Err(e) => {
                                                error!("Failed to acquire concurrency permit for hook: {}", e);
                                                return;
                                            }
                                        };

                                        debug!("Executing hook script: {} for task {} ({})", script, task_id.0, event_name);

                                        let execution_fut = executor_clone.execute(script.clone(), task_id, event_name, &extra_arg);

                                        let result = if let Some(secs) = timeout_secs {
                                            match tokio::time::timeout(std::time::Duration::from_secs(secs), execution_fut).await {
                                                Ok(res) => res,
                                                Err(_) => Err(HookError::Timeout { duration_secs: secs }),
                                            }
                                        } else {
                                            execution_fut.await
                                        };

                                        match result {
                                            Ok(()) => {
                                                info!("Hook script '{}' executed successfully for task {}", script, task_id.0);
                                            }
                                            Err(e) => {
                                                error!("Hook script '{}' failed for task {}: {}", script, task_id.0, e);
                                            }
                                        }
                                    });
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(count)) => {
                                warn!("HookManager event stream lagged by {} events", count);
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                debug!("Orchestrator event broadcast channel closed, shutting down HookManager.");
                                break;
                            }
                        }
                    }
                }
            }
        });

        HookServiceHandle {
            shutdown_tx,
            join_handle,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TaskId;
    use std::sync::Mutex;

    #[derive(Default, Clone)]
    struct MockExecutor {
        calls: Arc<Mutex<Vec<(String, TaskId, String, String)>>>,
    }

    #[async_trait::async_trait]
    impl CommandExecutor for MockExecutor {
        async fn execute(
            &self,
            script: String,
            task_id: TaskId,
            event_name: &str,
            extra_arg: &str,
        ) -> Result<(), HookError> {
            self.calls.lock().unwrap().push((
                script,
                task_id,
                event_name.to_string(),
                extra_arg.to_string(),
            ));
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_hook_manager_boot_and_trigger() {
        let (event_tx, event_rx) = broadcast::channel(16);
        let config = HookConfig {
            on_download_start: Some("notify_start.sh".into()),
            on_download_complete: Some("notify_complete.sh".into()),
            on_download_error: Some("notify_error.sh".into()),
            on_download_pause: None,
        };

        let mock_executor = MockExecutor::default();
        let handle = HookManager::boot(
            event_rx,
            config,
            mock_executor.clone(),
            HookOptions::default(),
        );

        // Trigger start event
        event_tx.send(Event::TaskAdded(TaskId(42))).unwrap();

        // Trigger error event
        event_tx
            .send(Event::TaskError {
                id: TaskId(42),
                message: "Disk Full".to_string(),
            })
            .unwrap();

        // Give a moment for background processing
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Verify calls captured
        let calls = mock_executor.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);

        assert_eq!(calls[0].0, "notify_start.sh");
        assert_eq!(calls[0].1, TaskId(42));
        assert_eq!(calls[0].2, "start");
        assert_eq!(calls[0].3, "");

        assert_eq!(calls[1].0, "notify_error.sh");
        assert_eq!(calls[1].1, TaskId(42));
        assert_eq!(calls[1].2, "error");
        assert_eq!(calls[1].3, "Disk Full");

        // Shutdown
        handle.shutdown().await.unwrap();
    }
}
