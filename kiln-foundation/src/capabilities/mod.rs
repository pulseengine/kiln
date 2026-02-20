//! Capability-Driven Memory Architecture
//!
//! This module implements the core capability system that replaces global state
//! with explicit capability-based access control. Every memory operation
//! requires explicit capability verification.
//!
//! SW-REQ-ID: REQ_CAP_001 - Capability-based access control
//! SW-REQ-ID: REQ_CAP_002 - Memory operation verification
//! SW-REQ-ID: REQ_CAP_003 - Capability delegation and composition

#![allow(unsafe_code)]
// SAFETY JUSTIFICATION: This module implements capability-based memory access
// control which requires unsafe operations for:
// 1. Low-level pointer manipulation for memory region verification
// 2. Hardware-specific memory protection unit (MPU) configuration
// 3. Atomic operations for capability state management
// All unsafe code is documented and verified through KANI formal verification
// SW-REQ-ID: REQ_SAFETY_UNSAFE_JUSTIFICATION_001

use core::{
    fmt::{
        self,
        Debug,
    },
    marker::PhantomData,
    ptr::NonNull,
};

#[cfg(any(feature = "std", feature = "alloc"))]
extern crate alloc;

#[cfg(all(not(feature = "std"), feature = "alloc"))]
use alloc::boxed::Box;
#[cfg(feature = "std")]
use std::boxed::Box;

use crate::{
    budget_aware_provider::CrateId,
    codes,
    verification::VerificationLevel,
    Error,
    ErrorCategory,
    Result,
};

pub mod atomic;
pub mod context;
pub mod dynamic;
pub mod factory;
pub mod macros;
pub mod memory_factory;
pub mod operations;
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod platform_bridge;
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod provider_bridge;
pub mod static_alloc;
pub mod verified;

// Re-export key types for convenience
pub use context::{
    AnyMemoryCapability,
    MemoryCapabilityContext,
};
pub use dynamic::DynamicMemoryCapability;
// CapabilityGuardedProvider is only available with std/alloc features
#[cfg(any(feature = "std", feature = "alloc"))]
pub use factory::CapabilityGuardedProvider;
pub use memory_factory::MemoryFactory;
#[cfg(any(feature = "std", feature = "alloc"))]
pub use platform_bridge::{
    PlatformAllocator,
    PlatformCapabilityBuilder,
    PlatformCapabilityProvider,
    PlatformMemoryProvider,
};
#[cfg(any(feature = "std", feature = "alloc"))]
pub use provider_bridge::{
    CapabilityAwareProvider,
    CapabilityProviderFactory,
    ProviderCapabilityExt,
};

// For no_std environments without alloc, we provide a simpler type alias
#[cfg(not(any(feature = "std", feature = "alloc")))]
pub type CapabilityAwareProvider<P> = P;

// No-std capability context placeholder
#[cfg(not(any(feature = "std", feature = "alloc")))]
#[derive(Debug, Clone, Copy)]
pub struct NoStdCapabilityContext {
    /// The crate ID
    pub crate_id: CrateId,
    /// The size
    pub size:     usize,
}

#[cfg(not(any(feature = "std", feature = "alloc")))]
impl NoStdCapabilityContext {
    /// Check if the context has capability for the given crate
    pub fn has_capability(&self, crate_id: CrateId) -> bool {
        self.crate_id == crate_id
    }
}

pub use static_alloc::StaticMemoryCapability;
pub use verified::VerifiedMemoryCapability;

/// Core capability trait that all memory capabilities must implement
///
/// This trait defines the fundamental operations that any memory capability
/// must support. All memory access is gated through capability verification.
pub trait MemoryCapability: Send + Sync + Debug {
    /// The type of memory region this capability can create
    type Region: MemoryRegion;

    /// The type of guard that protects access to the memory region
    type Guard: MemoryGuard;

    /// Verify that this capability allows the requested memory operation
    ///
    /// # Arguments
    /// * `operation` - The memory operation to verify
    ///
    /// # Returns
    /// * `Ok(())` if the operation is allowed
    /// * `Err(Error)` if the operation violates capability constraints
    fn verify_access(&self, operation: &MemoryOperation) -> Result<()>;

    /// Allocate a memory region using this capability
    ///
    /// # Arguments
    /// * `size` - Size of memory region to allocate
    /// * `crate_id` - Identifier of the crate requesting allocation
    ///
    /// # Returns
    /// * `Ok(Guard)` containing the protected memory region
    /// * `Err(Error)` if allocation fails or violates capability limits
    fn allocate_region(&self, size: usize, crate_id: CrateId) -> Result<Self::Guard>;

    /// Create a delegated capability with subset of permissions
    ///
    /// # Arguments
    /// * `subset` - Mask defining which permissions to delegate
    ///
    /// # Returns
    /// * `Ok(Box<dyn MemoryCapability>)` with delegated permissions
    /// * `Err(Error)` if delegation is not allowed or invalid
    #[cfg(any(feature = "std", feature = "alloc"))]
    fn delegate(
        &self,
        subset: CapabilityMask,
    ) -> Result<Box<dyn MemoryCapability<Region = Self::Region, Guard = Self::Guard>>>;

    /// Get the verification level required by this capability
    fn verification_level(&self) -> VerificationLevel;

    /// Get the maximum allocation size allowed by this capability
    fn max_allocation_size(&self) -> usize;

    /// Check if this capability supports the given operation type
    fn supports_operation(&self, op_type: MemoryOperationType) -> bool;
}

/// Memory region trait defining access patterns
pub trait MemoryRegion: Send + Sync + Debug {
    /// Get a pointer to the start of the memory region
    fn as_ptr(&self) -> NonNull<u8>;

    /// Get the size of the memory region in bytes
    fn size(&self) -> usize;

    /// Check if the region contains the given offset
    fn contains_offset(&self, offset: usize) -> bool {
        offset < self.size()
    }

    /// Check if the region contains the given range
    fn contains_range(&self, offset: usize, len: usize) -> bool {
        offset.saturating_add(len) <= self.size()
    }
}

/// Memory guard trait that protects access to memory regions
pub trait MemoryGuard: Send + Sync + Debug {
    type Region: MemoryRegion;

    /// Get a reference to the protected memory region
    fn region(&self) -> &Self::Region;

    /// Read data from the protected region with capability verification
    fn read_bytes(&self, offset: usize, len: usize) -> Result<&[u8]>;

    /// Write data to the protected region with capability verification
    fn write_bytes(&mut self, offset: usize, data: &[u8]) -> Result<()>;

    /// Get the capability that guards this memory
    fn capability(&self) -> &dyn MemoryCapability<Region = Self::Region, Guard = Self>;
}

/// Capability mask defining specific permissions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityMask {
    /// Can read from memory
    pub read:       bool,
    /// Can write to memory
    pub write:      bool,
    /// Can allocate new memory
    pub allocate:   bool,
    /// Can deallocate memory
    pub deallocate: bool,
    /// Can delegate capabilities
    pub delegate:   bool,
    /// Maximum allocation size (0 = no limit from this mask)
    pub max_size:   usize,
}

impl CapabilityMask {
    /// Create a mask with all permissions
    pub const fn all() -> Self {
        Self {
            read:       true,
            write:      true,
            allocate:   true,
            deallocate: true,
            delegate:   true,
            max_size:   0,
        }
    }

    /// Create a mask with read-only permissions
    pub const fn read_only() -> Self {
        Self {
            read:       true,
            write:      false,
            allocate:   false,
            deallocate: false,
            delegate:   false,
            max_size:   0,
        }
    }

    /// Create a mask with no permissions
    pub const fn none() -> Self {
        Self {
            read:       false,
            write:      false,
            allocate:   false,
            deallocate: false,
            delegate:   false,
            max_size:   0,
        }
    }

    /// Intersect this mask with another mask
    pub fn intersect(&self, other: &Self) -> Self {
        Self {
            read:       self.read && other.read,
            write:      self.write && other.write,
            allocate:   self.allocate && other.allocate,
            deallocate: self.deallocate && other.deallocate,
            delegate:   self.delegate && other.delegate,
            max_size:   if self.max_size == 0 {
                other.max_size
            } else if other.max_size == 0 {
                self.max_size
            } else {
                core::cmp::min(self.max_size, other.max_size)
            },
        }
    }
}

/// Types of memory operations that can be performed
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryOperation {
    /// Read operation with offset and length
    Read { offset: usize, len: usize },
    /// Write operation with offset and data length
    Write { offset: usize, len: usize },
    /// Allocation operation with size
    Allocate { size: usize },
    /// Deallocation operation
    Deallocate,
    /// Capability delegation operation
    Delegate { subset: CapabilityMask },
}

/// Categories of memory operation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryOperationType {
    Read,
    Write,
    Allocate,
    Deallocate,
    Delegate,
}

impl MemoryOperation {
    /// Get the operation type for this operation
    pub fn operation_type(&self) -> MemoryOperationType {
        match self {
            MemoryOperation::Read { .. } => MemoryOperationType::Read,
            MemoryOperation::Write { .. } => MemoryOperationType::Write,
            MemoryOperation::Allocate { .. } => MemoryOperationType::Allocate,
            MemoryOperation::Deallocate => MemoryOperationType::Deallocate,
            MemoryOperation::Delegate { .. } => MemoryOperationType::Delegate,
        }
    }

    /// Check if this operation requires the given capability mask
    pub fn requires_capability(&self, mask: &CapabilityMask) -> bool {
        match self {
            MemoryOperation::Read { .. } => mask.read,
            MemoryOperation::Write { .. } => mask.write,
            MemoryOperation::Allocate { size } => {
                mask.allocate && (mask.max_size == 0 || *size <= mask.max_size)
            },
            MemoryOperation::Deallocate => mask.deallocate,
            MemoryOperation::Delegate { .. } => mask.delegate,
        }
    }
}

/// Error creation helper functions for capability violations
pub mod capability_errors {
    use super::*;

    /// Create a capability violation error
    pub fn capability_violation(message: &'static str) -> Error {
        Error::new(ErrorCategory::Security, codes::ACCESS_DENIED, message)
    }

    /// Create a capability not found error
    pub fn no_capability(_crate_id: CrateId) -> Error {
        Error::security_access_denied("No capability found for crate")
    }

    /// Create a capability delegation error
    pub fn delegation_failed(reason: &'static str) -> Error {
        Error::new(
            ErrorCategory::Security,
            codes::OPERATION_NOT_PERMITTED,
            reason,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_mask_intersection() {
        let mask1 = CapabilityMask::all();
        let mask2 = CapabilityMask::read_only();
        let result = mask1.intersect(&mask2);

        assert!(result.read);
        assert!(!result.write);
        assert!(!result.allocate);
    }

    #[test]
    fn test_memory_operation_requires_capability() {
        let read_op = MemoryOperation::Read {
            offset: 0,
            len:    100,
        };
        let write_mask = CapabilityMask::read_only();
        let read_mask = CapabilityMask::all();

        assert!(!read_op.requires_capability(&write_mask));
        assert!(read_op.requires_capability(&read_mask));
    }
}
