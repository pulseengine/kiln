//! Runtime types for WebAssembly Component Model.
//!
//! This module provides simplified implementations of runtime types
//! needed for the component model implementation.

// Import from wrt-foundation instead of wrt-runtime
use wrt_foundation::types::{GlobalType, MemoryType, TableType};

use crate::prelude::*;

/// Placeholder runtime type for component execution
#[derive(Debug)]
pub struct Runtime {
    /// Runtime configuration
    pub config: RuntimeConfig,
}

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Maximum memory pages
    pub max_memory_pages: u32,
}

impl Runtime {
    pub fn new(config: RuntimeConfig) -> Self {
        Self { config }
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_memory_pages: 1024,
        }
    }
}

/// WebAssembly function type
#[derive(Debug, Clone, PartialEq)]
pub struct FuncType {
    /// Parameter types
    pub params: Vec<ValueType>,
    /// Result types
    pub results: Vec<ValueType>,
}

impl Default for FuncType {
    fn default() -> Self {
        Self {
            params: Vec::new(),
            results: Vec::new(),
        }
    }
}

/// Memory instance for components
#[derive(Debug)]
pub struct Memory {
    /// Memory type
    pub ty: MemoryType,
    /// Memory data
    data: Vec<u8>,
}

impl Memory {
    /// Create a new memory instance
    pub fn new(ty: MemoryType) -> Result<Self> {
        let data = vec![0; ty.limits.min as usize * 65536];
        Ok(Self { ty, data })
    }

    /// Get memory data
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Get mutable memory data
    pub fn data_mut(&mut self) -> &mut Vec<u8> {
        &mut self.data
    }
}

/// Table instance for components
#[derive(Debug)]
pub struct Table {
    /// Table type
    pub ty: TableType,
    /// Table elements
    elements: Vec<Option<usize>>,
}

impl Table {
    /// Create a new table instance
    pub fn new(ty: TableType) -> Result<Self> {
        let elements = vec![None; ty.limits.min as usize];
        Ok(Self { ty, elements })
    }

    /// Get table elements
    pub fn elements(&self) -> &[Option<usize>] {
        &self.elements
    }

    /// Get mutable table elements
    pub fn elements_mut(&mut self) -> &mut Vec<Option<usize>> {
        &mut self.elements
    }
}

/// WebAssembly global instance
#[derive(Debug, Clone)]
pub struct Global {
    /// Global type
    pub ty: GlobalType,
    /// Global value (simplified as u64)
    value: u64,
}

impl Global {
    /// Creates a new global instance
    pub fn new(ty: GlobalType) -> Result<Self> {
        Ok(Self { ty, value: 0 })
    }

    /// Gets the global value
    pub fn get(&self) -> u64 {
        self.value
    }

    /// Sets the global value
    pub fn set(&mut self, value: u64) -> Result<()> {
        if !self.ty.mutable {
            return Err(Error::runtime_execution_error(
                "Cannot set value on immutable global",
            ));
        }

        self.value = value;
        Ok(())
    }
}
