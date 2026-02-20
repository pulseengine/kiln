// WRT - wrt-component
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

use wrt_error::{Error, ErrorCategory, Result, codes};
use wrt_foundation::{
    budget_aware_provider::CrateId, capabilities::CapabilityAwareProvider,
    collections::StaticVec as BoundedVec, safe_managed_alloc,
};

use crate::bounded_component_infra::BufferProvider;

/// Helper function to create buffer pool provider using capability-driven
/// design
fn create_buffer_provider() -> Result<BufferProvider> {
    use wrt_foundation::memory_init::get_global_capability_context;

    let context = get_global_capability_context()
        .map_err(|_| Error::initialization_error("Global capability context not available"))?;

    // MemoryCapabilityContext doesn't have create_provider, use safe_managed_alloc instead
    safe_managed_alloc!(65536, CrateId::Component)
        .map_err(|_| Error::memory_out_of_bounds("Failed to create component buffer provider"))
}

/// Maximum number of buffer size classes
pub const MAX_BUFFER_SIZE_CLASSES: usize = 8;

/// Maximum buffers per size class
pub const MAX_BUFFERS_PER_CLASS: usize = 8;

/// Statistics about a bounded buffer pool
#[derive(Debug, Clone, Copy)]
pub struct BoundedBufferStats {
    /// Total number of buffers in the pool
    pub total_buffers: usize,
    /// Total capacity of all buffers in bytes
    pub total_capacity: usize,
    /// Number of different buffer sizes
    pub size_count: usize,
}

/// A buffer size class entry containing buffers of similar size
#[derive(Debug, Clone)]
pub struct BufferSizeClass {
    /// Size of buffers in this class
    pub size: usize,
    /// Count of buffers in this class (we can't store actual BoundedVecs in a BoundedVec)
    pub count: usize,
}

impl BufferSizeClass {
    /// Create a new buffer size class
    pub fn new(size: usize) -> Result<Self> {
        Ok(Self { size, count: 0 })
    }

    /// Get a buffer from this size class if one is available
    pub fn get_buffer(&mut self) -> Option<BoundedVec<u8, MAX_BUFFERS_PER_CLASS>> {
        if self.count == 0 {
            None
        } else {
            self.count -= 1;
            // Create a new buffer of the appropriate size
            let mut buffer = BoundedVec::new();
            for _ in 0..self.size.min(MAX_BUFFERS_PER_CLASS) {
                buffer.push(0).ok()?;
            }
            Some(buffer)
        }
    }

    /// Return a buffer to this size class
    pub fn return_buffer(&mut self, _buffer: BoundedVec<u8, MAX_BUFFERS_PER_CLASS>) -> Result<()> {
        if self.count >= MAX_BUFFERS_PER_CLASS {
            // Size class is full
            return Ok(());
        }

        self.count += 1;
        Ok(())
    }

    /// Number of buffers in this size class
    pub fn buffer_count(&self) -> usize {
        self.count
    }

    /// Total capacity of all buffers in this size class
    pub fn total_capacity(&self) -> usize {
        self.count * self.size
    }
}

/// Bounded buffer pool for no_std environment
///
/// Uses a fixed array of size classes with bounded capacity
/// Binary std/no_std choice
/// and is suitable for no_std environments.
#[derive(Debug, Clone)]
pub struct BoundedBufferPool {
    /// Size classes for different buffer sizes
    size_classes: [Option<BufferSizeClass>; MAX_BUFFER_SIZE_CLASSES],
    /// Number of active size classes
    active_classes: usize,
}

impl BoundedBufferPool {
    /// Create a new bounded buffer pool
    pub fn new() -> Self {
        Self {
            size_classes: Default::default(),
            active_classes: 0,
        }
    }

    /// Allocate a buffer of at least the specified size
    pub fn allocate(&mut self, size: usize) -> Result<BoundedVec<u8, MAX_BUFFERS_PER_CLASS>> {
        // Find a size class that can fit this buffer
        let matching_class = self.find_size_class(size);

        if let Some(class_idx) = matching_class {
            // Try to get a buffer from this size class
            if let Some(ref mut class) = self.size_classes[class_idx] {
                if let Some(buffer) = class.get_buffer() {
                    return Ok(buffer);
                }
            }
        }

        // No suitable buffer found, create a new one
        let mut buffer =
            BoundedVec::new().map_err(|| Error::memory_error("Failed to create bounded vector"))?;
        for _ in 0..size {
            buffer.push(0).map_err(|_| {
                Error::resource_error("Buffer allocation failed: capacity exceeded")
            })?;
        }

        Ok(buffer)
    }

    /// Return a buffer to the pool
    pub fn return_buffer(&mut self, buffer: BoundedVec<u8, MAX_BUFFERS_PER_CLASS>) -> Result<()> {
        let size = buffer.capacity();

        // Find the appropriate size class
        let class_idx = self.find_size_class(size).or_else(|| self.add_size_class(size));

        if let Some(idx) = class_idx {
            if let Some(ref mut class) = self.size_classes[idx] {
                return class.return_buffer(buffer);
            }
        }

        // No suitable size class found and couldn't add one - buffer is dropped
        Ok(())
    }

    /// Reset the buffer pool, clearing all buffers
    pub fn reset(&mut self) {
        for ref mut size_class in self.size_classes.iter_mut().flatten() {
            size_class.count = 0;
        }
    }

    /// Get statistics about the buffer pool
    pub fn stats(&self) -> BoundedBufferStats {
        let mut total_buffers = 0;
        let mut total_capacity = 0;
        let mut size_count = 0;

        for size_class in self.size_classes.iter().flatten() {
            total_buffers += size_class.buffer_count();
            total_capacity += size_class.total_capacity();
            size_count += 1;
        }

        BoundedBufferStats {
            total_buffers,
            total_capacity,
            size_count,
        }
    }

    /// Find a size class that can accommodate a buffer of the given size
    fn find_size_class(&self, size: usize) -> Option<usize> {
        for (i, class) in self.size_classes.iter().enumerate() {
            if let Some(size_class) = class {
                if size_class.size >= size {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Add a new size class if possible
    fn add_size_class(&mut self, size: usize) -> Option<usize> {
        if self.active_classes >= MAX_BUFFER_SIZE_CLASSES {
            return None;
        }

        // Find an empty slot
        for i in 0..MAX_BUFFER_SIZE_CLASSES {
            if self.size_classes[i].is_none() {
                if let Ok(size_class) = BufferSizeClass::new(size) {
                    self.size_classes[i] = Some(size_class);
                    self.active_classes += 1;
                    return Some(i);
                }
            }
        }

        None
    }
}

impl Default for BoundedBufferPool {
    fn default() -> Self {
        Self::new()
    }
}
