//! Safe Memory Allocation API - Capability-Based Version
//!
//! This module provides a completely safe alternative to the unsafe memory
//! allocation patterns using the new capability-based memory management system.
//! It eliminates the need for unsafe patterns by providing a safe factory
//! pattern that creates collections directly with capability-managed providers.
//!
//! SW-REQ-ID: REQ_SAFETY_001 - No unsafe code in allocation
//! SW-REQ-ID: REQ_MEM_001 - Memory bounds checking  
//! SW-REQ-ID: REQ_MEM_003 - Automatic cleanup
//! SW-REQ-ID: REQ_CAP_001 - Capability-based access control

// Legacy imports for backward compatibility (deprecated)
#[allow(deprecated)]
use crate::wrt_memory_system::{
    CapabilityWrtFactory,
    NoStdProviderFactory,
};
use crate::{
    bounded::{
        BoundedString,
        BoundedVec,
    },
    bounded_collections::BoundedMap,
    budget_aware_provider::CrateId,
    capabilities::{
        DynamicMemoryCapability,
        MemoryCapabilityContext,
        StaticMemoryCapability,
    },
    codes,
    safe_memory::NoStdProvider,
    traits::{
        Checksummable,
        FromBytes,
        ToBytes,
    },
    verification::VerificationLevel,
    Error,
    ErrorCategory,
    Result,
};

/// Capability-based safe factory for memory providers
pub struct CapabilityProviderFactory;

impl CapabilityProviderFactory {
    /// Create a static memory capability-managed provider for ASIL-B/C/D
    ///
    /// This creates a compile-time verified provider suitable for
    /// safety-critical levels.
    pub fn create_static_provider<const N: usize>(
        crate_id: CrateId,
        verification_level: VerificationLevel,
    ) -> Result<StaticMemoryCapability<N>> {
        Ok(StaticMemoryCapability::new(crate_id, verification_level))
    }

    /// Create a dynamic memory capability-managed provider for QM/ASIL-A
    ///
    /// This creates a runtime allocation provider suitable for
    /// non-safety-critical levels.
    pub fn create_dynamic_provider(
        crate_id: CrateId,
        max_allocation: usize,
        verification_level: VerificationLevel,
    ) -> Result<DynamicMemoryCapability> {
        Ok(DynamicMemoryCapability::new(
            max_allocation,
            crate_id,
            verification_level,
        ))
    }

    /// Create a capability-managed provider using context
    ///
    /// This integrates with the capability dependency injection system.
    pub fn create_context_managed_provider<const N: usize>(
        context: &mut MemoryCapabilityContext,
        crate_id: CrateId,
    ) -> Result<()> {
        context.register_static_capability::<N>(crate_id)
    }
}

// SafeProviderFactory removed - use MemoryFactory instead

/// Capability-based safe collection factory functions
///
/// These functions create bounded collections using capability-managed memory
/// allocation suitable for different ASIL levels without requiring unsafe code
/// blocks.
pub mod capability_factories {
    use super::*;

    /// Safely create a BoundedVec with static capability (ASIL-B/C/D)
    pub fn safe_static_bounded_vec<T, const CAPACITY: usize, const PROVIDER_SIZE: usize>(
        crate_id: CrateId,
        verification_level: VerificationLevel,
    ) -> Result<(
        BoundedVec<T, CAPACITY, NoStdProvider<PROVIDER_SIZE>>,
        StaticMemoryCapability<PROVIDER_SIZE>,
    )>
    where
        T: Sized + Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
    {
        let capability = CapabilityProviderFactory::create_static_provider::<PROVIDER_SIZE>(
            crate_id,
            verification_level,
        )?;

        // Create provider using safe default construction
        let provider = NoStdProvider::<PROVIDER_SIZE>::default();

        let vec = BoundedVec::new(provider).map_err(|_| {
            Error::memory_error("Failed to create capability-managed bounded vector")
        })?;

        Ok((vec, capability))
    }

    /// Create a memory capability context for dependency injection
    pub fn create_capability_context(
        verification_level: VerificationLevel,
        runtime_verification: bool,
    ) -> MemoryCapabilityContext {
        MemoryCapabilityContext::new(verification_level, runtime_verification)
    }
}

// safe_managed_alloc macro moved to lib.rs to avoid duplicates
// It's now the primary allocation interface for WRT

/// Capability-aware macro for creating providers
///
/// This macro uses the capability context for memory allocation verification.
///
/// # Examples
///
/// ```rust
/// use wrt_foundation::{
///     capability_managed_alloc,
///     CrateId,
///     MemoryCapabilityContext,
/// };
///
/// let context = MemoryCapabilityContext::new();
/// // Capability-based allocation with context verification
/// let provider = capability_managed_alloc!(131072, &context, CrateId::Runtime)?;
/// let vec = BoundedVec::new(provider)?;
/// ```
#[macro_export]
macro_rules! capability_managed_alloc {
    ($size:expr, $context:expr, $crate_id:expr) => {
        $crate::safe_allocation::CapabilityProviderFactory::create_context_managed_provider::<$size>($context, $crate_id)
    };
}
