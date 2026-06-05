use super::tests::make_test_orchestrator;
use crate::orchestrator::command::AddTaskArgs;
use crate::task::TaskType;
use crate::TaskId;

#[tokio::test]
async fn test_dag_cycle_detection_via_add_and_change_options() {
    let (mut orch, _storage_rx, _temp_dir) = make_test_orchestrator();

    // 1. Add Task A
    let res = orch
        .handle_add_task(AddTaskArgs {
            id: TaskId(1),
            tenant_id: None,
            name: "task_a".to_string(),
            sources: vec![("http://a".to_string(), TaskType::Http)],
            checksum: None,
            priority: 3,
            streaming_mode: false,
            depends_on: Vec::new(),
            follow_on: None,
        })
        .await;
    assert!(res.is_ok());

    // 2. Add Task B depending on A
    let res = orch
        .handle_add_task(AddTaskArgs {
            id: TaskId(2),
            tenant_id: None,
            name: "task_b".to_string(),
            sources: vec![("http://b".to_string(), TaskType::Http)],
            checksum: None,
            priority: 3,
            streaming_mode: false,
            depends_on: vec![TaskId(1)],
            follow_on: None,
        })
        .await;
    assert!(res.is_ok());

    // 3. Attempting to add Task A depending on B should fail (cycle)
    let res = orch
        .handle_change_option(TaskId(1), None, Some(vec![TaskId(2)]), None, None)
        .await;

    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("cycle"));

    // Check that dependencies for Task A were rolled back (still empty)
    assert!(orch.tasks.get(&TaskId(1)).unwrap().depends_on.is_empty());
}
