use crate::storage::scheduler::{IoPriority, IoScheduler, IoTask};
use crate::TaskId;
use tokio::time::Instant;

#[test]
fn test_io_scheduler_priority() {
    let mut scheduler = IoScheduler::new();
    let now = Instant::now();

    scheduler.enqueue(IoTask {
        task_id: TaskId(1),
        offset: 0,
        data: vec![],
        priority: IoPriority::Normal,
        deadline: now + std::time::Duration::from_secs(10),
    });
    scheduler.enqueue(IoTask {
        task_id: TaskId(2),
        offset: 100,
        data: vec![],
        priority: IoPriority::High,
        deadline: now + std::time::Duration::from_secs(5),
    });
    scheduler.enqueue(IoTask {
        task_id: TaskId(3),
        offset: 200,
        data: vec![],
        priority: IoPriority::Normal,
        deadline: now + std::time::Duration::from_secs(2),
    });

    assert_eq!(scheduler.pop().unwrap().task_id, TaskId(2)); // High priority
    assert_eq!(scheduler.pop().unwrap().task_id, TaskId(3)); // Normal, earlier deadline
    assert_eq!(scheduler.pop().unwrap().task_id, TaskId(1)); // Normal, later deadline
}
