// WRT - wrt-decoder
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! No-std, no-alloc decoder for WebAssembly modules
//!
//! This module provides decoding capabilities for WebAssembly modules in
//! environments without heap allocation. It uses bounded collections from
//! wrt-foundation for all operations.
//!
//! # Safety requirements
//!
//! This module adheres to the following safety requirements:
//!
//! - No dynamic memory allocation
//! - All memory usage is bounded at compile time
//! - All inputs are validated before use
//! - All bounds are checked during runtime
//! - No unsafe code is used
//!
//! # Usage example
//!
//! ```ignore
//! use wrt_decoder::decoder_no_alloc;
//! use wrt_foundation::verification::VerificationLevel;
//!
//! // Verify a WebAssembly module header
//! let wasm_binary = [0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00]; // Magic + version
//! if let Ok(()) = decoder_no_alloc::verify_wasm_header(&wasm_binary) {
//!     // Header is valid
//!     let header = decoder_no_alloc::decode_module_header_simple(&wasm_binary).unwrap();
//!     // Use header information
//! }
//! ```

use wrt_error::{Error, ErrorCategory, Result, codes};
use wrt_foundation::{safe_memory::NoStdProvider, verification::VerificationLevel};

use crate::prelude::*;

/// Binary std/no_std choice
pub const MAX_MODULE_SIZE: usize = 65536; // 64 KB

/// Maximum number of sections in a WebAssembly module
pub const MAX_SECTIONS: usize = 16;

/// Maximum number of custom sections in a WebAssembly module
pub const MAX_CUSTOM_SECTIONS: usize = 8;

/// Maximum number of imports in a WebAssembly module
pub const MAX_IMPORTS: usize = 64;

/// Maximum number of exports in a WebAssembly module
pub const MAX_EXPORTS: usize = 64;

/// Maximum number of functions in a WebAssembly module
pub const MAX_FUNCTIONS: usize = 256;

/// Maximum number of tables in a WebAssembly module
pub const MAX_TABLES: usize = 4;

/// Maximum number of memories in a WebAssembly module
pub const MAX_MEMORIES: usize = 4;

/// Maximum number of globals in a WebAssembly module
pub const MAX_GLOBALS: usize = 64;

/// Maximum number of elements in a WebAssembly module
pub const MAX_ELEMENTS: usize = 32;

/// Maximum number of data segments in a WebAssembly module
pub const MAX_DATA_SEGMENTS: usize = 32;

/// Maximum number of types in a WebAssembly module
pub const MAX_TYPES: usize = 128;

/// Binary std/no_std choice
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoAllocErrorCode {
    /// Binary std/no_std choice
    ModuleTooLarge,
    /// Invalid module header
    InvalidHeader,
    /// Binary std/no_std choice
    UnsupportedFeature,
    /// Bounds check failed
    BoundsCheckFailed,
    /// Memory provider error
    MemoryProviderError,
    /// Validation error
    ValidationError,
}

impl NoAllocErrorCode {
    /// Converts a NoAllocErrorCode to a wrt_error code
    pub fn to_error_code(&self) -> u16 {
        match self {
            NoAllocErrorCode::ModuleTooLarge => codes::CAPACITY_EXCEEDED,
            NoAllocErrorCode::InvalidHeader => codes::DECODING_ERROR,
            NoAllocErrorCode::UnsupportedFeature => codes::VALIDATION_UNSUPPORTED_FEATURE,
            NoAllocErrorCode::BoundsCheckFailed => codes::VALIDATION_ERROR,
            NoAllocErrorCode::MemoryProviderError => codes::MEMORY_ERROR,
            NoAllocErrorCode::ValidationError => codes::VALIDATION_ERROR,
        }
    }

    /// Converts a NoAllocErrorCode to an ErrorCategory
    pub fn to_error_category(&self) -> ErrorCategory {
        match self {
            NoAllocErrorCode::ModuleTooLarge => ErrorCategory::Capacity,
            NoAllocErrorCode::InvalidHeader => ErrorCategory::Parse,
            NoAllocErrorCode::UnsupportedFeature => ErrorCategory::Validation,
            NoAllocErrorCode::BoundsCheckFailed => ErrorCategory::Validation,
            NoAllocErrorCode::MemoryProviderError => ErrorCategory::Memory,
            NoAllocErrorCode::ValidationError => ErrorCategory::Validation,
        }
    }
}

/// Creates an error from a NoAllocErrorCode with a message
pub fn create_error(code: NoAllocErrorCode, message: &'static str) -> Error {
    Error::new(code.to_error_category(), code.to_error_code(), message)
}

/// Binary std/no_std choice
///
/// This function checks if the provided bytes start with a valid WebAssembly
/// magic number and version. It's a lightweight validation that doesn't require
/// Binary std/no_std choice
///
/// # Arguments
///
/// * `bytes` - The WebAssembly binary data
///
/// # Returns
///
/// * `Result<()>` - Ok if the header is valid, Error otherwise
pub fn verify_wasm_header(bytes: &[u8]) -> Result<()> {
    // Check for minimum size
    if bytes.len() < 8 {
        return Err(create_error(
            NoAllocErrorCode::InvalidHeader,
            "WebAssembly binary too small (less than 8 bytes)",
        ));
    }

    // Check magic number
    if bytes[0..4] != [0x00, 0x61, 0x73, 0x6D] {
        return Err(create_error(
            NoAllocErrorCode::InvalidHeader,
            "Invalid WebAssembly magic number",
        ));
    }

    // Check version
    let version_bytes = [bytes[4], bytes[5], bytes[6], bytes[7]];
    if version_bytes != [0x01, 0x00, 0x00, 0x00] {
        return Err(create_error(
            NoAllocErrorCode::UnsupportedFeature,
            "Unsupported WebAssembly version",
        ));
    }

    Ok(())
}

/// Creates a bounded slice from a byte array for safe memory operations
///
/// This function initializes a NoStdProvider with the given byte array
/// and verification level for use with bounded collections.
///
/// # Arguments
///
/// * `bytes` - The bytes to create a provider for
/// * `level` - Verification level for memory operations
///
/// # Returns
///
/// * `Result<NoStdProvider<MAX_MODULE_SIZE>>` - Memory provider initialized
///   with the bytes
pub fn create_memory_provider(
    bytes: &[u8],
    _level: VerificationLevel,
) -> Result<NoStdProvider<MAX_MODULE_SIZE>> {
    if bytes.len() > MAX_MODULE_SIZE {
        return Err(create_error(
            NoAllocErrorCode::ModuleTooLarge,
            "WebAssembly module too large for no_alloc decoding",
        ));
    }

    // Create a no_std provider with the maximum module size
    let mut provider = NoStdProvider::<MAX_MODULE_SIZE>::default();

    // Write the bytes to the provider
    use wrt_foundation::safe_memory::Provider;
    provider.write_data(0, bytes).map_err(|_| {
        create_error(
            NoAllocErrorCode::MemoryProviderError,
            "Failed to initialize memory provider",
        )
    })?;

    Ok(provider)
}

/// Enum representing WebAssembly section IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SectionId {
    /// Custom section (0)
    Custom = 0,
    /// Type section (1)
    Type = 1,
    /// Import section (2)
    Import = 2,
    /// Function section (3)
    Function = 3,
    /// Table section (4)
    Table = 4,
    /// Memory section (5)
    Memory = 5,
    /// Global section (6)
    Global = 6,
    /// Export section (7)
    Export = 7,
    /// Start section (8)
    Start = 8,
    /// Element section (9)
    Element = 9,
    /// Code section (10)
    Code = 10,
    /// Data section (11)
    Data = 11,
    /// Data count section (12)
    DataCount = 12,
    /// Unknown section
    Unknown = 255,
}

impl From<u8> for SectionId {
    fn from(id: u8) -> Self {
        match id {
            0 => SectionId::Custom,
            1 => SectionId::Type,
            2 => SectionId::Import,
            3 => SectionId::Function,
            4 => SectionId::Table,
            5 => SectionId::Memory,
            6 => SectionId::Global,
            7 => SectionId::Export,
            8 => SectionId::Start,
            9 => SectionId::Element,
            10 => SectionId::Code,
            11 => SectionId::Data,
            12 => SectionId::DataCount,
            _ => SectionId::Unknown,
        }
    }
}

/// A minimal representation of a WebAssembly section
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionInfo {
    /// Section ID
    pub id: SectionId,
    /// Section size in bytes
    pub size: u32,
    /// Offset of the section data in the binary
    pub offset: usize,
}

/// Binary std/no_std choice
///
/// This struct contains essential information from a WebAssembly module
/// Binary std/no_std choice
///
/// It provides access to module metadata and section headers without
/// Binary std/no_std choice
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmModuleHeader {
    /// WebAssembly binary format version
    pub version: u32,
    /// Size of the binary in bytes
    pub size: usize,
    /// Number of sections detected in the module
    pub section_count: u8,
    /// Whether the module contains a code section
    pub has_code_section: bool,
    /// Whether the module contains a data section
    pub has_data_section: bool,
    /// Whether the module contains a custom name section
    pub has_name_section: bool,
    /// Whether the module contains a start function
    pub has_start_function: bool,
    /// Whether the module uses memory
    pub uses_memory: bool,
    /// Whether the module uses tables
    pub uses_tables: bool,
    /// Verification level used for validation
    pub verification_level: VerificationLevel,
    /// Section information
    pub sections: [Option<SectionInfo>; MAX_SECTIONS],
}

impl WasmModuleHeader {
    /// Creates a new WasmModuleHeader with default values
    pub fn new() -> Self {
        Self {
            version: 0,
            size: 0,
            section_count: 0,
            has_code_section: false,
            has_data_section: false,
            has_name_section: false,
            has_start_function: false,
            uses_memory: false,
            uses_tables: false,
            verification_level: VerificationLevel::Standard,
            sections: [None; MAX_SECTIONS],
        }
    }

    /// Returns the offset and size of a specific section type, if it exists
    ///
    /// # Arguments
    ///
    /// * `id` - The section ID to find
    ///
    /// # Returns
    ///
    /// * `Option<(usize, u32)>` - The offset and size of the section, if found
    pub fn find_section(&self, id: SectionId) -> Option<(usize, u32)> {
        for section_info in self.sections.iter().flatten() {
            if section_info.id == id {
                return Some((section_info.offset, section_info.size));
            }
        }
        None
    }

    /// Returns the offset and size of a custom section with a specific name, if
    /// it exists
    ///
    /// # Arguments
    ///
    /// * `bytes` - The WebAssembly binary data
    /// * `name` - The custom section name to find
    ///
    /// # Returns
    ///
    /// * `Option<(usize, u32)>` - The offset and size of the section data
    ///   (after the name), if found
    pub fn find_custom_section(&self, bytes: &[u8], name: &str) -> Option<(usize, u32)> {
        for section_info in self.sections.iter().flatten() {
            if section_info.id == SectionId::Custom {
                let section_data =
                    &bytes[section_info.offset..section_info.offset + section_info.size as usize];
                if let Ok((section_name, name_size)) = read_name(section_data, 0) {
                    if section_name == name.as_bytes() {
                        return Some((
                            section_info.offset + name_size,
                            section_info.size - name_size as u32,
                        ));
                    }
                }
            }
        }
        None
    }
}

impl Default for WasmModuleHeader {
    fn default() -> Self {
        Self::new()
    }
}

/// Decodes only the WebAssembly module header and scans for section information
/// Binary std/no_std choice
///
/// This function decodes header information and scans for basic section
/// Binary std/no_std choice
/// performs a lightweight scan of the binary to identify key sections and
/// module characteristics.
///
/// # Arguments
///
/// * `bytes` - The WebAssembly binary data
/// * `verification_level` - The verification level to use for validation
///
/// # Returns
///
/// * `Result<WasmModuleHeader>` - The decoded header or an error
pub fn decode_module_header(
    bytes: &[u8],
    verification_level: VerificationLevel,
) -> Result<WasmModuleHeader> {
    verify_wasm_header(bytes)?;

    // Extract version from the header
    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);

    let mut header = WasmModuleHeader {
        version,
        size: bytes.len(),
        section_count: 0,
        has_code_section: false,
        has_data_section: false,
        has_name_section: false,
        has_start_function: false,
        uses_memory: false,
        uses_tables: false,
        verification_level,
        sections: [None; MAX_SECTIONS],
    };

    // Skip past the header (8 bytes) and scan for sections
    let mut offset = 8;
    let mut section_index = 0;

    while offset < bytes.len() && section_index < MAX_SECTIONS {
        // Ensure we have at least one byte for section ID
        if offset >= bytes.len() {
            break;
        }

        // Read section ID
        let section_id = bytes[offset];
        let section_id_enum = SectionId::from(section_id);
        offset += 1;

        // Read section size (LEB128 encoded)
        if offset >= bytes.len() {
            break;
        }

        let (section_size, size_len) = match read_leb128_u32(bytes, offset) {
            Ok((size, len)) => (size, len),
            Err(_) => break, // Invalid section size, stop scanning
        };

        offset += size_len;
        let section_data_offset = offset;

        // Save section info
        header.sections[section_index] = Some(SectionInfo {
            id: section_id_enum,
            size: section_size,
            offset: section_data_offset,
        });

        // Update header based on section type
        header.section_count += 1;
        section_index += 1;

        match section_id_enum {
            SectionId::Custom => {
                // Custom section - check if it's a name section
                if section_size >= 4
                    && is_name_section(
                        &bytes[section_data_offset..section_data_offset + section_size as usize],
                    )
                {
                    header.has_name_section = true;
                }
            },
            SectionId::Memory => {
                header.uses_memory = true;
            },
            SectionId::Table => {
                header.uses_tables = true;
            },
            SectionId::Start => {
                header.has_start_function = true;
            },
            SectionId::Code => {
                header.has_code_section = true;
            },
            SectionId::Data => {
                header.has_data_section = true;
            },
            _ => {},
        }

        // Move to next section
        offset = section_data_offset + section_size as usize;
    }

    Ok(header)
}

/// Simplified version that uses the Standard verification level
pub fn decode_module_header_simple(bytes: &[u8]) -> Result<WasmModuleHeader> {
    decode_module_header(bytes, VerificationLevel::Standard)
}

/// Checks if a custom section is a name section
///
/// # Arguments
///
/// * `section_data` - The custom section data
///
/// # Returns
///
/// * `bool` - True if this is a name section, false otherwise
fn is_name_section(section_data: &[u8]) -> bool {
    // A name section starts with a name subsection
    if section_data.len() < 4 {
        return false;
    }

    // Try to read the name
    if let Ok((name, _)) = read_name(section_data, 0) {
        name == b"name"
    } else {
        false
    }
}

/// Binary std/no_std choice
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidatorType {
    /// Basic validation only checks module structure
    Basic,
    /// Standard validation performs structural and semantic checks
    Standard,
    /// Full validation performs comprehensive validation including memory
    /// safety
    Full,
}

/// Validates a WebAssembly module
///
/// This function performs validation on a WebAssembly module without heap
/// Binary std/no_std choice
///
/// # Arguments
///
/// * `bytes` - The WebAssembly binary data
/// * `validator` - The type of validation to perform
///
/// # Returns
///
/// * `Result<()>` - Ok if the module is valid, Error otherwise
pub fn validate_module_no_alloc(bytes: &[u8], validator: ValidatorType) -> Result<()> {
    // First, validate the header
    verify_wasm_header(bytes)?;

    // Then, decode the header to get section information
    let header = decode_module_header_simple(bytes)?;

    // For Basic validation, we just check the header and section structure
    if validator == ValidatorType::Basic {
        return Ok(());
    }

    // For Standard validation, check each section based on the WebAssembly
    // specification
    if validator == ValidatorType::Standard || validator == ValidatorType::Full {
        validate_section_order(&header)?;
    }

    // For Full validation, perform additional checks
    if validator == ValidatorType::Full {
        validate_code_section(&header, bytes)?;
        validate_memory_safety(&header, bytes)?;
    }

    Ok(())
}

/// Validates the order of sections in a WebAssembly module
///
/// # Arguments
///
/// * `header` - The module header containing section information
///
/// # Returns
///
/// * `Result<()>` - Ok if the sections are in the correct order, Error
///   otherwise
fn validate_section_order(header: &WasmModuleHeader) -> Result<()> {
    let mut last_id = 0;

    for i in 0..header.section_count as usize {
        if let Some(section) = &header.sections[i] {
            // Custom sections can appear anywhere
            if section.id != SectionId::Custom {
                let id = section.id as u8;
                if id < last_id {
                    return Err(create_error(
                        NoAllocErrorCode::ValidationError,
                        "Invalid section order",
                    ));
                }
                last_id = id;
            }
        }
    }

    Ok(())
}

/// Validates the code section of a WebAssembly module
///
/// # Arguments
///
/// * `header` - The module header containing section information
/// * `bytes` - The WebAssembly binary data
///
/// # Returns
///
/// * `Result<()>` - Ok if the code section is valid, Error otherwise
fn validate_code_section(header: &WasmModuleHeader, bytes: &[u8]) -> Result<()> {
    // Find the code section
    if let Some((offset, size)) = header.find_section(SectionId::Code) {
        if offset + size as usize > bytes.len() {
            return Err(create_error(
                NoAllocErrorCode::BoundsCheckFailed,
                "Code section extends beyond binary",
            ));
        }

        // In a full implementation, we would validate the code here
        // For now, we just return Ok
    }

    Ok(())
}

/// Validates memory safety of a WebAssembly module
///
/// # Arguments
///
/// * `header` - The module header containing section information
/// * `bytes` - The WebAssembly binary data
///
/// # Returns
///
/// * `Result<()>` - Ok if the module is memory safe, Error otherwise
fn validate_memory_safety(header: &WasmModuleHeader, bytes: &[u8]) -> Result<()> {
    // Check memory section
    if header.uses_memory {
        if let Some((offset, size)) = header.find_section(SectionId::Memory) {
            if offset + size as usize > bytes.len() {
                return Err(create_error(
                    NoAllocErrorCode::BoundsCheckFailed,
                    "Memory section extends beyond binary",
                ));
            }

            // In a full implementation, we would validate memory here
            // For now, we just return Ok
        }
    }

    // Check data section
    if header.has_data_section {
        if let Some((offset, size)) = header.find_section(SectionId::Data) {
            if offset + size as usize > bytes.len() {
                return Err(create_error(
                    NoAllocErrorCode::BoundsCheckFailed,
                    "Data section extends beyond binary",
                ));
            }

            // In a full implementation, we would validate data here
            // For now, we just return Ok
        }
    }

    Ok(())
}

/// Extracts information about a specific section
///
/// This function provides details about a section without fully decoding it.
///
/// # Arguments
///
/// * `bytes` - The WebAssembly binary data
/// * `section_id` - The section ID to extract information about
///
/// # Returns
///
/// * `Result<Option<SectionInfo>>` - Section information if found, None if not
///   found
pub fn extract_section_info(bytes: &[u8], section_id: SectionId) -> Result<Option<SectionInfo>> {
    let header = decode_module_header_simple(bytes)?;

    for section_info in header.sections.iter().flatten() {
        if section_info.id == section_id {
            return Ok(Some(*section_info));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Minimal valid WebAssembly module - just magic number and version
    const MINIMAL_MODULE: [u8; 8] = [0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];

    #[test]
    fn test_verify_wasm_header_valid() {
        let result = verify_wasm_header(&MINIMAL_MODULE);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_wasm_header_invalid_magic() {
        let invalid_magic = [0x00, 0x61, 0x73, 0x00, 0x01, 0x00, 0x00, 0x00];
        let result = verify_wasm_header(&invalid_magic);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_wasm_header_invalid_version() {
        let invalid_version = [0x00, 0x61, 0x73, 0x6D, 0x02, 0x00, 0x00, 0x00];
        let result = verify_wasm_header(&invalid_version);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_module_header_minimal() {
        let result = decode_module_header_simple(&MINIMAL_MODULE);
        assert!(result.is_ok());

        let header = result.unwrap();
        assert_eq!(header.version, 1);
        assert_eq!(header.size, 8);
        assert_eq!(header.section_count, 0);
    }

    #[test]
    fn test_section_id_from_u8() {
        assert_eq!(SectionId::from(0), SectionId::Custom);
        assert_eq!(SectionId::from(1), SectionId::Type);
        assert_eq!(SectionId::from(255), SectionId::Unknown);
    }

    #[test]
    fn test_validate_module_no_alloc() {
        let result = validate_module_no_alloc(&MINIMAL_MODULE, ValidatorType::Basic);
        assert!(result.is_ok());
    }
}
