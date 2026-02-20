//! Compression utilities for WebAssembly state serialization.
//!
//! This module provides compression algorithms for WebAssembly state data,
//! focusing on run-length encoding (RLE) which is efficient for memory
//! sections.

#[cfg(not(feature = "std"))]
use core::cmp;
#[cfg(feature = "std")]
use std::cmp;

#[cfg(feature = "std")]
use wrt_error::{Error, ErrorCategory, Result, codes};
#[cfg(not(any(feature = "std")))]
use wrt_error::{Error, ErrorCategory, Result, codes};
#[cfg(not(any(feature = "std")))]
use wrt_foundation::MemoryProvider;

#[cfg(feature = "std")]
use crate::Vec;
#[cfg(not(any(feature = "std")))]
use crate::WasmVec;

/// Supported compression types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionType {
    /// No compression
    None = 0,
    /// Run-length encoding
    RLE = 1,
}

impl CompressionType {
    /// Convert a u8 to a CompressionType
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::None),
            1 => Some(Self::RLE),
            _ => None,
        }
    }
}

/// Run-length encode a byte array
///
/// This implementation uses a simple format:
/// - For runs of 4+ identical bytes: [0x00, count, value]
/// - For literal sequences: [count, byte1, byte2, ...]
///
/// Where count is a single byte (0-255)
#[cfg(feature = "std")]
pub fn rle_encode(data: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    let mut i = 0;

    while i < data.len() {
        // Current byte to check for runs
        let current = data[i];

        // Find run length (max 255 for single-byte count)
        let mut run_length = 1;
        while i + run_length < data.len() && data[i + run_length] == current && run_length < 255 {
            run_length += 1;
        }

        if run_length >= 4 {
            // Encode as RLE: [0x00, count, value]
            result.push(0x00); // RLE marker
            result.push(run_length as u8);
            result.push(current);
            i += run_length;
        } else {
            // For runs < 4 bytes, use literal encoding
            // [count, byte1, byte2, ...]
            let literal_length = cmp::min(255, data.len() - i);
            result.push(literal_length as u8);
            for j in 0..literal_length {
                result.push(data[i + j]);
            }
            i += literal_length;
        }
    }

    result
}

/// Run-length decode a byte array
///
/// This function decodes data created by rle_encode.
/// Format:
/// - [0x00, count, value] for runs of repeated bytes
/// - [count, byte1, byte2, ...] for literal sequences
#[cfg(feature = "std")]
pub fn rle_decode(input: &[u8]) -> Result<Vec<u8>> {
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let mut result = Vec::new();
    let mut i = 0;

    while i < input.len() {
        if i >= input.len() {
            return Err(Error::validation_parse_error("Truncated RLE data"));
        }

        let control = input[i];
        i += 1;

        if control == 0x00 {
            // RLE sequence: [0x00, count, value]
            if i + 1 >= input.len() {
                return Err(Error::validation_parse_error("Truncated RLE sequence"));
            }
            let count = input[i] as usize;
            i += 1;
            let value = input[i];
            i += 1;

            for _ in 0..count {
                result.push(value);
            }
        } else {
            // Literal sequence: [count, byte1, byte2, ...]
            let count = control as usize;
            if i + count > input.len() {
                return Err(Error::validation_parse_error("Truncated literal sequence"));
            }

            result.extend_from_slice(&input[i..i + count]);
            i += count;
        }
    }

    Ok(result)
}

/// Run-length encode a byte array (no_std version)
///
/// This implementation uses a simple format:
/// - For runs of 4+ identical bytes: [0x00, count, value]
/// - For literal sequences: [count, byte1, byte2, ...]
///
/// Where count is a single byte (0-255)
#[cfg(not(any(feature = "std")))]
pub fn rle_encode<P: MemoryProvider + Clone + Default + Eq>(data: &[u8]) -> Result<WasmVec<u8, P>> {
    let mut result = WasmVec::new(P::default())
        .map_err(|_| Error::memory_error("Failed to create result vector"))?;
    let mut i = 0;

    while i < data.len() {
        // Current byte to check for runs
        let current = data[i];

        // Find run length (max 255 for single-byte count)
        let mut run_length = 1;
        while i + run_length < data.len() && data[i + run_length] == current && run_length < 255 {
            run_length += 1;
        }

        if run_length >= 4 {
            // Encode as RLE: [0x00, count, value]
            result.push(0x00).map_err(|_| Error::memory_error("Buffer overflow"))?;
            result
                .push(run_length as u8)
                .map_err(|_| Error::memory_error("Buffer overflow"))?;
            result.push(current).map_err(|_| Error::memory_error("Buffer overflow"))?;
            i += run_length;
        } else {
            // For runs < 4 bytes, use literal encoding
            // [count, byte1, byte2, ...]
            let literal_length = cmp::min(255, data.len() - i);
            result
                .push(literal_length as u8)
                .map_err(|_| Error::memory_error("Buffer overflow"))?;
            for j in 0..literal_length {
                result.push(data[i + j]).map_err(|_| Error::memory_error("Buffer overflow"))?;
            }
            i += literal_length;
        }
    }

    Ok(result)
}

/// Run-length decode a byte array (no_std version)
///
/// This function decodes data created by rle_encode.
/// Format:
/// - [0x00, count, value] for runs of repeated bytes
/// - [count, byte1, byte2, ...] for literal sequences
#[cfg(not(any(feature = "std")))]
pub fn rle_decode<P: MemoryProvider + Clone + Default + Eq>(
    input: &[u8],
) -> Result<WasmVec<u8, P>> {
    if input.is_empty() {
        return WasmVec::new(P::default())
            .map_err(|_| Error::memory_error("Failed to create result vector"));
    }

    let mut result = WasmVec::new(P::default())
        .map_err(|_| Error::memory_error("Failed to create result vector"))?;
    let mut i = 0;

    while i < input.len() {
        if i >= input.len() {
            return Err(Error::validation_parse_error("Truncated RLE data"));
        }

        let control = input[i];
        i += 1;

        if control == 0x00 {
            // RLE sequence: [0x00, count, value]
            if i + 1 >= input.len() {
                return Err(Error::validation_parse_error("Truncated RLE sequence"));
            }
            let count = input[i] as usize;
            i += 1;
            let value = input[i];
            i += 1;

            for _ in 0..count {
                result.push(value).map_err(|_| Error::memory_error("Buffer overflow"))?;
            }
        } else {
            // Literal sequence: [count, byte1, byte2, ...]
            let count = control as usize;
            if i + count > input.len() {
                return Err(Error::validation_parse_error("Truncated literal sequence"));
            }

            for j in 0..count {
                result.push(input[i + j]).map_err(|_| Error::memory_error("Buffer overflow"))?;
            }
            i += count;
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "std"))]
    extern crate alloc;
    #[cfg(not(feature = "std"))]
    use alloc::vec;
    #[cfg(feature = "std")]
    use std::vec;

    use super::*;

    #[test]
    #[cfg(feature = "std")]
    fn test_rle_encode_decode() {
        let empty: Vec<u8> = vec![];
        assert_eq!(rle_encode(&empty), empty);
        assert_eq!(rle_decode(&empty).unwrap(), empty);

        let single = vec![42];
        let encoded = rle_encode(&single);
        // The implementation encodes single values as a literal sequence [length,
        // value] where length is 1 for a single byte
        assert_eq!(encoded, vec![1, 42]);
        assert_eq!(rle_decode(&encoded).unwrap(), single);

        let repeated = vec![5, 5, 5, 5, 5];
        let encoded = rle_encode(&repeated);
        // For 5 repeating elements (runs >= 4), it would encode as [0, 5, 5]
        // where 0 is the marker, 5 is the count, and 5 is the value
        assert_eq!(encoded, vec![0, 5, 5]);
        assert_eq!(rle_decode(&encoded).unwrap(), repeated);

        let mixed = vec![1, 1, 2, 3, 3, 3, 3, 4, 5, 5];
        let encoded = rle_encode(&mixed);
        // This would encode as:
        // [2, 1, 1]  - Literal sequence of two bytes (1, 1)
        // [1, 2]     - Literal sequence of one byte (2)
        // [0, 4, 3]  - Run of four 3's
        // [3, 4, 5, 5] - Literal sequence of three bytes (4, 5, 5)
        assert_eq!(rle_decode(&encoded).unwrap(), mixed);
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_rle_decode_errors() {
        // Test truncated input
        let truncated = vec![0]; // RLE marker without count and value
        assert!(rle_decode(&truncated).is_err());

        // Test truncated RLE sequence
        let truncated_rle = vec![0, 5]; // Missing value after count
        assert!(rle_decode(&truncated_rle).is_err());

        // Test truncated literal sequence
        let truncated_literal = vec![5, 1, 2]; // Expecting 5 bytes but only have 2
        assert!(rle_decode(&truncated_literal).is_err());

        // Test for a zero-length RLE sequence with zero count
        let zero_count = vec![0, 0, 42]; // RLE sequence: [0x00, count=0, value=42]
        let result = rle_decode(&zero_count).unwrap();
        assert_eq!(result, vec![]); // Should decode to empty array since count
        // is 0
    }
}
