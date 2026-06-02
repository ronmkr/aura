use super::*;
use std::time::Duration;

#[test]
fn test_scheduler_priority_and_deadline() {
    let mut scheduler = IoScheduler::new();
    let now = Instant::now();

    scheduler.enqueue(IoTask {
        task_id: TaskId(1),
        offset: 0,
        data: vec![],
        deadline: now + Duration::from_millis(500),
        priority: IoPriority::Normal,
    });

    scheduler.enqueue(IoTask {
        task_id: TaskId(2),
        offset: 0,
        data: vec![],
        deadline: now + Duration::from_millis(600),
        priority: IoPriority::High,
    });

    scheduler.enqueue(IoTask {
        task_id: TaskId(3),
        offset: 0,
        data: vec![],
        deadline: now + Duration::from_millis(400),
        priority: IoPriority::Normal,
    });

    assert_eq!(scheduler.pop().unwrap().task_id, TaskId(2)); // High priority
    assert_eq!(scheduler.pop().unwrap().task_id, TaskId(3)); // Normal, earlier deadline
    assert_eq!(scheduler.pop().unwrap().task_id, TaskId(1)); // Normal, later deadline
    assert!(scheduler.is_empty());
}
