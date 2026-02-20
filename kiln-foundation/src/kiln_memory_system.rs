//! WRT-Specific Memory System Implementation
//!
//! This module provides the WRT-specific implementation of the generic memory
//! system, using the generic components to create a complete solution.
//!
//! SW-REQ-ID: REQ_MEM_001 - Memory bounds checking
//! SW-REQ-ID: REQ_MEM_002 - Budget enforcement
//! SW-REQ-ID: REQ_MEM_003 - Automatic cleanup

use wrt_error::{
    helpers::memory_limit_exceeded_error,
    Result,
};

use crate::{
    budget_aware_provider::CrateId,
    codes,
    generic_memory_guard::{
        GenericMemoryGuard,
        ManagedMemoryProvider,
        MemoryCoordinator,
    },
    generic_provider_factory::{
        GenericBudgetAwareFactory,
        ProviderFactory,
    },
    memory_coordinator::{
        AllocationId,
        CrateIdentifier,
        GenericMemoryCoordinator,
    },
    safe_memory::NoStdProvider,
    Error,
    ErrorCategory,
};

/// Maximum number of crates in the WRT system
pub const WRT_MAX_CRATES: usize = 32;

/// WRT-specific memory coordinator
pub type WrtMemoryCoordinator = GenericMemoryCoordinator<CrateId, WRT_MAX_CRATES>;

/// WRT-specific memory guard for NoStdProvider
pub type WrtMemoryGuard<const N: usize> =
    GenericMemoryGuard<NoStdProvider<N>, WrtMemoryCoordinator, CrateId>;

// REMOVED: Legacy global memory coordinator eliminated in favor of
// capability-based system Use MemoryCapabilityContext through
// get_global_capability_context() for memory management

// CrateIdentifier implementation is in budget_aware_provider.rs

// Implement ManagedMemoryProvider for NoStdProvider
impl<const N: usize> ManagedMemoryProvider for NoStdProvider<N> {
    fn allocation_size(&self) -> usize {
        N
    }
}

// Implement MemoryCoordinator trait for WrtMemoryCoordinator
impl MemoryCoordinator<CrateId> for WrtMemoryCoordinator {
    type AllocationId = AllocationId;

    fn register_allocation(&self, crate_id: CrateId, size: usize) -> Result<Self::AllocationId> {
        GenericMemoryCoordinator::register_allocation(self, crate_id, size)
    }

    fn return_allocation(
        &self,
        crate_id: CrateId,
        id: Self::AllocationId,
        size: usize,
    ) -> Result<()> {
        GenericMemoryCoordinator::return_allocation(self, crate_id, id, size)
    }
}

/// Factory for creating NoStdProviders
pub struct NoStdProviderFactory;

impl NoStdProviderFactory {
    /// Create a provider with deprecated constructor (temporary)
    fn create_provider_internal<const N: usize>() -> NoStdProvider<N> {
        // Use safe default construction
        NoStdProvider::<N>::default()
    }
}

/// Generic implementation for any size
pub struct SizedNoStdProviderFactory<const N: usize>;

impl<const N: usize> ProviderFactory for SizedNoStdProviderFactory<N> {
    type Provider = NoStdProvider<N>;

    fn create_provider(&self, size: usize) -> Result<Self::Provider> {
        if size > N {
            return Err(memory_limit_exceeded_error(
                "Requested size exceeds provider capacity",
            ));
        }

        #[allow(deprecated)]
        Ok(NoStdProviderFactory::create_provider_internal::<N>())
    }
}

/// WRT-specific budget-aware factory
pub type WrtBudgetAwareFactory<const N: usize> =
    GenericBudgetAwareFactory<SizedNoStdProviderFactory<N>, WrtMemoryCoordinator, CrateId>;

// Legacy WrtProviderFactory has been removed. Use CapabilityWrtFactory instead.

/// Modern capability-based factory for WRT memory providers
///
/// This replaces the deprecated WrtProviderFactory with a capability-driven
/// approach that integrates with the MemoryCapabilityContext system.
pub struct CapabilityWrtFactory;

impl CapabilityWrtFactory {
    /// Create a capability-gated provider using the global capability context
    pub fn create_provider<const N: usize>(
        crate_id: CrateId,
    ) -> Result<crate::safe_memory::NoStdProvider<N>> {
        use crate::memory_init::get_global_capability_context;
        let context = get_global_capability_context()?;

        // Use the context to create a capability-guarded provider
        crate::capabilities::memory_factory::MemoryFactory::create_with_context::<N>(
            context, crate_id,
        )
    }

    /// Initialize the capability system with default crate budgets
    pub fn initialize_default() -> Result<()> {
        // The capability system is initialized through memory_init::MemoryInitializer
        crate::memory_init::MemoryInitializer::initialize()
    }
}

/// Convenience macro for creating capability-gated WRT providers
#[macro_export]
macro_rules! wrt_provider {
    ($size:expr, $crate_id:expr) => {
        $crate::wrt_memory_system::CapabilityWrtFactory::create_provider::<$size>($crate_id)
    };
}

