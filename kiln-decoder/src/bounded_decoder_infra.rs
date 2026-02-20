//! Bounded Infrastructure for Decoder
//!
//! This module provides static, inline-storage collections for the decoder
//! to ensure compile-time memory allocation with zero runtime overhead.
//!
//! Migrated from Provider-based BoundedVec to heapless-inspired StaticVec.

use wrt_foundation::collections::{StaticMap, StaticVec};

pub use wrt_foundation::bounded::WasmName;
pub use wrt_foundation::safe_memory::NoStdProvider;
/// New static collections eliminate Provider abstraction
/// All memory is inline with compile-time capacity enforcement
// Re-export foundation types for simplified migration
// These will be fully migrated later when wrt-foundation types are updated
pub use wrt_foundation::types::{FuncType, GlobalType, Import, ImportDesc, MemoryType, TableType};

// Temporary: Keep DecoderProvider alias for gradual migration
// TODO: Remove once all wrt-foundation types are migrated to StaticVec
pub type DecoderProvider = NoStdProvider<65536>;

/// Maximum number of sections in a module
pub const MAX_SECTIONS: usize = 32;

/// Maximum number of types in a module
pub const MAX_TYPES: usize = 1024;

/// Maximum number of imports in a module
pub const MAX_IMPORTS: usize = 1024;

/// Maximum number of exports in a module
pub const MAX_EXPORTS: usize = 1024;

/// Maximum number of functions in a module
pub const MAX_FUNCTIONS: usize = 4096;

/// Maximum number of table entries
pub const MAX_TABLE_ENTRIES: usize = 1024;

/// Maximum number of memory definitions
pub const MAX_MEMORIES: usize = 8;

/// Maximum number of globals
pub const MAX_GLOBALS: usize = 1024;

/// Maximum number of elements
pub const MAX_ELEMENTS: usize = 1024;

/// Maximum number of data segments
pub const MAX_DATA_SEGMENTS: usize = 1024;

/// Maximum number of locals per function
pub const MAX_LOCALS_PER_FUNCTION: usize = 256;

/// Maximum length for names (modules, functions, etc.)
pub const MAX_NAME_LENGTH: usize = 256;

/// Maximum custom section data size
pub const MAX_CUSTOM_SECTION_SIZE: usize = 16384; // 16KB

/// Maximum number of function parameters
pub const MAX_FUNCTION_PARAMS: usize = 128;

/// Maximum number of function results
pub const MAX_FUNCTION_RESULTS: usize = 16;

/// Maximum number of entries in name maps
pub const MAX_NAME_MAP_ENTRIES: usize = 1024;

// Type aliases using new static collections with inline storage
// No Provider abstraction - all memory is inline at compile time

/// Section vector with static capacity
pub type BoundedSectionVec<T> = StaticVec<T, MAX_SECTIONS>;

/// Type vector with static capacity
pub type BoundedTypeVec<T> = StaticVec<T, MAX_TYPES>;

/// Import vector with static capacity
pub type BoundedImportVec<T> = StaticVec<T, MAX_IMPORTS>;

/// Export vector with static capacity
pub type BoundedExportVec<T> = StaticVec<T, MAX_EXPORTS>;

/// Function vector with static capacity
pub type BoundedFunctionVec<T> = StaticVec<T, MAX_FUNCTIONS>;

/// Table vector with static capacity
pub type BoundedTableVec<T> = StaticVec<T, MAX_TABLE_ENTRIES>;

/// Memory vector with static capacity
pub type BoundedMemoryVec<T> = StaticVec<T, MAX_MEMORIES>;

/// Global vector with static capacity
pub type BoundedGlobalVec<T> = StaticVec<T, MAX_GLOBALS>;

/// Element vector with static capacity
pub type BoundedElementVec<T> = StaticVec<T, MAX_ELEMENTS>;

/// Data vector with static capacity
pub type BoundedDataVec<T> = StaticVec<T, MAX_DATA_SEGMENTS>;

/// Module vector with static capacity
pub type BoundedModuleVec<T> = StaticVec<T, MAX_MODULES_PER_COMPONENT>;

/// Name string with static capacity (simple string wrapper)
/// TODO: Implement StaticString or use StaticVec<u8, MAX_NAME_LENGTH> directly
pub type BoundedNameString = StaticVec<u8, MAX_NAME_LENGTH>;

/// Name map with static capacity
pub type BoundedNameMap<V> = StaticMap<BoundedNameString, V, MAX_NAME_MAP_ENTRIES>;

/// Custom data with static capacity
pub type BoundedCustomData = StaticVec<u8, MAX_CUSTOM_SECTION_SIZE>;

/// Create a new section vector with inline storage
pub fn new_section_vec<T>() -> BoundedSectionVec<T> {
    StaticVec::new()
}

/// Create a new type vector with inline storage
/// Note: FuncType still needs Provider parameter until fully migrated
pub fn new_type_vec() -> BoundedTypeVec<wrt_foundation::types::FuncType> {
    StaticVec::new()
}

/// Create a new type vector (generic version) with inline storage
pub fn new_type_vec_generic<T>() -> BoundedTypeVec<T> {
    StaticVec::new()
}

/// Create a new import vector with inline storage
/// Note: Import still needs Provider parameter until fully migrated
pub fn new_import_vec() -> BoundedImportVec<wrt_foundation::types::Import<DecoderProvider>> {
    StaticVec::new()
}

/// Create a new import vector (generic version) with inline storage
pub fn new_import_vec_generic<T>() -> BoundedImportVec<T> {
    StaticVec::new()
}

/// Create a new export vector with inline storage
pub fn new_export_vec() -> BoundedExportVec<wrt_format::module::Export> {
    StaticVec::new()
}

/// Create a new export vector (generic version) with inline storage
pub fn new_export_vec_generic<T>() -> BoundedExportVec<T> {
    StaticVec::new()
}

/// Create a new function vector with inline storage
pub fn new_function_vec() -> BoundedFunctionVec<u32> {
    StaticVec::new()
}

/// Create a new function vector (generic version) with inline storage
pub fn new_function_vec_generic<T>() -> BoundedFunctionVec<T> {
    StaticVec::new()
}

/// Create a new name string with inline storage
pub fn new_name_string() -> BoundedNameString {
    StaticVec::new()
}

/// Create a name string from a str with inline storage
pub fn bounded_name_from_str(s: &str) -> wrt_error::Result<BoundedNameString> {
    let mut name = StaticVec::new();
    for byte in s.bytes() {
        name.push(byte)?;
    }
    Ok(name)
}

/// Create a new name map with inline storage
pub fn new_name_map<V>() -> BoundedNameMap<V> {
    StaticMap::new()
}

// Additional concrete vector factory functions

/// Create a new params vector (for function parameters) with inline storage
pub fn new_params_vec() -> StaticVec<wrt_format::types::ValueType, MAX_FUNCTION_PARAMS> {
    StaticVec::new()
}

/// Create a new results vector (for function results) with inline storage
pub fn new_results_vec() -> StaticVec<wrt_format::types::ValueType, MAX_FUNCTION_RESULTS> {
    StaticVec::new()
}

/// Create a new table vector with inline storage
pub fn new_table_vec() -> BoundedTableVec<wrt_foundation::types::TableType> {
    StaticVec::new()
}

/// Create a new memory vector with inline storage
pub fn new_memory_vec() -> BoundedMemoryVec<wrt_foundation::types::MemoryType> {
    StaticVec::new()
}

/// Create a new global vector with inline storage
pub fn new_global_vec() -> BoundedGlobalVec<wrt_foundation::types::GlobalType> {
    StaticVec::new()
}

/// Create a new element vector with inline storage
pub fn new_element_vec() -> BoundedElementVec<wrt_format::module::Element> {
    StaticVec::new()
}

/// Create a new data vector with inline storage
pub fn new_data_vec() -> BoundedDataVec<wrt_format::pure_format_types::PureDataSegment> {
    StaticVec::new()
}

/// Create a new code bodies vector with inline storage
pub fn new_code_bodies_vec() -> BoundedFunctionVec<BoundedCustomData> {
    StaticVec::new()
}

// Component Model specific constants

/// Maximum number of modules in a component
pub const MAX_MODULES_PER_COMPONENT: usize = 64;

/// Maximum number of types in a component
pub const MAX_TYPES_PER_COMPONENT: usize = 512;

/// Maximum number of instances in a component
pub const MAX_INSTANCES_PER_COMPONENT: usize = 128;

/// Maximum number of aliases in a component
pub const MAX_ALIASES_PER_COMPONENT: usize = 256;

/// Create a decoder provider (temporary - for gradual migration)
/// TODO: Remove once all code uses StaticVec instead of Provider-based collections
pub fn create_decoder_provider<const N: usize>() -> wrt_error::Result<NoStdProvider<N>> {
    use wrt_foundation::{budget_aware_provider::CrateId, safe_managed_alloc};
    safe_managed_alloc!(N, CrateId::Decoder)
}
