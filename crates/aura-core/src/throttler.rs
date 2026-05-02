//! throttler: Implements a hierarchical Token Bucket for bandwidth control.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Semaphore;
use tokio::time::{interval, Duration};

pub struct TokenBucket {
    rate_per_sec: Arc<AtomicU64>,
    capacity: Arc<AtomicU64>,
    available: Arc<Semaphore>,
}

impl TokenBucket {
    pub fn new(rate_per_sec: u64) -> Self {
        let capacity = rate_per_sec; 
        let available = Arc::new(Semaphore::new(capacity as usize));
        let rate_atomic = Arc::new(AtomicU64::new(rate_per_sec));
        let capacity_atomic = Arc::new(AtomicU64::new(capacity));
        
        let available_clone = available.clone();
        let rate_clone = rate_atomic.clone();
        let cap_clone = capacity_atomic.clone();

        tokio::spawn(async move {
            let mut tick = interval(Duration::from_millis(100));
            loop {
                tick.tick().await;
                let rate = rate_clone.load(Ordering::Relaxed);
                let cap = cap_clone.load(Ordering::Relaxed);
                
                if rate == 0 {
                    // If unlimited, keep semaphore full
                    let current = available_clone.available_permits();
                    if current < 1_000_000_000 {
                        available_clone.add_permits(1_000_000_000 - current);
                    }
                    continue;
                }

                let refill_amount = rate / 10;
                let current = available_clone.available_permits();
                if current < cap as usize {
                    let to_add = std::cmp::min(refill_amount as usize, cap as usize - current);
                    available_clone.add_permits(to_add);
                }
            }
        });

        Self {
            rate_per_sec: rate_atomic,
            capacity: capacity_atomic,
            available,
        }
    }

    pub fn set_rate(&self, new_rate: u64) {
        self.rate_per_sec.store(new_rate, Ordering::Relaxed);
        self.capacity.store(new_rate, Ordering::Relaxed);
    }

    pub async fn acquire(&self, amount: u64) {
        if self.rate_per_sec.load(Ordering::Relaxed) == 0 {
            return; // Unlimited
        }
        let cap = self.capacity.load(Ordering::Relaxed);
        let permits = std::cmp::min(amount as usize, cap as usize);
        if permits > 0 {
            let _ = self.available.acquire_many(permits as u32).await;
        }
    }
}

pub struct Throttler {
    global_download: TokenBucket,
}

impl Throttler {
    pub fn new(global_download_rate: u64) -> Self {
        Self {
            global_download: TokenBucket::new(global_download_rate),
        }
    }

    pub fn set_limit(&self, new_rate: u64) {
        self.global_download.set_rate(new_rate);
    }

    pub async fn consume_download(&self, amount: u64) {
        self.global_download.acquire(amount).await;
    }
}
