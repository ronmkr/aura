//! throttler: Implements a hierarchical Token Bucket for bandwidth control.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tokio::time::{interval, Duration};

pub struct TokenBucket {
    rate_per_sec: Arc<AtomicU64>,
    capacity: Arc<AtomicU64>,
    available: Arc<Semaphore>,
}

impl TokenBucket {
    pub fn new(rate_per_sec: u64) -> Self {
        // Initial capacity is either the rate or a sensible default for unlimited (1GB)
        let initial_cap = if rate_per_sec == 0 {
            1_000_000_000
        } else {
            rate_per_sec
        };
        // Start with one tick's worth of tokens to avoid massive initial burst in tests
        let initial_permits = if rate_per_sec == 0 {
            1_000_000_000
        } else {
            rate_per_sec / 10
        };
        let available = Arc::new(Semaphore::new(initial_permits as usize));
        let rate_atomic = Arc::new(AtomicU64::new(rate_per_sec));
        let capacity_atomic = Arc::new(AtomicU64::new(initial_cap));

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
                    if to_add > 0 {
                        available_clone.add_permits(to_add);
                    }
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
        let cap = if new_rate == 0 {
            1_000_000_000
        } else {
            new_rate
        };
        self.capacity.store(cap, Ordering::Relaxed);

        // Drain excess permits if we reduced capacity
        let current = self.available.available_permits();
        if current > cap as usize {
            // We can't safely "set" semaphore permits, but we can acquire the excess
            // We use try_acquire_many to avoid blocking.
            let excess = current - cap as usize;
            let _ = self.available.try_acquire_many(excess as u32);
        }
    }

    pub async fn acquire(&self, amount: u64) {
        if self.rate_per_sec.load(Ordering::Relaxed) == 0 {
            return; // Unlimited
        }
        let cap = self.capacity.load(Ordering::Relaxed) as usize;
        let mut remaining = amount as usize;

        while remaining > 0 {
            let permits = std::cmp::min(remaining, cap);
            if permits > 0 {
                let _ = self.available.acquire_many(permits as u32).await;
                remaining -= permits;
            } else {
                break;
            }
        }
    }
}

use crate::TaskId;

pub struct Throttler {
    global_download: TokenBucket,
    global_upload: TokenBucket,
    task_download: Arc<RwLock<HashMap<TaskId, Arc<TokenBucket>>>>,
    task_upload: Arc<RwLock<HashMap<TaskId, Arc<TokenBucket>>>>,
}

impl Throttler {
    pub fn new(global_download_rate: u64, global_upload_rate: u64) -> Self {
        Self {
            global_download: TokenBucket::new(global_download_rate),
            global_upload: TokenBucket::new(global_upload_rate),
            task_download: Arc::new(RwLock::new(HashMap::new())),
            task_upload: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn set_global_download_limit(&self, new_rate: u64) {
        self.global_download.set_rate(new_rate);
    }

    pub fn set_global_upload_limit(&self, new_rate: u64) {
        self.global_upload.set_rate(new_rate);
    }

    pub async fn register_task(&self, id: TaskId, dl_limit: u64, ul_limit: u64) {
        let mut dl = self.task_download.write().await;
        dl.insert(id, Arc::new(TokenBucket::new(dl_limit)));
        let mut ul = self.task_upload.write().await;
        ul.insert(id, Arc::new(TokenBucket::new(ul_limit)));
    }

    pub async fn unregister_task(&self, id: TaskId) {
        let mut dl = self.task_download.write().await;
        dl.remove(&id);
        let mut ul = self.task_upload.write().await;
        ul.remove(&id);
    }

    pub async fn acquire_download(&self, id: TaskId, amount: u64) {
        // Hierarchical acquisition: Global -> Task
        // Note: Global should be acquired first to ensure we don't exceed global bandwidth
        // while waiting for task-level tokens.
        self.global_download.acquire(amount).await;

        let task_bucket = {
            let read = self.task_download.read().await;
            read.get(&id).cloned()
        };

        if let Some(bucket) = task_bucket {
            bucket.acquire(amount).await;
        }
    }

    pub async fn acquire_upload(&self, id: TaskId, amount: u64) {
        self.global_upload.acquire(amount).await;

        let task_bucket = {
            let read = self.task_upload.read().await;
            read.get(&id).cloned()
        };

        if let Some(bucket) = task_bucket {
            bucket.acquire(amount).await;
        }
    }
}
