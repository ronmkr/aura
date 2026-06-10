//! throttler: Implements a hierarchical Token Bucket for bandwidth control.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::TokenBucket;

use crate::TaskId;

pub struct Throttler {
    global_download: TokenBucket,
    global_upload: TokenBucket,
    task_download: Arc<RwLock<HashMap<TaskId, Arc<TokenBucket>>>>,
    task_upload: Arc<RwLock<HashMap<TaskId, Arc<TokenBucket>>>>,
    global_download_limit: Arc<AtomicU64>,
    global_upload_limit: Arc<AtomicU64>,
    task_priorities: Arc<RwLock<HashMap<TaskId, u32>>>,
    task_dl_configured_limits: Arc<RwLock<HashMap<TaskId, u64>>>,
    task_ul_configured_limits: Arc<RwLock<HashMap<TaskId, u64>>>,
    refill_interval_ms: u64,
}

impl Throttler {
    pub fn new(
        global_download_rate: u64,
        global_upload_rate: u64,
        refill_interval_ms: u64,
    ) -> Self {
        Self {
            global_download: TokenBucket::new(global_download_rate, refill_interval_ms),
            global_upload: TokenBucket::new(global_upload_rate, refill_interval_ms),
            task_download: Arc::new(RwLock::new(HashMap::new())),
            task_upload: Arc::new(RwLock::new(HashMap::new())),
            global_download_limit: Arc::new(AtomicU64::new(global_download_rate)),
            global_upload_limit: Arc::new(AtomicU64::new(global_upload_rate)),
            task_priorities: Arc::new(RwLock::new(HashMap::new())),
            task_dl_configured_limits: Arc::new(RwLock::new(HashMap::new())),
            task_ul_configured_limits: Arc::new(RwLock::new(HashMap::new())),
            refill_interval_ms,
        }
    }

    pub fn set_global_download_limit(&self, new_rate: u64) {
        self.global_download.set_rate(new_rate);
        self.global_download_limit
            .store(new_rate, Ordering::Relaxed);

        let task_priorities = self.task_priorities.clone();
        let global_download_limit = self.global_download_limit.clone();
        let global_upload_limit = self.global_upload_limit.clone();
        let task_download = self.task_download.clone();
        let task_upload = self.task_upload.clone();
        let task_dl_configured_limits = self.task_dl_configured_limits.clone();
        let task_ul_configured_limits = self.task_ul_configured_limits.clone();
        tokio::spawn(async move {
            Self::recalculate_limits_internal(
                task_priorities,
                global_download_limit,
                global_upload_limit,
                task_download,
                task_upload,
                task_dl_configured_limits,
                task_ul_configured_limits,
            )
            .await;
        });
    }

    pub fn set_global_upload_limit(&self, new_rate: u64) {
        self.global_upload.set_rate(new_rate);
        self.global_upload_limit.store(new_rate, Ordering::Relaxed);

        let task_priorities = self.task_priorities.clone();
        let global_download_limit = self.global_download_limit.clone();
        let global_upload_limit = self.global_upload_limit.clone();
        let task_download = self.task_download.clone();
        let task_upload = self.task_upload.clone();
        let task_dl_configured_limits = self.task_dl_configured_limits.clone();
        let task_ul_configured_limits = self.task_ul_configured_limits.clone();
        tokio::spawn(async move {
            Self::recalculate_limits_internal(
                task_priorities,
                global_download_limit,
                global_upload_limit,
                task_download,
                task_upload,
                task_dl_configured_limits,
                task_ul_configured_limits,
            )
            .await;
        });
    }

    pub async fn register_task(&self, id: TaskId, dl_limit: u64, ul_limit: u64, priority: u32) {
        {
            let mut priorities = self.task_priorities.write().await;
            priorities.insert(id, priority);
        }
        {
            let mut dl_conf = self.task_dl_configured_limits.write().await;
            dl_conf.insert(id, dl_limit);
        }
        {
            let mut ul_conf = self.task_ul_configured_limits.write().await;
            ul_conf.insert(id, ul_limit);
        }
        {
            let mut dl = self.task_download.write().await;
            dl.insert(
                id,
                Arc::new(TokenBucket::new(dl_limit, self.refill_interval_ms)),
            );
        }
        {
            let mut ul = self.task_upload.write().await;
            ul.insert(
                id,
                Arc::new(TokenBucket::new(ul_limit, self.refill_interval_ms)),
            );
        }

        self.recalculate_limits().await;
    }

    pub async fn unregister_task(&self, id: TaskId) {
        {
            let mut priorities = self.task_priorities.write().await;
            priorities.remove(&id);
        }
        {
            let mut dl_conf = self.task_dl_configured_limits.write().await;
            dl_conf.remove(&id);
        }
        {
            let mut ul_conf = self.task_ul_configured_limits.write().await;
            ul_conf.remove(&id);
        }
        {
            let mut dl = self.task_download.write().await;
            dl.remove(&id);
        }
        {
            let mut ul = self.task_upload.write().await;
            ul.remove(&id);
        }

        self.recalculate_limits().await;
    }

    pub async fn update_task_priority(&self, id: TaskId, priority: u32) {
        {
            let mut priorities = self.task_priorities.write().await;
            priorities.insert(id, priority);
        }
        self.recalculate_limits().await;
    }

    pub async fn recalculate_limits(&self) {
        Self::recalculate_limits_internal(
            self.task_priorities.clone(),
            self.global_download_limit.clone(),
            self.global_upload_limit.clone(),
            self.task_download.clone(),
            self.task_upload.clone(),
            self.task_dl_configured_limits.clone(),
            self.task_ul_configured_limits.clone(),
        )
        .await;
    }

    async fn recalculate_limits_internal(
        task_priorities: Arc<RwLock<HashMap<TaskId, u32>>>,
        global_download_limit: Arc<AtomicU64>,
        global_upload_limit: Arc<AtomicU64>,
        task_download: Arc<RwLock<HashMap<TaskId, Arc<TokenBucket>>>>,
        task_upload: Arc<RwLock<HashMap<TaskId, Arc<TokenBucket>>>>,
        task_dl_configured_limits: Arc<RwLock<HashMap<TaskId, u64>>>,
        task_ul_configured_limits: Arc<RwLock<HashMap<TaskId, u64>>>,
    ) {
        let priorities = task_priorities.read().await;
        let dl_limit = global_download_limit.load(Ordering::Relaxed);
        let ul_limit = global_upload_limit.load(Ordering::Relaxed);

        let mut total_weight = 0u64;
        for &p in priorities.values() {
            let clamped_p = p.min(5);
            let weight = 1 << (5 - clamped_p);
            total_weight += weight;
        }

        if total_weight == 0 {
            return;
        }

        let dls = task_download.read().await;
        let uls = task_upload.read().await;
        let dl_confs = task_dl_configured_limits.read().await;
        let ul_confs = task_ul_configured_limits.read().await;

        for (id, &p) in priorities.iter() {
            let clamped_p = p.min(5);
            let weight = 1 << (5 - clamped_p);

            // Proportional download
            let dl_conf = dl_confs.get(id).cloned().unwrap_or(0);
            let dl_rate = if dl_limit == 0 {
                dl_conf
            } else {
                let proportional = (dl_limit * weight) / total_weight;
                if dl_conf > 0 {
                    proportional.min(dl_conf)
                } else {
                    proportional
                }
            };
            if let Some(bucket) = dls.get(id) {
                bucket.set_rate(dl_rate);
            }

            // Proportional upload
            let ul_conf = ul_confs.get(id).cloned().unwrap_or(0);
            let ul_rate = if ul_limit == 0 {
                ul_conf
            } else {
                let proportional = (ul_limit * weight) / total_weight;
                if ul_conf > 0 {
                    proportional.min(ul_conf)
                } else {
                    proportional
                }
            };
            if let Some(bucket) = uls.get(id) {
                bucket.set_rate(ul_rate);
            }
        }
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

#[cfg(test)]
#[path = "logic_tests.rs"]
mod tests;
