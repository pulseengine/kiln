// WRT - wrt-decoder
// Module: Optimized String Processing
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Optimized string processing utilities that avoid unnecessary allocations

#[cfg(feature = "std")]
use alloc::string::String;
use core::str;

use wrt_error::{Error, ErrorCategory, Result, codes};
#[cfg(not(feature = "std"))]
use wrt_foundation::BoundedString;

use crate::prelude::read_name;

/// Binary std/no_std choice
#[cfg(feature = "std")]
pub fn parse_utf8_string_inplace(
    bytes: &[u8],
    offset: usize,
) -> Result<(alloc::string::String, usize)> {
    let (name_bytes, new_offset) = read_name(bytes, offset)?;

    // Validate UTF-8 without creating intermediate Vec
    let string_str = str::from_utf8(name_bytes)
        .map_err(|_| Error::parse_error("malformed UTF-8 encoding"))?;

    Ok((alloc::string::String::from(string_str), new_offset))
}

#[cfg(not(feature = "std"))]
pub fn parse_utf8_string_inplace(
    bytes: &[u8],
    offset: usize,
) -> Result<(wrt_foundation::BoundedString<256>, usize)> {
    let (name_bytes, new_offset) = read_name(bytes, offset)?;

    // Validate UTF-8 without creating intermediate Vec
    let string_str = str::from_utf8(name_bytes).map_err(|_| {
        Error::parse_error("malformed UTF-8 encoding")
    })?;

    use wrt_foundation::{CrateId, safe_managed_alloc};
    let provider = safe_managed_alloc!(4096, CrateId::Decoder)?;
    let bounded_string = wrt_foundation::BoundedString::try_from_str(string_str)
        .map_err(|_| Error::runtime_execution_error("Failed to create bounded string"))?;
    Ok((bounded_string, new_offset))
}

/// Binary std/no_std choice
pub fn validate_utf8_name(bytes: &[u8], offset: usize) -> Result<(&str, usize)> {
    let (name_bytes, new_offset) = read_name(bytes, offset)?;

    let string_str = str::from_utf8(name_bytes).map_err(|_| {
        Error::parse_error("malformed UTF-8 encoding")
    })?;

    Ok((string_str, new_offset))
}

/// Copy validated UTF-8 to a bounded buffer in no_std environments
#[cfg(not(any(feature = "std")))]
pub fn copy_utf8_to_bounded(
    bytes: &[u8],
    offset: usize,
    buffer: &mut [u8],
) -> Result<(usize, usize)> {
    let (name_bytes, new_offset) = read_name(bytes, offset)?;

    // Validate UTF-8 first
    str::from_utf8(name_bytes)
        .map_err(|_| Error::parse_error("malformed UTF-8 encoding"))?;

    // Check if it fits in the buffer
    if name_bytes.len() > buffer.len() {
        return Err(Error::new(
            ErrorCategory::Parse,
            codes::BUFFER_TOO_SMALL,
            "Buffer too small for string",
        ));
    }

    // Copy to buffer
    buffer[..name_bytes.len()].copy_from_slice(name_bytes);
    Ok((name_bytes.len(), new_offset))
}
