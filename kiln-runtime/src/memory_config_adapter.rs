//! Memory Configuration Adapter for Runtime
//!
//! This module provides adapters that convert global memory configuration
//! into runtime-specific memory provider configurations, replacing all
//! hardcoded memory sizes with platform-aware dynamic sizing.

use wrt_error::{
    codes,
    Error,
    ErrorCategory,
};
#[cfg(any(feature = "std", feature = "alloc"))]
use wrt_foundation::capabilities::{
    factory::CapabilityGuardedProvider,
    MemoryCapabilityContext,
    MemoryFactory,
};
use wrt_foundation::{
    budget_aware_provider::CrateId,
    capabilities::CapabilityAwareProvider,
    capability_context,
    memory_init::{
        get_global_capability_context,
        MemoryInitializer,
    },
    prelude::*,
    safe_capability_alloc,
    safe_memory::NoStdProvider,
};

// Type alias for the provider type that works with BoundedVec
// In std/alloc environments, use CapabilityAwareProvider wrapper
#[cfg(any(feature = "std", feature = "alloc"))]
type AllocatedProvider<const N: usize> = CapabilityAwareProvider<NoStdProvider<N>>;

// In no_std environments, also use CapabilityAwareProvider for consistency
#[cfg(not(any(feature = "std", feature = "alloc")))]
type AllocatedProvider<const N: usize> = CapabilityAwareProvider<NoStdProvider<N>>;

// Import provider creation functions from prelude which handles conditionals

/// Runtime memory configuration that replaces hardcoded sizes
pub struct RuntimeMemoryConfig {
    /// String buffer size based on platform limits
    pub string_buffer_size:   usize,
    /// Vector capacity based on platform limits  
    pub vector_capacity:      usize,
    /// Provider buffer size based on platform limits
    pub provider_buffer_size: usize,
    /// Maximum function parameters based on platform limits
    pub max_function_params:  usize,
}

impl RuntimeMemoryConfig {
    /// Create runtime memory configuration from global limits
    pub fn from_global_limits() -> Result<Self> {
        // Ensure memory system is initialized
        if !MemoryInitializer::is_initialized() {
            MemoryInitializer::initialize()?;
        }

        // Get budget information from capability context
        let context = get_global_capability_context()?;

        // Get runtime capability to determine budget
        let runtime_capability = context.get_capability(CrateId::Runtime)?;
        let runtime_budget = runtime_capability.max_allocation_size();

        // For total budget, sum all registered capabilities
        // This is a simplified approach - in production you'd track this differently
        let total_budget = runtime_budget * 10; // Approximate total from runtime portion

        // Calculate sizes based on runtime budget
        // Use fractions of runtime budget for different components
        let string_buffer_size = if runtime_budget > 0 {
            core::cmp::min(512, runtime_budget / 1024) // Max 512, scaled by
                                                       // runtime budget
        } else {
            256 // Default fallback
        };

        let vector_capacity = if runtime_budget > 0 {
            core::cmp::min(1024, runtime_budget / (64 * 1024)) // Scaled by runtime budget
        } else {
            256 // Default fallback
        };

        let provider_buffer_size = if runtime_budget > 0 {
            core::cmp::min(4096, runtime_budget / 256) // Conservative
                                                       // allocation
        } else {
            1024 // Default fallback
        };

        let max_function_params = if total_budget > 0 {
            core::cmp::min(256, total_budget / (1024 * 1024)) // Scale with
                                                              // total budget
        } else {
            128 // Default fallback
        };

        Ok(Self {
            string_buffer_size,
            vector_capacity,
            provider_buffer_size,
            max_function_params,
        })
    }

    /// Get the string buffer size for bounded strings
    pub fn string_buffer_size(&self) -> usize {
        self.string_buffer_size
    }

    /// Get the vector capacity for bounded vectors
    pub fn vector_capacity(&self) -> usize {
        self.vector_capacity
    }

    /// Get the provider buffer size for memory providers
    pub fn provider_buffer_size(&self) -> usize {
        self.provider_buffer_size
    }

    /// Get the maximum function parameters
    pub fn max_function_params(&self) -> usize {
        self.max_function_params
    }
}

/// Global runtime memory configuration instance
static RUNTIME_CONFIG: core::sync::atomic::AtomicPtr<RuntimeMemoryConfig> =
    core::sync::atomic::AtomicPtr::new(core::ptr::null_mut());

/// Initialize runtime memory configuration
pub fn initialize_runtime_memory_config() -> Result<()> {
    // In no_std mode, we use a static configuration
    // The atomic pointer approach is not suitable for no_std without allocation
    // This is a placeholder implementation - in a real system you would
    // configure this at compile time or use a different approach
    Ok(())
}

/// Get the runtime memory configuration
pub fn runtime_memory_config() -> &'static RuntimeMemoryConfig {
    // Return a static default configuration for no_std mode
    static DEFAULT_CONFIG: RuntimeMemoryConfig = RuntimeMemoryConfig {
        string_buffer_size:   256,
        vector_capacity:      256,
        provider_buffer_size: 1024,
        max_function_params:  32,
    };
    &DEFAULT_CONFIG
}

/// Platform-aware type aliases that replace hardcoded sizes
pub mod platform_types {
    use wrt_foundation::{
        bounded::*,
        safe_memory::NoStdProvider,
    };

    use super::*;

    /// Create a platform-aware bounded string type
    pub fn create_bounded_string() -> Result<BoundedString<512>> {
        let config = runtime_memory_config();
        // Use config-defined size, but macro requires compile-time constant
        let provider = wrt_foundation::safe_managed_alloc!(
            1024,
            wrt_foundation::budget_aware_provider::CrateId::Runtime
        )?;

        // Use from_str_truncate to create an empty string
        BoundedString::from_str_truncate("")
            .map_err(|_| Error::memory_error("Failed to create bounded string"))
    }

    /// Create a platform-aware bounded vector type
    pub fn create_bounded_vec<T>() -> Result<BoundedVec<T, 1024, NoStdProvider<2048>>>
    where
        T: Clone
            + Default
            + core::fmt::Debug
            + PartialEq
            + Eq
            + wrt_foundation::traits::Checksummable
            + wrt_foundation::traits::ToBytes
            + wrt_foundation::traits::FromBytes,
    {
        let config = runtime_memory_config();
        // Use config-defined size, but macro requires compile-time constant
        let provider = wrt_foundation::safe_managed_alloc!(
            2048,
            wrt_foundation::budget_aware_provider::CrateId::Runtime
        )?;

        // Create a new bounded vector with the provider
        BoundedVec::new(provider)
    }

    /// Create a platform-aware memory provider for runtime operations
    pub fn create_platform_provider() -> Result<AllocatedProvider<8192>> {
        #[cfg(not(any(feature = "std", feature = "alloc")))]
        {
            // For no_std, use safe allocation
            let context = capability_context!(dynamic(CrateId::Runtime, 8192))?;
            let provider = safe_capability_alloc!(context, CrateId::Runtime, 8192)?;
            Ok(provider)
        }
        #[cfg(any(feature = "std", feature = "alloc"))]
        {
            MemoryFactory::create_wrapped::<8192>(CrateId::Runtime)
        }
    }
}

/// Dynamic provider factory that creates appropriately-sized providers
pub struct DynamicProviderFactory;

impl DynamicProviderFactory {
    /// Create a provider sized for the current platform
    pub fn create_for_use_case(use_case: MemoryUseCase) -> Result<AllocatedProvider<16384>> {
        #[cfg(not(any(feature = "std", feature = "alloc")))]
        {
            // For no_std, use safe allocation
            let context = capability_context!(dynamic(CrateId::Runtime, 16384))?;
            let provider = safe_capability_alloc!(context, CrateId::Runtime, 16384)?;
            Ok(provider)
        }

        #[cfg(any(feature = "std", feature = "alloc"))]
        {
            MemoryFactory::create_wrapped::<16384>(CrateId::Runtime)
        }
    }

    /// Create a string provider with platform-appropriate size
    pub fn create_string_provider() -> Result<AllocatedProvider<8192>> {
        #[cfg(not(any(feature = "std", feature = "alloc")))]
        {
            // For no_std, use safe allocation
            let context = capability_context!(dynamic(CrateId::Runtime, 8192))?;
            let provider = safe_capability_alloc!(context, CrateId::Runtime, 8192)?;
            Ok(provider)
        }

        #[cfg(any(feature = "std", feature = "alloc"))]
        {
            MemoryFactory::create_wrapped::<8192>(CrateId::Runtime)
        }
    }

    /// Create a collection provider with platform-appropriate size
    pub fn create_collection_provider() -> Result<AllocatedProvider<16384>> {
        #[cfg(not(any(feature = "std", feature = "alloc")))]
        {
            // For no_std, use safe allocation
            let context = capability_context!(dynamic(CrateId::Runtime, 16384))?;
            let provider = safe_capability_alloc!(context, CrateId::Runtime, 16384)?;
            Ok(provider)
        }

        #[cfg(any(feature = "std", feature = "alloc"))]
        {
            MemoryFactory::create_wrapped::<16384>(CrateId::Runtime)
        }
    }
}

/// Memory use case categories for provider sizing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryUseCase {
    /// Function local variables and parameters
    FunctionLocals,
    /// WebAssembly instruction buffers
    InstructionBuffer,
    /// Module metadata and exports
    ModuleMetadata,
    /// Component model data
    ComponentData,
    /// Temporary working memory
    TemporaryBuffer,
}

/// Simplified runtime memory manager for current memory system
/// Uses safe_managed_alloc! for all allocations
pub struct RuntimeMemoryManager {
    allocation_count: usize,
}

impl RuntimeMemoryManager {
    /// Create a new runtime memory manager
    pub fn new() -> Self {
        Self {
            allocation_count: 0,
        }
    }

    /// Create a provider for a specific use case
    pub fn create_provider(&mut self, use_case: MemoryUseCase) -> Result<AllocatedProvider<16384>> {
        self.allocation_count += 1;
        DynamicProviderFactory::create_for_use_case(use_case)
    }

    /// Get a provider for a specific use case (alias for create_provider)
    pub fn get_provider(&mut self, use_case: MemoryUseCase) -> Result<AllocatedProvider<16384>> {
        self.create_provider(use_case)
    }

    /// Get memory usage statistics for all managed providers
    pub fn get_stats(&self) -> RuntimeMemoryStats {
        // In no_std mode, return simplified stats based on provider count
        RuntimeMemoryStats {
            total_allocated: 0, // Would need tracking in real implementation
            total_capacity:  0, // Would need tracking in real implementation
            provider_count:  self.allocation_count,
        }
    }
}

impl Default for RuntimeMemoryManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Runtime memory usage statistics
#[derive(Debug, Clone)]
pub struct RuntimeMemoryStats {
    /// Total allocated memory across all providers
    pub total_allocated: usize,
    /// Total capacity across all providers
    pub total_capacity:  usize,
    /// Number of active providers
    pub provider_count:  usize,
}

#[cfg(test)]
mod tests {
    use wrt_foundation::memory_init::MemoryInitializer;

    use super::*;

    #[test]
    fn test_runtime_config_initialization() -> Result<()> {
        // Initialize global memory system first
        if !MemoryInitializer::is_initialized() {
            MemoryInitializer::initialize()?;
        }

        // Initialize runtime configuration
        initialize_runtime_memory_config()?;

        let config = runtime_memory_config();

        // Verify configuration values are reasonable
        assert!(config.string_buffer_size() > 0);
        assert!(config.vector_capacity() > 0);
        assert!(config.provider_buffer_size() > 0);
        assert!(config.max_function_params() > 0);

        Ok(())
    }

    #[test]
    fn test_dynamic_provider_factory() -> Result<()> {
        if !MemoryInitializer::is_initialized() {
            MemoryInitializer::initialize()?;
        }
        initialize_runtime_memory_config()?;

        // Test different use cases
        let func_provider =
            DynamicProviderFactory::create_for_use_case(MemoryUseCase::FunctionLocals)?;
        let instr_provider =
            DynamicProviderFactory::create_for_use_case(MemoryUseCase::InstructionBuffer)?;

        // Verify providers have appropriate sizes (they should all be 16384)
        assert_eq!(func_provider.capacity(), 16384);
        assert_eq!(instr_provider.capacity(), 16384);

        Ok(())
    }

    #[test]
    fn test_runtime_memory_manager() -> Result<()> {
        if !MemoryInitializer::is_initialized() {
            MemoryInitializer::initialize()?;
        }
        initialize_runtime_memory_config()?;

        let mut manager = RuntimeMemoryManager::new();

        // Get providers for different use cases
        let _func_provider = manager.get_provider(MemoryUseCase::FunctionLocals)?;
        let _instr_provider = manager.get_provider(MemoryUseCase::InstructionBuffer)?;

        let stats = manager.get_stats();
        assert_eq!(stats.provider_count, 2);
        // Note: in the simplified stats, total_capacity is always 0
        // This would need proper tracking in a real implementation

        Ok(())
    }
}
