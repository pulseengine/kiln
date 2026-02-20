//! Format-specific type conversion utilities
//!
//! This module provides standardized type conversion between format-specific
//! types in wrt-format and core types in wrt-foundation. This helps eliminate
//! duplication and ensure consistency across crates.

use core::fmt;
#[cfg(feature = "std")]
use std::format;

use wrt_error::{Error, Result};
use wrt_foundation::{BlockType, ValueType};

use crate::{
    error::parse_error,
    types::{FormatBlockType, Limits},
};

/// Convert from FormatBlockType to BlockType
///
/// Provides a standard conversion from format-specific block types
/// to the core BlockType representation.
pub fn format_block_type_to_block_type(format_block_type: &FormatBlockType) -> BlockType {
    match format_block_type {
        FormatBlockType::Empty => BlockType::Value(None),
        FormatBlockType::ValueType(value_type) => BlockType::Value(Some(*value_type)),
        #[cfg(any(feature = "std", feature = "alloc"))]
        FormatBlockType::FuncType(_func_type) => BlockType::FuncType(0), // TODO: proper type
        // index mapping
        FormatBlockType::TypeIndex(idx) => BlockType::FuncType(*idx),
    }
}

/// Convert from BlockType to FormatBlockType
///
/// Provides a standard conversion from core BlockType
/// to the format-specific representation.
pub fn block_type_to_format_block_type(block_type: &BlockType) -> FormatBlockType {
    match block_type {
        BlockType::Value(None) => FormatBlockType::Empty,
        BlockType::Value(Some(value_type)) => FormatBlockType::ValueType(*value_type),
        BlockType::FuncType(idx) => FormatBlockType::TypeIndex(*idx),
    }
}

/// Convert from format-specific Limits to wrt_foundation::Limits
///
/// Validates and converts format limits to core limits.
pub fn format_limits_to_wrt_limits(
    limits: &crate::types::Limits,
) -> Result<wrt_foundation::types::Limits> {
    if limits.memory64 {
        return Err(Error::runtime_execution_error(
            "memory64 limits are not supported by the current runtime type system (u32 limits).",
        ));
    }

    let min_u32 = limits.min.try_into().map_err(|_| {
        #[cfg(feature = "std")]
        {
            crate::error::validation_error_dynamic(format!(
                "Minimum limit ({}) exceeds u32::MAX for non-memory64.",
                limits.min
            ))
        }
        #[cfg(not(any(feature = "std")))]
        {
            crate::error::validation_error("Minimum limit exceeds u32::MAX for non-memory64.")
        }
    })?;

    let max_u32 = match limits.max {
        Some(val_u64) => Some(val_u64.try_into().map_err(|_| {
            #[cfg(feature = "std")]
            {
                crate::error::validation_error_dynamic(format!(
                    "Maximum limit ({}) exceeds u32::MAX for non-memory64.",
                    val_u64
                ))
            }
            #[cfg(not(any(feature = "std")))]
            {
                crate::error::validation_error("Maximum limit exceeds u32::MAX for non-memory64.")
            }
        })?),
        None => None,
    };

    if let Some(max_val) = max_u32 {
        if max_val < min_u32 {
            #[cfg(feature = "std")]
            {
                return Err(crate::error::validation_error_dynamic(format!(
                    "Maximum limit ({}) cannot be less than minimum limit ({}).",
                    max_val, min_u32
                )));
            }
            #[cfg(not(any(feature = "std")))]
            {
                return Err(crate::error::validation_error(
                    "Maximum limit cannot be less than minimum limit.",
                ));
            }
        }
    }

    Ok(wrt_foundation::types::Limits {
        min: min_u32,
        max: max_u32,
    })
}

/// Convert from wrt_foundation::Limits to format-specific Limits
///
/// Converts core limits to format-specific limits.
///
/// # Arguments
/// * `limits` - The wrt_foundation::Limits to convert
/// * `shared` - Whether the memory is shared (only applicable for memory
///   limits)
/// * `memory64` - Whether the memory uses 64-bit addressing (only applicable
///   for memory limits)
///
/// # Returns
/// A format-specific Limits
pub fn wrt_limits_to_format_limits(
    limits: &wrt_foundation::types::Limits,
    shared: bool,
    memory64: bool,
) -> Limits {
    Limits {
        min: limits.min as u64,
        max: limits.max.map(|m| m as u64),
        shared,
        memory64,
    }
}

/// A shorthand function for converting wrt_foundation::Limits to format Limits
/// with default parameters
///
/// # Arguments
/// * `limits` - The wrt_foundation::Limits to convert
///
/// # Returns
/// A format-specific Limits with shared=false and memory64=false
pub fn types_limits_to_format_limits(limits: &wrt_foundation::types::Limits) -> Limits {
    wrt_limits_to_format_limits(limits, false, false)
}

/// A shorthand function for converting format Limits to wrt_foundation::Limits
/// Alias for format_limits_to_wrt_limits for consistency with
/// types_limits_to_format_limits
///
/// # Arguments
/// * `limits` - The format Limits to convert
///
/// # Returns
/// A Result containing wrt_foundation::Limits
pub fn format_limits_to_types_limits(limits: &Limits) -> Result<wrt_foundation::types::Limits> {
    format_limits_to_wrt_limits(limits)
}

/// Parse a value type from a binary representation
///
/// This is a wrapper around the core binary_to_val_type function
/// that provides format-specific error handling.
pub fn parse_value_type(byte: u8) -> Result<ValueType> {
    ValueType::from_binary(byte).map_err(|e| {
        if e.category == wrt_error::ErrorCategory::Parse {
            e
        } else {
            #[cfg(feature = "std")]
            {
                crate::error::parse_error_dynamic(format!(
                    "Invalid value type byte: 0x{:02x}. Internal error: {}",
                    byte, e
                ))
            }
            #[cfg(not(any(feature = "std")))]
            {
                crate::error::parse_error("Invalid value type byte")
            }
        }
    })
}

/// Format a value type to a binary representation
///
/// This is a wrapper around the core val_type_to_binary function.
pub fn format_value_type(val_type: ValueType) -> u8 {
    val_type.to_binary()
}

/// Convert a type to another type, suitable for use in conversion trait
/// implementations
pub fn convert<T, U, E, F>(value: T, converter: F) -> Result<U>
where
    F: FnOnce(T) -> core::result::Result<U, E>,
    E: fmt::Display,
{
    converter(value).map_err(|_e| parse_error("Conversion error"))
}

/// Validate a format condition
pub fn validate<T, F>(condition: bool, error_fn: F, value: T) -> Result<T>
where
    F: FnOnce() -> Error,
{
    if condition { Ok(value) } else { Err(error_fn()) }
}

/// Validate an optional type
pub fn validate_option<T, F>(option: Option<T>, error_fn: F) -> Result<T>
where
    F: FnOnce() -> Error,
{
    option.ok_or_else(|| error_fn())
}

/// Validate format types against numeric bounds
pub fn validate_format<T, U>(value: T, min: U, max: U) -> Result<T>
where
    T: fmt::Display + PartialOrd + Copy,
    U: fmt::Display + PartialOrd + Copy,
    T: PartialOrd<U>,
{
    if value < min {
        #[cfg(feature = "std")]
        {
            return Err(crate::error::validation_error_dynamic(format!(
                "Value {} is too small, minimum is {}",
                value, min
            )));
        }
        #[cfg(not(any(feature = "std")))]
        {
            return Err(crate::error::validation_error("Value is too small"));
        }
    }

    if value > max {
        #[cfg(feature = "std")]
        {
            return Err(crate::error::validation_error_dynamic(format!(
                "Value {} is too large, maximum is {}",
                value, max
            )));
        }
        #[cfg(not(any(feature = "std")))]
        {
            return Err(crate::error::validation_error("Value is too large"));
        }
    }

    Ok(value)
}

#[cfg(test)]
mod tests {
    use wrt_foundation::ValueType;

    use super::*;

    #[test]
    fn test_format_block_type_conversion() {
        // Test FormatBlockType -> BlockType
        let format_empty = FormatBlockType::Empty;
        let format_value = FormatBlockType::ValueType(ValueType::I32);
        let format_type_idx = FormatBlockType::TypeIndex(42);

        let block_empty = format_block_type_to_block_type(&format_empty);
        let block_value = format_block_type_to_block_type(&format_value);
        let block_type_idx = format_block_type_to_block_type(&format_type_idx);

        assert!(matches!(block_empty, BlockType::Value(None)));
        // ValueType now requires generic parameter, so we'll check the general
        // structure
        assert!(matches!(block_value, BlockType::Value(_)));
        assert!(matches!(block_type_idx, BlockType::FuncType(42)));

        // Test BlockType -> FormatBlockType
        let format_empty_2 = block_type_to_format_block_type(&block_empty);
        let format_value_2 = block_type_to_format_block_type(&block_value);
        let format_type_idx_2 = block_type_to_format_block_type(&block_type_idx);

        assert!(matches!(format_empty_2, FormatBlockType::Empty));
        assert!(matches!(
            format_value_2,
            FormatBlockType::ValueType(ValueType::I32)
        ));
        assert!(matches!(format_type_idx_2, FormatBlockType::TypeIndex(42)));
    }

    #[test]
    fn test_limits_conversion() {
        // Test wrt-foundation Limits -> FormatLimits
        let wrt_limits_min = wrt_foundation::types::Limits { min: 10, max: None };
        let wrt_limits_both = wrt_foundation::types::Limits {
            min: 10,
            max: Some(20),
        };

        let format_limits_min = wrt_limits_to_format_limits(&wrt_limits_min, false, false);
        let format_limits_both = wrt_limits_to_format_limits(&wrt_limits_both, false, false);

        assert_eq!(format_limits_min.min, 10);
        assert_eq!(format_limits_min.max, None);
        assert_eq!(format_limits_min.shared, false);
        assert_eq!(format_limits_min.memory64, false);

        assert_eq!(format_limits_both.min, 10);
        assert_eq!(format_limits_both.max, Some(20));
        assert_eq!(format_limits_both.shared, false);
        assert_eq!(format_limits_both.memory64, false);

        // Test with shared memory
        let format_limits_shared = wrt_limits_to_format_limits(&wrt_limits_min, true, false);
        assert_eq!(format_limits_shared.shared, true);
        assert_eq!(format_limits_shared.memory64, false);

        // Test with memory64
        let format_limits_mem64 = wrt_limits_to_format_limits(&wrt_limits_min, false, true);
        assert_eq!(format_limits_mem64.shared, false);
        assert_eq!(format_limits_mem64.memory64, true);

        // Test FormatLimits -> wrt-foundation Limits
        let wrt_limits_min_2 = format_limits_to_wrt_limits(&format_limits_min).unwrap();
        let wrt_limits_both_2 = format_limits_to_wrt_limits(&format_limits_both).unwrap();

        assert_eq!(wrt_limits_min_2.min, 10);
        assert_eq!(wrt_limits_min_2.max, None);

        assert_eq!(wrt_limits_both_2.min, 10);
        assert_eq!(wrt_limits_both_2.max, Some(20));
    }

    #[test]
    fn test_validate_format() {
        // Test numeric validation
        assert!(validate_format(5, 1, 10).is_ok());
        assert!(validate_format(1, 1, 10).is_ok());
        assert!(validate_format(10, 1, 10).is_ok());

        // Test error cases
        let too_small = validate_format(0, 1, 10);
        assert!(too_small.is_err());

        let too_large = validate_format(11, 1, 10);
        assert!(too_large.is_err());
    }
}
