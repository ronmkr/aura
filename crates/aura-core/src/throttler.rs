//! throttler: Implements a hierarchical Token Bucket for bandwidth control.

use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::time::{interval, Duration};

pub struct TokenBucket {
    rate_per_sec: u64,
    capacity: u64,
    available: Arc<Semaphore>,
}

impl TokenBucket {
    pub fn new(rate_per_sec: u64) -> Self {
        // We use a semaphore as the "bucket".
        // For rate limiting, we periodically add permits back to the semaphore.
        let capacity = rate_per_sec; // Max burst = 1 second worth of data
        let available = Arc::new(Semaphore::new(capacity as usize));
        
        let available_clone = available.clone();
        tokio::spawn(async move {
            let mut tick = interval(Duration::from_millis(100));
            let refill_amount = rate_per_sec / 10; // Refill 10 times per second
            loop {
                tick.tick().await;
                let current = available_clone.available_permits();
                if current < capacity as usize {
                    let to_add = std::cmp::min(refill_amount as usize, capacity as usize - current);
                    available_clone.add_permits(to_add);
                }
            }
        });

        Self {
            rate_per_sec,
            capacity,
            available,
        }
    }

    pub async fn acquire(&self, amount: u64) {
        if self.rate_per_sec == 0 {
            return; // Unlimited
        }
        let permits = std::cmp::min(amount as usize, self.capacity as usize);
        let _ = self.available.acquire_many(permits as u32).await;
    }
}

pub struct Throttler {
    global_download: TokenBucket,
    // Task-level buckets could be added here in a HashMap
}

impl Throttler {
    pub fn new(global_download_rate: u64) -> Self {
        Self {
            global_download: TokenBucket::new(global_download_rate),
        }
    }

    pub async fn consume_download(&self, amount: u64) {
        self.global_download.acquire(amount).await;
    }
}
