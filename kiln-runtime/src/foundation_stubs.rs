// WRT - wrt-runtime
// Module: Foundation Type Stubs
//
// Copyright (c) 2025 The WRT Project Developers
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Foundation type stubs for runtime integration
//!
//! These types provide the interface between the runtime
//! and the unified type system implementation.

#![allow(dead_code)] // Allow during stub phase

use wrt_error::{
    Error,
    ErrorCategory,
    Result,
};
use wrt_foundation::safe_memory::NoStdProvider;

/// Temporary stub for unified memory provider
pub trait UnifiedMemoryProvider: Send + Sync {
    fn allocate(&mut self, size: usize) -> Result<&mut [u8]>;
    fn deallocate(&mut self, ptr: &mut [u8]) -> Result<()>;
    fn available_memory(&self) -> usize;
    fn total_memory(&self) -> usize;
}

/// Temporary stub for configurable memory provider
pub struct ConfigurableProvider<const SIZE: usize> {
    buffer:    [u8; SIZE],
    allocated: usize,
}

impl<const SIZE: usize> ConfigurableProvider<SIZE> {
    pub fn new() -> Self {
        Self {
            buffer:    [0u8; SIZE],
            allocated: 0,
        }
    }
}

impl<const SIZE: usize> Default for ConfigurableProvider<SIZE> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const SIZE: usize> UnifiedMemoryProvider for ConfigurableProvider<SIZE> {
    fn allocate(&mut self, size: usize) -> Result<&mut [u8]> {
        if self.allocated + size > SIZE {
            return Err(Error::runtime_execution_error("Insufficient memory"));
        }

        let start = self.allocated;
        self.allocated += size;
        Ok(&mut self.buffer[start..self.allocated])
    }

    fn deallocate(&mut self, _ptr: &mut [u8]) -> Result<()> {
        // Simple implementation - reset allocation
        self.allocated = 0;
        Ok(())
    }

    fn available_memory(&self) -> usize {
        SIZE - self.allocated
    }

    fn total_memory(&self) -> usize {
        SIZE
    }
}

/// Type aliases for different sizes
pub type SmallProvider = ConfigurableProvider<8192>; // 8KB
pub type MediumProvider = ConfigurableProvider<65536>; // 64KB
pub type LargeProvider = ConfigurableProvider<1048576>; // 1MB

/// Temporary unified types - removed inherent associated types for stable
/// compiler compatibility
pub struct UnifiedTypes<const SMALL: usize, const MEDIUM: usize, const LARGE: usize> {
    _phantom: core::marker::PhantomData<()>,
}

// Convert to regular type aliases for stable compiler compatibility
pub type UnifiedSmallVec<T> = wrt_foundation::bounded::BoundedVec<T, 64, NoStdProvider<1024>>;
pub type UnifiedMediumVec<T> = wrt_foundation::bounded::BoundedVec<T, 1024, NoStdProvider<8192>>;
pub type UnifiedLargeVec<T> = wrt_foundation::bounded::BoundedVec<T, 65536, NoStdProvider<65536>>;
pub type UnifiedRuntimeString = wrt_foundation::bounded::BoundedString<1024>;

/// Default type configuration
pub type DefaultTypes = UnifiedTypes<64, 1024, 65536>;
pub type SmallVec<T> =
    wrt_foundation::bounded::BoundedVec<T, 64, wrt_foundation::NoStdProvider<8192>>;
pub type MediumVec<T> =
    wrt_foundation::bounded::BoundedVec<T, 1024, wrt_foundation::NoStdProvider<65536>>;
pub type LargeVec<T> =
    wrt_foundation::bounded::BoundedVec<T, 65536, wrt_foundation::NoStdProvider<1048576>>;

/// ASIL levels for safety context
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AsilLevel {
    /// QM: Quality Management (no ASIL requirement)
    QM,
    /// ASIL-A: Lowest safety criticality
    A,
    /// ASIL-B: Low safety criticality
    B,
    /// ASIL-C: Medium safety criticality
    C,
    /// ASIL-D: Highest safety criticality
    D,
}

/// Safety context stub
#[derive(Debug, Clone)]
pub struct SafetyContext {
    pub compile_time_asil: AsilLevel,
    pub runtime_asil:      Option<AsilLevel>,
}

impl SafetyContext {
    pub const fn new(compile_time: AsilLevel) -> Self {
        Self {
            compile_time_asil: compile_time,
            runtime_asil:      None,
        }
    }

    pub fn effective_asil(&self) -> AsilLevel {
        self.runtime_asil.unwrap_or(self.compile_time_asil)
    }
}

impl Default for SafetyContext {
    fn default() -> Self {
        Self::new(AsilLevel::QM)
    }
}

/// Value type stub
pub use wrt_foundation::values::Value;
