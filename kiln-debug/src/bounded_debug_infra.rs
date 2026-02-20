//! Bounded Infrastructure for Debug
//!
//! This module provides bounded alternatives for debug collections
//! to ensure static memory allocation throughout the debug system.

// Import standard traits for bounds
use core::{
    clone::Clone,
    cmp::{Eq, PartialEq},
    default::Default,
};

use wrt_error::Result;
use wrt_foundation::{
    bounded::{BoundedString, BoundedVec},
    bounded_collections::BoundedMap as BoundedHashMap,
    budget_aware_provider::CrateId,
    safe_managed_alloc,
    safe_memory::NoStdProvider,
    traits::{Checksummable, FromBytes, ToBytes},
};

/// Maximum stack trace depth
pub const MAX_STACK_TRACE_DEPTH: usize = 256;

/// Maximum number of source files
pub const MAX_SOURCE_FILES: usize = 512;

/// Maximum source file path length
pub const MAX_FILE_PATH_LEN: usize = 512;

/// Maximum function name length
pub const MAX_FUNCTION_NAME_LEN: usize = 256;

/// Maximum number of breakpoints
pub const MAX_BREAKPOINTS: usize = 1024;

/// Maximum number of watch expressions
pub const MAX_WATCH_EXPRESSIONS: usize = 256;

/// Maximum number of local variables per frame
pub const MAX_LOCALS_PER_FRAME: usize = 256;

/// Maximum number of type definitions for debugging
pub const MAX_DEBUG_TYPE_DEFINITIONS: usize = 2048;

/// Maximum source mapping entries
pub const MAX_SOURCE_MAP_ENTRIES: usize = 4096;

/// Maximum diagnostic messages
pub const MAX_DIAGNOSTIC_MESSAGES: usize = 128;

/// Debug provider size (32KB)
pub const DEBUG_PROVIDER_SIZE: usize = 32768;

/// Provider type alias for debug crate
pub type DebugProvider = NoStdProvider<DEBUG_PROVIDER_SIZE>;

/// Create a debug-specific string type
pub fn create_debug_string(s: &str) -> Result<BoundedString<MAX_FILE_PATH_LEN>> {
    let _guard = safe_managed_alloc!(DEBUG_PROVIDER_SIZE, CrateId::Debug)?;
    BoundedString::try_from_str(s)
        .map_err(|_| wrt_error::Error::memory_error("Failed to create debug string"))
}

/// Create a debug-specific vector
pub fn create_debug_vec<T, const N: usize>() -> Result<BoundedVec<T, N, DebugProvider>>
where
    T: Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
{
    let guard = safe_managed_alloc!(DEBUG_PROVIDER_SIZE, CrateId::Debug)?;
    BoundedVec::new(guard.clone())
        .map_err(|_| wrt_error::Error::memory_error("Failed to create debug vector"))
}

/// Macro to simplify debug vector creation
#[macro_export]
macro_rules! debug_vec {
    ($capacity:expr) => {{ $crate::bounded_debug_infra::create_debug_vec::<_, $capacity>() }};
}

/// Macro to simplify debug string creation
#[macro_export]
macro_rules! debug_string {
    ($s:expr) => {{ $crate::bounded_debug_infra::create_debug_string($s) }};
}

/// Maximum diagnostic message length
pub const MAX_DIAGNOSTIC_MESSAGE_LEN: usize = 1024;

/// Maximum symbol table entries
pub const MAX_SYMBOL_TABLE_ENTRIES: usize = 8192;

/// Maximum debug string length
pub const MAX_DEBUG_STRING_LEN: usize = 512;

// Type aliases for cleaner usage (using factory pattern instead of direct
// provider)
/// Bounded vector for stack frames
pub type BoundedStackTraceVec<T> = BoundedVec<T, MAX_STACK_TRACE_DEPTH, DebugProvider>;

/// Bounded vector for source files
pub type BoundedSourceFileVec<T> = BoundedVec<T, MAX_SOURCE_FILES, DebugProvider>;

/// Bounded string for file paths
pub type BoundedFilePath = BoundedString<MAX_FILE_PATH_LEN>;

/// Bounded string for function names
pub type BoundedFunctionName = BoundedString<MAX_FUNCTION_NAME_LEN>;

/// Bounded vector for breakpoints
pub type BoundedBreakpointVec<T> = BoundedVec<T, MAX_BREAKPOINTS, DebugProvider>;

/// Bounded vector for watch expressions
pub type BoundedWatchVec<T> = BoundedVec<T, MAX_WATCH_EXPRESSIONS, DebugProvider>;

/// Bounded vector for local variables
pub type BoundedLocalsDebugVec<T> = BoundedVec<T, MAX_LOCALS_PER_FRAME, DebugProvider>;

/// Bounded map for type definitions
pub type BoundedDebugTypeMap<V> = BoundedHashMap<u32, V, MAX_DEBUG_TYPE_DEFINITIONS, DebugProvider>;

/// Bounded map for source mapping
pub type BoundedSourceMap<V> = BoundedHashMap<u32, V, MAX_SOURCE_MAP_ENTRIES, DebugProvider>;

/// Bounded vector for diagnostic messages
pub type BoundedDiagnosticVec<T> = BoundedVec<T, MAX_DIAGNOSTIC_MESSAGES, DebugProvider>;

/// Bounded string for diagnostic messages
pub type BoundedDiagnosticMessage = BoundedString<MAX_DIAGNOSTIC_MESSAGE_LEN>;

/// Bounded map for symbol table
pub type BoundedSymbolMap<V> =
    BoundedHashMap<BoundedFunctionName, V, MAX_SYMBOL_TABLE_ENTRIES, DebugProvider>;

/// Bounded string for debug output
pub type BoundedDebugString = BoundedString<MAX_DEBUG_STRING_LEN>;

/// Create a new bounded stack trace vector
pub fn new_stack_trace_vec<T>() -> Result<BoundedStackTraceVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let guard = safe_managed_alloc!(DEBUG_PROVIDER_SIZE, CrateId::Debug)?;
    let provider = guard.clone();
    BoundedVec::new(provider)
        .map_err(|_| wrt_error::Error::memory_error("Failed to create stack trace vector"))
}

/// Create a new bounded source file vector
pub fn new_source_file_vec<T>() -> Result<BoundedSourceFileVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let guard = safe_managed_alloc!(DEBUG_PROVIDER_SIZE, CrateId::Debug)?;
    let provider = guard.clone();
    BoundedVec::new(provider)
        .map_err(|_| wrt_error::Error::memory_error("Failed to create source file vector"))
}

/// Create a new bounded file path
pub fn new_file_path() -> Result<BoundedFilePath> {
    let _guard = safe_managed_alloc!(DEBUG_PROVIDER_SIZE, CrateId::Debug)?;
    let _provider = _guard.clone();
    BoundedString::try_from_str("")
        .map_err(|_| wrt_error::Error::memory_error("Failed to create file path"))
}

/// Create a bounded file path from str
pub fn bounded_file_path_from_str(s: &str) -> Result<BoundedFilePath> {
    let _guard = safe_managed_alloc!(DEBUG_PROVIDER_SIZE, CrateId::Debug)?;
    let _provider = _guard.clone();
    BoundedString::try_from_str(s)
        .map_err(|_| wrt_error::Error::memory_error("Failed to create file path from str"))
}

/// Create a new bounded function name
pub fn new_function_name() -> Result<BoundedFunctionName> {
    let _guard = safe_managed_alloc!(DEBUG_PROVIDER_SIZE, CrateId::Debug)?;
    let _provider = _guard.clone();
    BoundedString::try_from_str("")
        .map_err(|_| wrt_error::Error::memory_error("Failed to create function name"))
}

/// Create a bounded function name from str
pub fn bounded_function_name_from_str(s: &str) -> Result<BoundedFunctionName> {
    let _guard = safe_managed_alloc!(DEBUG_PROVIDER_SIZE, CrateId::Debug)?;
    let _provider = _guard.clone();
    BoundedString::try_from_str(s)
        .map_err(|_| wrt_error::Error::memory_error("Failed to create function name from str"))
}

/// Create a new bounded breakpoint vector
pub fn new_breakpoint_vec<T>() -> Result<BoundedBreakpointVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let guard = safe_managed_alloc!(DEBUG_PROVIDER_SIZE, CrateId::Debug)?;
    let provider = guard.clone();
    BoundedVec::new(provider)
        .map_err(|_| wrt_error::Error::memory_error("Failed to create breakpoint vector"))
}

/// Create a new bounded watch vector
pub fn new_watch_vec<T>() -> Result<BoundedWatchVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let guard = safe_managed_alloc!(DEBUG_PROVIDER_SIZE, CrateId::Debug)?;
    let provider = guard.clone();
    BoundedVec::new(provider)
        .map_err(|_| wrt_error::Error::memory_error("Failed to create watch vector"))
}

/// Create a new bounded locals debug vector
pub fn new_locals_debug_vec<T>() -> Result<BoundedLocalsDebugVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let guard = safe_managed_alloc!(DEBUG_PROVIDER_SIZE, CrateId::Debug)?;
    let provider = guard.clone();
    BoundedVec::new(provider)
        .map_err(|_| wrt_error::Error::memory_error("Failed to create locals debug vector"))
}

/// Create a new bounded debug type map
pub fn new_debug_type_map<V>() -> Result<BoundedDebugTypeMap<V>>
where
    V: Checksummable + ToBytes + FromBytes + Clone + Default + PartialEq + Eq,
{
    let guard = safe_managed_alloc!(DEBUG_PROVIDER_SIZE, CrateId::Debug)?;
    let provider = guard.clone();
    BoundedHashMap::new(provider)
        .map_err(|_| wrt_error::Error::memory_error("Failed to create debug type map"))
}

/// Create a new bounded source map
pub fn new_source_map<V>() -> Result<BoundedSourceMap<V>>
where
    V: Checksummable + ToBytes + FromBytes + Clone + Default + PartialEq + Eq,
{
    let guard = safe_managed_alloc!(DEBUG_PROVIDER_SIZE, CrateId::Debug)?;
    let provider = guard.clone();
    BoundedHashMap::new(provider)
        .map_err(|_| wrt_error::Error::memory_error("Failed to create source map"))
}

/// Create a new bounded diagnostic vector
pub fn new_diagnostic_vec<T>() -> Result<BoundedDiagnosticVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let guard = safe_managed_alloc!(DEBUG_PROVIDER_SIZE, CrateId::Debug)?;
    let provider = guard.clone();
    BoundedVec::new(provider)
        .map_err(|_| wrt_error::Error::memory_error("Failed to create diagnostic vector"))
}

/// Create a new bounded diagnostic message
pub fn new_diagnostic_message() -> Result<BoundedDiagnosticMessage> {
    let _guard = safe_managed_alloc!(DEBUG_PROVIDER_SIZE, CrateId::Debug)?;
    let _provider = _guard.clone();
    BoundedString::try_from_str("")
        .map_err(|_| wrt_error::Error::memory_error("Failed to create diagnostic message"))
}

/// Create a bounded diagnostic message from str
pub fn bounded_diagnostic_from_str(s: &str) -> Result<BoundedDiagnosticMessage> {
    let _guard = safe_managed_alloc!(DEBUG_PROVIDER_SIZE, CrateId::Debug)?;
    let _provider = _guard.clone();
    BoundedString::try_from_str(s).map_err(|_e| {
        wrt_error::Error::memory_error("Failed to create diagnostic message from str")
    })
}

/// Create a new bounded symbol map
pub fn new_symbol_map<V>() -> Result<
    BoundedHashMap<
        BoundedString<MAX_FUNCTION_NAME_LEN>,
        V,
        MAX_SYMBOL_TABLE_ENTRIES,
        DebugProvider,
    >,
>
where
    V: Checksummable + ToBytes + FromBytes + Clone + Default + PartialEq + Eq,
{
    let guard = safe_managed_alloc!(DEBUG_PROVIDER_SIZE, CrateId::Debug)?;
    let provider = guard.clone();
    BoundedHashMap::new(provider)
        .map_err(|_| wrt_error::Error::memory_error("Failed to create symbol map"))
}

/// Create a new bounded debug string
pub fn new_debug_string() -> Result<BoundedString<MAX_DEBUG_STRING_LEN>> {
    let _guard = safe_managed_alloc!(DEBUG_PROVIDER_SIZE, CrateId::Debug)?;
    let _provider = _guard.clone();
    BoundedString::try_from_str("")
        .map_err(|_| wrt_error::Error::memory_error("Failed to create debug string"))
}

/// Create a bounded debug string from str
pub fn bounded_debug_string_from_str(s: &str) -> Result<BoundedString<MAX_DEBUG_STRING_LEN>> {
    let _guard = safe_managed_alloc!(DEBUG_PROVIDER_SIZE, CrateId::Debug)?;
    let _provider = _guard.clone();
    BoundedString::try_from_str(s)
        .map_err(|_| wrt_error::Error::memory_error("Failed to create debug string from str"))
}
