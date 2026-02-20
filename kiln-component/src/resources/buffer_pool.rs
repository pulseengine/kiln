// Use BTreeMap for all cases to ensure deterministic ordering and no_std
// compatibility
#[cfg(feature = "std")]
use std::{collections::BTreeMap, vec::Vec};
#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, vec::Vec};

/// Binary std/no_std choice
#[cfg(feature = "std")]
pub struct BufferPool {
    /// Map of buffer sizes to pools of buffers
    pools: BTreeMap<usize, Vec<Vec<u8>>>,
    /// Maximum buffer size to keep in the pool
    max_buffer_size: usize,
    /// Maximum number of buffers per size
    max_buffers_per_size: usize,
}

/// A simplified buffer pool for no_std environments
#[cfg(not(feature = "std"))]
pub struct BufferPool {
    /// Simplified buffer management for no_std
    max_buffer_size: usize,
    max_buffers_per_size: usize,
}

#[cfg(feature = "std")]
impl BufferPool {
    /// Create a new buffer pool with default settings
    pub fn new() -> Self {
        Self {
            pools: BTreeMap::new(),
            max_buffer_size: 1024 * 1024, // 1MB default max size
            max_buffers_per_size: 10,
        }
    }

    /// Create a new buffer pool with custom max buffer size
    pub fn new_with_config(max_buffer_size: usize, max_buffers_per_size: usize) -> Self {
        Self {
            pools: BTreeMap::new(),
            max_buffer_size,
            max_buffers_per_size,
        }
    }

    /// Allocate a buffer of at least the specified size
    pub fn allocate(&mut self, min_size: usize) -> Vec<u8> {
        // Try to find a buffer of the right size
        if let Some(buffers) = self.pools.get_mut(&min_size) {
            if let Some(buffer) = buffers.pop() {
                return buffer;
            }
        }

        // No buffer available, create a new one
        Vec::with_capacity(min_size)
    }

    /// Return a buffer to the pool
    pub fn return_buffer(&mut self, mut buffer: Vec<u8>) {
        let size = buffer.capacity();

        // Only keep reasonably sized buffers
        if size <= self.max_buffer_size {
            // Clear the buffer before returning it to the pool
            buffer.clear();

            // Add to the pool if we have space
            let buffers = self.pools.entry(size).or_insert_with(Vec::new);
            if buffers.len() < self.max_buffers_per_size {
                buffers.push(buffer);
            }
        }
    }

    /// Reset the buffer pool, clearing all pooled buffers
    pub fn reset(&mut self) {
        self.pools.clear();
    }

    /// Get stats about the buffer pool
    pub fn stats(&self) -> BufferPoolStats {
        let mut total_buffers = 0;
        let mut total_capacity = 0;

        for (size, buffers) in &self.pools {
            total_buffers += buffers.len();
            total_capacity += size * buffers.len();
        }

        BufferPoolStats {
            total_buffers,
            total_capacity,
            size_count: self.pools.len(),
        }
    }
}

#[cfg(not(feature = "std"))]
impl BufferPool {
    /// Create a new buffer pool with default settings
    pub fn new() -> Self {
        Self {
            max_buffer_size: 1024,   // 1KB default max size for no_std
            max_buffers_per_size: 2, // Reduced for no_std
        }
    }

    /// Create a new buffer pool with custom max buffer size
    pub fn new_with_config(max_buffer_size: usize, max_buffers_per_size: usize) -> Self {
        Self {
            max_buffer_size,
            max_buffers_per_size,
        }
    }

    /// Allocate a buffer of at least the specified size (simplified for no_std)
    pub fn allocate(&mut self, min_size: usize) -> [u8; 64] {
        // In no_std mode, return a fixed-size buffer
        [0u8; 64]
    }

    /// Return a buffer to the pool (no-op in no_std mode)
    pub fn deallocate(&mut self, _buffer: [u8; 64]) {
        // No-op in no_std mode
    }

    /// Get statistics about the buffer pool (simplified for no_std)
    pub fn stats(&self) -> BufferPoolStats {
        BufferPoolStats {
            total_buffers: 0,
            total_capacity: 0,
            size_count: 0,
        }
    }
}

/// Statistics about a buffer pool
pub struct BufferPoolStats {
    /// Total number of buffers in the pool
    pub total_buffers: usize,
    /// Total capacity of all buffers in bytes
    pub total_capacity: usize,
    /// Number of different buffer sizes
    pub size_count: usize,
}
