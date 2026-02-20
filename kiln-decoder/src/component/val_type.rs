// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! WebAssembly Component Model value type encoding utilities
//!
//! This module provides helpers for encoding component value types.

// Component value type encoding is only available with std feature
#[cfg(feature = "std")]
mod component_val_type {
    use wrt_error::{Error, ErrorCategory, Result, codes};
    use wrt_format::component::FormatValType;

    use crate::prelude::*;

    /// Helper function to encode a value type to binary format
    pub fn encode_val_type(result: &mut Vec<u8>, val_type: &FormatValType) -> Result<()> {
        match val_type {
            FormatValType::Bool => result.push(0x07),
            FormatValType::S8 => result.push(0x08),
            FormatValType::U8 => result.push(0x09),
            FormatValType::S16 => result.push(0x0A),
            FormatValType::U16 => result.push(0x0B),
            FormatValType::String => result.push(0x0C),
            FormatValType::List(inner) => {
                result.push(0x0D);
                encode_val_type(result, inner)?;
            },
            FormatValType::S32 => result.push(0x01),
            FormatValType::U32 => result.push(0x02),
            FormatValType::S64 => result.push(0x03),
            FormatValType::U64 => result.push(0x04),
            FormatValType::F32 => result.push(0x05),
            FormatValType::F64 => result.push(0x06),
            FormatValType::Record(fields) => {
                result.push(0x0E);
                result.extend_from_slice(&write_leb128_u32(fields.len() as u32));
                for (name, field_type) in fields {
                    result.extend_from_slice(&write_string(name));
                    encode_val_type(result, field_type)?;
                }
            },
            FormatValType::Variant(cases) => {
                result.push(0x0F);
                result.extend_from_slice(&write_leb128_u32(cases.len() as u32));
                for (case_name, case_type) in cases {
                    result.extend_from_slice(&write_string(case_name));
                    if let Some(ty) = case_type {
                        result.push(0x01); // has type
                        encode_val_type(result, ty)?;
                    } else {
                        result.push(0x00); // no type
                    }
                }
            },
            FormatValType::Tuple(types) => {
                result.push(0x10);
                result.extend_from_slice(&write_leb128_u32(types.len() as u32));
                for ty in types {
                    encode_val_type(result, ty)?;
                }
            },
            FormatValType::Option(inner) => {
                result.push(0x11);
                encode_val_type(result, inner)?;
            },
            // Handle Result type - assuming it's a tuple with optional ok and err values
            FormatValType::Result(inner) => {
                // For now, assume it's an ok-only type by default
                result.push(0x12);
                result.push(0x01); // ok only
                encode_val_type(result, inner)?;
            },
            FormatValType::Enum(cases) => {
                result.push(0x13);
                result.extend_from_slice(&write_leb128_u32(cases.len() as u32));
                for case_name in cases {
                    result.extend_from_slice(&write_string(case_name));
                }
            },
            FormatValType::Flags(names) => {
                result.push(0x14);
                result.extend_from_slice(&write_leb128_u32(names.len() as u32));
                for name in names {
                    result.extend_from_slice(&write_string(name));
                }
            },
            FormatValType::Ref(idx) => {
                result.push(0x15);
                result.extend_from_slice(&write_leb128_u32(*idx));
            },
            FormatValType::Own(_) | FormatValType::Borrow(_) => {
                return Err(Error::parse_error(
                    "Resource types are not supported for encoding yet",
                ));
            },
            FormatValType::Char => result.push(0x16),
            FormatValType::FixedList(inner, size) => {
                // Fixed-length lists are encoded as a list tag followed by the element type and
                // size
                result.push(0x17); // Example tag for fixed list
                encode_val_type(result, inner)?;

                // Encode size
                result.extend_from_slice(&write_leb128_u32(*size));
            },
            FormatValType::ErrorContext => {
                // Error context is a simple type
                result.push(0x18); // Example tag for error context
            },
            FormatValType::Void => {
                // Void is a simple type
                result.push(0x19); // Example tag for void
            },
            // Add a catch-all for any new variants that might be added in the future
            _ => {
                return Err(Error::parse_error("Unsupported value type for encoding"));
            },
        }
        Ok(())
    }
} // end of component_val_type module

// Re-export public APIs when std feature is enabled
#[cfg(feature = "std")]
pub use component_val_type::encode_val_type;

// No-std stub implementations
#[cfg(not(feature = "std"))]
pub mod no_std_stubs {
    use wrt_error::{Error, ErrorCategory, Result, codes};

    /// Stub value type for no_std encoding
    #[derive(Debug, Clone)]
    pub struct FormatValType;

    /// Encode value type (no_std stub)  
    pub fn encode_val_type(
        _result: &mut wrt_foundation::BoundedVec<u8, 1024, wrt_foundation::NoStdProvider<2048>>,
        _val_type: &FormatValType,
    ) -> Result<()> {
        Err(Error::runtime_execution_error(
            "No-std encoding not supported",
        ))
    }
}

#[cfg(not(feature = "std"))]
pub use no_std_stubs::*;
