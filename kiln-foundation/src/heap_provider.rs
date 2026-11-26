//! Heap-based memory provider to avoid stack overflow
//!
//! This module provides a heap-allocated alternative to NoStdProvider
//! that prevents stack overflow for large allocations.

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
#[cfg(feature = "std")]
use std::vec::Vec;

use kiln_error::{Error, Result};

use crate::{
    budget_aware_provider::CrateId,
    safe_memory::{Allocator, Provider, Slice, SliceMut, Stats},
    verification::VerificationLevel,
};

/// A heap-based memory provider that avoids stack allocation
pub struct HeapProvider {
    /// The written prefix of the buffer (heap-allocated). It grows on write
    /// up to `capacity` and always covers `used` bytes (SR-129).
    data: Vec<u8>,
    /// The most bytes this provider may hold
    capacity: usize,
    /// Current usage of the buffer
    used: usize,
    /// Counter for access operations
    access_count: AtomicUsize,
    /// Last access offset for validation
    last_access_offset: AtomicUsize,
    /// Last access length for validation
    last_access_length: AtomicUsize,
    /// Verification level for runtime checks
    verification_level: VerificationLevel,
}

impl HeapProvider {
    /// Create a new heap provider with specified capacity
    pub fn new(capacity: usize) -> Result<Self> {
        Self::new_with_verification(capacity, VerificationLevel::default())
    }

    /// Create a new heap provider with specified capacity and verification
    /// level
    pub fn new_with_verification(
        capacity: usize,
        verification_level: VerificationLevel,
    ) -> Result<Self> {
        // The buffer is allocated as it is written (grow_to), so creating
        // and cloning a provider costs what it holds, not its capacity.
        Ok(Self {
            data: Vec::new(),
            capacity,
            used: 0,
            access_count: AtomicUsize::new(0),
            last_access_offset: AtomicUsize::new(0),
            last_access_length: AtomicUsize::new(0),
            verification_level,
        })
    }

    /// Get the capacity of this provider
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Allocate the buffer up to `end` bytes (within capacity), zero-filled.
    fn grow_to(&mut self, end: usize) -> Result<()> {
        if end > self.capacity {
            return Err(Error::memory_out_of_bounds("Access out of bounds"));
        }
        if end > self.data.len() {
            self.data
                .try_reserve(end - self.data.len())
                .map_err(|_| Error::memory_error("Failed to allocate heap memory"))?;
            self.data.resize(end, 0);
        }
        Ok(())
    }
}

impl Default for HeapProvider {
    fn default() -> Self {
        // Use a reasonable default capacity of 4KB
        Self::new(4096).expect("Failed to create default HeapProvider")
    }
}

impl Provider for HeapProvider {
    type Allocator = Self;

    fn borrow_slice(&self, offset: usize, len: usize) -> Result<Slice<'_>> {
        self.verify_access(offset, len)?;
        self.access_count.fetch_add(1, Ordering::Relaxed);
        self.last_access_offset.store(offset, Ordering::Relaxed);
        self.last_access_length.store(len, Ordering::Relaxed);

        // Bytes past the written prefix were never written; reading them is
        // an error, not a silent zero (SR-129).
        let bytes = self
            .data
            .get(offset..offset + len)
            .ok_or_else(|| Error::memory_out_of_bounds("Read of bytes never written"))?;
        Slice::with_verification_level(bytes, self.verification_level)
    }

    fn write_data(&mut self, offset: usize, data: &[u8]) -> Result<()> {
        self.verify_access(offset, data.len())?;
        self.access_count.fetch_add(1, Ordering::Relaxed);
        self.last_access_offset.store(offset, Ordering::Relaxed);
        self.last_access_length.store(data.len(), Ordering::Relaxed);

        self.grow_to(offset + data.len())?;
        self.data[offset..offset + data.len()].copy_from_slice(data);
        self.used = core::cmp::max(self.used, offset + data.len());
        Ok(())
    }

    fn verify_access(&self, offset: usize, len: usize) -> Result<()> {
        if offset.checked_add(len).map_or(true, |end| end > self.capacity) {
            return Err(Error::memory_out_of_bounds("Access out of bounds"));
        }
        Ok(())
    }

    fn size(&self) -> usize {
        self.used
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn verify_integrity(&self) -> Result<()> {
        // Basic integrity check
        if self.used > self.data.len() || self.data.len() > self.capacity {
            return Err(Error::validation_error("Corrupted state: used > capacity"));
        }
        Ok(())
    }

    fn set_verification_level(&mut self, level: VerificationLevel) {
        self.verification_level = level;
    }

    fn verification_level(&self) -> VerificationLevel {
        self.verification_level
    }

    fn memory_stats(&self) -> Stats {
        Stats {
            total_size: self.capacity,
            access_count: self.access_count.load(Ordering::Relaxed),
            unique_regions: 0, // Not tracking this for simplicity
            max_access_size: self.last_access_length.load(Ordering::Relaxed),
        }
    }

    fn get_slice_mut(&mut self, offset: usize, len: usize) -> Result<SliceMut<'_>> {
        self.verify_access(offset, len)?;
        self.grow_to(offset + len)?;
        self.used = core::cmp::max(self.used, offset + len);
        SliceMut::with_verification_level(
            &mut self.data[offset..offset + len],
            self.verification_level,
        )
    }

    fn copy_within(&mut self, src_offset: usize, dst_offset: usize, len: usize) -> Result<()> {
        self.verify_access(src_offset, len)?;
        self.verify_access(dst_offset, len)?;
        if src_offset + len > self.data.len() {
            return Err(Error::memory_out_of_bounds("Read of bytes never written"));
        }
        self.grow_to(dst_offset + len)?;

        self.data.copy_within(src_offset..src_offset + len, dst_offset);
        self.used = core::cmp::max(self.used, dst_offset + len);
        Ok(())
    }

    fn ensure_used_up_to(&mut self, byte_offset: usize) -> Result<()> {
        if byte_offset > self.capacity {
            return Err(Error::memory_error("Offset exceeds capacity"));
        }
        self.grow_to(byte_offset)?;
        self.used = core::cmp::max(self.used, byte_offset);
        Ok(())
    }

    fn get_allocator(&self) -> &Self::Allocator {
        self
    }

    fn acquire_memory(&self, layout: core::alloc::Layout) -> kiln_error::Result<*mut u8> {
        self.allocate(layout)
    }

    fn release_memory(&self, ptr: *mut u8, layout: core::alloc::Layout) -> kiln_error::Result<()> {
        self.deallocate(ptr, layout)
    }

    fn new_handler(&self) -> Result<crate::safe_memory::SafeMemoryHandler<Self>>
    where
        Self: Sized + Clone,
    {
        Ok(crate::safe_memory::SafeMemoryHandler::new(self.clone()))
    }
}

impl core::fmt::Debug for HeapProvider {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HeapProvider")
            .field("capacity", &self.data.len())
            .field("used", &self.used)
            .field("access_count", &self.access_count.load(Ordering::Relaxed))
            .field("verification_level", &self.verification_level)
            .finish()
    }
}

impl Allocator for HeapProvider {
    fn allocate(&self, layout: core::alloc::Layout) -> kiln_error::Result<*mut u8> {
        // This is a simple heap provider that doesn't support raw allocation
        // It's meant to be used through the Provider interface
        Err(Error::memory_error(
            "HeapProvider does not support raw allocation",
        ))
    }

    fn deallocate(&self, _ptr: *mut u8, _layout: core::alloc::Layout) -> kiln_error::Result<()> {
        // No-op for this simple implementation
        Ok(())
    }
}

impl Clone for HeapProvider {
    fn clone(&self) -> Self {
        // Copies the written prefix only: the cost follows what the provider
        // holds, not its capacity (8 MiB for the runtime provider, SR-129).
        Self {
            data: self.data.clone(),
            capacity: self.capacity,
            used: self.used,
            access_count: AtomicUsize::new(self.access_count.load(Ordering::Relaxed)),
            last_access_offset: AtomicUsize::new(self.last_access_offset.load(Ordering::Relaxed)),
            last_access_length: AtomicUsize::new(self.last_access_length.load(Ordering::Relaxed)),
            verification_level: self.verification_level,
        }
    }
}

impl PartialEq for HeapProvider {
    fn eq(&self, other: &Self) -> bool {
        self.used == other.used
            && self.verification_level == other.verification_level
            && self.data[..self.used] == other.data[..other.used]
    }
}

impl Eq for HeapProvider {}

// HeapProvider is Send and Sync by default since:
// - Vec<u8> is Send and Sync
// - AtomicUsize is Send and Sync
// - VerificationLevel is Send and Sync
// No explicit unsafe impl needed

#[cfg(test)]
mod tests {
    use super::*;

    /// SR-129: a clone has the same capacity and written bytes, is
    /// independent of the original, and holds only what was written.
    #[test]
    fn clone_keeps_capacity_and_written_bytes() {
        let mut original = HeapProvider::new(64 * 1024).unwrap();
        original.write_data(100, &[1, 2, 3, 4]).unwrap();

        let mut copy = original.clone();
        assert_eq!(Provider::capacity(&copy), 64 * 1024);
        assert_eq!(copy.size(), 104);
        assert_eq!(
            copy.borrow_slice(100, 4).unwrap().data().unwrap(),
            &[1, 2, 3, 4]
        );
        assert!(copy.borrow_slice(0, 100).unwrap().data().unwrap().iter().all(|&b| b == 0));
        assert_eq!(copy.data.len(), 104);

        copy.write_data(0, &[9]).unwrap();
        assert_eq!(original.borrow_slice(0, 1).unwrap().data().unwrap(), &[0]);
    }

    /// Writes stay bounded by capacity; reads of bytes never written fail.
    #[test]
    fn writes_bounded_by_capacity_and_unwritten_reads_fail() {
        let mut p = HeapProvider::new(16).unwrap();
        assert!(p.write_data(12, &[0; 5]).is_err());
        p.write_data(12, &[7; 4]).unwrap();
        assert_eq!(p.size(), 16);
        assert!(p.borrow_slice(15, 2).is_err());

        let fresh = HeapProvider::new(16).unwrap();
        assert!(fresh.borrow_slice(0, 1).is_err());
        assert!(fresh.borrow_slice(0, 0).is_ok());

        let mut grown = HeapProvider::new(16).unwrap();
        grown.ensure_used_up_to(8).unwrap();
        assert_eq!(grown.borrow_slice(0, 8).unwrap().data().unwrap(), &[0; 8]);
    }
}
