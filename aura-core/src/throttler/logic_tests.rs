use super::*;

#[tokio::test]
async fn test_proportional_throttling() {
    let throttler = Throttler::new(100_000, 100_000, 100);

    // Register 2 tasks:
    // Task 1: Priority 0 -> Weight = 32
    // Task 2: Priority 5 -> Weight = 1
    // Total weight = 33
    throttler.register_task(TaskId(1), 0, 0, 0).await;
    throttler.register_task(TaskId(2), 0, 0, 5).await;

    let (t1_rate, t2_rate, t1_bucket, t2_bucket) = {
        let dls = throttler.task_download.read().await;
        let t1_bucket = dls.get(&TaskId(1)).unwrap().clone();
        let t2_bucket = dls.get(&TaskId(2)).unwrap().clone();
        let t1_rate = t1_bucket.rate_per_sec.load(Ordering::Relaxed);
        let t2_rate = t2_bucket.rate_per_sec.load(Ordering::Relaxed);
        (t1_rate, t2_rate, t1_bucket, t2_bucket)
    };

    // Expected proportional rates:
    // Task 1: (100_000 * 32) / 33 = 96,969
    // Task 2: (100_000 * 1) / 33 = 3,030
    assert!(t1_rate > 95_000, "t1_rate was {}", t1_rate);
    assert!(t2_rate < 4_000, "t2_rate was {}", t2_rate);

    // Update task 2 to priority 0 -> weight = 32
    // Total weight = 64
    // Both should get 50,000
    throttler.update_task_priority(TaskId(2), 0).await;

    let t1_rate_new = t1_bucket.rate_per_sec.load(Ordering::Relaxed);
    let t2_rate_new = t2_bucket.rate_per_sec.load(Ordering::Relaxed);

    assert_eq!(t1_rate_new, 50_000);
    assert_eq!(t2_rate_new, 50_000);
}
