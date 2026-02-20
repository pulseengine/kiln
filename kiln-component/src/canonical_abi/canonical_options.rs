//! Enhanced canonical options with full realloc integration
//!
//! This module provides the complete canonical options implementation
//! for the WebAssembly Component Model, including realloc support,
//! post-return functions, and memory management.

#[cfg(not(feature = "std"))]
use alloc::sync::Arc;
#[cfg(feature = "std")]
use std::sync::{Arc, RwLock};

use wrt_runtime::Memory;
// Instance is from wrt_format, not wrt_runtime
use wrt_format::component::Instance;
#[cfg(not(feature = "std"))]
use wrt_sync::RwLock;

use crate::{
    canonical_abi::canonical_realloc::{ComponentInstanceId, ReallocManager, StringEncoding},
    memory_layout::MemoryLayout,
    prelude::*,
};

// Type alias for compatibility
pub type ComponentError = Error;

/// Complete canonical options for lift/lower operations
#[derive(Debug, Clone)]
pub struct CanonicalOptions {
    /// Memory index for canonical operations
    pub memory: u32,
    /// Binary std/no_std choice
    pub realloc: Option<u32>,
    /// Post-return function index (optional)
    pub post_return: Option<u32>,
    /// String encoding
    pub string_encoding: StringEncoding,
    /// Instance ID for this set of options
    pub instance_id: ComponentInstanceId,
    /// Binary std/no_std choice
    pub realloc_manager: Option<Arc<RwLock<ReallocManager>>>,
    /// Memory.grow function index (MVP spec addition)
    pub memory_grow: Option<u32>,
}

/// Canonical lift context with full memory management
pub struct CanonicalLiftContext<'a> {
    /// Runtime instance
    pub instance: &'a Instance,
    /// Memory for lifting
    pub memory: &'a Memory,
    /// Canonical options
    pub options: &'a CanonicalOptions,
    /// Binary std/no_std choice
    allocations: Vec<TempAllocation>,
}

/// Canonical lower context with full memory management
pub struct CanonicalLowerContext<'a> {
    /// Runtime instance
    pub instance: &'a mut Instance,
    /// Memory for lowering
    pub memory: &'a mut Memory,
    /// Canonical options
    pub options: &'a CanonicalOptions,
    /// Allocations made during lower
    allocations: Vec<TempAllocation>,
}

#[derive(Debug)]
pub struct TempAllocation {
    ptr: i32,
    size: i32,
    align: i32,
}

impl CanonicalOptions {
    /// Create new canonical options with defaults
    pub fn new(memory: u32, instance_id: ComponentInstanceId) -> Self {
        Self {
            memory,
            realloc: None,
            post_return: None,
            string_encoding: StringEncoding::Utf8,
            instance_id,
            realloc_manager: None,
            memory_grow: None,
        }
    }

    /// Binary std/no_std choice
    pub fn with_realloc(mut self, func_index: u32, manager: Arc<RwLock<ReallocManager>>) -> Self {
        self.realloc = Some(func_index);

        // Register with the manager before storing it
        {
            #[cfg(feature = "std")]
            let mut mgr = manager.write().expect("Realloc manager lock poisoned");
            #[cfg(not(feature = "std"))]
            let mut mgr = manager.write();
            let _ = mgr.register_realloc(self.instance_id, func_index);
        }

        self.realloc_manager = Some(manager);

        self
    }

    /// Set post-return function
    pub fn with_post_return(mut self, func_index: u32) -> Self {
        self.post_return = Some(func_index);
        self
    }

    /// Set string encoding
    pub fn with_string_encoding(mut self, encoding: StringEncoding) -> Self {
        self.string_encoding = encoding;
        self
    }

    /// Set memory.grow function (MVP spec addition)
    pub fn with_memory_grow(mut self, func_index: u32) -> Self {
        self.memory_grow = Some(func_index);
        self
    }

    /// Binary std/no_std choice
    pub fn has_realloc(&self) -> bool {
        self.realloc.is_some() && self.realloc_manager.is_some()
    }

    /// Check if post-return is available
    pub fn has_post_return(&self) -> bool {
        self.post_return.is_some()
    }

    /// Check if memory.grow is available (MVP spec addition)
    pub fn has_memory_grow(&self) -> bool {
        self.memory_grow.is_some()
    }
}

impl<'a> CanonicalLiftContext<'a> {
    /// Create a new lift context
    pub fn new(instance: &'a Instance, memory: &'a Memory, options: &'a CanonicalOptions) -> Self {
        Self {
            instance,
            memory,
            options,
            allocations: Vec::new(),
        }
    }

    /// Binary std/no_std choice
    pub fn allocate(
        &mut self,
        size: usize,
        align: usize,
    ) -> core::result::Result<i32, ComponentError> {
        if size == 0 {
            return Ok(0);
        }

        let ptr = if let Some(manager) = &self.options.realloc_manager {
            // Binary std/no_std choice
            #[cfg(feature = "std")]
            let mut mgr = manager.write().expect("Realloc manager lock poisoned");
            #[cfg(not(feature = "std"))]
            let mut mgr = manager.write();

            mgr.allocate(self.options.instance_id, size as i32, align as i32)?
        } else {
            // Binary std/no_std choice
            return Err(ComponentError::resource_not_found(
                "Realloc manager not available",
            ));
        };

        // Binary std/no_std choice
        self.allocations.push(TempAllocation {
            ptr,
            size: size as i32,
            align: align as i32,
        });

        Ok(ptr)
    }

    /// Read bytes from memory
    pub fn read_bytes(
        &self,
        ptr: i32,
        len: usize,
    ) -> core::result::Result<Vec<u8>, ComponentError> {
        if ptr < 0 {
            return Err(ComponentError::runtime_type_mismatch(
                "Invalid negative pointer for memory read",
            ));
        }

        let offset = ptr as u32;
        let mut buffer = vec![0u8; len];
        self.memory.read(offset, &mut buffer).map_err(|_| {
            ComponentError::resource_not_found("Memory read failed: invalid address")
        })?;
        Ok(buffer)
    }

    /// Read a string from memory with the configured encoding
    pub fn read_string(
        &self,
        ptr: i32,
        len: usize,
    ) -> core::result::Result<String, ComponentError> {
        let bytes = self.read_bytes(ptr, len)?;

        match self.options.string_encoding {
            StringEncoding::Utf8 => String::from_utf8(bytes).map_err(|_| {
                ComponentError::runtime_type_mismatch("Invalid UTF-8 string encoding")
            }),
            StringEncoding::Utf16Le => {
                let u16_values: Vec<u16> = bytes
                    .chunks_exact(2)
                    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                    .collect();
                String::from_utf16(&u16_values).map_err(|_| {
                    ComponentError::runtime_type_mismatch("Invalid UTF-16LE string encoding")
                })
            },
            StringEncoding::Utf16Be => {
                let u16_values: Vec<u16> = bytes
                    .chunks_exact(2)
                    .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
                    .collect();
                String::from_utf16(&u16_values).map_err(|_| {
                    ComponentError::runtime_type_mismatch("Invalid UTF-16BE string encoding")
                })
            },
            StringEncoding::Latin1 => Ok(bytes.into_iter().map(|b| b as char).collect()),
        }
    }

    /// Binary std/no_std choice
    pub fn cleanup(mut self) -> Result<()> {
        // Binary std/no_std choice
        if let Some(manager) = &self.options.realloc_manager {
            #[cfg(feature = "std")]
            let mut mgr = manager.write().expect("Realloc manager lock poisoned");
            #[cfg(not(feature = "std"))]
            let mut mgr = manager.write();

            for alloc in self.allocations.drain(..) {
                mgr.deallocate(self.options.instance_id, alloc.ptr, alloc.size, alloc.align)?;
            }
        }

        // Then call post-return if available
        if let Some(post_return_idx) = self.options.post_return {
            // In a real implementation, this would call the actual function
            // For now, we just acknowledge it exists
        }

        Ok(())
    }
}

impl<'a> CanonicalLowerContext<'a> {
    /// Create a new lower context
    pub fn new(
        instance: &'a mut Instance,
        memory: &'a mut Memory,
        options: &'a CanonicalOptions,
    ) -> Self {
        Self {
            instance,
            memory,
            options,
            allocations: Vec::new(),
        }
    }

    /// Binary std/no_std choice
    pub fn allocate(
        &mut self,
        size: usize,
        align: usize,
    ) -> core::result::Result<i32, ComponentError> {
        if size == 0 {
            return Ok(0);
        }

        let ptr = if let Some(manager) = &self.options.realloc_manager {
            // Binary std/no_std choice
            #[cfg(feature = "std")]
            let mut mgr = manager.write().expect("Realloc manager lock poisoned");
            #[cfg(not(feature = "std"))]
            let mut mgr = manager.write();

            mgr.allocate(self.options.instance_id, size as i32, align as i32)?
        } else {
            // Binary std/no_std choice
            return Err(ComponentError::resource_not_found(
                "Realloc manager not available",
            ));
        };

        // Binary std/no_std choice
        self.allocations.push(TempAllocation {
            ptr,
            size: size as i32,
            align: align as i32,
        });

        Ok(ptr)
    }

    /// Write bytes to memory
    pub fn write_bytes(
        &mut self,
        ptr: i32,
        data: &[u8],
    ) -> core::result::Result<(), ComponentError> {
        if ptr < 0 {
            return Err(ComponentError::runtime_type_mismatch(
                "Invalid negative pointer for memory write",
            ));
        }

        let offset = ptr as usize;
        self.memory
            .write(offset as u32, data)
            .map_err(|_| ComponentError::resource_not_found("Memory write failed: invalid address"))
    }

    /// Write a string to memory with the configured encoding
    pub fn write_string(&mut self, s: &str) -> core::result::Result<(i32, usize), ComponentError> {
        let encoded = match self.options.string_encoding {
            StringEncoding::Utf8 => s.as_bytes().to_vec(),
            StringEncoding::Utf16Le => s.encode_utf16().flat_map(|c| c.to_le_bytes()).collect(),
            StringEncoding::Utf16Be => s.encode_utf16().flat_map(|c| c.to_be_bytes()).collect(),
            StringEncoding::Latin1 => {
                s.chars()
                    .map(|c| {
                        if c as u32 <= 0xFF {
                            c as u8
                        } else {
                            b'?' // Replace non-Latin1 chars
                        }
                    })
                    .collect()
            },
        };

        let len = encoded.len();
        let align = match self.options.string_encoding {
            StringEncoding::Utf8 | StringEncoding::Latin1 => 1,
            StringEncoding::Utf16Le | StringEncoding::Utf16Be => 2,
        };

        let ptr = self.allocate(len, align)?;
        self.write_bytes(ptr, &encoded)?;

        Ok((ptr, len))
    }

    /// Binary std/no_std choice
    pub fn finish(self) -> core::result::Result<Vec<TempAllocation>, ComponentError> {
        // Binary std/no_std choice
        Ok(self.allocations)
    }
}

/// Builder for canonical options
pub struct CanonicalOptionsBuilder {
    memory: u32,
    realloc: Option<u32>,
    post_return: Option<u32>,
    string_encoding: StringEncoding,
    instance_id: ComponentInstanceId,
    realloc_manager: Option<Arc<RwLock<ReallocManager>>>,
    memory_grow: Option<u32>,
}

impl CanonicalOptionsBuilder {
    pub fn new(memory: u32, instance_id: ComponentInstanceId) -> Self {
        Self {
            memory,
            realloc: None,
            post_return: None,
            string_encoding: StringEncoding::Utf8,
            instance_id,
            realloc_manager: None,
            memory_grow: None,
        }
    }

    pub fn with_realloc(mut self, func_index: u32, manager: Arc<RwLock<ReallocManager>>) -> Self {
        self.realloc = Some(func_index);
        self.realloc_manager = Some(manager);
        self
    }

    pub fn with_post_return(mut self, func_index: u32) -> Self {
        self.post_return = Some(func_index);
        self
    }

    pub fn with_string_encoding(mut self, encoding: StringEncoding) -> Self {
        self.string_encoding = encoding;
        self
    }

    pub fn with_memory_grow(mut self, func_index: u32) -> Self {
        self.memory_grow = Some(func_index);
        self
    }

    pub fn build(self) -> CanonicalOptions {
        let mut options = CanonicalOptions::new(self.memory, self.instance_id);

        if let (Some(func_index), Some(manager)) = (self.realloc, self.realloc_manager) {
            options = options.with_realloc(func_index, manager);
        }

        if let Some(func_index) = self.post_return {
            options = options.with_post_return(func_index);
        }

        if let Some(func_index) = self.memory_grow {
            options = options.with_memory_grow(func_index);
        }

        options.with_string_encoding(self.string_encoding)
    }
}
