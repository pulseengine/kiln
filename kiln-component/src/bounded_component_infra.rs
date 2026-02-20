//! Bounded Infrastructure for Component Model
//!
//! This module provides bounded alternatives for component collections
//! to ensure static memory allocation throughout the component model.
//!
//! Migrated to StaticVec - no Provider abstraction needed.

use wrt_foundation::collections::{StaticMap, StaticVec};
use wrt_foundation::safe_memory::NoStdProvider;

use crate::prelude::*;

/// Default memory provider for component infrastructure (4KB buffer)
pub type ComponentProvider = NoStdProvider<4096>;

/// Memory provider for instantiation operations (64KB buffer for larger needs)
pub type InstantiationProvider = NoStdProvider<65536>;

/// Memory provider for buffer pool operations (64KB buffer)
pub type BufferProvider = NoStdProvider<65536>;

/// Maximum number of component instances
pub const MAX_COMPONENT_INSTANCES: usize = 256;

/// Maximum number of component exports
pub const MAX_COMPONENT_EXPORTS: usize = 512;

/// Maximum number of component imports
pub const MAX_COMPONENT_IMPORTS: usize = 512;

/// Maximum number of resource handles per table
pub const MAX_RESOURCE_HANDLES: usize = 4096;

/// Maximum number of resource borrows
pub const MAX_RESOURCE_BORROWS: usize = 256;

/// Maximum call stack depth
pub const MAX_CALL_STACK_DEPTH: usize = 1024;

/// Maximum operand stack size
pub const MAX_OPERAND_STACK_SIZE: usize = 4096;

/// Maximum number of locals per function
pub const MAX_LOCALS_COUNT: usize = 512;

/// Maximum number of memory instances
pub const MAX_MEMORY_INSTANCES: usize = 16;

/// Maximum number of table instances
pub const MAX_TABLE_INSTANCES: usize = 16;

/// Maximum number of global instances
pub const MAX_GLOBAL_INSTANCES: usize = 1024;

/// Maximum number of host functions
pub const MAX_HOST_FUNCTIONS: usize = 1024;

/// Maximum component name length
pub const MAX_COMPONENT_NAME_LEN: usize = 256;

/// Maximum export/import name length
pub const MAX_EXPORT_NAME_LEN: usize = 256;

/// Maximum type definitions
pub const MAX_TYPE_DEFINITIONS: usize = 2048;

/// Maximum resource types
pub const MAX_RESOURCE_TYPES: usize = 256;

/// Maximum post-return callbacks
pub const MAX_POST_RETURN_CALLBACKS: usize = 64;

/// Bounded vector for component instances
pub type BoundedComponentVec<T> = StaticVec<T, MAX_COMPONENT_INSTANCES>;

/// Bounded vector for exports
pub type BoundedExportVec<T> = StaticVec<T, MAX_COMPONENT_EXPORTS>;

/// Bounded vector for imports
pub type BoundedImportVec<T> = StaticVec<T, MAX_COMPONENT_IMPORTS>;

/// Bounded vector for resource handles
pub type BoundedResourceVec<T> = StaticVec<T, MAX_RESOURCE_HANDLES>;

/// Bounded vector for resource borrows
pub type BoundedBorrowVec<T> = StaticVec<T, MAX_RESOURCE_BORROWS>;

/// Bounded stack for call frames
pub type BoundedCallStack<T> = StaticVec<T, MAX_CALL_STACK_DEPTH>;

/// Bounded stack for operands
pub type BoundedOperandStack<T> = StaticVec<T, MAX_OPERAND_STACK_SIZE>;

/// Bounded vector for locals
pub type BoundedLocalsVec<T> = StaticVec<T, MAX_LOCALS_COUNT>;

/// Bounded vector for memory instances
pub type BoundedMemoryVec<T> = StaticVec<T, MAX_MEMORY_INSTANCES>;

/// Bounded vector for table instances
pub type BoundedTableVec<T> = StaticVec<T, MAX_TABLE_INSTANCES>;

/// Bounded vector for global instances
pub type BoundedGlobalVec<T> = StaticVec<T, MAX_GLOBAL_INSTANCES>;

/// Bounded vector for host functions
pub type BoundedHostFunctionVec<T> = StaticVec<T, MAX_HOST_FUNCTIONS>;

/// Bounded string for component names (using StaticVec<u8, N>)
pub type BoundedComponentName = StaticVec<u8, MAX_COMPONENT_NAME_LEN>;

/// Bounded string for export/import names (using StaticVec<u8, N>)
pub type BoundedExportName = StaticVec<u8, MAX_EXPORT_NAME_LEN>;

/// Bounded map for exports
pub type BoundedExportMap<V> = StaticMap<BoundedExportName, V, MAX_COMPONENT_EXPORTS>;

/// Bounded map for imports
pub type BoundedImportMap<V> = StaticMap<BoundedExportName, V, MAX_COMPONENT_IMPORTS>;

/// Bounded map for type definitions
pub type BoundedTypeMap<V> = StaticMap<u32, V, MAX_TYPE_DEFINITIONS>;

/// Bounded map for resource types
pub type BoundedResourceTypeMap<V> = StaticMap<u32, V, MAX_RESOURCE_TYPES>;

/// Bounded vector for post-return callbacks
pub type BoundedPostReturnVec<T> = StaticVec<T, MAX_POST_RETURN_CALLBACKS>;

/// Create a new bounded component vector
pub fn new_component_vec<T>() -> BoundedComponentVec<T> {
    StaticVec::new()
}

/// Create a new bounded export vector
pub fn new_export_vec<T>() -> BoundedExportVec<T> {
    StaticVec::new()
}

/// Create a new bounded import vector
pub fn new_import_vec<T>() -> BoundedImportVec<T> {
    StaticVec::new()
}

/// Create a new bounded resource vector
pub fn new_resource_vec<T>() -> BoundedResourceVec<T> {
    StaticVec::new()
}

/// Create a new bounded call stack
pub fn new_call_stack<T>() -> BoundedCallStack<T> {
    StaticVec::new()
}

/// Create a new bounded operand stack
pub fn new_operand_stack<T>() -> BoundedOperandStack<T> {
    StaticVec::new()
}

/// Create a new bounded locals vector
pub fn new_locals_vec<T>() -> BoundedLocalsVec<T> {
    StaticVec::new()
}

/// Create a new bounded component name
pub fn new_component_name() -> BoundedComponentName {
    StaticVec::new()
}

/// Create a bounded component name from str
pub fn bounded_component_name_from_str(s: &str) -> wrt_error::Result<BoundedComponentName> {
    let mut name = StaticVec::new();
    for byte in s.bytes() {
        name.push(byte)?;
    }
    Ok(name)
}

/// Create a new bounded export name
pub fn new_export_name() -> BoundedExportName {
    StaticVec::new()
}

/// Create a bounded export name from str
pub fn bounded_export_name_from_str(s: &str) -> wrt_error::Result<BoundedExportName> {
    let mut name = StaticVec::new();
    for byte in s.bytes() {
        name.push(byte)?;
    }
    Ok(name)
}

/// Create a new bounded export map
pub fn new_export_map<V>() -> BoundedExportMap<V> {
    StaticMap::new()
}

/// Create a new bounded import map
pub fn new_import_map<V>() -> BoundedImportMap<V> {
    StaticMap::new()
}

/// Create a new bounded type map
pub fn new_type_map<V>() -> BoundedTypeMap<V> {
    StaticMap::new()
}

/// Create a new bounded resource type map
pub fn new_resource_type_map<V>() -> BoundedResourceTypeMap<V> {
    StaticMap::new()
}
