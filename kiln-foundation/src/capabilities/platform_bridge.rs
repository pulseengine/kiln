//! Platform-Capability Bridge
//!
//! This module provides a simple bridge for platform-specific allocators
//! to work with the capability system.

#[cfg(feature = "std")]
use std::{
    sync::Arc,
    vec::Vec,
};

use crate::{
    capabilities::AnyMemoryCapability,
    Error,
    Result,
};
#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{
    sync::Arc,
    vec,
    vec::Vec,
};

/// Types of capabilities supported by the platform bridge
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityType {
    /// Dynamic capability with runtime checks
    Dynamic,
    /// Static capability with compile-time guarantees
    Static,
    /// Verified capability with formal proofs
    Verified,
}

/// Simple platform capability provider
pub struct PlatformCapabilityProvider {
    allocator: Arc<dyn PlatformAllocator>,
    max_size:  usize,
}

/// Trait for platform-specific allocators
pub trait PlatformAllocator: Send + Sync {
    /// Get available memory from platform
    fn available_memory(&self) -> usize;

    /// Get platform identifier
    fn platform_id(&self) -> &str;
}

impl PlatformCapabilityProvider {
    /// Create a new platform capability provider
    pub fn new(allocator: Arc<dyn PlatformAllocator>, max_size: usize) -> Self {
        Self {
            allocator,
            max_size,
        }
    }

    /// Get maximum allocation size
    pub fn max_allocation_size(&self) -> usize {
        self.max_size
    }
}

/// Simple memory provider for demonstration
pub struct PlatformMemoryProvider {
    buffer: Vec<u8>,
}

impl PlatformMemoryProvider {
    /// Create a new provider with given size
    pub fn new(size: usize) -> Result<Self> {
        Ok(Self {
            buffer: vec![0; size],
        })
    }

    /// Get the size of the buffer
    pub fn size(&self) -> usize {
        self.buffer.len()
    }
}

/// Builder for creating platform capability providers
pub struct PlatformCapabilityBuilder {
    memory_limit: usize,
}

impl PlatformCapabilityBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            memory_limit: 1024 * 1024 * 1024, // 1GB default
        }
    }

    /// Set memory limit
    pub fn with_memory_limit(mut self, limit: usize) -> Self {
        self.memory_limit = limit;
        self
    }

    /// Build the platform capability provider
    pub fn build(
        self,
        allocator: Arc<dyn PlatformAllocator>,
    ) -> Result<PlatformCapabilityProvider> {
        Ok(PlatformCapabilityProvider::new(
            allocator,
            self.memory_limit,
        ))
    }
}

impl Default for PlatformCapabilityBuilder {
    fn default() -> Self {
        Self::new()
    }
}
