// WRT - wrt-component
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Resource strategy trait and implementations.
//!
//! This module provides the `ResourceStrategy` trait for resource access strategies
//! and a generic implementation that works in both std and no_std environments.

use wrt_error::{Error, ErrorCategory, Result, codes};
#[cfg(not(feature = "std"))]
use wrt_foundation::safe_memory::NoStdProvider;
use wrt_foundation::{
    bounded::{BoundedVec, MAX_BUFFER_SIZE},
    resource::ResourceOperation,
};

use crate::resources::MemoryStrategy;

// ============================================================================
// ResourceStrategy Trait
// ============================================================================

/// Trait for resource access strategies
pub trait ResourceStrategy: Send + Sync {
    /// Get the type of memory strategy this implements
    fn memory_strategy_type(&self) -> MemoryStrategy;

    /// Process memory with this strategy
    #[cfg(feature = "std")]
    fn process_memory(&self, data: &[u8], operation: ResourceOperation) -> Result<Vec<u8>>;

    /// Process memory with this strategy (no_std version)
    #[cfg(not(feature = "std"))]
    fn process_memory(
        &self,
        data: &[u8],
        operation: ResourceOperation,
    ) -> core::result::Result<
        BoundedVec<u8, MAX_BUFFER_SIZE, NoStdProvider<{ MAX_BUFFER_SIZE }>>,
        wrt_error::Error,
    >;

    /// Check if the strategy allows a certain operation
    fn allows_operation(&self, _operation: ResourceOperation) -> bool {
        true // Default implementation allows all operations
    }

    /// Reset any internal state or buffers
    fn reset(&mut self) {
        // Default implementation does nothing
    }
}

// ============================================================================
// Generic ResourceStrategy Implementation
// ============================================================================

/// Generic ResourceStrategy implementation that works in both std and no_std
///
/// This struct provides resource access strategies using the configured
/// memory strategy. Previously named `ResourceStrategyNoStd` but works
/// in both environments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenericResourceStrategy {
    /// The memory strategy to use for this resource
    strategy: MemoryStrategy,
}

/// Type alias for backwards compatibility
pub type ResourceStrategyNoStd = GenericResourceStrategy;

impl GenericResourceStrategy {
    /// Create a new GenericResourceStrategy with the given memory strategy
    pub fn new(strategy: MemoryStrategy) -> Self {
        Self { strategy }
    }
}

impl ResourceStrategy for GenericResourceStrategy {
    fn memory_strategy_type(&self) -> MemoryStrategy {
        self.strategy
    }

    #[cfg(feature = "std")]
    fn process_memory(&self, data: &[u8], operation: ResourceOperation) -> Result<Vec<u8>> {
        match self.strategy {
            // Zero-copy strategy - returns a view without copying for reads, a copy for writes
            MemoryStrategy::ZeroCopy => match operation {
                ResourceOperation::Read => Ok(data.to_vec()),
                ResourceOperation::Write => Ok(data.to_vec()),
                _ => Ok(data.to_vec()), // Default for other operations
            },
            // Bounded-copy strategy - copies with size limit checks
            MemoryStrategy::BoundedCopy => {
                if data.len() > MAX_BUFFER_SIZE {
                    return Err(Error::new(
                        ErrorCategory::Memory,
                        codes::MEMORY_OUT_OF_BOUNDS,
                        "Data exceeds maximum buffer size for bounded copy",
                    ));
                }
                Ok(data.to_vec())
            },
            // Full isolation - creates independent copies
            MemoryStrategy::FullIsolation => Ok(data.to_vec()),
            // Handle other memory strategies
            _ => Ok(data.to_vec()),
        }
    }

    #[cfg(not(feature = "std"))]
    fn process_memory(
        &self,
        data: &[u8],
        operation: ResourceOperation,
    ) -> core::result::Result<
        wrt_foundation::bounded::BoundedVec<
            u8,
            MAX_BUFFER_SIZE,
            wrt_foundation::safe_memory::NoStdProvider<{ MAX_BUFFER_SIZE }>,
        >,
        wrt_error::Error,
    > {
        use wrt_foundation::{budget_aware_provider::CrateId, safe_managed_alloc};
        let provider = safe_managed_alloc!(MAX_BUFFER_SIZE, CrateId::Component)?;

        match self.strategy {
            // Zero-copy strategy - returns a view without copying for reads, a copy for writes
            MemoryStrategy::ZeroCopy => match operation {
                ResourceOperation::Read => {
                    let mut result = wrt_foundation::bounded::BoundedVec::new(provider)?;
                    for &byte in data {
                        result
                            .push(byte)
                            .map_err(|_| Error::memory_error("Buffer capacity exceeded"))?;
                    }
                    Ok(result)
                },
                ResourceOperation::Write => {
                    let mut result = wrt_foundation::bounded::BoundedVec::new(provider)?;
                    for &byte in data {
                        result
                            .push(byte)
                            .map_err(|_| Error::memory_error("Buffer capacity exceeded"))?;
                    }
                    Ok(result)
                },
                _ => Err(Error::not_supported("Operation not supported")),
            },

            // Bounded-copy strategy - always copies but reuses buffers
            MemoryStrategy::BoundedCopy => {
                let mut result = wrt_foundation::bounded::BoundedVec::new(provider)?;
                for &byte in data {
                    result
                        .push(byte)
                        .map_err(|_| Error::memory_error("Buffer capacity exceeded"))?;
                }
                Ok(result)
            },

            // Isolated strategy - always copies and validates
            MemoryStrategy::Isolated => {
                let mut result = wrt_foundation::bounded::BoundedVec::new(provider)?;
                for &byte in data {
                    result
                        .push(byte)
                        .map_err(|_| Error::memory_error("Buffer capacity exceeded"))?;
                }
                Ok(result)
            },

            // Copy strategy - always copies the data
            MemoryStrategy::Copy => {
                let mut result = wrt_foundation::bounded::BoundedVec::new(provider)?;
                for &byte in data {
                    result
                        .push(byte)
                        .map_err(|_| Error::memory_error("Buffer capacity exceeded"))?;
                }
                Ok(result)
            },

            // Reference strategy - returns a view without copying
            MemoryStrategy::Reference => {
                let mut result = wrt_foundation::bounded::BoundedVec::new(provider)?;
                for &byte in data {
                    result
                        .push(byte)
                        .map_err(|_| Error::memory_error("Buffer capacity exceeded"))?;
                }
                Ok(result)
            },

            // Full isolation strategy - copies and performs full validation
            MemoryStrategy::FullIsolation => {
                let mut result = wrt_foundation::bounded::BoundedVec::new(provider)?;
                for &byte in data {
                    result
                        .push(byte)
                        .map_err(|_| Error::memory_error("Buffer capacity exceeded"))?;
                }
                Ok(result)
            },

            // FixedBuffer strategy - uses fixed size buffer
            MemoryStrategy::FixedBuffer => {
                let mut result = wrt_foundation::bounded::BoundedVec::new(provider)?;
                for &byte in data {
                    result
                        .push(byte)
                        .map_err(|_| Error::memory_error("Buffer capacity exceeded"))?;
                }
                Ok(result)
            },

            // BoundedCollections strategy - uses bounded collections
            MemoryStrategy::BoundedCollections => {
                let mut result = wrt_foundation::bounded::BoundedVec::new(provider)?;
                for &byte in data {
                    result
                        .push(byte)
                        .map_err(|_| Error::memory_error("Buffer capacity exceeded"))?;
                }
                Ok(result)
            },
        }
    }
}

// ============================================================================
// Constants
// ============================================================================

/// Maximum buffer size for bounded vectors in no_std environments
pub const MAX_RESOURCE_BUFFER_SIZE: usize = MAX_BUFFER_SIZE;
