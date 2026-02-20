//! String Encoding Support for WebAssembly Component Model
//!
//! This module provides encoding and decoding support for various string
//! encodings used in the WebAssembly Component Model canonical ABI.

use crate::prelude::*;

/// Supported string encodings in the Component Model
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringEncoding {
    /// UTF-8 encoding (default)
    Utf8,
    /// UTF-16 Little Endian encoding
    Utf16Le,
    /// UTF-16 Big Endian encoding
    Utf16Be,
    /// Latin-1 (ISO-8859-1) encoding
    Latin1,
}

impl Default for StringEncoding {
    fn default() -> Self {
        Self::Utf8
    }
}

/// Encode a Rust string into the specified encoding
pub fn encode_string(s: &str, encoding: StringEncoding) -> Result<Vec<u8>> {
    match encoding {
        StringEncoding::Utf8 => Ok(s.as_bytes().to_vec()),
        StringEncoding::Utf16Le => encode_utf16_le(s),
        StringEncoding::Utf16Be => encode_utf16_be(s),
        StringEncoding::Latin1 => encode_latin1(s),
    }
}

/// Decode bytes into a Rust string from the specified encoding
pub fn decode_string(bytes: &[u8], encoding: StringEncoding) -> Result<String> {
    match encoding {
        StringEncoding::Utf8 => decode_utf8(bytes),
        StringEncoding::Utf16Le => decode_utf16_le(bytes),
        StringEncoding::Utf16Be => decode_utf16_be(bytes),
        StringEncoding::Latin1 => decode_latin1(bytes),
    }
}

/// Calculate the byte length of a string in the specified encoding
pub fn string_byte_length(s: &str, encoding: StringEncoding) -> usize {
    match encoding {
        StringEncoding::Utf8 => s.len(),
        StringEncoding::Utf16Le | StringEncoding::Utf16Be => s.encode_utf16().count() * 2,
        StringEncoding::Latin1 => {
            // Latin-1 can only encode certain characters as single bytes
            s.chars().filter(|&c| (c as u32) <= 0xFF).count()
        },
    }
}

/// Encode to UTF-16 Little Endian
fn encode_utf16_le(s: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();

    for code_unit in s.encode_utf16() {
        bytes.push((code_unit & 0xFF) as u8);
        bytes.push((code_unit >> 8) as u8);
    }

    Ok(bytes)
}

/// Encode to UTF-16 Big Endian
fn encode_utf16_be(s: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();

    for code_unit in s.encode_utf16() {
        bytes.push((code_unit >> 8) as u8);
        bytes.push((code_unit & 0xFF) as u8);
    }

    Ok(bytes)
}

/// Encode to Latin-1 (ISO-8859-1)
fn encode_latin1(s: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();

    for c in s.chars() {
        let code_point = c as u32;
        if code_point > 0xFF {
            return Err(Error::component_not_found("Error occurred"));
        }
        bytes.push(code_point as u8);
    }

    Ok(bytes)
}

/// Decode from UTF-8
fn decode_utf8(bytes: &[u8]) -> Result<String> {
    core::str::from_utf8(bytes)
        .map(|s| s.to_string())
        .map_err(|e| Error::component_not_found("Error occurred"))
}

/// Decode from UTF-16 Little Endian
fn decode_utf16_le(bytes: &[u8]) -> Result<String> {
    if bytes.len() % 2 != 0 {
        return Err(Error::runtime_execution_error("Error occurred"));
    }

    let mut code_units = Vec::new();
    for chunk in bytes.chunks_exact(2) {
        let code_unit = u16::from_le_bytes([chunk[0], chunk[1]]);
        code_units.push(code_unit);
    }

    String::from_utf16(&code_units)
        .map_err(|e| Error::component_not_found("Unknown string encoding"))
}

/// Decode from UTF-16 Big Endian
fn decode_utf16_be(bytes: &[u8]) -> Result<String> {
    if bytes.len() % 2 != 0 {
        return Err(Error::runtime_execution_error("Error occurred"));
    }

    let mut code_units = Vec::new();
    for chunk in bytes.chunks_exact(2) {
        let code_unit = u16::from_be_bytes([chunk[0], chunk[1]]);
        code_units.push(code_unit);
    }

    String::from_utf16(&code_units)
        .map_err(|e| Error::component_not_found("Unknown string encoding"))
}

/// Decode from Latin-1 (ISO-8859-1)
fn decode_latin1(bytes: &[u8]) -> Result<String> {
    // Latin-1 is a direct mapping from bytes to Unicode code points 0x00-0xFF
    let chars: Vec<char> = bytes.iter().map(|&b| b as char).collect();
    Ok(chars.into_iter().collect())
}

/// String transcoding utilities for the canonical ABI
pub struct StringTranscoder {
    /// Source encoding
    source_encoding: StringEncoding,
    /// Target encoding
    target_encoding: StringEncoding,
}

impl StringTranscoder {
    /// Create a new transcoder
    pub fn new(source: StringEncoding, target: StringEncoding) -> Self {
        Self {
            source_encoding: source,
            target_encoding: target,
        }
    }

    /// Transcode bytes from source encoding to target encoding
    pub fn transcode(&self, input: &[u8]) -> Result<Vec<u8>> {
        if self.source_encoding == self.target_encoding {
            // No transcoding needed
            return Ok(input.to_vec());
        }

        // First decode from source encoding
        let string = decode_string(input, self.source_encoding)?;

        // Then encode to target encoding
        encode_string(&string, self.target_encoding)
    }

    /// Calculate the maximum output size for transcoding
    pub fn max_output_size(&self, input_size: usize) -> usize {
        match (self.source_encoding, self.target_encoding) {
            // UTF-8 to UTF-16: worst case is 4 bytes -> 4 bytes (surrogate pair)
            (StringEncoding::Utf8, StringEncoding::Utf16Le | StringEncoding::Utf16Be) => {
                input_size * 2
            },
            // UTF-16 to UTF-8: worst case is 2 bytes -> 4 bytes
            (StringEncoding::Utf16Le | StringEncoding::Utf16Be, StringEncoding::Utf8) => {
                input_size * 2
            },
            // Latin-1 to UTF-8: worst case is 1 byte -> 2 bytes
            (StringEncoding::Latin1, StringEncoding::Utf8) => input_size * 2,
            // Latin-1 to UTF-16: 1 byte -> 2 bytes
            (StringEncoding::Latin1, StringEncoding::Utf16Le | StringEncoding::Utf16Be) => {
                input_size * 2
            },
            // UTF-8/UTF-16 to Latin-1: may fail, but max is input size
            (
                StringEncoding::Utf8 | StringEncoding::Utf16Le | StringEncoding::Utf16Be,
                StringEncoding::Latin1,
            ) => input_size,
            // Same encoding
            _ => input_size,
        }
    }
}

/// Canonical ABI string options
#[derive(Debug, Clone)]
pub struct CanonicalStringOptions {
    /// String encoding to use
    pub encoding: StringEncoding,
    /// Maximum allowed string length in bytes
    pub max_length: Option<usize>,
    /// Whether to validate string content
    pub validate: bool,
}

impl Default for CanonicalStringOptions {
    fn default() -> Self {
        Self {
            encoding: StringEncoding::Utf8,
            max_length: Some(1024 * 1024), // 1MB default limit
            validate: true,
        }
    }
}

/// Lift a string from memory with the specified options
pub fn lift_string_with_options(
    addr: u32,
    memory: &[u8],
    options: &CanonicalStringOptions,
) -> Result<String> {
    // Check bounds for length prefix
    if addr as usize + 4 > memory.len() {
        return Err(Error::runtime_out_of_bounds("Error occurred"));
    }

    // Read length prefix
    let len_bytes = &memory[addr as usize..addr as usize + 4];
    let length =
        u32::from_le_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]) as usize;

    // Check length limit
    if let Some(max_len) = options.max_length {
        if length > max_len {
            return Err(Error::component_not_found("Error occurred"));
        }
    }

    // Check bounds for string data
    let data_start = addr as usize + 4;
    if data_start + length > memory.len() {
        return Err(Error::runtime_out_of_bounds("Error occurred"));
    }

    // Extract string bytes
    let string_bytes = &memory[data_start..data_start + length];

    // Decode the string
    let string = decode_string(string_bytes, options.encoding)?;

    // Validate if requested
    if options.validate {
        validate_string(&string)?;
    }

    Ok(string)
}

/// Lower a string to memory with the specified options
pub fn lower_string_with_options(
    string: &str,
    addr: u32,
    memory: &mut [u8],
    options: &CanonicalStringOptions,
) -> Result<()> {
    // Validate if requested
    if options.validate {
        validate_string(string)?;
    }

    // Encode the string
    let encoded = encode_string(string, options.encoding)?;

    // Check length limit
    if let Some(max_len) = options.max_length {
        if encoded.len() > max_len {
            return Err(Error::runtime_execution_error("Error occurred"));
        }
    }

    // Check bounds
    let total_size = 4 + encoded.len();
    if addr as usize + total_size > memory.len() {
        return Err(Error::runtime_out_of_bounds(
            "String data exceeds memory bounds",
        ));
    }

    // Write length prefix
    let len_bytes = (encoded.len() as u32).to_le_bytes();
    memory[addr as usize..addr as usize + 4].copy_from_slice(&len_bytes);

    // Write string data
    memory[addr as usize + 4..addr as usize + 4 + encoded.len()].copy_from_slice(&encoded);

    Ok(())
}

/// Validate a string according to Component Model rules
fn validate_string(s: &str) -> Result<()> {
    // Check for isolated surrogates (not allowed in Component Model)
    for ch in s.chars() {
        if (0xD800..=0xDFFF).contains(&(ch as u32)) {
            return Err(Error::runtime_execution_error("Error occurred"));
        }
    }

    Ok(())
}
