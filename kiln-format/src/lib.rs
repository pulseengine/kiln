// WRT - wrt-format
// Module: WebAssembly Binary Format Definitions
// SW-REQ-ID: REQ_021
// SW-REQ-ID: REQ_013
//
// Copyright (c) 2024 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)] // Rule 2
#![allow(missing_docs)] // Allow missing documentation for internal constants and utilities

//! WebAssembly format handling for WRT
//!
//! This crate defines and handles the WebAssembly binary format specifications,
//! including type encodings, section layouts, and module structures.
//!
//! It is designed to work in both std and no_std environments when configured
//! with the appropriate feature flags.

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
// Allow clippy warnings that would require substantial refactoring
#![allow(clippy::pedantic)]
#![allow(clippy::needless_continue)]
#![allow(clippy::if_not_else)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::elidable_lifetime_names)]
#![allow(clippy::unused_self)]
#![allow(clippy::ptr_as_ptr)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::similar_names)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::inline_always)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::semicolon_if_nothing_returned)]
#![allow(clippy::comparison_chain)]
#![allow(clippy::ignored_unit_patterns)]
#![allow(clippy::panic)]
#![allow(clippy::single_match_else)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::explicit_iter_loop)]
#![allow(clippy::bool_to_int_with_if)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::identity_op)]
#![allow(clippy::derivable_impls)]
#![allow(clippy::map_identity)]
#![allow(clippy::expect_used)]
#![allow(clippy::useless_conversion)]
#![allow(clippy::unnecessary_map_or)]
#![allow(clippy::doc_lazy_continuation)]
#![allow(clippy::manual_flatten)]
#![allow(clippy::float_arithmetic)]
#![allow(clippy::unimplemented)]
#![allow(clippy::useless_attribute)]
#![allow(clippy::manual_div_ceil)]
#![allow(clippy::never_loop)]
#![allow(clippy::while_immutable_condition)]
#![allow(clippy::needless_lifetimes)]
#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::redundant_pattern_matching)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::let_and_return)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::empty_line_after_doc_comments)]
#![allow(clippy::unwrap_or_default)]
#![allow(clippy::new_without_default)]
#![allow(clippy::result_large_err)]
#![allow(let_underscore_drop)]

// Import std when available
#[cfg(feature = "std")]
extern crate std;

// Standard library imports (grouped by feature flags)
#[cfg(feature = "std")]
use std::{format, string::String, vec::Vec};

pub use wrt_error::Result;
// External crates
pub use wrt_error::{Error, ErrorCategory};
// Internal crates (wrt_* imports)
#[cfg(not(feature = "std"))]
use wrt_foundation::bounded::{BoundedString, BoundedVec};
pub use wrt_foundation::resource::ResourceRepresentation;
// Re-export core types from wrt-foundation (note: these now have generic parameters)
// BlockType, FuncType, RefType, ValueType now require MemoryProvider parameters
// These will be re-exported as type aliases with default providers

// Collection types are imported privately above and used internally

// Binary std/no_std choice
#[cfg(not(any(feature = "std")))]
pub use wrt_foundation::{BoundedMap, BoundedSet};

// Type aliases for pure no_std mode
#[cfg(not(any(feature = "std")))]
pub type WasmString = BoundedString<MAX_WASM_STRING_SIZE>;
#[cfg(not(any(feature = "std")))]
pub type WasmVec<T, P> = BoundedVec<T, 1024, P>; // General purpose bounded vector
// Module type aliases for pure no_std mode
#[cfg(not(any(feature = "std")))]
pub type ModuleFunctions<P> = BoundedVec<crate::module::Function, MAX_MODULE_FUNCTIONS, P>;
#[cfg(not(any(feature = "std")))]
pub type ModuleImports<P> = BoundedVec<crate::module::Import<P>, MAX_MODULE_IMPORTS, P>;
#[cfg(not(any(feature = "std")))]
pub type ModuleExports<P> = BoundedVec<crate::module::Export, MAX_MODULE_EXPORTS, P>;
#[cfg(not(any(feature = "std")))]
pub type ModuleGlobals<P> = BoundedVec<crate::module::Global<P>, MAX_MODULE_GLOBALS, P>;
#[cfg(not(any(feature = "std")))]
pub type ModuleElements<P> = BoundedVec<crate::module::Element<P>, MAX_MODULE_ELEMENTS, P>;
#[cfg(not(any(feature = "std")))]
#[allow(deprecated)]
pub type ModuleData<P> = BoundedVec<crate::module::Data<P>, MAX_MODULE_DATA, P>;
#[cfg(not(any(feature = "std")))]
pub type ModuleCustomSections<P> = BoundedVec<crate::section::CustomSection, 64, P>;

// Type aliases for HashMap
#[cfg(not(feature = "std"))]
pub type HashMap<K, V> = wrt_foundation::BoundedMap<K, V, 256, wrt_foundation::NoStdProvider<1024>>; // Default capacity

#[cfg(feature = "std")]
pub type HashMap<K, V> = std::collections::BTreeMap<K, V>;

// Maximum recursion depth for recursive types to replace Box<T>
pub const MAX_TYPE_RECURSION_DEPTH: usize = 32;

// Type aliases for WebAssembly-specific collections
#[cfg(feature = "std")]
pub type WasmString = String;
#[cfg(feature = "std")]
pub type WasmVec<T> = Vec<T>;

// In pure no_std mode, we don't provide generic Vec/String aliases
// Individual modules should use the appropriate bounded types directly

// Helper macro for conditional type usage
#[macro_export]
macro_rules! collection_type {
    (Vec < $t:ty >) => {
        #[cfg(feature = "std")]
        type VecType = Vec<$t>;
        #[cfg(not(any(feature = "std")))]
        type VecType = $crate::WasmVec<$t, $crate::NoStdProvider<1024>>;
    };
    (String) => {
        #[cfg(feature = "std")]
        type StringType = String;
        #[cfg(not(any(feature = "std")))]
        type StringType = $crate::WasmString<$crate::NoStdProvider<1024>>;
    };
}

// Compile-time capacity constants for bounded collections
// Increased limits for better no_std usability
pub const MAX_MODULE_TYPES: usize = 512; // was 256
pub const MAX_MODULE_FUNCTIONS: usize = 4096; // was 1024
pub const MAX_MODULE_IMPORTS: usize = 512; // was 256
pub const MAX_MODULE_EXPORTS: usize = 512; // was 256
pub const MAX_MODULE_GLOBALS: usize = 512; // was 256
pub const MAX_MODULE_TABLES: usize = 128; // was 64
pub const MAX_MODULE_MEMORIES: usize = 128; // was 64
pub const MAX_MODULE_ELEMENTS: usize = 512; // was 256
pub const MAX_MODULE_DATA: usize = 512; // was 256
pub const MAX_WASM_STRING_SIZE: usize = 1024; // was 256
pub const MAX_BINARY_SIZE: usize = 4 * 1024 * 1024; // 4MB max module size, was 1MB
pub const MAX_LEB128_BUFFER: usize = 10; // Max bytes for LEB128 u64
pub const MAX_INSTRUCTION_OPERANDS: usize = 32; // was 16
pub const MAX_STACK_DEPTH: usize = 2048; // was 1024

// Component model constants (increased for better support)
pub const MAX_COMPONENT_INSTANCES: usize = 256; // was 128
pub const MAX_COMPONENT_TYPES: usize = 512; // was 256
pub const MAX_COMPONENT_IMPORTS: usize = 512; // was 256
pub const MAX_COMPONENT_EXPORTS: usize = 512; // was 256

// Additional no_std specific constants
pub const MAX_SECTION_SIZE_NO_STD: usize = 256 * 1024; // 256KB, was 64KB
pub const MAX_BOUNDED_AST_NODES: usize = 256;
pub const MAX_BOUNDED_TOKENS: usize = 512;
pub const MAX_STATIC_TYPES: usize = 64;

// For no_std mode, provide format! macro replacement using static strings
#[cfg(not(any(feature = "std")))]
#[macro_export]
macro_rules! format {
    ($lit:literal) => {
        $lit
    };
    ($fmt:literal, $($arg:expr),*) => {
        // In pure no_std mode, we can't format strings dynamically
        // Return a static error message instead
        "formatting not available in no_std mode"
    };
}

/// Abstract Syntax Tree types for WIT parsing (simplified version)
#[cfg(feature = "std")]
pub mod ast_simple;
#[cfg(feature = "std")]
pub use ast_simple as ast;
/// WebAssembly binary format parsing and access
pub mod binary;
/// Bounded infrastructure for static memory allocation
#[cfg(not(feature = "std"))]
pub mod bounded_format_infra;
/// WebAssembly canonical format
#[cfg(feature = "std")]
pub mod canonical;
/// WebAssembly component model format
#[cfg(feature = "std")]
pub mod component;
/// Conversion utilities for component model types
#[cfg(feature = "std")]
pub mod component_conversion;
/// Compression utilities for WebAssembly modules
pub mod compression;
/// Conversion utilities for type system standardization
pub mod conversion;
/// Error utilities for working with wrt-error types
pub mod error;
/// Incremental parser for efficient WIT re-parsing
#[cfg(feature = "std")]
pub mod incremental_parser;
/// Interface demonstration (clean separation)
pub mod interface_demo;
/// Basic LSP (Language Server Protocol) infrastructure
#[cfg(all(feature = "std", feature = "lsp"))]
pub mod lsp_server;
/// Safety-critical memory limits
#[cfg(feature = "safety-critical")]
pub mod memory_limits;
/// WebAssembly module format
pub mod module;
/// Common imports for convenience
pub mod prelude;
/// Pure format representation types
pub mod pure_format_types;
/// Runtime bridge interface
pub mod runtime_bridge;
/// Safe memory operations
pub mod safe_memory;
pub mod section;
/// Streaming parser for no_std environments
pub mod streaming;
/// Type storage system for Component Model
#[cfg(feature = "std")]
pub mod type_store;
pub mod types;
/// Validation utilities
pub mod validation;
/// ValType builder utilities
#[cfg(feature = "std")]
pub mod valtype_builder;
pub mod verify;
pub mod version;
// Binary std/no_std choice
// Temporarily disabled - causes circular dependency issues
// #[cfg(feature = "std")]
// pub mod wit_parser_types;
// #[cfg(feature = "std")]
// pub mod wit_parser_traits;
// #[cfg(feature = "std")]
// pub mod wit_parser;
// Bounded WIT parser for no_std environments
#[cfg(feature = "wit-parsing")]
pub mod wit_parser_bounded;
// Enhanced bounded WIT parser with configurable limits (Agent C)
pub mod bounded_wit_parser;
// Temporarily disable enhanced parser until compilation issues fixed
// #[cfg(feature = "std")]
// pub mod wit_parser_enhanced;
// Temporarily disable problematic parsers
// #[cfg(feature = "std")]
// pub mod wit_parser_complex;
// #[cfg(feature = "std")]
// pub mod wit_parser_old;
// #[cfg(feature = "std")]
// pub mod wit_parser_traits;

// Test modules
#[cfg(test)]
mod ast_simple_tests;

// Re-export binary constants (always available)
// Binary std/no_std choice
// Pure format parsing functions (recommended)
#[cfg(feature = "std")]
pub use binary::with_alloc::{parse_data, parse_element_segment_pure};
#[cfg(feature = "std")]
pub use binary::with_alloc::{
    read_name,
    read_string,
};

// Binary std/no_std choice
#[cfg(feature = "std")]
pub use binary::with_alloc::{
    write_leb128_i32, write_leb128_i64, write_leb128_u32, write_leb128_u64, write_string,
};
// Re-export binary parsing functions
// Core parsing functions available in all configurations
pub use binary::{
    read_leb128_i32, read_leb128_i64, read_leb128_u32, read_leb128_u64, read_u8, read_u32,
};
// Re-export no_std write functions
pub use binary::{
    COMPONENT_CORE_SORT_FUNC, COMPONENT_CORE_SORT_GLOBAL, COMPONENT_CORE_SORT_INSTANCE,
    COMPONENT_CORE_SORT_MEMORY, COMPONENT_CORE_SORT_MODULE, COMPONENT_CORE_SORT_TABLE,
    COMPONENT_CORE_SORT_TYPE, COMPONENT_MAGIC, COMPONENT_SORT_COMPONENT, COMPONENT_SORT_CORE,
    COMPONENT_SORT_FUNC, COMPONENT_SORT_INSTANCE, COMPONENT_SORT_TYPE, COMPONENT_SORT_VALUE,
    COMPONENT_VERSION, WASM_MAGIC, WASM_VERSION,
};
#[cfg(not(any(feature = "std")))]
pub use binary::{
    write_leb128_u32_bounded, write_leb128_u32_to_slice, write_string_bounded,
    write_string_to_slice,
};
#[cfg(feature = "std")]
pub use component::Component;
pub use compression::CompressionType;
#[cfg(feature = "std")]
pub use compression::{rle_decode, rle_encode};
// Re-export conversion utilities
pub use conversion::{
    block_type_to_format_block_type, format_block_type_to_block_type, format_limits_to_wrt_limits,
    wrt_limits_to_format_limits,
};
pub use error::{
    parse_error, wrt_runtime_error as runtime_error, wrt_type_error as type_error,
    wrt_validation_error as validation_error,
};
// Note: Data, DataMode, ElementMode are deprecated - use pure_format_types instead
#[allow(deprecated)]
#[deprecated(note = "Use pure_format_types::PureDataSegment for clean separation")]
pub use module::Data;
pub use module::{Element, ElementInit, Module};
// New pure format types (recommended)
pub use pure_format_types::{PureDataMode, PureDataSegment, PureElementMode, PureElementSegment};
// DataMode and ElementMode exports removed - use pure_format_types instead

// Type aliases for compatibility (recommended to use pure_format_types
// directly)
pub type DataSegment = pure_format_types::PureDataSegment;
pub type ElementSegment = pure_format_types::PureElementSegment;
// Legacy aliases (deprecated)
#[allow(deprecated)]
#[deprecated(note = "Use pure_format_types::PureDataSegment directly")]
pub type LegacyDataSegment = module::Data;
#[deprecated(note = "Use pure_format_types::PureElementSegment directly")]
pub type LegacyElementSegment = module::Element;
// Re-export safe memory utilities
// Re-export enhanced bounded WIT parser (Agent C)
pub use bounded_wit_parser::{
    BoundedWitParser as EnhancedBoundedWitParser, WarningSeverity, WitParseMetadata,
    WitParseResult, WitParseWarning, WitParsingLimits, parse_wit_embedded, parse_wit_linux,
    parse_wit_qnx, parse_wit_with_limits,
};
pub use safe_memory::safe_slice;
pub use section::{CustomSection, Section};
// Use the conversion module versions for consistency
pub use types::{FormatBlockType, Limits, MemoryIndexType};
pub use validation::Validatable;
pub use version::{
    ComponentModelFeature, ComponentModelVersion, FeatureStatus, STATE_VERSION, VersionInfo,
};
#[cfg(feature = "wit-parsing")]
pub use wit_parser_bounded::{
    BoundedWitExport, BoundedWitFunction, BoundedWitImport, BoundedWitInterface, BoundedWitParser,
    BoundedWitType, BoundedWitWorld, HAS_BOUNDED_WIT_PARSING_NO_STD, parse_wit_bounded,
};

// Public functions for feature detection
/// Check if a component model feature is available in a binary
pub fn is_feature_available(info: &VersionInfo, feature: ComponentModelFeature) -> bool {
    info.is_feature_available(feature)
}

/// Get the status of a component model feature
pub fn get_feature_status(info: &VersionInfo, feature: ComponentModelFeature) -> FeatureStatus {
    info.get_feature_status(feature)
}

/// Check if a binary uses any experimental features
pub fn uses_experimental_features(binary: &[u8]) -> bool {
    let version_bytes = if binary.len() >= 8 {
        [binary[4], binary[5], binary[6], binary[7]]
    } else {
        return false;
    };

    let mut info = VersionInfo::from_version_bytes(version_bytes);
    info.detect_experimental_features(binary)
}

// For formal verification when the 'kani' feature is enabled
#[cfg(feature = "kani")]
pub mod verification {
    /// Verify LEB128 encoding and decoding
    #[cfg(all(kani, feature = "std"))]
    #[kani::proof]
    fn verify_leb128_roundtrip() {
        let value: u32 = kani::any();
        // Limit to reasonable values for test
        kani::assume(value <= 0xFFFF);

        let encoded = super::binary::with_alloc::write_leb128_u32(value);
        let (decoded, _) = super::binary::read_leb128_u32(&encoded, 0).unwrap();

        assert_eq!(value, decoded);
    }
}

/// Demonstration of pure no_std WebAssembly format handling
#[cfg(not(any(feature = "std")))]
pub mod no_std_demo {
    use wrt_foundation::NoStdProvider;

    use super::*;

    /// Example showing TypeRef system working
    #[cfg(feature = "std")]
    pub fn demo_type_system() -> wrt_error::Result<()> {
        use crate::component::{FormatValType, TypeRegistry};

        // Create a type registry
        let mut registry = TypeRegistry::new();

        // Add a primitive type
        let bool_ref = registry.add_type(FormatValType::Bool)?;

        // Add a list type that references the bool type
        let bool_list_ref = registry.add_type(FormatValType::List(bool_ref))?;

        // Verify we can retrieve the types
        assert!(registry.get_type(bool_ref).is_some());
        assert!(registry.get_type(bool_list_ref).is_some());

        Ok(())
    }

    /// Example showing bounded string working
    pub fn demo_bounded_string() -> Result<()> {
        let _provider = wrt_foundation::safe_managed_alloc!(
            1024,
            wrt_foundation::budget_aware_provider::CrateId::Format
        )?;
        let wasm_str = WasmString::try_from_str("hello")
            .map_err(|_| wrt_foundation::bounded::CapacityError)?;
        assert_eq!(wasm_str.as_str().unwrap(), "hello");
        Ok(())
    }

    /// Binary std/no_std choice
    pub fn demo_leb128_parsing() -> crate::Result<()> {
        let data = [0x80, 0x01]; // LEB128 encoding of 128
        let (value, consumed) = crate::binary::read_leb128_u32(&data, 0)?;
        assert_eq!(value, 128);
        assert_eq!(consumed, 2);
        Ok(())
    }

    /// Example showing complete no_std WebAssembly parsing workflow
    pub fn demo_no_std_parsing_workflow() -> crate::Result<()> {
        use wrt_foundation::NoStdProvider;

        use crate::streaming::StreamingParser;

        // Binary std/no_std choice
        let wasm_data = [
            // Magic bytes: \0asm
            0x00, 0x61, 0x73, 0x6D, // Version: 1.0.0.0
            0x01, 0x00, 0x00, 0x00,
            // Empty module (no sections)
        ];

        // Create streaming parser with bounded memory
        let provider = wrt_foundation::safe_managed_alloc!(
            1024,
            wrt_foundation::budget_aware_provider::CrateId::Format
        )?;
        let mut parser = StreamingParser::new(provider)?;

        // Process the WebAssembly data
        let result = parser.process_chunk(&wasm_data)?;

        // Verify parsing completed successfully
        match result {
            crate::streaming::ParseResult::Complete(_) => {
                assert_eq!(parser.bytes_processed(), 8); // 4 magic + 4 version bytes
                Ok(())
            },
            _ => Err(crate::Error::runtime_execution_error(
                "Unexpected parse result",
            )),
        }
    }

    /// Example showing module creation in pure no_std mode
    #[cfg(feature = "std")]
    pub fn demo_module_creation() -> Result<()> {
        use wrt_foundation::NoStdProvider;

        use crate::module::Module;

        // This demonstrates that the Module type system works in pure no_std
        let provider = wrt_foundation::safe_managed_alloc!(
            1024,
            wrt_foundation::budget_aware_provider::CrateId::Format
        )?;
        let _module = Module::<NoStdProvider<1024>>::default();

        // Binary std/no_std choice
        Ok(())
    }
}

// Panic handler disabled to avoid conflicts with other crates
// // Provide a panic handler only when wrt-format is being tested in isolation
// #[cfg(all(not(feature = "std"), not(test), not(feature =
// "disable-panic-handler")))] #[panic_handler]
// fn panic(_info: &core::panic::PanicInfo) -> ! {
//     loop {}
// }
