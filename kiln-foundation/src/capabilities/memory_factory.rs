//! Memory Factory - Unified Memory Provider Creation
//!
//! This module provides a single, simple factory for creating memory providers
//! with capability verification. Replaces all previous factory patterns.

#[cfg(any(feature = "std", feature = "alloc"))]
use super::provider_bridge::CapabilityAwareProvider;
use super::{
    context::MemoryCapabilityContext,
    MemoryOperation,
};
use crate::{
    budget_aware_provider::CrateId,
    memory_init::get_global_capability_context,
    safe_memory::{
        NoStdProvider,
        Provider,
    },
    safety_monitor::with_safety_monitor,
    telemetry::{
        self,
        event_codes,
        Category,
        Severity,
    },
    Result,
};

/// Unified memory provider factory
///
/// This factory provides a simple API for creating memory providers with
/// capability verification. It replaces SafeProviderFactory and
/// CapabilityMemoryFactory.
pub struct MemoryFactory;

impl MemoryFactory {
    /// Create a memory provider with capability verification
    ///
    /// This is the primary method for creating providers. It uses the global
    /// capability context and returns a standard NoStdProvider.
    ///
    /// # Arguments
    /// * `crate_id` - The crate requesting the provider
    ///
    /// # Returns
    /// * `Ok(NoStdProvider<N>)` - A verified provider ready to use
    /// * `Err(Error)` - If capability verification fails
    pub fn create<const N: usize>(crate_id: CrateId) -> Result<NoStdProvider<N>> {
        let context = get_global_capability_context()?;
        Self::create_with_context(context, crate_id)
    }

    /// Create a memory provider with explicit context
    ///
    /// Use this when you have a specific capability context to use.
    ///
    /// # Arguments
    /// * `context` - The capability context to use for verification
    /// * `crate_id` - The crate requesting the provider
    ///
    /// # Returns
    /// * `Ok(NoStdProvider<N>)` - A verified provider ready to use
    /// * `Err(Error)` - If capability verification fails
    pub fn create_with_context<const N: usize>(
        context: &MemoryCapabilityContext,
        crate_id: CrateId,
    ) -> Result<NoStdProvider<N>> {
        // Verify allocation capability
        let operation = MemoryOperation::Allocate { size: N };
        let verification_result = context.verify_operation(crate_id, &operation);

        // Record safety monitoring events
        with_safety_monitor(|monitor| {
            match &verification_result {
                Ok(_) => {
                    // Record successful allocation
                    monitor.record_allocation(N);
                    // Record telemetry for successful allocation
                    telemetry::record_event(
                        Severity::Info,
                        Category::Memory,
                        event_codes::MEM_ALLOC_SUCCESS,
                        N as u64,
                        crate_id as u64,
                    );
                },
                Err(_) => {
                    // Record allocation failure and capability violation
                    monitor.record_allocation_failure(N);
                    monitor.record_capability_violation(crate_id);
                    // Record telemetry for allocation failure
                    telemetry::record_event(
                        Severity::Error,
                        Category::Memory,
                        event_codes::MEM_ALLOC_FAILURE,
                        N as u64,
                        crate_id as u64,
                    );
                    // Record capability violation
                    telemetry::record_event(
                        Severity::Critical,
                        Category::Capability,
                        event_codes::CAP_VIOLATION,
                        N as u64,
                        crate_id as u64,
                    );
                },
            }
        });

        // Return verification result
        verification_result?;

        // Create the provider directly to avoid circular dependency
        // The capability verification above ensures this allocation is authorized
        Ok(NoStdProvider::<N>::default())
    }

    /// Create a capability-aware provider wrapper
    ///
    /// Use this when you need a provider that implements the Provider trait
    /// with built-in capability verification for all operations.
    ///
    /// # Arguments
    /// * `crate_id` - The crate requesting the provider
    ///
    /// # Returns
    /// * `Ok(CapabilityAwareProvider<NoStdProvider<N>>)` - A wrapped provider
    /// * `Err(Error)` - If capability verification fails
    #[cfg(any(feature = "std", feature = "alloc"))]
    pub fn create_wrapped<const N: usize>(
        crate_id: CrateId,
    ) -> Result<CapabilityAwareProvider<NoStdProvider<N>>> {
        let context = get_global_capability_context()?;
        Self::create_wrapped_with_context(context, crate_id)
    }

    /// Create a capability-aware provider wrapper with explicit context
    ///
    /// # Arguments
    /// * `context` - The capability context to use for verification
    /// * `crate_id` - The crate requesting the provider
    ///
    /// # Returns
    /// * `Ok(CapabilityAwareProvider<NoStdProvider<N>>)` - A wrapped provider
    /// * `Err(Error)` - If capability verification fails
    #[cfg(any(feature = "std", feature = "alloc"))]
    pub fn create_wrapped_with_context<const N: usize>(
        context: &MemoryCapabilityContext,
        crate_id: CrateId,
    ) -> Result<CapabilityAwareProvider<NoStdProvider<N>>> {
        // Get the capability for this crate
        let capability_result = context.get_capability(crate_id);

        // Verify allocation operation
        let operation = MemoryOperation::Allocate { size: N };
        let verification_result = match &capability_result {
            Ok(capability) => capability.verify_access(&operation),
            Err(e) => Err(*e),
        };

        // Record safety monitoring events
        with_safety_monitor(|monitor| {
            match &verification_result {
                Ok(_) => {
                    // Record successful allocation
                    monitor.record_allocation(N);
                },
                Err(_) => {
                    // Record allocation failure and capability violation
                    monitor.record_allocation_failure(N);
                    monitor.record_capability_violation(crate_id);
                },
            }
        });

        // Return verification result
        let capability = capability_result?;
        verification_result?;

        // Create the underlying provider directly to avoid circular dependency
        // The capability verification above ensures this allocation is authorized
        // FIX: Use heap allocation for large providers to avoid stack overflow
        let provider = if N > 8192 {
            NoStdProvider::<N>::new_heap_allocated()
        } else {
            NoStdProvider::<N>::default()
        };

        // Wrap with capability verification
        Ok(CapabilityAwareProvider::new(
            provider,
            capability.clone_capability(),
            crate_id,
        ))
    }

    /// Create a provider with explicit capability verification level
    ///
    /// This method verifies that the requesting crate has the necessary
    /// capability with the required verification level before creating the
    /// provider.
    ///
    /// # Arguments
    /// * `crate_id` - The crate requesting the provider
    /// * `required_verification_level` - The minimum verification level
    ///   required
    ///
    /// # Returns
    /// * `Ok(NoStdProvider<N>)` - A verified provider ready to use
    /// * `Err(Error)` - If capability verification fails or verification level
    ///   is insufficient
    pub fn create_verified<const N: usize>(
        crate_id: CrateId,
        required_verification_level: crate::verification::VerificationLevel,
    ) -> Result<NoStdProvider<N>> {
        let context = get_global_capability_context()?;
        Self::create_verified_with_context(context, crate_id, required_verification_level)
    }

    /// Create a provider with explicit context and verification level
    ///
    /// # Arguments
    /// * `context` - The capability context to use for verification
    /// * `crate_id` - The crate requesting the provider
    /// * `required_verification_level` - The minimum verification level
    ///   required
    ///
    /// # Returns
    /// * `Ok(NoStdProvider<N>)` - A verified provider ready to use
    /// * `Err(Error)` - If capability verification fails or verification level
    ///   is insufficient
    pub fn create_verified_with_context<const N: usize>(
        context: &MemoryCapabilityContext,
        crate_id: CrateId,
        required_verification_level: crate::verification::VerificationLevel,
    ) -> Result<NoStdProvider<N>> {
        let capability_result = context.get_capability(crate_id);

        // Check verification levels and perform allocation verification
        let final_result = match &capability_result {
            Ok(capability) => {
                // Check if capability meets required verification level
                if capability.verification_level() < required_verification_level {
                    Err(crate::Error::runtime_execution_error(
                        "Capability verification level too low for verified provider",
                    ))
                } else {
                    // Verify allocation operation
                    let operation = MemoryOperation::Allocate { size: N };
                    capability.verify_access(&operation)
                }
            },
            Err(e) => Err(*e),
        };

        // Record safety monitoring events
        with_safety_monitor(|monitor| {
            match &final_result {
                Ok(_) => {
                    // Record successful allocation
                    monitor.record_allocation(N);
                },
                Err(_) => {
                    // Record allocation failure and capability violation
                    monitor.record_allocation_failure(N);
                    monitor.record_capability_violation(crate_id);
                },
            }
        });

        // Return verification result
        final_result?;

        // Create the provider directly to avoid circular dependency
        // The capability verification above ensures this allocation is authorized
        Ok(NoStdProvider::<N>::default())
    }

    /// Get current safety monitoring report
    ///
    /// This provides insights into memory allocation safety and system health.
    ///
    /// # Returns
    /// * `SafetyReport` - Current safety metrics including health score
    pub fn get_safety_report() -> crate::safety_monitor::SafetyReport {
        with_safety_monitor(|monitor| {
            let report = monitor.get_safety_report();

            // Record telemetry for health status
            if report.health_score < 80 {
                let critical_violations =
                    report.budget_violations + report.capability_violations + report.fatal_errors;
                telemetry::record_event(
                    Severity::Warning,
                    Category::Safety,
                    event_codes::SAFETY_HEALTH_DEGRADED,
                    report.health_score as u64,
                    critical_violations,
                );
            }

            report
        })
    }

    /// Check if the memory allocation system is healthy
    ///
    /// # Returns
    /// * `true` if system health score >= 80
    /// * `false` if system is experiencing safety issues
    pub fn is_system_healthy() -> bool {
        with_safety_monitor(|monitor| monitor.is_healthy())
    }

    /// Get count of critical safety violations
    ///
    /// # Returns
    /// * Total count of budget violations, capability violations, double-frees,
    ///   and fatal errors
    pub fn get_critical_violations() -> u64 {
        with_safety_monitor(|monitor| monitor.get_critical_violations())
    }

    /// Record manual deallocation for safety tracking
    ///
    /// Call this when memory is manually deallocated to maintain accurate
    /// tracking.
    ///
    /// # Arguments
    /// * `size` - Size of memory being deallocated
    pub fn record_deallocation(size: usize) {
        with_safety_monitor(|monitor| {
            monitor.record_deallocation(size);
            // Record telemetry for deallocation
            telemetry::record_event(
                Severity::Info,
                Category::Memory,
                event_codes::MEMORY_DEALLOCATION,
                size as u64,
                0, // No specific crate context for manual deallocation
            );
        });
    }

    /// Enter a module-level memory scope with budget tracking
    ///
    /// This creates a scope using the VerifiedAllocator for the specified crate.
    /// All Vec/Box allocations within this scope will be tracked and limited
    /// by the budget. When the returned ScopeGuard drops, memory resets to
    /// the checkpoint.
    ///
    /// # Arguments
    /// * `crate_id` - The crate entering the scope
    ///
    /// # Returns
    /// * `Ok(ScopeGuard)` - RAII guard that exits scope on drop
    /// * `Err` - If scope creation fails
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use wrt_foundation::{
    ///     capabilities::MemoryFactory,
    ///     budget_aware_provider::CrateId,
    /// };
    ///
    /// // Enter a module parsing scope
    /// let _scope = MemoryFactory::enter_module_scope(CrateId::Decoder)?;
    ///
    /// // All Vec allocations now tracked against 64KB budget
    /// let mut functions = Vec::new();  // Uses VerifiedAllocator
    /// functions.push(parse_function()?);
    ///
    /// // When _scope drops, memory resets to checkpoint
    /// # Ok::<(), wrt_foundation::Error>(())
    /// ```
    pub fn enter_module_scope(
        crate_id: CrateId,
    ) -> Result<crate::verified_allocator::ScopeGuard<'static>> {
        use crate::verified_allocator;

        // Get the allocator for this crate
        let allocator = verified_allocator::global_allocators::get_crate_allocator(crate_id);

        // Enter scope with MAX_MODULE_SIZE budget (64 KB)
        allocator.enter_scope(crate_id, verified_allocator::MAX_MODULE_SIZE)
    }

    /// Enter a custom-sized memory scope
    ///
    /// Like `enter_module_scope` but with a custom budget size.
    ///
    /// # Arguments
    /// * `crate_id` - The crate entering the scope
    /// * `budget` - Maximum bytes this scope can allocate
    ///
    /// # Returns
    /// * `Ok(ScopeGuard)` - RAII guard that exits scope on drop
    /// * `Err` - If scope creation fails
    pub fn enter_scope(
        crate_id: CrateId,
        budget: usize,
    ) -> Result<crate::verified_allocator::ScopeGuard<'static>> {
        use crate::verified_allocator;

        let allocator = verified_allocator::global_allocators::get_crate_allocator(crate_id);
        allocator.enter_scope(crate_id, budget)
    }

    /// Example demonstrating integrated safety monitoring and telemetry
    ///
    /// This shows how MemoryFactory automatically tracks safety metrics
    /// and emits telemetry events for production monitoring.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use wrt_foundation::{
    ///     capabilities::MemoryFactory,
    ///     budget_aware_provider::CrateId,
    ///     telemetry_info,
    /// };
    ///
    /// // Memory allocations are automatically monitored
    /// let provider = MemoryFactory::create::<4096>(CrateId::Foundation)?;
    ///
    /// // Check system health
    /// if !MemoryFactory::is_system_healthy() {
    ///     let report = MemoryFactory::get_safety_report();
    ///     eprintln!("System health degraded: score={}", report.health_score)));
    /// }
    ///
    /// // Manual deallocation tracking
    /// MemoryFactory::record_deallocation(4096);
    ///
    /// // Get comprehensive safety metrics
    /// let report = MemoryFactory::get_safety_report();
    /// println!("Allocations: {}, Failures: {}, Health: {}",
    ///          report.total_allocations,
    ///          report.failed_allocations,
    ///          report.health_score);
    /// # Ok::<(), wrt_foundation::Error>(())
    /// ```
    #[cfg(doc)]
    pub fn example() {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_factory_api() {
        // Test that the API compiles correctly
        let _test_fn =
            || -> Result<NoStdProvider<1024>> { MemoryFactory::create(CrateId::Foundation) };

        let _test_fn2 = |context: &MemoryCapabilityContext| -> Result<NoStdProvider<1024>> {
            MemoryFactory::create_with_context(context, CrateId::Foundation)
        };

        #[cfg(any(feature = "std", feature = "alloc"))]
        let _test_fn3 = || -> Result<CapabilityAwareProvider<NoStdProvider<1024>>> {
            MemoryFactory::create_wrapped(CrateId::Foundation)
        };
    }

    #[test]
    fn test_safety_monitor_integration() {
        // Reset monitor for clean test state
        with_safety_monitor(|monitor| {
            #[cfg(test)]
            monitor.reset();
        });

        // Initial state should be healthy
        assert!(MemoryFactory::is_system_healthy());
        assert_eq!(MemoryFactory::get_critical_violations(), 0);

        // Test safety monitoring access
        let initial_report = MemoryFactory::get_safety_report();
        assert_eq!(initial_report.total_allocations, 0);
        assert_eq!(initial_report.health_score, 100);
    }

    #[test]
    fn test_safety_monitor_allocation_tracking() {
        // Reset monitor for clean test state
        with_safety_monitor(|monitor| {
            #[cfg(test)]
            monitor.reset();
        });

        // Create a capability context for testing
        use crate::{
            capabilities::MemoryCapabilityContext,
            verification::VerificationLevel,
        };
        let mut context = MemoryCapabilityContext::new(VerificationLevel::Standard, false);

        // Register a capability for testing
        let _ = context.register_dynamic_capability(CrateId::Foundation, 4096);

        // Test successful allocation tracking
        let result = MemoryFactory::create_with_context::<1024>(&context, CrateId::Foundation);
        assert!(result.is_ok());

        // Verify safety monitoring recorded the allocation
        let report = MemoryFactory::get_safety_report();
        assert_eq!(report.total_allocations, 1);
        assert_eq!(report.failed_allocations, 0);
        assert_eq!(report.current_memory_bytes, 1024);
        assert!(MemoryFactory::is_system_healthy());

        // Test deallocation tracking
        MemoryFactory::record_deallocation(1024);
        let report = MemoryFactory::get_safety_report();
        assert_eq!(report.current_memory_bytes, 0);
    }

    #[test]
    fn test_safety_monitor_failure_tracking() {
        // Reset monitor for clean test state
        with_safety_monitor(|monitor| {
            #[cfg(test)]
            monitor.reset();
        });

        // Create a capability context with no capabilities
        use crate::{
            capabilities::MemoryCapabilityContext,
            verification::VerificationLevel,
        };
        let context = MemoryCapabilityContext::new(VerificationLevel::Standard, false);

        // Test failed allocation tracking
        let result = MemoryFactory::create_with_context::<1024>(&context, CrateId::Foundation);
        assert!(result.is_err());

        // Verify safety monitoring recorded the failure
        let report = MemoryFactory::get_safety_report();
        assert_eq!(report.total_allocations, 0); // No successful allocations
        assert_eq!(report.failed_allocations, 1);
        assert_eq!(report.capability_violations, 1);
        assert!(report.health_score < 100); // Health should be degraded

        // System should still be functional but with recorded violations
        assert_eq!(MemoryFactory::get_critical_violations(), 1);
    }

    #[test]
    fn test_safety_monitor_verification_level_tracking() {
        // Reset monitor for clean test state
        with_safety_monitor(|monitor| {
            #[cfg(test)]
            monitor.reset();
        });

        // Create a capability context with low verification level
        use crate::{
            capabilities::MemoryCapabilityContext,
            verification::VerificationLevel,
        };
        let mut context = MemoryCapabilityContext::new(VerificationLevel::Basic, false);

        // Register a basic capability
        let _ = context.register_dynamic_capability(CrateId::Foundation, 4096);

        // Test that requesting higher verification level fails and is tracked
        let result = MemoryFactory::create_verified_with_context::<1024>(
            &context,
            CrateId::Foundation,
            VerificationLevel::Redundant,
        );
        assert!(result.is_err());

        // Verify safety monitoring recorded the violation
        let report = MemoryFactory::get_safety_report();
        assert_eq!(report.failed_allocations, 1);
        assert_eq!(report.capability_violations, 1);
    }
}
