use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::time::{interval, Duration};

pub struct TokenBucket {
    pub(crate) rate_per_sec: Arc<AtomicU64>,
    pub(crate) capacity: Arc<AtomicU64>,
    pub(crate) available: Arc<Semaphore>,
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

                let refill_amount = rate.div_ceil(10);
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
                match self.available.acquire_many(permits as u32).await {
                    Ok(permit) => {
                        permit.forget();
                        remaining -= permits;
                    }
                    Err(_) => break, // Semaphore closed
                }
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
#[path = "bucket_tests.rs"]
mod tests;
