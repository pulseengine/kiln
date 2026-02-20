// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

use wrt_format::binary;
#[cfg(feature = "std")]
use wrt_format::component::FormatValType;

use crate::{Error, Result, prelude::*};

/// Add a section to the binary with the given ID and content
#[cfg(feature = "std")]
pub fn add_section(binary: &mut Vec<u8>, section_id: u8, content: &[u8]) {
    binary.push(section_id);
    binary.extend(write_leb128_u32(content.len() as u32));
    binary.extend_from_slice(content);
}

/// Add a section to the binary with the given ID and content (no_std version)
#[cfg(not(feature = "std"))]
pub fn add_section(
    binary: &mut wrt_foundation::BoundedVec<
        u8,
        256,
        wrt_foundation::safe_memory::NoStdProvider<4096>,
    >,
    section_id: u8,
    content: &[u8],
) {
    let _ = binary.try_push(section_id);
    let leb_bytes = write_leb128_u32(content.len() as u32);
    if let Ok(slice) = leb_bytes.as_slice() {
        for &byte in slice {
            let _ = binary.try_push(byte);
        }
    }
    for &byte in content {
        let _ = binary.try_push(byte);
    }
}

/// Check if a given string is a valid semantic version
pub fn is_valid_semver(version: &str) -> bool {
    // Simple semver validation (major.minor.patch)
    // Count dots instead of using collect() to avoid Vec allocation
    let mut part_count = 0;
    let mut last_start = 0;

    for (i, ch) in version.char_indices() {
        if ch == '.' {
            let part = &version[last_start..i];
            if part.parse::<u32>().is_err() {
                return false;
            }
            part_count += 1;
            last_start = i + 1;
        }
    }

    // Check last part
    if last_start < version.len() {
        let part = &version[last_start..];
        if part.parse::<u32>().is_err() {
            return false;
        }
        part_count += 1;
    }

    part_count == 3
}

/// Check if a string represents a valid integrity hash
pub fn is_valid_integrity(integrity: &str) -> bool {
    // Simple integrity validation (algo-VALUE)
    // Find the first dash instead of using split/collect
    if let Some(dash_pos) = integrity.find('-') {
        if dash_pos == 0 || dash_pos == integrity.len() - 1 {
            return false;
        }

        let algorithm = &integrity[..dash_pos];
        matches!(algorithm, "sha256" | "sha384" | "sha512")
    } else {
        false
    }
}

/// Check if the binary is a WebAssembly component
pub fn is_component(bytes: &[u8]) -> Result<bool> {
    if bytes.len() < 8 {
        return Err(Error::parse_error(
            "Binary too short for WebAssembly header",
        ));
    }

    if bytes[0..4] != binary::WASM_MAGIC {
        return Err(Error::parse_error("Invalid WebAssembly magic bytes"));
    }

    // Check for component layer
    Ok(bytes[6] == binary::COMPONENT_LAYER[0] && bytes[7] == binary::COMPONENT_LAYER[1])
}

/// Parse a ValType from binary format
#[cfg(feature = "std")]
pub fn parse_val_type(bytes: &[u8], offset: usize) -> Result<(FormatValType, usize)> {
    if offset >= bytes.len() {
        return Err(Error::parse_error(
            "Unexpected end of binary when parsing ValType",
        ));
    }

    let val_type_byte = bytes[offset];
    let val_type = match val_type_byte {
        0x00 => FormatValType::Bool,
        0x01 => FormatValType::S8,
        0x02 => FormatValType::U8,
        0x03 => FormatValType::S16,
        0x04 => FormatValType::U16,
        0x05 => FormatValType::S32,
        0x06 => FormatValType::U32,
        0x07 => FormatValType::S64,
        0x08 => FormatValType::U64,
        0x09 => FormatValType::F32,
        0x0A => FormatValType::F64,
        0x0B => FormatValType::Char,
        0x0C => FormatValType::String,
        _ => {
            return Err(Error::parse_error("Unknown ValType byte"));
        },
    };

    Ok((val_type, 1))
}

/// Parse a ValType from binary format (no_std stub)
#[cfg(not(feature = "std"))]
pub fn parse_val_type(_bytes: &[u8], _offset: usize) -> Result<(u8, usize)> {
    use wrt_error::{ErrorCategory, codes};
    Err(Error::runtime_execution_error(
        "Value type parsing not implemented in no_std",
    ))
}

pub fn invalid_component_format(_message: &str) -> Error {
    use wrt_error::{ErrorCategory, codes};
    Error::validation_error("Invalid component format")
}

pub fn invalid_component_data(_message: &str) -> Error {
    use wrt_error::{ErrorCategory, codes};
    Error::validation_error("Invalid component data ")
}

pub fn invalid_component_section(_message: &str) -> Error {
    use wrt_error::{ErrorCategory, codes};
    Error::validation_error("Invalid component section ")
}

pub fn invalid_component_value(_message: &str) -> Error {
    use wrt_error::{ErrorCategory, codes};
    Error::validation_error("Invalid component value ")
}

pub fn parse_error(_message: &str) -> Error {
    use wrt_error::{ErrorCategory, codes};
    Error::parse_error("Parse error ")
}

pub fn parse_error_with_context(_message: &str, _context: &str) -> Error {
    use wrt_error::{ErrorCategory, codes};
    Error::parse_error("Parse error with context ")
}

pub fn parse_error_with_position(_message: &str, _position: usize) -> Error {
    use wrt_error::{ErrorCategory, codes};
    Error::parse_error("Position parse failed ")
}
