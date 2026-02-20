//! Capability-driven Memory Provider Factory
//!
//! This module provides capability-driven memory allocation with explicit
//! capability verification and authorization for all memory operations.
//!
//! NOTE: This factory requires alloc feature for proper operation.

#[cfg(any(feature = "std", feature = "alloc"))]
use core::marker::PhantomData;

#[cfg(any(feature = "std", feature = "alloc"))]
extern crate alloc;

#[cfg(all(not(feature = "std"), feature = "alloc"))]
use alloc::boxed::Box;
#[cfg(feature = "std")]
use std::boxed::Box;

#[cfg(any(feature = "std", feature = "alloc"))]
use super::{
    context::MemoryCapabilityContext,
    MemoryCapability,
    MemoryOperation,
};
#[cfg(any(feature = "std", feature = "alloc"))]
use crate::{
    budget_aware_provider::CrateId,
    codes,
    safe_memory::{
        NoStdProvider,
        Provider,
    },
    Error,
    ErrorCategory,
    Result,
};

/// A memory provider that is protected by capability verification
///
/// This provides capability-gated access to memory allocation.
#[cfg(any(feature = "std", feature = "alloc"))]
pub struct CapabilityGuardedProvider<const N: usize> {
    capability: Box<dyn super::AnyMemoryCapability>,
    provider:   Option<NoStdProvider<N>>,
    _phantom:   PhantomData<[u8; N]>,
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<const N: usize> CapabilityGuardedProvider<N> {
    /// Create a new capability-guarded provider
    pub(crate) fn new(capability: Box<dyn super::AnyMemoryCapability>) -> Result<Self> {
        // Verify the capability allows allocation of this size
        let operation = MemoryOperation::Allocate { size: N };
        capability.verify_access(&operation)?;

        if capability.max_allocation_size() < N {
            return Err(Error::security_access_denied(
                "Provider size exceeds capability limit",
            ));
        }

        Ok(Self {
            capability,
            provider: None,
            _phantom: PhantomData,
        })
    }

    /// Initialize the underlying provider (lazy initialization)
    fn ensure_provider(&mut self) -> Result<&mut NoStdProvider<N>> {
        if self.provider.is_none() {
            // Create the underlying provider using safe_managed_alloc
            use crate::{
                budget_aware_provider::CrateId,
                safe_managed_alloc,
            };
            let provider = safe_managed_alloc!(N, CrateId::Foundation)
                .map_err(|_| Error::memory_error("Failed to allocate provider"))?;
            self.provider = Some(provider);
        }

        self.provider
            .as_mut()
            .ok_or_else(|| Error::memory_error("Provider not initialized"))
    }

    /// Read data with capability verification
    pub fn read_bytes(&mut self, offset: usize, len: usize) -> Result<&[u8]> {
        let operation = MemoryOperation::Read { offset, len };
        self.capability.verify_access(&operation)?;

        let provider = self.ensure_provider()?;

        // Delegate to the provider's slice implementation
        if offset + len > N {
            return Err(Error::runtime_execution_error(
                "Read range exceeds provider capacity",
            ));
        }

        // Return a slice from the provider's buffer
        let slice = provider.borrow_slice(offset, len)?;
        slice.data()
    }

    /// Write data with capability verification
    pub fn write_bytes(&mut self, offset: usize, data: &[u8]) -> Result<()> {
        let operation = MemoryOperation::Write {
            offset,
            len: data.len(),
        };
        self.capability.verify_access(&operation)?;

        let provider = self.ensure_provider()?;

        if offset + data.len() > N {
            return Err(Error::new(
                ErrorCategory::Memory,
                codes::OUT_OF_BOUNDS,
                "Write range exceeds provider capacity",
            ));
        }

        // Write to the provider's buffer
        let mut slice = provider.get_slice_mut(offset, data.len())?;
        slice.data_mut()?.copy_from_slice(data);
        Ok(())
    }

    /// Get the size of this provider
    pub const fn size(&self) -> usize {
        N
    }

    /// Get the capability that guards this provider
    pub fn capability(&self) -> &dyn super::AnyMemoryCapability {
        self.capability.as_ref()
    }

    /// Get a read-only slice of the entire buffer
    pub fn as_slice(&mut self) -> Result<&[u8]> {
        self.read_bytes(0, N)
    }

    /// Get a mutable slice of the entire buffer
    pub fn as_slice_mut(&mut self) -> Result<crate::safe_memory::SliceMut<'_>> {
        let operation = MemoryOperation::Write {
            offset: 0,
            len:    N,
        };
        self.capability.verify_access(&operation)?;

        let provider = self.ensure_provider()?;
        provider.get_slice_mut(0, N)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<const N: usize> core::fmt::Debug for CapabilityGuardedProvider<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CapabilityGuardedProvider")
            .field("size", &N)
            .field("capability", &self.capability)
            .field("initialized", &self.provider.is_some())
            .finish()
    }
}

// Safety: CapabilityGuardedProvider can be sent between threads safely
// The capability system ensures thread-safe access
#[cfg(any(feature = "std", feature = "alloc"))]
unsafe impl<const N: usize> Send for CapabilityGuardedProvider<N> {}
#[cfg(any(feature = "std", feature = "alloc"))]
unsafe impl<const N: usize> Sync for CapabilityGuardedProvider<N> {}

#[cfg(all(test, any(feature = "std", feature = "alloc")))]
mod tests {
    use super::*;
    use crate::verification::VerificationLevel;

    #[test]
    fn test_capability_guarded_provider() {
        // Test the CapabilityGuardedProvider directly using the new MemoryFactory
        use crate::capabilities::MemoryFactory;

        let provider = MemoryFactory::create::<1024>(CrateId::Foundation).unwrap();

        // Test basic provider properties
        assert_eq!(provider.size(), 1024);
    }
}
