use crate::TenantId;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

/// Monitors and governs memory allocations dynamically to prevent OOM errors.
#[derive(Debug)]
pub struct ResourceGovernor {
    limit: usize,
    safety_margin: usize,
    current_allocated: AtomicUsize,
    tenant_allocations: Mutex<HashMap<TenantId, usize>>,
}

impl ResourceGovernor {
    /// Creates a new ResourceGovernor with the specified memory limit and safety margin in bytes.
    /// A limit of 0 denotes unlimited memory allocations.
    pub fn new(limit: usize, safety_margin: usize) -> Self {
        Self {
            limit,
            safety_margin,
            current_allocated: AtomicUsize::new(0),
            tenant_allocations: Mutex::new(HashMap::new()),
        }
    }

    /// Attempts to reserve memory. Returns true if within limit, false if budget exceeded.
    pub fn request_allocation(
        &self,
        tenant_id: &Option<TenantId>,
        requested_bytes: usize,
        is_metadata: bool,
    ) -> bool {
        if self.limit == 0 {
            return true;
        }
        let mut allocs = self.tenant_allocations.lock().unwrap();
        let current = self.current_allocated.load(Ordering::Relaxed);

        // Standard piece downloads are rejected if within safety margin
        let effective_limit = if is_metadata {
            self.limit
        } else {
            self.limit.saturating_sub(self.safety_margin)
        };

        if current + requested_bytes > effective_limit {
            return false;
        }

        // Fair-share limit checks for standard downloads when other tenants are active
        if !is_metadata {
            if let Some(ref tid) = tenant_id {
                let active_tenants = allocs.iter().filter(|(k, &v)| v > 0 && *k != tid).count();
                let current_tenant_usage = allocs.get(tid).copied().unwrap_or(0);
                if active_tenants > 0 {
                    let total_active = active_tenants + 1;
                    let fair_share = self.limit / total_active;
                    if current_tenant_usage + requested_bytes > fair_share {
                        return false;
                    }
                }
            }
        }

        if let Some(ref tid) = tenant_id {
            let entry = allocs.entry(tid.clone()).or_insert(0);
            *entry += requested_bytes;
        }

        self.current_allocated
            .fetch_add(requested_bytes, Ordering::SeqCst);
        true
    }

    /// Releases memory previously reserved.
    pub fn release_allocation(&self, tenant_id: &Option<TenantId>, released_bytes: usize) {
        if self.limit == 0 {
            return;
        }
        let mut allocs = self.tenant_allocations.lock().unwrap();
        if let Some(ref tid) = tenant_id {
            if let Some(val) = allocs.get_mut(tid) {
                if *val >= released_bytes {
                    *val -= released_bytes;
                } else {
                    *val = 0;
                }
            }
        }

        let current = self.current_allocated.load(Ordering::Relaxed);
        if current >= released_bytes {
            self.current_allocated
                .fetch_sub(released_bytes, Ordering::SeqCst);
        } else {
            self.current_allocated.store(0, Ordering::SeqCst);
        }
    }

    /// Returns the current global memory allocation in bytes.
    pub fn current_usage(&self) -> usize {
        self.current_allocated.load(Ordering::Relaxed)
    }

    /// Returns the configured global memory limit in bytes.
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Returns the memory usage of a specific tenant.
    pub fn tenant_usage(&self, tenant_id: &TenantId) -> usize {
        let allocs = self.tenant_allocations.lock().unwrap();
        allocs.get(tenant_id).copied().unwrap_or(0)
    }
}

/// An RAII guard that automatically releases memory when dropped.
#[derive(Debug, Clone)]
pub struct MemoryGuard {
    governor: Arc<ResourceGovernor>,
    tenant_id: Option<TenantId>,
    size: usize,
}

impl MemoryGuard {
    pub fn new(governor: Arc<ResourceGovernor>, tenant_id: Option<TenantId>, size: usize) -> Self {
        Self {
            governor,
            tenant_id,
            size,
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }
}

impl Drop for MemoryGuard {
    fn drop(&mut self) {
        self.governor.release_allocation(&self.tenant_id, self.size);
    }
}

#[cfg(test)]
#[path = "resource_governor_tests.rs"]
mod tests;
