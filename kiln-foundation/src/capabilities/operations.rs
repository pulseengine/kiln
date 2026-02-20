//! Standard Memory Operations for Capability System
//!
//! This module defines standard memory operations that work with the
//! capability system to provide safe, verified memory access.

use core::fmt;

use super::{
    context::{
        CapabilityGuardedProvider,
        MemoryCapabilityContext,
    },
    CapabilityMask,
    MemoryOperation,
    MemoryOperationType,
};
use crate::{
    budget_aware_provider::CrateId,
    codes,
    Error,
    ErrorCategory,
    Result,
};

/// Standard memory operations that can be performed with capability
/// verification
pub struct CapabilityMemoryOps;

impl CapabilityMemoryOps {
    /// Allocate memory with capability verification
    ///
    /// This is the capability-aware replacement for safe_managed_alloc!
    pub fn allocate<const N: usize>(
        _context: &MemoryCapabilityContext,
        _crate_id: CrateId,
    ) -> Result<CapabilityGuardedProvider<N>> {
        // TODO: Update to use new factory pattern
        Err(Error::initialization_not_implemented(
            "Capability operations are being refactored",
        ))
    }

    /// Read memory with capability verification
    pub fn read_memory(
        context: &MemoryCapabilityContext,
        crate_id: CrateId,
        offset: usize,
        len: usize,
    ) -> Result<()> {
        let operation = MemoryOperation::Read { offset, len };
        context.verify_operation(crate_id, &operation)
    }

    /// Write memory with capability verification
    pub fn write_memory(
        context: &MemoryCapabilityContext,
        crate_id: CrateId,
        offset: usize,
        data_len: usize,
    ) -> Result<()> {
        let operation = MemoryOperation::Write {
            offset,
            len: data_len,
        };
        context.verify_operation(crate_id, &operation)
    }

    /// Check if a crate can perform a specific operation
    pub fn can_perform_operation(
        context: &MemoryCapabilityContext,
        crate_id: CrateId,
        operation: &MemoryOperation,
    ) -> bool {
        context.verify_operation(crate_id, operation).is_ok()
    }

    /// Get the maximum allocation size for a crate
    pub fn max_allocation_size(
        context: &MemoryCapabilityContext,
        crate_id: CrateId,
    ) -> Result<usize> {
        let capability = context.get_capability(crate_id)?;
        Ok(capability.max_allocation_size())
    }
}

/// Capability-aware memory allocation function
///
/// This replaces the old safe_managed_alloc! macro with capability verification
pub fn capability_managed_alloc<const N: usize>(
    context: &super::MemoryCapabilityContext,
    crate_id: crate::budget_aware_provider::CrateId,
) -> crate::Result<super::context::CapabilityGuardedProvider<N>> {
    CapabilityMemoryOps::allocate::<N>(context, crate_id)
}

/// Memory operation builder for complex operations
pub struct MemoryOperationBuilder {
    operation_type: Option<MemoryOperationType>,
    offset:         Option<usize>,
    length:         Option<usize>,
    size:           Option<usize>,
    mask:           Option<CapabilityMask>,
}

impl MemoryOperationBuilder {
    /// Create a new operation builder
    pub fn new() -> Self {
        Self {
            operation_type: None,
            offset:         None,
            length:         None,
            size:           None,
            mask:           None,
        }
    }

    /// Set this as a read operation
    pub fn read(mut self, offset: usize, len: usize) -> Self {
        self.operation_type = Some(MemoryOperationType::Read);
        self.offset = Some(offset);
        self.length = Some(len);
        self
    }

    /// Set this as a write operation
    pub fn write(mut self, offset: usize, len: usize) -> Self {
        self.operation_type = Some(MemoryOperationType::Write);
        self.offset = Some(offset);
        self.length = Some(len);
        self
    }

    /// Set this as an allocation operation
    pub fn allocate(mut self, size: usize) -> Self {
        self.operation_type = Some(MemoryOperationType::Allocate);
        self.size = Some(size);
        self
    }

    /// Set this as a deallocation operation
    pub fn deallocate(mut self) -> Self {
        self.operation_type = Some(MemoryOperationType::Deallocate);
        self
    }

    /// Set this as a delegation operation
    pub fn delegate(mut self, mask: CapabilityMask) -> Self {
        self.operation_type = Some(MemoryOperationType::Delegate);
        self.mask = Some(mask);
        self
    }

    /// Build the memory operation
    pub fn build(self) -> Result<MemoryOperation> {
        match self.operation_type {
            Some(MemoryOperationType::Read) => {
                let offset = self.offset.ok_or_else(|| {
                    Error::parameter_invalid_parameter("Read operation requires offset")
                })?;
                let len = self.length.ok_or_else(|| {
                    Error::parameter_invalid_parameter("Read operation requires length")
                })?;
                Ok(MemoryOperation::Read { offset, len })
            },
            Some(MemoryOperationType::Write) => {
                let offset = self.offset.ok_or_else(|| {
                    Error::parameter_invalid_parameter("Write operation requires offset")
                })?;
                let len = self.length.ok_or_else(|| {
                    Error::parameter_invalid_parameter("Write operation requires length")
                })?;
                Ok(MemoryOperation::Write { offset, len })
            },
            Some(MemoryOperationType::Allocate) => {
                let size = self.size.ok_or_else(|| {
                    Error::parameter_invalid_parameter("Allocate operation requires size")
                })?;
                Ok(MemoryOperation::Allocate { size })
            },
            Some(MemoryOperationType::Deallocate) => Ok(MemoryOperation::Deallocate),
            Some(MemoryOperationType::Delegate) => {
                let subset = self.mask.ok_or_else(|| {
                    Error::parameter_invalid_parameter(
                        "Delegate operation requires capability mask",
                    )
                })?;
                Ok(MemoryOperation::Delegate { subset })
            },
            None => Err(Error::parameter_invalid_parameter(
                "Operation type not specified",
            )),
        }
    }
}

impl Default for MemoryOperationBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for MemoryOperationBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoryOperationBuilder")
            .field("operation_type", &self.operation_type)
            .field("offset", &self.offset)
            .field("length", &self.length)
            .field("size", &self.size)
            .finish()
    }
}

/// Capability verification utilities
pub struct CapabilityVerifier;

impl CapabilityVerifier {
    /// Verify that a capability mask allows a specific operation
    pub fn verify_mask_allows_operation(
        mask: &CapabilityMask,
        operation: &MemoryOperation,
    ) -> Result<()> {
        if !operation.requires_capability(mask) {
            return Err(Error::capability_violation(
                "Capability mask does not allow operation",
            ));
        }
        Ok(())
    }

    /// Check if two capability masks are compatible for delegation
    pub fn are_masks_compatible(parent: &CapabilityMask, child: &CapabilityMask) -> bool {
        // Child mask should be a subset of parent mask
        (!child.read || parent.read)
            && (!child.write || parent.write)
            && (!child.allocate || parent.allocate)
            && (!child.deallocate || parent.deallocate)
            && (!child.delegate || parent.delegate)
            && (child.max_size == 0 || parent.max_size == 0 || child.max_size <= parent.max_size)
    }

    /// Create a safe intersection of two capability masks
    pub fn intersect_masks(mask1: &CapabilityMask, mask2: &CapabilityMask) -> CapabilityMask {
        mask1.intersect(mask2)
    }
}

/// Memory statistics for capability monitoring
#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityMemoryStats {
    /// Total number of operations performed
    pub total_operations:        u64,
    /// Number of read operations
    pub read_operations:         u64,
    /// Number of write operations
    pub write_operations:        u64,
    /// Number of allocation operations
    pub allocation_operations:   u64,
    /// Number of deallocation operations
    pub deallocation_operations: u64,
    /// Number of delegation operations
    pub delegation_operations:   u64,
    /// Number of capability violations
    pub violations:              u64,
}

impl CapabilityMemoryStats {
    /// Create new empty statistics
    pub fn new() -> Self {
        Self {
            total_operations:        0,
            read_operations:         0,
            write_operations:        0,
            allocation_operations:   0,
            deallocation_operations: 0,
            delegation_operations:   0,
            violations:              0,
        }
    }

    /// Record an operation
    pub fn record_operation(&mut self, operation: &MemoryOperation) {
        self.total_operations += 1;
        match operation {
            MemoryOperation::Read { .. } => self.read_operations += 1,
            MemoryOperation::Write { .. } => self.write_operations += 1,
            MemoryOperation::Allocate { .. } => self.allocation_operations += 1,
            MemoryOperation::Deallocate => self.deallocation_operations += 1,
            MemoryOperation::Delegate { .. } => self.delegation_operations += 1,
        }
    }

    /// Record a capability violation
    pub fn record_violation(&mut self) {
        self.violations += 1;
    }

    /// Get the violation rate as a percentage
    pub fn violation_rate(&self) -> f64 {
        if self.total_operations == 0 {
            0.0
        } else {
            (self.violations as f64 / self.total_operations as f64) * 100.0
        }
    }
}

impl Default for CapabilityMemoryStats {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::context::MemoryCapabilityContext;

    #[test]
    fn test_memory_operation_builder() {
        let operation = MemoryOperationBuilder::new().read(100, 50).build().unwrap();

        match operation {
            MemoryOperation::Read { offset, len } => {
                assert_eq!(offset, 100);
                assert_eq!(len, 50);
            },
            _ => panic!("Expected read operation"),
        }
    }

    #[test]
    fn test_capability_verifier() {
        let full_mask = CapabilityMask::all();
        let read_only_mask = CapabilityMask::read_only();

        assert!(CapabilityVerifier::are_masks_compatible(
            &full_mask,
            &read_only_mask
        ));
        assert!(!CapabilityVerifier::are_masks_compatible(
            &read_only_mask,
            &full_mask
        ));
    }

    #[test]
    fn test_memory_stats() {
        let mut stats = CapabilityMemoryStats::new();

        let read_op = MemoryOperation::Read {
            offset: 0,
            len:    100,
        };
        stats.record_operation(&read_op);

        assert_eq!(stats.total_operations, 1);
        assert_eq!(stats.read_operations, 1);
        assert_eq!(stats.violation_rate(), 0.0);

        stats.record_violation();
        assert_eq!(stats.violation_rate(), 100.0);
    }

    #[test]
    fn test_mask_intersection() {
        let mask1 = CapabilityMask::all();
        let mask2 = CapabilityMask::read_only();

        let intersection = CapabilityVerifier::intersect_masks(&mask1, &mask2);

        assert!(intersection.read);
        assert!(!intersection.write);
        assert!(!intersection.allocate);
    }
}
