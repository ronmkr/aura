use crate::TaskId;
use bytes::BytesMut;
use std::cmp::Ordering;
use tokio::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IoPriority {
    Background = 0,
    Normal = 1,
    High = 2,
}

#[derive(Debug)]
pub struct IoTask {
    pub task_id: TaskId,
    pub offset: u64,
    pub data: Vec<BytesMut>,
    pub deadline: Instant,
    pub priority: IoPriority,
}

impl PartialEq for IoTask {
    fn eq(&self, other: &Self) -> bool {
        self.task_id == other.task_id && self.offset == other.offset
    }
}

impl Eq for IoTask {}

impl PartialOrd for IoTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for IoTask {
    fn cmp(&self, other: &Self) -> Ordering {
        let prio_cmp = self.priority.cmp(&other.priority);
        if prio_cmp != Ordering::Equal {
            return prio_cmp; // Higher priority wins
        }
        // Earlier deadline wins (BinaryHeap is a max-heap, so we reverse comparison)
        other.deadline.cmp(&self.deadline)
    }
}

#[derive(Default)]
pub struct IoScheduler {
    queue: std::collections::BinaryHeap<IoTask>,
}

impl IoScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enqueue(&mut self, task: IoTask) {
        self.queue.push(task);
    }

    pub fn pop(&mut self) -> Option<IoTask> {
        self.queue.pop()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn extract_all_for_task(&mut self, id: TaskId) -> Vec<IoTask> {
        let mut extracted = Vec::new();
        let mut remaining = std::collections::BinaryHeap::new();
        while let Some(task) = self.queue.pop() {
            if task.task_id == id {
                extracted.push(task);
            } else {
                remaining.push(task);
            }
        }
        self.queue = remaining;
        extracted
    }
}

#[cfg(test)]
mod tests {
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
}
