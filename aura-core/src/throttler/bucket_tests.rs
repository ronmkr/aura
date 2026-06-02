use super::*;

#[tokio::test]
async fn test_token_bucket_throttling() {
    let rate = 100; // 100 bytes/sec
    let bucket = TokenBucket::new(rate);

    // Wait for initial burst to subside and refill to stabilize
    tokio::time::sleep(Duration::from_millis(500)).await;

    let start = std::time::Instant::now();
    bucket.acquire(300).await;
    let elapsed = start.elapsed();

    assert!(
        elapsed >= Duration::from_millis(1500),
        "Throttling failed: took only {:?}",
        elapsed
    );
}
