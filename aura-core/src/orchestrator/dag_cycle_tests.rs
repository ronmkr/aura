use super::tests_racing::make_test_orchestrator;
use crate::orchestrator::command::AddTaskArgs;
use crate::task::TaskType;
use crate::TaskId;

#[tokio::test]
async fn test_dag_cycle_detection_via_add_and_change_options() {
    let (mut orch, _storage_rx, _temp_dir) = make_test_orchestrator();

    // 1. Add Task 1
    let res = orch
        .handle_add_task(AddTaskArgs {
            id: TaskId(1),
            tenant_id: None,
            name: "task1".to_string(),
            sources: vec![("http://test1".to_string(), TaskType::Http)],
            checksum: None,
            priority: 3,
            streaming_mode: false,
            depends_on: Vec::new(),
            follow_on: None,
        })
        .await;
    assert!(res.is_ok());

    // 2. Add Task 2 depending on 1
    let res = orch
        .handle_add_task(AddTaskArgs {
            id: TaskId(2),
            tenant_id: None,
            name: "task2".to_string(),
            sources: vec![("http://test2".to_string(), TaskType::Http)],
            checksum: None,
            priority: 3,
            streaming_mode: false,
            depends_on: vec![TaskId(1)],
            follow_on: None,
        })
        .await;
    assert!(res.is_ok());

    // 3. Try to add Task 1 depending on 2 (Immediate Cycle)
    let res = orch
        .handle_add_task(AddTaskArgs {
            id: TaskId(1),
            tenant_id: None,
            name: "task1_cycle".to_string(),
            sources: vec![("http://test1".to_string(), TaskType::Http)],
            checksum: None,
            priority: 3,
            streaming_mode: false,
            depends_on: vec![TaskId(2)],
            follow_on: None,
        })
        .await;
    assert!(res.is_err());
}
