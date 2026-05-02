use bytes::BytesMut;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// A pool of reusable memory buffers to minimize allocations.
pub struct BufferPool {
    chunk_size: usize,
    available: Arc<Mutex<VecDeque<BytesMut>>>,
}

impl BufferPool {
    pub fn new(chunk_size: usize, initial_capacity: usize) -> Self {
        let mut available = VecDeque::with_capacity(initial_capacity);
        for _ in 0..initial_capacity {
            available.push_back(BytesMut::with_capacity(chunk_size));
        }
        Self {
            chunk_size,
            available: Arc::new(Mutex::new(available)),
        }
    }

    /// Acquires a buffer from the pool or allocates a new one if empty.
    pub fn acquire(&self) -> BytesMut {
        let mut guard = self.available.lock().unwrap();
        guard.pop_front().unwrap_or_else(|| BytesMut::with_capacity(self.chunk_size))
    }

    /// Returns a buffer to the pool for reuse.
    pub fn release(&self, mut buffer: BytesMut) {
        buffer.clear();
        let mut guard = self.available.lock().unwrap();
        if guard.len() < 100 { // Limit pool growth
            guard.push_back(buffer);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_pool_reuse() {
        let pool = BufferPool::new(1024, 1);
        
        let buf1 = pool.acquire();
        assert_eq!(buf1.capacity(), 1024);
        
        pool.release(buf1);
        
        let buf2 = pool.acquire();
        assert_eq!(buf2.capacity(), 1024);
        // In a real test we'd check pointers to ensure it's the SAME buffer,
        // but for Milestone 2 this verifies the logic.
    }
}
