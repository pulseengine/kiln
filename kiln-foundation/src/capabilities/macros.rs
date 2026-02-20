//! Capability-Driven Memory Allocation Macros
//!
//! This module provides macros that replace the global coordinator-based
//! macros with capability-driven alternatives.

/// Create a capability-verified memory provider
///
/// This macro replaces the old `safe_managed_alloc!` with capability
/// verification. Instead of using a global coordinator, it requires explicit
/// capability injection.
///
/// # Arguments
/// * `$capability_context` - The MemoryCapabilityContext to use
/// * `$crate_id` - The CrateId requesting allocation
/// * `$size` - The size of memory to allocate (as const generic or usize)
///
/// # Examples
///
/// ```rust,no_run
/// use wrt_foundation::capabilities::{
///     MemoryCapabilityContext, CapabilityFactoryBuilder
/// };
/// use wrt_foundation::budget_aware_provider::CrateId;
///
/// # fn example() -> wrt_foundation::Result<()> {
/// // Create capability context
/// let mut factory = CapabilityFactoryBuilder::new()
///     .with_dynamic_capability(CrateId::Foundation, 4096)?
///     .build);
///
/// // Use the capability-driven macro
/// let provider = safe_capability_alloc!(factory, CrateId::Foundation, 1024)?;
/// # Ok(())
/// # }
/// ```
#[macro_export]
#[cfg(any(feature = "std", feature = "alloc"))]
macro_rules! safe_capability_alloc {
    ($capability_context:expr, $crate_id:expr, $size:expr) => {{
        use $crate::capabilities::{
            MemoryFactory,
            MemoryOperation,
        };

        // CRITICAL FIX: Use the passed context instead of ignoring it
        // This eliminates circular dependency in provider creation
        MemoryFactory::create_wrapped_with_context::<$size>(&$capability_context, $crate_id)
    }};
}

/// No-std version of safe_capability_alloc! macro
///
/// In no_std environments without alloc, we directly create a NoStdProvider
/// without the capability wrapper since we can't have dynamic dispatch.
#[macro_export]
#[cfg(not(any(feature = "std", feature = "alloc")))]
macro_rules! safe_capability_alloc {
    ($capability_context:expr, $crate_id:expr, $size:expr) => {{
        // In no_std, ignore the context and create provider directly
        let _ = $capability_context;
        let _ = $crate_id;
        Ok($crate::safe_memory::NoStdProvider::<$size>::default())
    }};
}

/// Create a capability-verified provider with explicit verification level
///
/// This macro allows specifying the required verification level for the
/// allocation.
///
/// # Arguments
/// * `$crate_id` - The CrateId requesting allocation
/// * `$size` - The size of memory to allocate
/// * `$verification_level` - Required VerificationLevel
///
/// # Examples
///
/// ```rust,no_run
/// use wrt_foundation::{
///     budget_aware_provider::CrateId,
///     verification::VerificationLevel,
/// };
///
/// # fn example() -> wrt_foundation::Result<()> {
/// let provider = safe_verified_alloc!(CrateId::Foundation, 1024, VerificationLevel::Redundant)?;
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! safe_verified_alloc {
    ($crate_id:expr, $size:expr, $verification_level:expr) => {{
        use $crate::capabilities::MemoryFactory;

        MemoryFactory::create_verified::<$size>($crate_id, $verification_level)
    }};
}

/// Wrap an existing Provider with capability verification
///
/// This macro provides a convenient way to add capability verification
/// to existing Provider implementations.
///
/// # Arguments
/// * `$provider` - The existing Provider instance
/// * `$capability` - The capability that guards access
/// * `$crate_id` - The crate that owns this provider
///
/// # Examples
///
/// ```rust,no_run
/// use wrt_foundation::{
///     capabilities::{DynamicMemoryCapability, ProviderCapabilityExt},
///     safe_memory::NoStdProvider,
///     budget_aware_provider::CrateId,
///     verification::VerificationLevel
/// };
///
/// # fn example() -> wrt_foundation::Result<()> {
/// let provider = NoStdProvider::<1024>::new();
/// let capability = Box::new(DynamicMemoryCapability::new(
///     1024,
///     CrateId::Foundation,
///     VerificationLevel::Standard,
/// ;
///
/// let wrapped = capability_wrap_provider!(provider, capability, CrateId::Foundation;
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! capability_wrap_provider {
    ($provider:expr, $capability:expr, $crate_id:expr) => {{
        #[cfg(any(feature = "std", feature = "alloc"))]
        {
            use $crate::capabilities::ProviderCapabilityExt;
            $provider.with_capability($capability, $crate_id)
        }
        #[cfg(not(any(feature = "std", feature = "alloc")))]
        {
            // In no_std without alloc, we just return the provider as-is
            // since capability wrapping requires allocation
            Ok($provider)
        }
    }};
}

/// Create a capability context with common configurations
///
/// This macro simplifies creating capability contexts for common use cases.
///
/// # Variants
/// * `dynamic($crate_id, $max_size)` - Create with dynamic capability
/// * `static($crate_id, $size)` - Create with static capability
/// * `verified($crate_id, $size, $proofs)` - Create with verified capability
///
/// # Examples
///
/// ```rust,no_run
/// use wrt_foundation::{budget_aware_provider::CrateId, capabilities::VerificationProofs};
///
/// # fn example() -> wrt_foundation::Result<()> {
/// // Dynamic capability context
/// let dynamic_context = capability_context!(dynamic(CrateId::Foundation, 4096))?;
///
/// // Static capability context  
/// let static_context = capability_context!(static(CrateId::Foundation, 2048))?;
///
/// # Ok(())
/// # }
/// ```
#[macro_export]
#[cfg(any(feature = "std", feature = "alloc"))]
macro_rules! capability_context {
    (dynamic($crate_id:expr, $max_size:expr)) => {{
        use $crate::{
            capabilities::{
                DynamicMemoryCapability,
                MemoryCapabilityContext,
            },
            verification::VerificationLevel,
        };

        // Create a simple context with dynamic capability for direct use with
        // MemoryFactory
        let mut context = MemoryCapabilityContext::new(VerificationLevel::Standard, false);
        context.register_dynamic_capability($crate_id, $max_size).map(|_| context)
    }};

    (static($crate_id:expr, $size:expr)) => {{
        use $crate::{
            capabilities::MemoryCapabilityContext,
            verification::VerificationLevel,
        };

        // Create a simple context with static capability for direct use with
        // MemoryFactory
        let mut context = MemoryCapabilityContext::new(VerificationLevel::Standard, false);
        context.register_static_capability::<$size>($crate_id).map(|_| context)
    }};

    (verified($crate_id:expr, $size:expr, $proofs:expr)) => {{
        use $crate::{
            capabilities::MemoryCapabilityContext,
            verification::VerificationLevel,
        };

        // Create a simple context with verified capability for direct use with
        // MemoryFactory
        let mut context = MemoryCapabilityContext::new(VerificationLevel::Redundant, true);
        context
            .register_verified_capability::<$size>($crate_id, $proofs)
            .map(|_| context)
    }};
}

/// No-std version of capability_context macro
///
/// In no_std environments without alloc, we can't have dynamic dispatch,
/// so this returns a NoStdCapabilityContext as a placeholder.
#[macro_export]
#[cfg(not(any(feature = "std", feature = "alloc")))]
macro_rules! capability_context {
    (dynamic($crate_id:expr, $max_size:expr)) => {{
        // Return a placeholder struct that the no_std macro can use
        Ok($crate::capabilities::NoStdCapabilityContext {
            crate_id: $crate_id,
            size:     $max_size,
        })
    }};

    (static($crate_id:expr, $size:expr)) => {{
        Ok($crate::capabilities::NoStdCapabilityContext {
            crate_id: $crate_id,
            size:     $size,
        })
    }};

    (verified($crate_id:expr, $size:expr, $proofs:expr)) => {{
        Ok($crate::capabilities::NoStdCapabilityContext {
            crate_id: $crate_id,
            size:     $size,
        })
    }};
}

/// Deprecated macro that warns about global state usage
///
/// This macro provides a migration path from the old `safe_managed_alloc!`
/// to the new capability-driven approach.
#[deprecated(
    since = "0.3.0",
    note = "Use safe_capability_alloc! with explicit capability context instead of global \
            coordinator"
)]
#[macro_export]
macro_rules! safe_managed_alloc_deprecated {
    ($crate_id:expr, $size:expr) => {{
        // Use the old implementation but warn about deprecation
        $crate::wrt_memory_system::CapabilityWrtFactory::create_provider::<$size>($crate_id)
    }};
}

#[cfg(test)]
mod tests {
    #[cfg(all(not(feature = "std"), feature = "alloc"))]
    use alloc::boxed::Box;
    #[cfg(feature = "std")]
    use std::boxed::Box;

    #[cfg(any(feature = "std", feature = "alloc"))]
    use crate::capabilities::CapabilityProviderFactory;
    use crate::{
        budget_aware_provider::CrateId,
        capabilities::DynamicMemoryCapability,
        safe_managed_alloc,
        safe_memory::{NoStdProvider, Provider},
        verification::VerificationLevel,
    };

    #[test]
    fn test_capability_context_macro() {
        let result: Result<_, wrt_error::Error> = capability_context!(dynamic(CrateId::Foundation, 1024));
        assert!(result.is_ok());

        let context = result.unwrap();
        assert!(context.has_capability(CrateId::Foundation));
    }

    #[test]
    #[cfg(any(feature = "std", feature = "alloc"))]
    fn test_capability_wrap_provider_macro() -> wrt_error::Result<()> {
        let provider = safe_managed_alloc!(1024, CrateId::Foundation)?;
        let capability = Box::new(DynamicMemoryCapability::new(
            1024,
            CrateId::Foundation,
            VerificationLevel::Standard,
        ));

        let wrapped = capability_wrap_provider!(provider, capability, CrateId::Foundation);
        assert_eq!(wrapped.capacity(), 1024);
        assert_eq!(wrapped.owner_crate(), CrateId::Foundation);
        Ok(())
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn test_safe_capability_alloc_macro() {
        let context = capability_context!(dynamic(CrateId::Foundation, 2048)).unwrap();

        let result = safe_capability_alloc!(context, CrateId::Foundation, 1024);
        assert!(result.is_ok());
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn test_safe_verified_alloc_macro() {
        let result = safe_verified_alloc!(CrateId::Foundation, 1024, VerificationLevel::Standard);
        assert!(result.is_ok());
    }
}
