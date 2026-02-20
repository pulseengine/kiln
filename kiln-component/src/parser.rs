//! WebAssembly module parser using wrt-decoder
//!
//! This module provides functionality to parse WebAssembly modules
//! using the project's own wrt-decoder implementation.

#[cfg(not(feature = "std"))]
use alloc::collections::BTreeSet as HashSet;
#[cfg(feature = "std")]
use std::collections::HashSet;

use wrt_error::kinds::DecodingError;
use wrt_foundation::builtin::BuiltinType;

use crate::prelude::*;

/// Scan a WebAssembly module for built-in imports
///
/// This function now uses the unified loader and shared cache for efficient
/// parsing. It leverages cached import data to avoid redundant section parsing.
///
/// # Arguments
///
/// * `binary` - The WebAssembly module binary
///
/// # Returns
///
/// A Result containing a vector of built-in names found in the import section
pub fn scan_for_builtins(binary: &[u8]) -> Result<Vec<String>> {
    #[cfg(feature = "decoder")]
    {
        use wrt_decoder::load_wasm_unified;
        // Try to use unified API with caching first
        match load_wasm_unified(binary) {
            Ok(wasm_info) => {
                // Use the cached builtin imports from unified API
                Ok(wasm_info.builtin_imports)
            },
            Err(_) => {
                // Fall back to manual parsing if unified API fails
                scan_for_builtins_fallback(binary)
            },
        }
    }
    #[cfg(not(feature = "decoder"))]
    {
        // Use fallback when decoder is not available
        scan_for_builtins_fallback(binary)
    }
}

/// Fallback builtin scanning using direct section parsing
///
/// This is used when the unified API fails or for compatibility
fn scan_for_builtins_fallback(binary: &[u8]) -> Result<Vec<String>> {
    use wrt_format::binary;

    // Validate WebAssembly magic number and version
    if binary.len() < 8 {
        return Err(Error::parse_error(
            "Binary too short to be a valid WebAssembly module",
        ));
    }

    // Check magic number
    if &binary[0..4] != b"\0asm" {
        return Err(Error::parse_error("Invalid WebAssembly magic number"));
    }

    let mut builtin_names = Vec::new();
    let mut offset = 8; // Skip magic number and version

    // Parse sections to find the import section
    while offset < binary.len() {
        // Read section ID
        if offset >= binary.len() {
            break;
        }
        let section_id = binary[offset];
        offset += 1;

        // Read section size
        let (section_size, new_offset) = binary::read_leb128_u32(binary, offset)
            .map_err(|e| Error::parse_error("Failed to read section size"))?;
        offset = new_offset;

        let section_end = offset + section_size as usize;
        if section_end > binary.len() {
            break;
        }

        // Check if this is the import section (ID = 2)
        if section_id == 2 {
            // Parse imports manually for builtin detection
            let section_data = &binary[offset..section_end];
            builtin_names = parse_builtins_from_import_section(section_data)?;
            break; // No need to continue after import section
        }

        // Skip to next section
        offset = section_end;
    }

    Ok(builtin_names)
}

/// Parse builtin imports from import section data
fn parse_builtins_from_import_section(data: &[u8]) -> Result<Vec<String>> {
    let mut builtin_names = Vec::new();
    let mut offset = 0;

    // Read import count
    let (count, bytes_read) = read_leb128_u32(data, offset)?;
    offset += bytes_read;

    // Parse each import
    for _ in 0..count {
        // Read module name
        let (module_len, bytes_read) = read_leb128_u32(data, offset)?;
        offset += bytes_read;

        if offset + module_len as usize > data.len() {
            break;
        }

        let module_name =
            core::str::from_utf8(&data[offset..offset + module_len as usize]).unwrap_or("Error");
        offset += module_len as usize;

        // Read import name
        let (name_len, bytes_read) = read_leb128_u32(data, offset)?;
        offset += bytes_read;

        if offset + name_len as usize > data.len() {
            break;
        }

        let import_name =
            core::str::from_utf8(&data[offset..offset + name_len as usize]).unwrap_or("Error");
        offset += name_len as usize;

        // Check if this is a wasi_builtin import
        if module_name == "wasi_builtin" {
            builtin_names.push(import_name.to_string());
        }

        // Skip import kind and type info
        if offset < data.len() {
            offset += 1; // Skip import kind
            // Skip additional type-specific data (simplified)
            if offset < data.len() {
                offset += 1;
            }
        }
    }

    Ok(builtin_names)
}

/// Helper function to read LEB128 unsigned 32-bit integer
fn read_leb128_u32(data: &[u8], offset: usize) -> Result<(u32, usize)> {
    let mut result = 0u32;
    let mut shift = 0;
    let mut bytes_read = 0;

    for i in 0..5 {
        // Max 5 bytes for u32
        if offset + i >= data.len() {
            return Err(Error::parse_error(
                "Unexpected end of data while reading LEB128",
            ));
        }

        let byte = data[offset + i];
        bytes_read += 1;

        result |= ((byte & 0x7F) as u32) << shift;

        if byte & 0x80 == 0 {
            break;
        }

        shift += 7;
        if shift >= 32 {
            return Err(Error::parse_error("LEB128 value too large for u32"));
        }
    }

    Ok((result, bytes_read))
}

/// Scan a WebAssembly binary for built-in imports and map them to built-in
/// types
///
/// # Arguments
///
/// * `binary` - The WebAssembly module binary
///
/// # Returns
///
/// A Result containing a set of required built-in types
pub fn get_required_builtins(binary: &[u8]) -> Result<HashSet<BuiltinType>> {
    let builtin_names = scan_for_builtins(binary)?;
    let mut required_builtins = HashSet::new();

    for name in builtin_names {
        if let Some(builtin_type) = map_import_to_builtin(&name) {
            required_builtins.insert(builtin_type);
        }
    }

    Ok(required_builtins)
}

/// Map an import name to a built-in type
///
/// # Arguments
///
/// * `import_name` - The name of the import function
///
/// # Returns
///
/// An Option containing the corresponding built-in type if recognized
pub fn map_import_to_builtin(import_name: &str) -> Option<BuiltinType> {
    match import_name {
        // Generic resource operations
        "resource.create" => Some(BuiltinType::ResourceCreate),
        "resource.drop" => Some(BuiltinType::ResourceDrop),
        "resource.rep" => Some(BuiltinType::ResourceRep),
        "resource.get" => Some(BuiltinType::ResourceGet),

        // Feature-gated async operations
        #[cfg(feature = "component-model-async")]
        "async.new" => Some(BuiltinType::AsyncNew),
        #[cfg(feature = "component-model-async")]
        "async.get" => Some(BuiltinType::AsyncGet),
        #[cfg(feature = "component-model-async")]
        "async.poll" => Some(BuiltinType::AsyncPoll),
        #[cfg(feature = "component-model-async")]
        "async.wait" => Some(BuiltinType::AsyncWait),

        // Feature-gated error context operations
        #[cfg(feature = "component-model-error-context")]
        "error.new" => Some(BuiltinType::ErrorNew),
        #[cfg(feature = "component-model-error-context")]
        "error.trace" => Some(BuiltinType::ErrorTrace),

        // Feature-gated threading operations
        #[cfg(feature = "component-model-threading")]
        "threading.spawn" => Some(BuiltinType::ThreadingSpawn),
        #[cfg(feature = "component-model-threading")]
        "threading.join" => Some(BuiltinType::ThreadingJoin),
        #[cfg(feature = "component-model-threading")]
        "threading.sync" => Some(BuiltinType::ThreadingSync),

        // Unknown import name (including "random_get_bytes" which is handled separately)
        _ => None,
    }
}
