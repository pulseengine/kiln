//! Engine Factory Pattern Implementation
//!
//! This module provides factory patterns for creating different types of
//! WebAssembly engines with clear separation of concerns and configurable
//! capabilities.

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::{
    boxed::Box,
    vec,
    vec::Vec,
};
#[cfg(feature = "std")]
use alloc::{
    boxed::Box,
    vec,
    vec::Vec,
};

use wrt_error::{
    Error,
    Result,
};
use wrt_foundation::CrateId;

use crate::prelude::*;

/// Available engine types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineType {
    /// Core stackless engine for minimal overhead
    Stackless,
    /// Full capability-aware engine for production
    CapabilityAware,
    /// WAST testing engine for test suite execution
    Wast,
}

/// Memory provider configuration types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryProviderType {
    /// Basic memory provider for core functionality
    Basic,
    /// Capability-aware provider with security checks
    CapabilityAware,
    /// Test-optimized provider with extended buffers
    Test,
}

/// Engine configuration builder
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Type of engine to create
    pub engine_type:     EngineType,
    /// Memory provider configuration
    pub memory_provider: MemoryProviderType,
    /// Initial memory budget in bytes
    pub memory_budget:   usize,
    /// Whether to enable debug features
    pub debug_mode:      bool,
    /// Maximum number of function calls
    pub max_call_depth:  Option<u32>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            engine_type:     EngineType::Stackless,
            memory_provider: MemoryProviderType::Basic,
            memory_budget:   65536, // 64KB default
            debug_mode:      false,
            max_call_depth:  Some(1024),
        }
    }
}

impl EngineConfig {
    /// Create a new engine configuration
    pub fn new(engine_type: EngineType) -> Self {
        Self {
            engine_type,
            ..Default::default()
        }
    }

    /// Set memory provider type
    pub fn with_memory_provider(mut self, provider_type: MemoryProviderType) -> Self {
        self.memory_provider = provider_type;
        self
    }

    /// Set memory budget
    pub fn with_memory_budget(mut self, budget: usize) -> Self {
        self.memory_budget = budget;
        self
    }

    /// Enable debug mode
    pub fn with_debug_mode(mut self, debug: bool) -> Self {
        self.debug_mode = debug;
        self
    }

    /// Set maximum call depth
    pub fn with_max_call_depth(mut self, depth: u32) -> Self {
        self.max_call_depth = Some(depth);
        self
    }
}

/// Main engine factory
pub struct EngineFactory;

impl EngineFactory {
    /// Create an engine with the specified configuration
    pub fn create(config: EngineConfig) -> Result<Box<dyn RuntimeEngine>> {
        // Create memory provider based on configuration
        Self::create_memory_provider(config.memory_provider, config.memory_budget)?;

        match config.engine_type {
            EngineType::Stackless => {
                // Create basic stackless engine for minimal overhead
                let engine = crate::stackless::StacklessEngine::new();
                Ok(Box::new(engine))
            },
            EngineType::CapabilityAware => {
                // Create capability-aware engine with security checks
                // For now using StacklessEngine as base, but with capability-aware memory
                // provider
                let engine = crate::stackless::StacklessEngine::new();
                Ok(Box::new(engine))
            },
            EngineType::Wast => {
                // Create WAST testing engine with extended testing capabilities
                let engine = crate::stackless::StacklessEngine::new();
                Ok(Box::new(engine))
            },
        }
    }

    /// Create a memory provider based on configuration
    fn create_memory_provider(provider_type: MemoryProviderType, _budget: usize) -> Result<()> {
        use wrt_foundation::{
            capabilities::MemoryFactory,
            CrateId,
        };

        match provider_type {
            MemoryProviderType::Basic => {
                // Use basic memory provider for core functionality
                let _provider = MemoryFactory::create::<4096>(CrateId::Runtime)?;
                Ok(())
            },
            MemoryProviderType::CapabilityAware => {
                // Use capability-aware provider with security checks
                let _provider = MemoryFactory::create::<8192>(CrateId::Runtime)?;
                Ok(())
            },
            MemoryProviderType::Test => {
                // Use test-optimized provider with extended buffers
                let _provider = MemoryFactory::create::<16384>(CrateId::Runtime)?;
                Ok(())
            },
        }
    }

    /// Create a preconfigured stackless engine
    pub fn stackless() -> Result<Box<dyn RuntimeEngine>> {
        Self::create(EngineConfig::new(EngineType::Stackless))
    }

    /// Create a preconfigured capability-aware engine
    pub fn capability_aware() -> Result<Box<dyn RuntimeEngine>> {
        Self::create(
            EngineConfig::new(EngineType::CapabilityAware)
                .with_memory_provider(MemoryProviderType::CapabilityAware)
                .with_memory_budget(262144), // 256KB for production
        )
    }

    /// Create a preconfigured WAST testing engine
    #[cfg(feature = "std")]
    pub fn wast_testing() -> Result<Box<dyn RuntimeEngine>> {
        Self::create(
            EngineConfig::new(EngineType::Wast)
                .with_memory_provider(MemoryProviderType::Test)
                .with_memory_budget(524288) // 512KB for extensive testing
                .with_debug_mode(true),
        )
    }
}

/// Trait for runtime engines to ensure consistent interface
pub trait RuntimeEngine {
    /// Load a WebAssembly module
    fn load_module(&mut self, name: Option<&str>, binary: &[u8]) -> Result<()>;

    /// Invoke a function by name
    fn invoke_function(
        &mut self,
        module: Option<&str>,
        function: &str,
        args: &[wrt_foundation::Value],
    ) -> Result<Vec<wrt_foundation::Value>>;

    /// Get engine statistics
    fn get_statistics(&self) -> EngineStatistics;

    /// Set the host function registry for imported function calls
    #[cfg(feature = "std")]
    fn set_host_registry(&mut self, registry: std::sync::Arc<wrt_host::CallbackRegistry>);
}

// Implement RuntimeEngine for StacklessEngine
impl RuntimeEngine for crate::stackless::StacklessEngine {
    fn load_module(&mut self, name: Option<&str>, binary: &[u8]) -> Result<()> {
        // For now, just return Ok - proper implementation would parse and load module
        let _ = (name, binary);
        Ok(())
    }

    fn invoke_function(
        &mut self,
        module: Option<&str>,
        function: &str,
        args: &[wrt_foundation::Value],
    ) -> Result<Vec<wrt_foundation::Value>> {
        // For now, return empty result - proper implementation would execute function
        let _ = (module, function, args);
        // Return empty result using bounded collection
        #[cfg(feature = "std")]
        {
            Ok(vec![])
        }
        #[cfg(not(feature = "std"))]
        {
            use wrt_foundation::bounded::BoundedVec;
            let provider = wrt_foundation::safe_managed_alloc!(
                512,
                wrt_foundation::budget_aware_provider::CrateId::Runtime
            )?;
            let bounded_vec = BoundedVec::<wrt_foundation::values::Value, 16, _>::new(provider)?;
            Ok(bounded_vec.into_vec())
        }
    }

    fn get_statistics(&self) -> EngineStatistics {
        EngineStatistics::default()
    }

    #[cfg(feature = "std")]
    fn set_host_registry(&mut self, registry: std::sync::Arc<wrt_host::CallbackRegistry>) {
        self.set_host_registry(registry);
    }
}

/// Engine execution statistics
#[derive(Debug, Default, Clone)]
pub struct EngineStatistics {
    /// Number of modules loaded.
    pub modules_loaded:          u32,
    /// Number of functions executed.
    pub functions_executed:      u64,
    /// Total execution time in milliseconds.
    pub total_execution_time_ms: u64,
    /// Current memory usage in bytes.
    pub memory_used:             usize,
    /// Maximum memory usage observed in bytes.
    pub max_memory_used:         usize,
}

// WAST engine adapter removed - using StacklessEngine directly for now

/// Lazy engine wrapper for deferred initialization
pub struct LazyEngine {
    config: EngineConfig,
    engine: Option<Box<dyn RuntimeEngine>>,
}

impl LazyEngine {
    /// Create a new lazy engine with the specified configuration
    pub fn new(config: EngineConfig) -> Self {
        Self {
            config,
            engine: None,
        }
    }

    /// Get or create the underlying engine
    pub fn get_or_create(&mut self) -> Result<&mut dyn RuntimeEngine> {
        if self.engine.is_none() {
            self.engine = Some(EngineFactory::create(self.config.clone())?);
        }
        Ok(self.engine.as_mut().unwrap().as_mut())
    }

    /// Check if the engine has been initialized
    pub fn is_initialized(&self) -> bool {
        self.engine.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_config_builder() {
        let config = EngineConfig::new(EngineType::Stackless)
            .with_memory_budget(131072)
            .with_debug_mode(true)
            .with_max_call_depth(512);

        assert_eq!(config.engine_type, EngineType::Stackless);
        assert_eq!(config.memory_budget, 131072);
        assert_eq!(config.debug_mode, true);
        assert_eq!(config.max_call_depth, Some(512));
    }

    #[test]
    fn test_memory_provider_types() {
        assert_ne!(
            MemoryProviderType::Basic,
            MemoryProviderType::CapabilityAware
        );
        assert_ne!(MemoryProviderType::Basic, MemoryProviderType::Test);
        assert_ne!(
            MemoryProviderType::CapabilityAware,
            MemoryProviderType::Test
        );
    }

    #[test]
    fn test_engine_types() {
        assert_ne!(EngineType::Stackless, EngineType::CapabilityAware);
        assert_ne!(EngineType::Stackless, EngineType::Wast);
        assert_ne!(EngineType::CapabilityAware, EngineType::Wast);
    }

    #[test]
    fn test_lazy_engine_creation() {
        let config = EngineConfig::new(EngineType::Stackless);
        let lazy_engine = LazyEngine::new(config);

        assert!(!lazy_engine.is_initialized());
    }

    #[test]
    fn test_engine_statistics() {
        let mut stats = EngineStatistics::default();
        stats.modules_loaded = 5;
        stats.functions_executed = 100;

        assert_eq!(stats.modules_loaded, 5);
        assert_eq!(stats.functions_executed, 100);
    }
}
