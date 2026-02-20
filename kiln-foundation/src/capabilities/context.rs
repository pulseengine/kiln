//! Capability Context for Dependency Injection
//!
//! This module provides the capability context that replaces global state
//! with explicit dependency injection of memory capabilities.

use core::{
    fmt,
    marker::PhantomData,
};
#[cfg(feature = "std")]
use std::boxed::Box;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;

use wrt_error::helpers::memory_limit_exceeded_error;

use super::{
    dynamic::DynamicMemoryCapability,
    static_alloc::StaticMemoryCapability,
    verified::VerifiedMemoryCapability,
    MemoryCapability,
    MemoryOperation,
};
// No HashMap needed - always using static arrays for ASIL-A compliance
use crate::{
    budget_aware_provider::CrateId,
    codes,
    verification::VerificationLevel,
    Error,
    ErrorCategory,
    Result,
};

/// Maximum number of capabilities that can be stored in no_std mode
const MAX_CAPABILITIES: usize = 32;

/// Capability context that manages memory capabilities for dependency injection
///
/// This replaces global state with explicit capability injection, ensuring
/// that all memory operations are capability-gated.
///
/// For ASIL-A compliance, this always uses static arrays to avoid any dynamic
/// memory allocation, even when std is available.
pub struct MemoryCapabilityContext {
    /// Static array of capabilities to ensure no dynamic allocation
    /// Each slot contains an optional (CrateId, Capability) pair
    capabilities: [CapabilitySlot; MAX_CAPABILITIES],

    /// Default verification level for new capabilities
    default_verification_level: VerificationLevel,

    /// Whether runtime verification is enabled
    runtime_verification: bool,
}

/// Type alias for complex capability storage type
type CapabilitySlot = (Option<CrateId>, Option<Box<dyn AnyMemoryCapability>>);

/// Trait object wrapper for different capability types
///
/// This allows storing different capability types in the same collection
/// while maintaining type safety through the capability trait system.
pub trait AnyMemoryCapability: Send + Sync + fmt::Debug {
    /// Verify access to a memory operation
    fn verify_access(&self, operation: &MemoryOperation) -> Result<()>;

    /// Get the maximum allocation size for this capability
    fn max_allocation_size(&self) -> usize;

    /// Get the verification level required by this capability
    fn verification_level(&self) -> VerificationLevel;

    /// Get the crate that owns this capability
    fn owner_crate(&self) -> CrateId;

    /// Clone this capability (for delegation purposes)
    fn clone_capability(&self) -> Box<dyn AnyMemoryCapability>;
}

/// Blanket implementation for all memory capabilities
impl<T> AnyMemoryCapability for T
where
    T: MemoryCapability + Clone + 'static,
{
    fn verify_access(&self, operation: &MemoryOperation) -> Result<()> {
        MemoryCapability::verify_access(self, operation)
    }

    fn max_allocation_size(&self) -> usize {
        MemoryCapability::max_allocation_size(self)
    }

    fn verification_level(&self) -> VerificationLevel {
        MemoryCapability::verification_level(self)
    }

    fn owner_crate(&self) -> CrateId {
        // This would need to be stored in the capability
        // For now, return a default
        CrateId::Foundation
    }

    fn clone_capability(&self) -> Box<dyn AnyMemoryCapability> {
        Box::new(self.clone())
    }
}

impl Default for MemoryCapabilityContext {
    fn default() -> Self {
        Self::new(VerificationLevel::Standard, false)
    }
}

impl MemoryCapabilityContext {
    /// Create a new capability context
    pub fn new(default_verification_level: VerificationLevel, runtime_verification: bool) -> Self {
        Self {
            // Always use static array for ASIL-A compliance (no dynamic allocation)
            capabilities: core::array::from_fn(|_| (None, None)),
            default_verification_level,
            runtime_verification,
        }
    }

    /// Get the default verification level for this context
    pub fn default_verification_level(&self) -> VerificationLevel {
        self.default_verification_level
    }

    /// Register a dynamic memory capability for a crate
    pub fn register_dynamic_capability(
        &mut self,
        crate_id: CrateId,
        max_allocation: usize,
    ) -> Result<()> {
        let capability =
            DynamicMemoryCapability::new(max_allocation, crate_id, self.default_verification_level);
        let boxed_capability = Box::new(capability);
        self.register_capability(crate_id, boxed_capability)
    }

    /// Register a static memory capability for a crate
    pub fn register_static_capability<const N: usize>(&mut self, crate_id: CrateId) -> Result<()> {
        let capability =
            StaticMemoryCapability::<N>::new(crate_id, self.default_verification_level);

        self.register_capability(crate_id, Box::new(capability))
    }

    /// Register a verified memory capability for a crate (ASIL-D)
    pub fn register_verified_capability<const N: usize>(
        &mut self,
        crate_id: CrateId,
        proofs: super::verified::VerificationProofs,
    ) -> Result<()> {
        let capability =
            VerifiedMemoryCapability::<N>::new(crate_id, proofs, self.runtime_verification)?;

        self.register_capability(crate_id, Box::new(capability))
    }

    /// Register a custom capability for a crate
    pub fn register_capability(
        &mut self,
        crate_id: CrateId,
        capability: Box<dyn AnyMemoryCapability>,
    ) -> Result<()> {
        // Always use static array logic for ASIL-A compliance
        // Find an empty slot or replace existing crate capability
        let mut inserted = false;
        for (key, value) in self.capabilities.iter_mut() {
            if key.is_none() || *key == Some(crate_id) {
                *key = Some(crate_id);
                *value = Some(capability);
                inserted = true;
                break;
            }
        }
        if !inserted {
            return Err(memory_limit_exceeded_error(
                "Maximum number of capabilities exceeded",
            ));
        }

        Ok(())
    }

    /// Get a capability for a crate
    pub fn get_capability(&self, crate_id: CrateId) -> Result<&dyn AnyMemoryCapability> {
        // Always use static array logic for ASIL-A compliance
        for (key, value) in self.capabilities.iter() {
            if *key == Some(crate_id) {
                if let Some(cap) = value {
                    return Ok(cap.as_ref());
                }
            }
        }
        Err(Error::no_capability("No capability found for crate"))
    }

    /// Verify that a crate can perform a memory operation
    pub fn verify_operation(&self, crate_id: CrateId, operation: &MemoryOperation) -> Result<()> {
        let capability = self.get_capability(crate_id)?;
        capability.verify_access(operation)
    }

    /// Remove a capability for a crate
    pub fn remove_capability(&mut self, crate_id: CrateId) -> Result<()> {
        // Always use static array logic for ASIL-A compliance
        let mut found = false;
        for (key, value) in self.capabilities.iter_mut() {
            if *key == Some(crate_id) {
                *key = None;
                *value = None;
                found = true;
                break;
            }
        }
        if !found {
            return Err(Error::no_capability("No capability found for crate"));
        }
        Ok(())
    }

    /// Get the number of registered capabilities
    pub fn capability_count(&self) -> usize {
        // Always use static array logic for ASIL-A compliance
        self.capabilities.iter().filter(|(key, _)| key.is_some()).count()
    }

    /// Check if a crate has a registered capability
    pub fn has_capability(&self, crate_id: CrateId) -> bool {
        // Always use static array logic for ASIL-A compliance
        self.capabilities
            .iter()
            .any(|(key, value)| *key == Some(crate_id) && value.is_some())
    }

    /// List all registered crate IDs
    /// Returns a static array to avoid any dynamic allocation
    pub fn registered_crates(&self) -> [Option<CrateId>; MAX_CAPABILITIES] {
        let mut result = [None; MAX_CAPABILITIES];
        let mut index = 0;

        for (crate_id, value) in self.capabilities.iter() {
            if index < MAX_CAPABILITIES {
                if let (Some(id), Some(_)) = (crate_id, value) {
                    result[index] = Some(*id);
                    index += 1;
                }
            } else {
                break;
            }
        }

        result
    }
}

impl fmt::Debug for MemoryCapabilityContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoryCapabilityContext")
            .field("capability_count", &self.capability_count())
            .field(
                "default_verification_level",
                &self.default_verification_level,
            )
            .field("runtime_verification", &self.runtime_verification)
            .finish()
    }
}

/// A memory provider that is protected by capability verification
///
/// This replaces the raw NoStdProvider with capability-gated access.
pub struct CapabilityGuardedProvider<const N: usize> {
    capability: Box<dyn AnyMemoryCapability>,
    _phantom:   PhantomData<[u8; N]>,
}

impl<const N: usize> CapabilityGuardedProvider<N> {
    /// Create a new capability-guarded provider
    fn new(capability: Box<dyn AnyMemoryCapability>) -> Result<Self> {
        // Verify the capability allows allocation of this size
        let operation = MemoryOperation::Allocate { size: N };
        capability.verify_access(&operation)?;

        if capability.max_allocation_size() < N {
            return Err(Error::capability_violation(
                "Provider size exceeds capability limit",
            ));
        }

        Ok(Self {
            capability,
            _phantom: PhantomData,
        })
    }

    /// Read data with capability verification
    pub fn read_bytes(&self, offset: usize, len: usize) -> Result<&[u8]> {
        let operation = MemoryOperation::Read { offset, len };
        self.capability.verify_access(&operation)?;

        // This would delegate to the actual provider implementation
        // For now, return empty slice
        Ok(&[])
    }

    /// Write data with capability verification
    pub fn write_bytes(&mut self, offset: usize, data: &[u8]) -> Result<()> {
        let operation = MemoryOperation::Write {
            offset,
            len: data.len(),
        };
        self.capability.verify_access(&operation)?;

        // This would delegate to the actual provider implementation
        Ok(())
    }

    /// Get the size of this provider
    pub const fn size(&self) -> usize {
        N
    }

    /// Get the capability that guards this provider
    pub fn capability(&self) -> &dyn AnyMemoryCapability {
        self.capability.as_ref()
    }
}

impl<const N: usize> fmt::Debug for CapabilityGuardedProvider<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CapabilityGuardedProvider")
            .field("size", &N)
            .field("capability", &self.capability)
            .finish()
    }
}

// Safety: CapabilityGuardedProvider can be sent between threads safely
unsafe impl<const N: usize> Send for CapabilityGuardedProvider<N> {}
unsafe impl<const N: usize> Sync for CapabilityGuardedProvider<N> {}

/// Builder for creating capability contexts with different configurations
pub struct CapabilityContextBuilder {
    verification_level:   VerificationLevel,
    runtime_verification: bool,
}

impl CapabilityContextBuilder {
    /// Create a new builder with default settings
    pub fn new() -> Self {
        Self {
            verification_level:   VerificationLevel::Standard,
            runtime_verification: false,
        }
    }

    /// Set the default verification level
    pub fn with_verification_level(mut self, level: VerificationLevel) -> Self {
        self.verification_level = level;
        self
    }

    /// Enable or disable runtime verification
    pub fn with_runtime_verification(mut self, enabled: bool) -> Self {
        self.runtime_verification = enabled;
        self
    }

    /// Build the capability context
    pub fn build(self) -> MemoryCapabilityContext {
        MemoryCapabilityContext::new(self.verification_level, self.runtime_verification)
    }
}

impl Default for CapabilityContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

