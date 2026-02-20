//! WebAssembly Component Model Canonical ABI
//!
//! This module provides implementations of the Canonical ABI for the
//! WebAssembly Component Model, including lifting, lowering, and memory
//! allocation functions.

pub mod canonical;
pub mod canonical_abi;
pub mod canonical_options;
pub mod canonical_realloc;
pub mod post_return;

// Re-export from canonical module (primary implementation)
pub use canonical::*;
// Re-export key types from canonical_abi module for consistent access
pub use canonical_abi::{
    CanonicalABI, CanonicalCallContext, CanonicalMemory, ComponentType, ComponentValue, CoreValue,
    LiftResult, LowerResult,
};

#[cfg(feature = "std")]
pub use canonical_abi::SimpleMemory;
pub use canonical_options::*;
#[cfg(feature = "std")]
pub use canonical_realloc::ReallocCallback;
#[cfg(feature = "std")]
pub use canonical_realloc::engine_integration;
pub use canonical_realloc::{
    CanonicalOptionsWithRealloc, MemoryLayout, ReallocFn, ReallocManager, StringEncoding,
};
pub use post_return::*;

// Placeholder types for async canonical ABI support
#[derive(Debug, Clone)]
pub struct LiftingContext {
    pub options: canonical_options::CanonicalOptions,
}

impl LiftingContext {
    pub fn new(options: canonical_options::CanonicalOptions) -> Self {
        Self { options }
    }
}

/// Context for lowering values from host to WebAssembly memory
///
/// The LoweringContext carries all configuration needed for canonical ABI
/// lowering operations, including memory allocation support via cabi_realloc.
#[derive(Debug, Clone)]
pub struct LoweringContext {
    /// Canonical ABI options (memory index, string encoding, etc.)
    pub options: canonical_options::CanonicalOptions,
    /// Component instance ID for realloc operations
    pub instance_id: Option<u32>,
}

impl LoweringContext {
    /// Create a new lowering context with the given options
    pub fn new(options: canonical_options::CanonicalOptions) -> Self {
        Self {
            options,
            instance_id: None,
        }
    }

    /// Set the instance ID for realloc operations
    ///
    /// This must be set before lowering strings or lists that require
    /// memory allocation via cabi_realloc.
    pub fn with_instance_id(mut self, id: u32) -> Self {
        self.instance_id = Some(id);
        self
    }

    /// Get the instance ID, if configured
    pub fn instance_id(&self) -> Option<u32> {
        self.instance_id
    }
}
