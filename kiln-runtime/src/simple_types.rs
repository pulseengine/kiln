//! Simplified Type System for WRT Runtime - COMPILATION FIX
//!
//! This module provides a simplified unified type system to resolve compilation
//! errors. It focuses on concrete types rather than generic type aliases.

use wrt_foundation::{
    bounded::{
        BoundedString,
        BoundedVec,
    },
    prelude::{
        Clone,
        Copy,
        Debug,
    },
    safe_memory::NoStdProvider,
    traits::{
        Checksummable,
        FromBytes,
        ToBytes,
    },
};
use wrt_instructions::Value;

use crate::bounded_runtime_infra::RuntimeProvider;

// =============================================================================
// CONCRETE RUNTIME TYPES
// =============================================================================

// RuntimeProvider definition moved to bounded_runtime_infra.rs to avoid
// conflicts

/// Vector for local variables in function execution
pub type LocalsVec = BoundedVec<Value, 64, RuntimeProvider>;

/// Stack for WebAssembly values during execution
pub type ValueStackVec = BoundedVec<Value, 1024, RuntimeProvider>;

/// Buffer for linear memory content  
pub type MemoryBuffer = BoundedVec<u8, 65536, RuntimeProvider>;

/// String for runtime identifiers and names
pub type RuntimeString = BoundedString<256>;

/// String for component and module names
pub type ComponentName = BoundedString<64>;

/// Vector for function parameters
pub type ParameterVec = BoundedVec<Value, 16, RuntimeProvider>;

/// Vector for function results
pub type ResultVec = BoundedVec<Value, 16, RuntimeProvider>;

// =============================================================================
// PLATFORM CONFIGURATION
// =============================================================================

/// Platform capacity configuration
#[derive(Debug, Clone, Copy)]
pub struct PlatformCapacities {
    /// Small collection capacity limit
    pub small_capacity:       usize,
    /// Medium collection capacity limit
    pub medium_capacity:      usize,
    /// Large collection capacity limit
    pub large_capacity:       usize,
    /// Memory provider buffer size in bytes
    pub memory_provider_size: usize,
}

impl PlatformCapacities {
    /// Create default platform capacities for standard environments
    #[must_use]
    pub const fn default() -> Self {
        Self {
            small_capacity:       64,
            medium_capacity:      1024,
            large_capacity:       65536,
            memory_provider_size: 1048576,
        }
    }

    /// Create platform capacities optimized for embedded environments
    #[must_use]
    pub const fn embedded() -> Self {
        Self {
            small_capacity:       16,
            medium_capacity:      256,
            large_capacity:       8192,
            memory_provider_size: 32768,
        }
    }
}

// =============================================================================
// COMPATIBILITY LAYER
// =============================================================================

/// Compatibility types for gradual migration
pub mod compat {
    use super::{
        BoundedString,
        BoundedVec,
        RuntimeProvider,
    };

    /// Small vector for limited collections (T must implement all required
    /// traits)
    pub type SmallVec<T> = BoundedVec<T, 64, RuntimeProvider>;

    /// Medium vector for standard collections (T must implement all required
    /// traits)
    pub type MediumVec<T> = BoundedVec<T, 1024, RuntimeProvider>;

    /// Large vector for big collections (T must implement all required traits)
    pub type LargeVec<T> = BoundedVec<T, 65536, RuntimeProvider>;

    /// Compatibility string type
    pub type String = BoundedString<256>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_capacities() {
        let default_caps = PlatformCapacities::default();
        assert_eq!(default_caps.small_capacity, 64);

        let embedded_caps = PlatformCapacities::embedded();
        assert!(embedded_caps.small_capacity < default_caps.small_capacity);
    }
}
