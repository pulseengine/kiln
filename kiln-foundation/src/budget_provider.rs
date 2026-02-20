//! Budget Provider Compatibility Layer (DEPRECATED)
//!
//! This module has been DEPRECATED in favor of the capability-driven memory
//! system. Use CapabilityFactoryBuilder and safe_capability_alloc! macro
//! instead.

#[cfg(any(feature = "std", feature = "alloc"))]
extern crate alloc;

#[cfg(any(feature = "std", feature = "alloc"))]
use crate::capabilities::{
    CapabilityAwareProvider,
    DynamicMemoryCapability,
    ProviderCapabilityExt,
};
use crate::{
    budget_aware_provider::CrateId,
    safe_memory::NoStdProvider,
    verification::VerificationLevel,
    Result,
};

/// DEPRECATED: Use capability-driven memory management instead
///
/// This struct is deprecated as it violates capability-based design principles.
/// Use MemoryFactory for safe memory management.
#[deprecated(
    since = "0.3.0",
    note = "Use CapabilityFactoryBuilder and safe_capability_alloc! macro instead"
)]
pub struct BudgetProvider;

#[allow(deprecated)]
impl BudgetProvider {
    /// DEPRECATED: Use capability-driven allocation instead
    ///
    /// # Migration
    ///
    /// Instead of:
    /// ```ignore
    /// let provider = BudgetProvider::new::<1024>(crate_id)?;
    /// ```
    ///
    /// Use:
    /// ```ignore
    /// let context = capability_context!(dynamic(crate_id, 1024))?;
    /// let provider = safe_capability_alloc!(context, crate_id, 1024)?;
    /// ```
    #[cfg(any(feature = "std", feature = "alloc"))]
    #[deprecated(
        since = "0.3.0",
        note = "Use safe_capability_alloc! macro with capability context"
    )]
    pub fn create<const N: usize>(
        crate_id: CrateId,
    ) -> Result<CapabilityAwareProvider<NoStdProvider<N>>> {
        // Create raw provider and wrap with capability
        use alloc::boxed::Box;

        use crate::{
            capabilities::DynamicMemoryCapability,
            verification::VerificationLevel,
        };

        let provider = NoStdProvider::<N>::default();
        let capability = Box::new(DynamicMemoryCapability::new(
            N,
            crate_id,
            VerificationLevel::Standard,
        ));

        Ok(provider.with_capability(capability, crate_id))
    }

    #[cfg(not(any(feature = "std", feature = "alloc")))]
    #[deprecated(
        since = "0.3.0",
        note = "Use NoStdProvider::new() directly in no_std environments"
    )]
    pub fn create_nostd<const N: usize>(_crate_id: CrateId) -> Result<NoStdProvider<N>> {
        Ok(NoStdProvider::<N>::default())
    }

    /// DEPRECATED: Use capability-driven allocation instead
    #[cfg(any(feature = "std", feature = "alloc"))]
    #[deprecated(
        since = "0.3.0",
        note = "Use safe_capability_alloc! macro with capability context"
    )]
    pub fn with_size<const N: usize>(
        crate_id: CrateId,
        _size: usize,
    ) -> Result<CapabilityAwareProvider<NoStdProvider<N>>> {
        // Ignore the size parameter and use const generic N
        Self::create::<N>(crate_id)
    }

    #[cfg(not(any(feature = "std", feature = "alloc")))]
    #[deprecated(
        since = "0.3.0",
        note = "Use NoStdProvider::new() directly in no_std environments"
    )]
    pub fn with_size<const N: usize>(_crate_id: CrateId, _size: usize) -> Result<NoStdProvider<N>> {
        Ok(NoStdProvider::<N>::default())
    }
}
