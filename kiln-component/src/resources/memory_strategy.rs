// WRT - wrt-component
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

use wrt_error::{Error, Result};
#[cfg(not(feature = "std"))]
use wrt_foundation::safe_memory::NoStdProvider;
use wrt_foundation::{bounded::MAX_BUFFER_SIZE, collections::StaticVec as BoundedVec};

#[cfg(feature = "std")]
use super::resource_table::MemoryStrategy;
#[cfg(not(feature = "std"))]
use super::resource_table_no_std::MemoryStrategy;

use super::resource_strategy::ResourceStrategy;
use wrt_foundation::resource::ResourceOperation;

#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(feature = "std")]
impl ResourceStrategy for MemoryStrategy {
    fn memory_strategy_type(&self) -> MemoryStrategy {
        *self
    }

    fn process_memory(&self, data: &[u8], operation: ResourceOperation) -> Result<Vec<u8>> {
        match self {
            // Zero-copy strategy - returns a view without copying for reads, a copy for writes
            MemoryStrategy::ZeroCopy => match operation {
                ResourceOperation::Read => Ok(data.to_vec()),
                ResourceOperation::Write => Ok(data.to_vec()),
                _ => Err(Error::runtime_execution_error("Error occurred")),
            },

            // Bounded-copy strategy - always copies but reuses buffers
            MemoryStrategy::BoundedCopy => Ok(data.to_vec()),

            // Isolated strategy - always copies and validates
            MemoryStrategy::Isolated => {
                // In a real implementation this would include validation
                Ok(data.to_vec())
            },

            // Copy strategy - always copies the data
            MemoryStrategy::Copy => Ok(data.to_vec()),

            // Reference strategy - returns a view without copying
            MemoryStrategy::Reference => {
                // In a real implementation, this would return a reference
                // For testing purposes, we'll still return a vec
                Ok(data.to_vec())
            },

            // Full isolation strategy - copies and performs full validation
            MemoryStrategy::FullIsolation => {
                // In a real implementation this would include more extensive validation
                Ok(data.to_vec())
            },
        }
    }
}

#[cfg(not(feature = "std"))]
impl ResourceStrategy for MemoryStrategy {
    fn memory_strategy_type(&self) -> MemoryStrategy {
        *self
    }

    fn process_memory(
        &self,
        data: &[u8],
        operation: ResourceOperation,
    ) -> core::result::Result<
        wrt_foundation::bounded::BoundedVec<
            u8,
            MAX_BUFFER_SIZE,
            NoStdProvider<{ MAX_BUFFER_SIZE }>,
        >,
        wrt_error::Error,
    > {
        use wrt_foundation::{CrateId, safe_managed_alloc};
        let provider = safe_managed_alloc!(MAX_BUFFER_SIZE, CrateId::Component)?;

        match self {
            // Zero-copy strategy - returns a view without copying for reads, a copy for writes
            MemoryStrategy::ZeroCopy => match operation {
                ResourceOperation::Read => {
                    let mut result = wrt_foundation::bounded::BoundedVec::new(provider)?;

                    for &byte in data {
                        result
                            .push(byte)
                            .map_err(|e| Error::component_not_found("Error occurred"))?;
                    }
                    Ok(result)
                },
                ResourceOperation::Write => {
                    let mut result = wrt_foundation::bounded::BoundedVec::new(provider)?;

                    for &byte in data {
                        result
                            .push(byte)
                            .map_err(|e| Error::component_not_found("Error occurred"))?;
                    }
                    Ok(result)
                },
                _ => Err(Error::runtime_execution_error("Error occurred")),
            },

            // Bounded-copy strategy - always copies but reuses buffers
            MemoryStrategy::BoundedCopy => {
                let mut result = wrt_foundation::bounded::BoundedVec::new(provider)?;

                for &byte in data {
                    result.push(byte).map_err(|e| Error::component_not_found("Error occurred"))?;
                }
                Ok(result)
            },

            // Other strategies implemented similarly
            MemoryStrategy::Isolated
            | MemoryStrategy::Copy
            | MemoryStrategy::Reference
            | MemoryStrategy::FullIsolation
            | MemoryStrategy::FixedBuffer
            | MemoryStrategy::BoundedCollections => {
                let mut result = wrt_foundation::bounded::BoundedVec::new(provider)?;

                for &byte in data {
                    result.push(byte).map_err(|e| Error::component_not_found("Error occurred"))?;
                }
                Ok(result)
            },
        }
    }
}
