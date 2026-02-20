//! Capability-Aware Provider Bridge
//!
//! This module provides a bridge between the existing Provider trait
//! and the new capability system, ensuring all memory operations are
//! capability-verified.

use core::fmt;

#[cfg(any(feature = "std", feature = "alloc"))]
extern crate alloc;

#[cfg(all(not(feature = "std"), feature = "alloc"))]
use alloc::boxed::Box;
#[cfg(feature = "std")]
use std::boxed::Box;

use super::{
    AnyMemoryCapability,
    MemoryOperation,
    MemoryOperationType,
};
use crate::{
    budget_aware_provider::CrateId,
    safe_memory::{
        Provider,
        Slice,
        SliceMut,
        Stats,
    },
    verification::VerificationLevel,
    Error,
    Result,
};

/// A capability-aware wrapper around any Provider implementation
///
/// This bridge ensures that all Provider trait methods verify capabilities
/// before delegating to the underlying provider implementation.
pub struct CapabilityAwareProvider<P: Provider> {
    /// The underlying provider implementation
    provider:    P,
    /// The capability that guards access to this provider
    capability:  Box<dyn AnyMemoryCapability>,
    /// The crate that owns this provider
    owner_crate: CrateId,
}

impl<P: Provider + Clone> Clone for CapabilityAwareProvider<P> {
    fn clone(&self) -> Self {
        Self {
            provider:    self.provider.clone(),
            capability:  self.capability.clone_capability(),
            owner_crate: self.owner_crate,
        }
    }
}

impl<P: Provider + PartialEq> PartialEq for CapabilityAwareProvider<P> {
    fn eq(&self, other: &Self) -> bool {
        self.provider == other.provider && self.owner_crate == other.owner_crate
        // Note: We can't compare capabilities directly since they're trait
        // objects
    }
}

impl<P: Provider + Eq> Eq for CapabilityAwareProvider<P> {}

impl<P: Provider + Default> Default for CapabilityAwareProvider<P> {
    fn default() -> Self {
        use alloc::boxed::Box;

        use crate::capabilities::{
            DynamicMemoryCapability,
            VerificationLevel,
        };

        let provider = P::default();
        let capability = Box::new(DynamicMemoryCapability::new(
            4096,                // Default size
            CrateId::Foundation, // Default crate
            VerificationLevel::Standard,
        ));

        Self {
            provider,
            capability,
            owner_crate: CrateId::Foundation,
        }
    }
}

impl<P: Provider> CapabilityAwareProvider<P> {
    /// Create a new capability-aware provider wrapper
    ///
    /// # Arguments
    /// * `provider` - The underlying provider to wrap
    /// * `capability` - The capability that guards access
    /// * `owner_crate` - The crate that owns this provider
    pub fn new(
        provider: P,
        capability: Box<dyn AnyMemoryCapability>,
        owner_crate: CrateId,
    ) -> Self {
        Self {
            provider,
            capability,
            owner_crate,
        }
    }

    /// Get a reference to the underlying provider
    pub fn inner(&self) -> &P {
        &self.provider
    }

    /// Get a mutable reference to the underlying provider (with capability
    /// check)
    pub fn inner_mut(&mut self) -> Result<&mut P> {
        // Verify the capability allows write access
        let operation = MemoryOperation::Write {
            offset: 0,
            len:    1,
        };
        self.capability.verify_access(&operation)?;
        Ok(&mut self.provider)
    }

    /// Get the capability that guards this provider
    pub fn capability(&self) -> &dyn AnyMemoryCapability {
        self.capability.as_ref()
    }

    /// Get the owner crate
    pub fn owner_crate(&self) -> CrateId {
        self.owner_crate
    }

    /// Verify that a memory operation is allowed by the capability
    fn verify_capability_access(&self, operation: &MemoryOperation) -> Result<()> {
        self.capability.verify_access(operation)
    }
}

impl<P: Provider> Provider for CapabilityAwareProvider<P> {
    type Allocator = P::Allocator;

    fn borrow_slice(&self, offset: usize, len: usize) -> Result<Slice<'_>> {
        // Verify capability allows read access
        let operation = MemoryOperation::Read { offset, len };
        self.verify_capability_access(&operation)?;

        // Delegate to underlying provider
        self.provider.borrow_slice(offset, len)
    }

    fn write_data(&mut self, offset: usize, data: &[u8]) -> Result<()> {
        // Verify capability allows write access
        let operation = MemoryOperation::Write {
            offset,
            len: data.len(),
        };
        self.verify_capability_access(&operation)?;

        // Delegate to underlying provider
        self.provider.write_data(offset, data)
    }

    fn verify_access(&self, offset: usize, len: usize) -> Result<()> {
        // First verify capability allows access
        let read_operation = MemoryOperation::Read { offset, len };
        self.verify_capability_access(&read_operation)?;

        // Then delegate to provider's internal verification
        self.provider.verify_access(offset, len)
    }

    fn size(&self) -> usize {
        self.provider.size()
    }

    fn capacity(&self) -> usize {
        self.provider.capacity()
    }

    fn verify_integrity(&self) -> Result<()> {
        // No capability check needed for integrity verification
        self.provider.verify_integrity()
    }

    fn set_verification_level(&mut self, level: VerificationLevel) {
        // TODO: Consider if this should be capability-gated
        self.provider.set_verification_level(level)
    }

    fn verification_level(&self) -> VerificationLevel {
        self.provider.verification_level()
    }

    fn memory_stats(&self) -> Stats {
        self.provider.memory_stats()
    }

    fn get_slice_mut(&mut self, offset: usize, len: usize) -> Result<SliceMut<'_>> {
        // Verify capability allows write access
        let operation = MemoryOperation::Write { offset, len };
        self.verify_capability_access(&operation)?;

        // Delegate to underlying provider
        self.provider.get_slice_mut(offset, len)
    }

    fn copy_within(&mut self, src_offset: usize, dst_offset: usize, len: usize) -> Result<()> {
        // Verify capability allows read from source
        let read_operation = MemoryOperation::Read {
            offset: src_offset,
            len,
        };
        self.verify_capability_access(&read_operation)?;

        // Verify capability allows write to destination
        let write_operation = MemoryOperation::Write {
            offset: dst_offset,
            len,
        };
        self.verify_capability_access(&write_operation)?;

        // Delegate to underlying provider
        self.provider.copy_within(src_offset, dst_offset, len)
    }

    fn ensure_used_up_to(&mut self, byte_offset: usize) -> Result<()> {
        // Verify capability allows write access to the entire range
        let operation = MemoryOperation::Write {
            offset: 0,
            len:    byte_offset,
        };
        self.verify_capability_access(&operation)?;

        // Delegate to underlying provider
        self.provider.ensure_used_up_to(byte_offset)
    }

    fn acquire_memory(&self, layout: core::alloc::Layout) -> wrt_error::Result<*mut u8> {
        // Verify capability allows allocation
        let operation = MemoryOperation::Allocate {
            size: layout.size(),
        };
        self.verify_capability_access(&operation)?;

        // Delegate to underlying provider
        self.provider.acquire_memory(layout)
    }

    fn release_memory(&self, ptr: *mut u8, layout: core::alloc::Layout) -> wrt_error::Result<()> {
        // Verify capability allows deallocation
        let operation = MemoryOperation::Deallocate;
        self.verify_capability_access(&operation)?;

        // Delegate to underlying provider
        self.provider.release_memory(ptr, layout)
    }

    fn get_allocator(&self) -> &Self::Allocator {
        self.provider.get_allocator()
    }

    fn new_handler(&self) -> Result<crate::safe_memory::SafeMemoryHandler<Self>>
    where
        Self: Sized + Clone,
    {
        // Create a handler with this capability-aware provider
        Ok(crate::safe_memory::SafeMemoryHandler::new(self.clone()))
    }
}

impl<P: Provider> fmt::Debug for CapabilityAwareProvider<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CapabilityAwareProvider")
            .field("provider", &self.provider)
            .field("capability", &self.capability)
            .field("owner_crate", &self.owner_crate)
            .finish()
    }
}

// Safety: CapabilityAwareProvider is Send if the underlying provider is Send
unsafe impl<P: Provider> Send for CapabilityAwareProvider<P> {}

// Safety: CapabilityAwareProvider is Sync if the underlying provider is Sync
unsafe impl<P: Provider> Sync for CapabilityAwareProvider<P> {}

/// Factory for creating capability-aware providers
pub struct CapabilityProviderFactory;

impl CapabilityProviderFactory {
    /// Wrap an existing provider with capability verification
    ///
    /// # Arguments
    /// * `provider` - The provider to wrap
    /// * `capability` - The capability that guards access
    /// * `owner_crate` - The crate that owns this provider
    pub fn wrap_provider<P: Provider>(
        provider: P,
        capability: Box<dyn AnyMemoryCapability>,
        owner_crate: CrateId,
    ) -> CapabilityAwareProvider<P> {
        CapabilityAwareProvider::new(provider, capability, owner_crate)
    }

    /// Create a capability-aware provider and verify initial allocation
    ///
    /// This method ensures the capability allows the initial allocation size
    /// before creating the provider wrapper.
    pub fn create_verified_provider<P: Provider>(
        provider: P,
        capability: Box<dyn AnyMemoryCapability>,
        owner_crate: CrateId,
        initial_size: usize,
    ) -> Result<CapabilityAwareProvider<P>> {
        // Verify capability allows the initial allocation
        let operation = MemoryOperation::Allocate { size: initial_size };
        capability.verify_access(&operation)?;

        Ok(CapabilityAwareProvider::new(
            provider,
            capability,
            owner_crate,
        ))
    }
}

/// Extension trait for adding capability verification to existing providers
pub trait ProviderCapabilityExt: Provider + Sized {
    /// Wrap this provider with capability verification
    fn with_capability(
        self,
        capability: Box<dyn AnyMemoryCapability>,
        owner_crate: CrateId,
    ) -> CapabilityAwareProvider<Self> {
        CapabilityProviderFactory::wrap_provider(self, capability, owner_crate)
    }

    /// Wrap this provider with verified capability allocation
    fn with_verified_capability(
        self,
        capability: Box<dyn AnyMemoryCapability>,
        owner_crate: CrateId,
        initial_size: usize,
    ) -> Result<CapabilityAwareProvider<Self>> {
        CapabilityProviderFactory::create_verified_provider(
            self,
            capability,
            owner_crate,
            initial_size,
        )
    }
}

// Blanket implementation for all Provider types
impl<P: Provider> ProviderCapabilityExt for P {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        capabilities::{
            CapabilityMask,
            DynamicMemoryCapability,
        },
        safe_managed_alloc,
        safe_memory::NoStdProvider,
        verification::VerificationLevel,
    };

    #[test]
    fn test_capability_aware_provider_creation() -> crate::Result<()> {
        let provider = safe_managed_alloc!(1024, CrateId::Foundation)?;
        let capability = Box::new(DynamicMemoryCapability::new(
            1024,
            CrateId::Foundation,
            VerificationLevel::Standard,
        ));

        let wrapped = provider.with_capability(capability, CrateId::Foundation);
        assert_eq!(wrapped.capacity(), 1024);
        assert_eq!(wrapped.owner_crate(), CrateId::Foundation);
        Ok(())
    }

    #[test]
    fn test_capability_verification_on_access() -> crate::Result<()> {
        let provider = safe_managed_alloc!(1024, CrateId::Foundation)?;

        // Create a capability that only allows read access
        let mut capability =
            DynamicMemoryCapability::new(1024, CrateId::Foundation, VerificationLevel::Standard);

        let wrapped = provider.with_capability(Box::new(capability), CrateId::Foundation);

        // Read access should work
        let result = wrapped.verify_access(0, 100);
        assert!(result.is_ok());

        // Borrow slice should work (read operation)
        let result = wrapped.borrow_slice(0, 100);
        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn test_capability_verification_failure() -> crate::Result<()> {
        let provider = safe_managed_alloc!(100, CrateId::Foundation)?; // Small provider
        let capability = Box::new(DynamicMemoryCapability::new(
            1024, // Capability allows more than provider capacity
            CrateId::Foundation,
            VerificationLevel::Standard,
        ));

        let wrapped = provider.with_capability(capability, CrateId::Foundation);

        // Try to access beyond provider's capacity should fail at provider level
        let result = wrapped.borrow_slice(0, 200);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_verified_provider_creation() -> crate::Result<()> {
        let provider = safe_managed_alloc!(1024, CrateId::Foundation)?;
        let capability = Box::new(DynamicMemoryCapability::new(
            1024,
            CrateId::Foundation,
            VerificationLevel::Standard,
        ));

        // Should succeed with valid size
        let result = provider.with_verified_capability(capability, CrateId::Foundation, 512);
        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn test_verified_provider_creation_failure() -> crate::Result<()> {
        let provider = safe_managed_alloc!(1024, CrateId::Foundation)?;
        let capability = Box::new(DynamicMemoryCapability::new(
            512, // Capability only allows 512 bytes
            CrateId::Foundation,
            VerificationLevel::Standard,
        ));

        // Should fail with size exceeding capability
        let result = provider.with_verified_capability(capability, CrateId::Foundation, 1024);
        assert!(result.is_err());
        Ok(())
    }
}
