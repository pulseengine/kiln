//! Unified Type System for WRT Runtime - CRITICAL COMPILATION FIX
//!
//! This module provides a unified type system that resolves the 421+
//! compilation errors caused by incompatible bounded collection capacities
//! across crates. It implements platform-configurable memory providers and
//! collection types that can be externally configured based on platform limits.

use core::marker::PhantomData;

use wrt_error::{
    Error,
    ErrorCategory,
};
use wrt_foundation::{
    bounded::{
        BoundedString,
        BoundedVec,
    },
    bounded_collections::BoundedMap,
    prelude::{
        BoundedCapacity,
        Clone,
        Copy,
        Debug,
        Default,
        Eq,
        PartialEq,
        Value,
    },
    safe_memory::{
        MemoryProvider,
        NoStdProvider,
    },
    traits::{
        Checksummable,
        FromBytes,
        ToBytes,
    },
};

use crate::bounded_runtime_infra::{
    DefaultRuntimeProvider,
    RuntimeProvider,
};

// =============================================================================
// PLATFORM-AWARE CAPACITY CONSTANTS
// =============================================================================
// These must be externally configurable based on platform limits

/// Platform-specific capacity configuration for runtime types
///
/// This struct allows external configuration of collection capacities based on
/// platform memory constraints and safety requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformCapacities {
    /// Small collection capacity (default: 64) - for locals, small arrays
    pub small_capacity:       usize,
    /// Medium collection capacity (default: 1024) - for instructions, values
    pub medium_capacity:      usize,
    /// Large collection capacity (default: 65536) - for memory buffers, large
    /// data
    pub large_capacity:       usize,
    /// Memory provider size in bytes (default: 1MB)
    pub memory_provider_size: usize,
}

impl PlatformCapacities {
    /// Default capacities for general-purpose platforms
    #[must_use]
    pub const fn default() -> Self {
        Self {
            small_capacity:       64,
            medium_capacity:      1024,
            large_capacity:       65536,
            memory_provider_size: 1048576, // 1MB
        }
    }

    /// Reduced capacities for embedded platforms with limited memory
    #[must_use]
    pub const fn embedded() -> Self {
        Self {
            small_capacity:       16,
            medium_capacity:      256,
            large_capacity:       8192,
            memory_provider_size: 32768, // 32KB
        }
    }

    /// Safety-critical configuration with conservative limits
    #[must_use]
    pub const fn safety_critical() -> Self {
        Self {
            small_capacity:       32,
            medium_capacity:      512,
            large_capacity:       16384,
            memory_provider_size: 65536, // 64KB
        }
    }
}

/// Backward compatibility constants
pub const SMALL_CAPACITY: usize = PlatformCapacities::default().small_capacity;
/// Medium capacity for backward compatibility
pub const MEDIUM_CAPACITY: usize = PlatformCapacities::default().medium_capacity;
/// Large capacity for backward compatibility
pub const LARGE_CAPACITY: usize = PlatformCapacities::default().large_capacity;

// =============================================================================
// RUNTIME-CONFIGURABLE TYPE DEFINITIONS
// =============================================================================

// RuntimeProvider definitions moved to bounded_runtime_infra.rs to avoid
// conflicts Use crate::bounded_runtime_infra::RuntimeProvider instead

/// Embedded runtime provider with reduced capacity
pub type EmbeddedRuntimeProvider =
    NoStdProvider<{ PlatformCapacities::embedded().memory_provider_size }>;

/// Safety-critical runtime provider with conservative capacity
pub type SafetyCriticalRuntimeProvider =
    NoStdProvider<{ PlatformCapacities::safety_critical().memory_provider_size }>;

/// Universal bounded collection types with runtime configuration support
///
/// This struct provides type aliases for bounded collections with configurable
/// capacities and memory providers. It uses const generics to allow
/// compile-time configuration while maintaining type safety.
pub struct RuntimeTypes<
    const SMALL: usize = 64,
    const MEDIUM: usize = 1024,
    const LARGE: usize = 65536,
    Provider = DefaultRuntimeProvider,
> {
    _phantom: PhantomData<Provider>,
}

// Create concrete type aliases for the default runtime configuration
/// Default small bounded vector (64 elements) - T must implement required
/// traits
pub type DefaultSmallVec<T> = BoundedVec<T, 64, DefaultRuntimeProvider>;

/// Default medium bounded vector (1024 elements) - T must implement required
/// traits
pub type DefaultMediumVec<T> = BoundedVec<T, 1024, DefaultRuntimeProvider>;

/// Default large bounded vector (65536 elements) - T must implement required
/// traits
pub type DefaultLargeVec<T> = BoundedVec<T, 65536, DefaultRuntimeProvider>;

/// Default small bounded string
pub type DefaultSmallString = BoundedString<64>;

/// Default medium bounded string
pub type DefaultMediumString = BoundedString<1024>;

/// Default large bounded string
pub type DefaultLargeString = BoundedString<65536>;

/// Default runtime map - K and V must implement required traits
pub type DefaultRuntimeMap<K, V> = BoundedMap<K, V, 1024, DefaultRuntimeProvider>;

// =============================================================================
// PRE-CONFIGURED TYPE ALIASES FOR COMMON PLATFORMS
// =============================================================================

/// Default runtime types for backward compatibility and general use
pub type DefaultRuntimeTypes = RuntimeTypes<64, 1024, 65536, DefaultRuntimeProvider>;

/// Embedded runtime types for resource-constrained platforms
pub type EmbeddedRuntimeTypes = RuntimeTypes<16, 256, 8192, EmbeddedRuntimeProvider>;

/// Safety-critical runtime types with conservative limits
pub type SafetyCriticalRuntimeTypes = RuntimeTypes<32, 512, 16384, SafetyCriticalRuntimeProvider>;

// =============================================================================
// CORE RUNTIME COLLECTION ALIASES
// =============================================================================

/// Core runtime collection aliases using default capacities
/// These provide consistent types across the entire runtime system.
/// Vector for local variables in function execution
pub type LocalsVec = BoundedVec<Value, 64, DefaultRuntimeProvider>;

/// Stack for WebAssembly values during execution  
pub type ValueStackVec = BoundedVec<Value, 1024, DefaultRuntimeProvider>;

/// Vector for WebAssembly instructions (using u8 for now due to instruction
/// complexity)
pub type InstructionVec = BoundedVec<u8, 65536, DefaultRuntimeProvider>;

/// Buffer for linear memory content
pub type MemoryBuffer = BoundedVec<u8, 65536, DefaultRuntimeProvider>;

/// String for runtime identifiers and names
pub type RuntimeString = BoundedString<1024>;

/// String for component and module names
pub type ComponentName = BoundedString<64>;

/// Map for storing exports by name (using BTreeMap-style bounded map)
/// Note: T must implement Sized + Checksummable + `ToBytes` + `FromBytes` +
/// Default + Clone + `PartialEq` + Eq
pub type ExportMap<T> = BoundedMap<RuntimeString, T, 1024, DefaultRuntimeProvider>;

/// Map for storing imports by name
/// Note: T must implement Sized + Checksummable + `ToBytes` + `FromBytes` +
/// Default + Clone + `PartialEq` + Eq
pub type ImportMap<T> = BoundedMap<RuntimeString, T, 1024, DefaultRuntimeProvider>;

/// Vector for function parameters
pub type ParameterVec = BoundedVec<Value, 64, DefaultRuntimeProvider>;

/// Vector for function results
pub type ResultVec = BoundedVec<Value, 64, DefaultRuntimeProvider>;

// =============================================================================
// MEMORY ADAPTER UNIFICATION
// =============================================================================

/// Unified memory interface for all runtime components
///
/// This trait provides a common interface for memory management across
/// different runtime components, allowing platform-specific implementations
/// while maintaining a consistent API.
pub trait UnifiedMemoryAdapter: Send + Sync {
    /// The memory provider type used by this adapter
    type Provider: MemoryProvider;

    /// The error type returned by memory operations
    type Error: core::fmt::Debug;

    /// Allocate a block of memory of the specified size
    fn allocate(&mut self, size: usize) -> core::result::Result<&mut [u8], Self::Error>;

    /// Deallocate a previously allocated block of memory
    fn deallocate(&mut self, ptr: &mut [u8]) -> core::result::Result<(), Self::Error>;

    /// Get the amount of available memory
    fn available_memory(&self) -> usize;

    /// Get the total memory capacity
    fn total_memory(&self) -> usize;

    /// Get a reference to the underlying memory provider
    fn provider(&self) -> &Self::Provider;
}

/// Platform-configurable memory adapter
///
/// This adapter provides memory management with platform-specific limits
/// and safety constraints. It integrates with the unified type system
/// to provide consistent memory allocation across the runtime.
#[derive(Debug)]
pub struct PlatformMemoryAdapter<Provider = DefaultRuntimeProvider>
where
    Provider: MemoryProvider + Default,
{
    provider:        Provider,
    allocated_bytes: usize,
    max_memory:      usize,
}

impl<Provider> PlatformMemoryAdapter<Provider>
where
    Provider: MemoryProvider + Default,
{
    /// Create a new platform memory adapter with the specified memory limit
    pub fn new(max_memory: usize) -> core::result::Result<Self, Error> {
        Ok(Self {
            provider: Provider::default(),
            allocated_bytes: 0,
            max_memory,
        })
    }
}

impl<Provider> UnifiedMemoryAdapter for PlatformMemoryAdapter<Provider>
where
    Provider: MemoryProvider + Default,
{
    type Error = Error;
    type Provider = Provider;

    fn allocate(&mut self, size: usize) -> core::result::Result<&mut [u8], Self::Error> {
        if self.allocated_bytes + size > self.max_memory {
            return Err(Error::runtime_execution_error(
                "Memory allocation limit exceeded",
            ));
        }

        self.allocated_bytes += size;

        // Placeholder - real implementation would use provider
        Err(Error::new(
            ErrorCategory::Memory,
            wrt_error::codes::NOT_IMPLEMENTED,
            "Allocation not implemented",
        ))
    }

    fn deallocate(&mut self, ptr: &mut [u8]) -> core::result::Result<(), Self::Error> {
        let size = ptr.len();
        if self.allocated_bytes >= size {
            self.allocated_bytes -= size;
        }
        Ok(())
    }

    fn available_memory(&self) -> usize {
        self.max_memory - self.allocated_bytes
    }

    fn total_memory(&self) -> usize {
        self.max_memory
    }

    fn provider(&self) -> &Self::Provider {
        &self.provider
    }
}

// =============================================================================
// COMPATIBILITY LAYER
// =============================================================================

/// Re-export commonly used types for easy migration
pub mod compat {
    use super::{
        BoundedString,
        BoundedVec,
        DefaultRuntimeProvider,
    };

    /// Legacy vector type for backward compatibility (medium capacity)
    /// Note: T must implement Sized + Checksummable + `ToBytes` + `FromBytes` +
    /// Default + Clone + `PartialEq` + Eq
    pub type Vec<T> = BoundedVec<T, 1024, DefaultRuntimeProvider>;

    /// Legacy string type for backward compatibility
    pub type String = BoundedString<1024>;

    /// Legacy small vector type for backward compatibility
    /// Note: T must implement Sized + Checksummable + `ToBytes` + `FromBytes` +
    /// Default + Clone + `PartialEq` + Eq
    pub type SmallVec<T> = BoundedVec<T, 64, DefaultRuntimeProvider>;

    /// Legacy medium vector type for backward compatibility
    /// Note: T must implement Sized + Checksummable + `ToBytes` + `FromBytes` +
    /// Default + Clone + `PartialEq` + Eq
    pub type MediumVec<T> = BoundedVec<T, 1024, DefaultRuntimeProvider>;

    /// Legacy large vector type for backward compatibility
    /// Note: T must implement Sized + Checksummable + `ToBytes` + `FromBytes` +
    /// Default + Clone + `PartialEq` + Eq
    pub type LargeVec<T> = BoundedVec<T, 65536, DefaultRuntimeProvider>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_capacities() {
        let default_caps = PlatformCapacities::default();
        assert_eq!(default_caps.small_capacity, 64);
        assert_eq!(default_caps.medium_capacity, 1024);
        assert_eq!(default_caps.large_capacity, 65536);

        let embedded_caps = PlatformCapacities::embedded();
        assert!(embedded_caps.small_capacity < default_caps.small_capacity);
        assert!(embedded_caps.memory_provider_size < default_caps.memory_provider_size);

        let safety_caps = PlatformCapacities::safety_critical();
        assert!(safety_caps.medium_capacity < default_caps.medium_capacity);
    }

    #[test]
    fn test_platform_memory_adapter() {
        let adapter = PlatformMemoryAdapter::<DefaultRuntimeProvider>::new(1024 * 1024);
        assert!(adapter.is_ok());

        let adapter = adapter.unwrap();
        assert_eq!(adapter.total_memory(), 1024 * 1024);
        assert_eq!(adapter.available_memory(), 1024 * 1024);
    }
}
