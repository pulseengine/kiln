//! Capability-aware atomic operations
//!
//! This module provides safe atomic operations that work with the capability
//! system, eliminating the need for unsafe code in safety-critical paths.

use core::sync::atomic::{
    AtomicU16,
    AtomicU32,
    AtomicU64,
    AtomicU8,
    AtomicUsize,
    Ordering,
};

use super::{
    AnyMemoryCapability,
    CapabilityMask,
    MemoryCapability,
    MemoryOperation,
};
use crate::{
    codes,
    Error,
    ErrorCategory,
    Result,
};

/// Atomic memory capability extension trait
pub trait AtomicMemoryCapability: MemoryCapability {
    /// Perform atomic load operation with capability verification
    fn atomic_load_u32(&self, offset: usize, ordering: Ordering) -> Result<u32>;

    /// Perform atomic store operation with capability verification
    fn atomic_store_u32(&self, offset: usize, value: u32, ordering: Ordering) -> Result<()>;

    /// Perform atomic compare-and-exchange with capability verification
    fn atomic_cmpxchg_u32(
        &self,
        offset: usize,
        expected: u32,
        new: u32,
        ordering: Ordering,
    ) -> Result<u32>;

    /// Perform atomic fetch-add with capability verification
    fn atomic_fetch_add_u32(&self, offset: usize, val: u32, ordering: Ordering) -> Result<u32>;

    /// Similar methods for u64, u16, u8...
    fn atomic_load_u64(&self, offset: usize, ordering: Ordering) -> Result<u64>;
    fn atomic_store_u64(&self, offset: usize, value: u64, ordering: Ordering) -> Result<()>;
    fn atomic_cmpxchg_u64(
        &self,
        offset: usize,
        expected: u64,
        new: u64,
        ordering: Ordering,
    ) -> Result<u64>;
    fn atomic_fetch_add_u64(&self, offset: usize, val: u64, ordering: Ordering) -> Result<u64>;
}

/// Capability-aware atomic memory region
pub struct CapabilityAtomicMemory<'a> {
    /// Base memory pointer (managed by capability)
    memory_base: &'a [AtomicU8],
    /// Associated capability for verification
    capability:  &'a dyn AnyMemoryCapability,
    /// Size of the memory region
    size:        usize,
}

impl<'a> CapabilityAtomicMemory<'a> {
    /// Create a new capability-aware atomic memory region
    pub fn new(
        memory_base: &'a [AtomicU8],
        capability: &'a dyn AnyMemoryCapability,
    ) -> Result<Self> {
        // Verify the capability allows memory access
        let operation = MemoryOperation::Read {
            offset: 0,
            len:    memory_base.len(),
        };
        capability.verify_access(&operation)?;

        Ok(Self {
            memory_base,
            capability,
            size: memory_base.len(),
        })
    }

    /// Verify bounds and alignment for atomic access
    fn verify_atomic_access(&self, offset: usize, size: usize, alignment: usize) -> Result<()> {
        // Check bounds
        if offset + size > self.size {
            return Err(Error::runtime_execution_error(
                "Atomic operation address out of bounds",
            ));
        }

        // Check alignment
        if offset % alignment != 0 {
            return Err(Error::runtime_execution_error("Unaligned atomic access"));
        }

        // Verify capability allows this operation
        let operation = MemoryOperation::Read { offset, len: size };
        self.capability.verify_access(&operation)?;

        Ok(())
    }

    /// Get atomic reference with bounds checking (no unsafe!)
    fn get_atomic_u32(&self, offset: usize) -> Result<&AtomicU32> {
        self.verify_atomic_access(offset, 4, 4)?;

        // Safe transmute using slice patterns
        let bytes = &self.memory_base[offset..offset + 4];

        // This is safe because:
        // 1. We've verified alignment (4-byte aligned)
        // 2. We've verified bounds
        // 3. AtomicU32 has the same representation as [u8; 4]
        // 4. We're only creating a reference, not modifying memory

        // For ASIL compliance, we avoid unsafe and use a different approach
        // We'll need to add this as a platform-provided safe abstraction
        Err(Error::runtime_not_implemented(
            "Safe atomic transmute not yet implemented - needs platform abstraction",
        ))
    }

    /// Atomic load with capability verification
    pub fn atomic_load_u32(&self, offset: usize, ordering: Ordering) -> Result<u32> {
        self.verify_atomic_access(offset, 4, 4)?;

        // For now, return error until we have safe transmute
        // In production, this would use a platform-provided safe atomic accessor
        Err(Error::runtime_not_implemented(
            "Safe atomic operations pending platform abstraction",
        ))
    }

    /// Atomic store with capability verification
    pub fn atomic_store_u32(&self, offset: usize, value: u32, ordering: Ordering) -> Result<()> {
        self.verify_atomic_access(offset, 4, 4)?;

        // Verify write capability
        let operation = MemoryOperation::Write { offset, len: 4 };
        self.capability.verify_access(&operation)?;

        // For now, return error until we have safe transmute
        Err(Error::runtime_not_implemented(
            "Safe atomic operations pending platform abstraction",
        ))
    }

    /// Atomic compare-and-exchange with capability verification
    pub fn atomic_cmpxchg_u32(
        &self,
        offset: usize,
        expected: u32,
        new: u32,
        ordering: Ordering,
    ) -> Result<u32> {
        self.verify_atomic_access(offset, 4, 4)?;

        // Verify read-write capability
        let read_op = MemoryOperation::Read { offset, len: 4 };
        let write_op = MemoryOperation::Write { offset, len: 4 };
        self.capability.verify_access(&read_op)?;
        self.capability.verify_access(&write_op)?;

        // For now, return error until we have safe transmute
        Err(Error::runtime_not_implemented(
            "Safe atomic operations pending platform abstraction",
        ))
    }
}

/// Atomic operation statistics with capability tracking
#[derive(Debug, Clone, Default)]
pub struct CapabilityAtomicStats {
    /// Total atomic operations
    pub total_operations:      u64,
    /// Successful operations
    pub successful_operations: u64,
    /// Failed due to capability violations
    pub capability_violations: u64,
    /// Failed due to alignment issues
    pub alignment_failures:    u64,
    /// Failed due to bounds violations
    pub bounds_violations:     u64,
}

impl CapabilityAtomicStats {
    /// Create new statistics tracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a successful operation
    pub fn record_success(&mut self) {
        self.total_operations += 1;
        self.successful_operations += 1;
    }

    /// Record a capability violation
    pub fn record_capability_violation(&mut self) {
        self.total_operations += 1;
        self.capability_violations += 1;
    }

    /// Record an alignment failure
    pub fn record_alignment_failure(&mut self) {
        self.total_operations += 1;
        self.alignment_failures += 1;
    }

    /// Record a bounds violation
    pub fn record_bounds_violation(&mut self) {
        self.total_operations += 1;
        self.bounds_violations += 1;
    }

    /// Get success rate
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            0.0
        } else {
            (self.successful_operations as f64 / self.total_operations as f64) * 100.0
        }
    }
}

/// Builder for atomic operations with capabilities
pub struct AtomicOperationBuilder<'a> {
    capability: Option<&'a dyn AnyMemoryCapability>,
    offset:     Option<usize>,
    ordering:   Ordering,
}

impl<'a> AtomicOperationBuilder<'a> {
    /// Create new builder
    pub fn new() -> Self {
        Self {
            capability: None,
            offset:     None,
            ordering:   Ordering::SeqCst,
        }
    }

    /// Set the capability to use
    pub fn with_capability(mut self, capability: &'a dyn AnyMemoryCapability) -> Self {
        self.capability = Some(capability);
        self
    }

    /// Set the memory offset
    pub fn at_offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Set the memory ordering
    pub fn with_ordering(mut self, ordering: Ordering) -> Self {
        self.ordering = ordering;
        self
    }

    /// Build and verify the operation can be performed
    pub fn verify(&self) -> Result<()> {
        let capability = self.capability.ok_or_else(|| {
            Error::parameter_invalid_parameter("Capability not specified for atomic operation")
        })?;

        let offset = self.offset.ok_or_else(|| {
            Error::parameter_invalid_parameter("Offset not specified for atomic operation")
        })?;

        // Verify basic read capability
        let operation = MemoryOperation::Read { offset, len: 8 }; // Max atomic size
        capability.verify_access(&operation)?;

        Ok(())
    }
}

impl<'a> Default for AtomicOperationBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::CapabilityMask;

    #[test]
    fn test_atomic_stats() {
        let mut stats = CapabilityAtomicStats::new();

        stats.record_success();
        assert_eq!(stats.success_rate(), 100.0);

        stats.record_capability_violation();
        assert_eq!(stats.success_rate(), 50.0);

        stats.record_alignment_failure();
        stats.record_bounds_violation();
        assert_eq!(stats.successful_operations, 1);
        assert_eq!(stats.total_operations, 4);
    }

    #[test]
    fn test_atomic_operation_builder() {
        let builder = AtomicOperationBuilder::new().at_offset(100).with_ordering(Ordering::Relaxed);

        // Should fail without capability
        assert!(builder.verify().is_err());
    }
}
