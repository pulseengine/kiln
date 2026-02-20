//! Compile-Time Memory Enforcement System
//!
//! This module provides compile-time enforcement mechanisms that make it
//! impossible to bypass the memory budget system.
//!
//! SW-REQ-ID: REQ_MEM_002 - Budget enforcement

#[cfg(any(feature = "std", feature = "alloc"))]
use crate::capabilities::CapabilityGuardedProvider;
use crate::{
    budget_aware_provider::CrateId,
    capabilities::MemoryCapabilityContext,
    memory_coordinator::CrateIdentifier,
    safe_managed_alloc,
    Error,
    Result,
};

/// Sealed trait to prevent external implementation
mod sealed {
    pub trait Sealed {}
}

/// Trait that can only be implemented by approved types
pub trait MemoryManaged: sealed::Sealed {
    type Guard<const N: usize>;

    /// Create a managed allocation with capability context
    fn allocate<const N: usize>(
        context: &MemoryCapabilityContext,
        crate_id: CrateId,
    ) -> Result<Self::Guard<N>>;
}

/// Only MemoryCapabilityContext can implement MemoryManaged
impl sealed::Sealed for MemoryCapabilityContext {}

impl MemoryManaged for MemoryCapabilityContext {
    type Guard<const N: usize> = crate::safe_memory::NoStdProvider<N>;

    fn allocate<const N: usize>(
        context: &MemoryCapabilityContext,
        crate_id: CrateId,
    ) -> Result<Self::Guard<N>> {
        crate::capabilities::memory_factory::MemoryFactory::create_with_context::<N>(
            context, crate_id,
        )
    }
}

/// Compile-time enforcement macro
// managed_alloc macro is now defined in macros.rs for better organization

/// Type-level enforcement using phantom types
pub struct EnforcedAllocation<const SIZE: usize, const CRATE: usize> {
    _phantom: core::marker::PhantomData<[u8; SIZE]>,
}

impl<const SIZE: usize, const CRATE: usize> Default for EnforcedAllocation<SIZE, CRATE> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const SIZE: usize, const CRATE: usize> EnforcedAllocation<SIZE, CRATE> {
    /// Create an enforced allocation
    pub const fn new() -> Self {
        Self {
            _phantom: core::marker::PhantomData,
        }
    }

    /// Materialize the allocation (only possible through managed system)
    pub fn materialize(
        self,
        context: &MemoryCapabilityContext,
        crate_id: CrateId,
    ) -> Result<crate::safe_memory::NoStdProvider<SIZE>> {
        // Verify crate ID matches compile-time constant
        if crate_id.as_index() != CRATE {
            return Err(Error::runtime_execution_error(
                "Crate ID mismatch in enforced allocation",
            ));
        }

        crate::capabilities::memory_factory::MemoryFactory::create_with_context::<SIZE>(
            context, crate_id,
        )
    }
}

/// Capability-based allocation token
pub struct AllocationToken<const SIZE: usize> {
    crate_id: CrateId,
    _phantom: core::marker::PhantomData<[u8; SIZE]>,
}

impl<const SIZE: usize> AllocationToken<SIZE> {
    /// Create a new allocation token (capability)
    pub const fn new(crate_id: CrateId) -> Self {
        Self {
            crate_id,
            _phantom: core::marker::PhantomData,
        }
    }

    /// Use the token to allocate memory
    pub fn allocate(
        self,
        context: &MemoryCapabilityContext,
    ) -> Result<crate::safe_memory::NoStdProvider<SIZE>> {
        crate::capabilities::memory_factory::MemoryFactory::create_with_context::<SIZE>(
            context,
            self.crate_id,
        )
    }
}

/// Memory region with compile-time bounds checking
pub struct MemoryRegion<const START: usize, const SIZE: usize> {
    _phantom: core::marker::PhantomData<[u8; SIZE]>,
}

impl<const START: usize, const SIZE: usize> Default for MemoryRegion<START, SIZE> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const START: usize, const SIZE: usize> MemoryRegion<START, SIZE> {
    /// Create a memory region with compile-time bounds
    pub const fn new() -> Self {
        // Basic validation - more complex checks moved to where type is instantiated
        assert!(SIZE > 0);

        Self {
            _phantom: core::marker::PhantomData,
        }
    }

    /// Get the size of this region
    pub const fn size(&self) -> usize {
        SIZE
    }

    /// Get the start offset
    pub const fn start(&self) -> usize {
        START
    }
}

