//! Type conversion utilities for Component Model types
//!
//! This module provides bidirectional conversion between different
//! representations of Component Model types across crate boundaries.

// Re-export the bidirectional module
pub mod bidirectional;

// Export the wrappers module
pub mod wrappers;

// Export the new registry module
pub mod registry;

// Export the registry-based conversions module
pub mod registry_conversions;

// Test modules have been migrated to their corresponding implementation files

// Re-export the most important types and functions for backward compatibility
pub use bidirectional::{
    IntoFormatType, IntoRuntimeType, common_to_format_val_type, core_value_to_types_componentvalue,
    extern_type_to_func_type, format_constvalue_to_types_componentvalue, format_to_common_val_type,
    format_to_runtime_extern_type as format_to_types_extern_type, format_val_type_to_value_type,
    format_valtype_to_types_valtype, runtime_to_format_extern_type as types_to_format_extern_type,
    types_componentvalue_to_core_value, types_componentvalue_to_format_constvalue,
    types_valtype_to_format_valtype, value_type_to_format_val_type, value_type_to_types_valtype,
};
// Re-export registry types for easy access
pub use registry::{
    Conversion, ConversionError, ConversionErrorKind, Convertible, TypeConversionRegistry,
};
// Re-export registry conversion functions
pub use registry_conversions::{
    register_component_instancetype_conversions, register_externtype_conversions,
    register_valtype_conversions,
};
// Re-export wrapper types for easy access
pub use wrappers::{
    FormatComponentType, FormatInstanceType, IntoFormatComponentType, IntoFormatInstanceType,
    IntoRuntimeComponentType, IntoRuntimeInstanceType, RuntimeComponentType, RuntimeInstanceType,
};
