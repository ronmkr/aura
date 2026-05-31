use super::*;
use crate::TaskId;
use std::sync::Mutex;

type HookCall = (String, TaskId, String, String);

#[derive(Default, Clone)]
struct MockExecutor {
    calls: Arc<Mutex<Vec<HookCall>>>,
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
    {
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
    }

    // Shutdown
    handle.shutdown().await.unwrap();
}
