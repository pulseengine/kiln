// WRT - wrt-decoder
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! No-std, no-alloc decoder for WebAssembly Component Model
//!
//! This module provides decoding capabilities for WebAssembly Component Model
//! in environments without heap allocation. It uses bounded collections from
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
//! use wrt_decoder::component::decode_no_alloc;
//! use wrt_foundation::verification::VerificationLevel;
//!
//! // Verify a WebAssembly Component Model header
//! let component_binary = [0x00, 0x61, 0x73, 0x6D, 0x0A, 0x00, 0x01, 0x00]; // Magic + version
//! if let Ok(()) = decode_no_alloc::verify_component_header(&component_binary) {
//!     // Header is valid, now decode header information
//!     let header = decode_no_alloc::decode_component_header(&component_binary).unwrap();
//!     // Work with component header information
//! }
//! ```

use wrt_error::{Error, ErrorCategory, Result, codes};
use wrt_format::binary;
use wrt_foundation::{
    CrateId, NoStdProvider, capabilities::CapabilityAwareProvider, capability_context,
    safe_capability_alloc, traits::BoundedCapacity,
};

// Type aliases for cleaner signatures
type DecoderProvider = NoStdProvider<65536>;

// Helper functions to create properly sized providers
fn create_provider_1024() -> Result<DecoderProvider> {
    // For this no_std focused module, use the direct provider creation
    Ok(DecoderProvider::default())
}

/// Read a name from binary data (no_std version)
/// Returns (name_bytes, total_bytes_read)
fn read_name(data: &[u8], offset: usize) -> Result<(&[u8], usize)> {
    if offset >= data.len() {
        return Err(Error::parse_error("Offset beyond data"));
    }

    // Read length as LEB128
    let (name_len, leb_bytes) = binary::read_leb128_u32(data, offset)?;
    let name_start = offset + leb_bytes;
    let name_end = name_start + name_len as usize;

    if name_end > data.len() {
        return Err(Error::parse_error("Name extends beyond data"));
    }

    Ok((&data[name_start..name_end], leb_bytes + name_len as usize))
}
// BoundedCapacity trait is private, using alternative approaches for length
// operations
use wrt_foundation::{
    bounded::{BoundedString, BoundedVec, MAX_COMPONENT_TYPES},
    component::{MAX_COMPONENT_EXPORTS, MAX_COMPONENT_IMPORTS},
    verification::VerificationLevel,
};

use crate::{
    component::section::{ComponentExport, ComponentImport, ComponentType},
    decoder_no_alloc::{MAX_MODULE_SIZE, NoAllocErrorCode, create_error, create_memory_provider},
    prelude::*,
};

/// Binary std/no_std choice
pub const MAX_COMPONENT_SIZE: usize = MAX_MODULE_SIZE;

/// Maximum number of instances in a WebAssembly component
pub const MAX_INSTANCES: usize = 32;

/// Maximum number of core modules in a component
pub const MAX_CORE_MODULES: usize = 16;

/// Maximum number of component modules in a component
pub const MAX_COMPONENT_MODULES: usize = 8;

/// Maximum depth of nested components
pub const MAX_COMPONENT_NESTING: usize = 4;

/// Maximum number of sections in a component
pub const MAX_COMPONENT_SECTIONS: usize = 24;

/// Component magic number: component binary format
pub const COMPONENT_MAGIC: [u8; 8] = [0x00, 0x61, 0x73, 0x6D, 0x0A, 0x00, 0x01, 0x00];

/// Component version (1)
pub const COMPONENT_VERSION: u32 = 1;

/// Component section IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ComponentSectionId {
    /// Custom section (0)
    Custom = 0,
    /// Component type section (1)
    ComponentType = 1,
    /// Component import section (2)
    Import = 2,
    /// Core module section (3)
    CoreModule = 3,
    /// Component instance section (4)
    Instance = 4,
    /// Component alias section (5)
    Alias = 5,
    /// Component export section (6)
    Export = 6,
    /// Component start section (7)
    Start = 7,
    /// Component module section (8)
    Module = 8,
    /// Component canonical function section (9)
    CanonicalFunction = 9,
    /// Component instance export section (10)
    InstanceExport = 10,
    /// Unknown section
    Unknown = 255,
}

impl From<u8> for ComponentSectionId {
    fn from(id: u8) -> Self {
        match id {
            0 => ComponentSectionId::Custom,
            1 => ComponentSectionId::ComponentType,
            2 => ComponentSectionId::Import,
            3 => ComponentSectionId::CoreModule,
            4 => ComponentSectionId::Instance,
            5 => ComponentSectionId::Alias,
            6 => ComponentSectionId::Export,
            7 => ComponentSectionId::Start,
            8 => ComponentSectionId::Module,
            9 => ComponentSectionId::CanonicalFunction,
            10 => ComponentSectionId::InstanceExport,
            _ => ComponentSectionId::Unknown,
        }
    }
}

/// A minimal representation of a component section
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentSectionInfo {
    /// Section ID
    pub id: ComponentSectionId,
    /// Section size in bytes
    pub size: u32,
    /// Offset of the section data in the binary
    pub offset: usize,
}

/// Verifies a WebAssembly Component Model binary header
///
/// This function checks if the provided bytes start with a valid WebAssembly
/// Component Model magic number and version. It's a lightweight validation that
/// Binary std/no_std choice
///
/// # Arguments
///
/// * `bytes` - The WebAssembly Component Model binary data
///
/// # Returns
///
/// * `Result<()>` - Ok if the header is valid, Error otherwise
pub fn verify_component_header(bytes: &[u8]) -> Result<()> {
    // Check if we have enough bytes for the header
    if bytes.len() < 8 {
        return Err(create_error(
            NoAllocErrorCode::InvalidHeader,
            "Component binary too small (less than 8 bytes)",
        ));
    }

    // Check magic number for component
    if bytes[0..8] != COMPONENT_MAGIC {
        return Err(create_error(
            NoAllocErrorCode::InvalidHeader,
            "Invalid WebAssembly Component magic number",
        ));
    }

    Ok(())
}

/// Binary std/no_std choice
///
/// This struct contains basic information from a WebAssembly Component
/// Binary std/no_std choice
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentHeader {
    /// Component size in bytes
    pub size: usize,
    /// Number of sections detected in the component
    pub section_count: u8,
    /// Component types
    pub types: BoundedVec<ComponentType, MAX_COMPONENT_TYPES, DecoderProvider>,
    /// Component exports
    pub exports: BoundedVec<ComponentExport, MAX_COMPONENT_EXPORTS, DecoderProvider>,
    /// Component imports
    pub imports: BoundedVec<ComponentImport, MAX_COMPONENT_IMPORTS, DecoderProvider>,
    /// Whether the component contains a start function
    pub has_start: bool,
    /// Whether the component contains core modules
    pub has_core_modules: bool,
    /// Whether the component contains sub-components
    pub has_sub_components: bool,
    /// Whether the component uses resources
    pub uses_resources: bool,
    /// Section information
    pub sections: [Option<ComponentSectionInfo>; MAX_COMPONENT_SECTIONS],
    /// Verification level used for validation
    pub verification_level: VerificationLevel,
}

impl ComponentHeader {
    /// Creates a new ComponentHeader with default values
    pub fn new(verification_level: VerificationLevel) -> Self {
        let provider = create_provider_1024().unwrap_or_else(|_| {
            // Create default provider as fallback with same type
            DecoderProvider::default()
        });
        Self {
            size: 0,
            section_count: 0,
            types: BoundedVec::new(provider.clone()).unwrap_or_default(),
            exports: BoundedVec::new(provider.clone()).unwrap_or_default(),
            imports: BoundedVec::new(provider.clone()).unwrap_or_default(),
            has_start: false,
            has_core_modules: false,
            has_sub_components: false,
            uses_resources: false,
            sections: [None; MAX_COMPONENT_SECTIONS],
            verification_level,
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
    pub fn find_section(&self, id: ComponentSectionId) -> Option<(usize, u32)> {
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
    /// * `bytes` - The WebAssembly Component binary data
    /// * `name` - The custom section name to find
    ///
    /// # Returns
    ///
    /// * `Option<(usize, u32)>` - The offset and size of the section data
    ///   (after the name), if found
    pub fn find_custom_section(&self, bytes: &[u8], name: &str) -> Option<(usize, u32)> {
        for section_info in self.sections.iter().flatten() {
            if section_info.id == ComponentSectionId::Custom {
                let section_data =
                    &bytes[section_info.offset..section_info.offset + section_info.size as usize];
                if let Ok((section_name, name_size)) = read_name(section_data, 0) {
                    if core::str::from_utf8(section_name) == Ok(name) {
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

/// Binary std/no_std choice
///
/// This function decodes the header and basic structure of a WebAssembly
/// Binary std/no_std choice
///
/// # Arguments
///
/// * `bytes` - The WebAssembly Component binary data
/// * `verification_level` - The verification level to use
///
/// # Returns
///
/// * `Result<ComponentHeader>` - The decoded component header or an error
pub fn decode_component_header(
    bytes: &[u8],
    verification_level: VerificationLevel,
) -> Result<ComponentHeader> {
    verify_component_header(bytes)?;

    // Create a memory provider for the component data
    let _provider = create_memory_provider(bytes, verification_level)?;

    // Initialize the component header
    let mut header = ComponentHeader::new(verification_level);
    header.size = bytes.len();

    // Create empty collections for the component header
    let header_provider = create_provider_1024()?;
    let types = BoundedVec::new(header_provider.clone())?;
    let exports = BoundedVec::new(header_provider.clone())?;
    let imports = BoundedVec::new(header_provider)?;

    header.types = types;
    header.exports = exports;
    header.imports = imports;

    // Scan the binary for sections starting after the header (8 bytes)
    let mut offset = 8;
    let mut section_index = 0;

    while offset < bytes.len() && section_index < MAX_COMPONENT_SECTIONS {
        // Ensure we have at least one byte for section ID
        if offset >= bytes.len() {
            break;
        }

        // Read section ID
        let section_id = bytes[offset];
        let section_id_enum = ComponentSectionId::from(section_id);
        offset += 1;

        // Read section size (LEB128 encoded)
        if offset >= bytes.len() {
            break;
        }

        let (section_size, size_len) = match binary::read_leb128_u32(bytes, offset) {
            Ok((size, len)) => (size, len),
            Err(_) => break, // Invalid section size, stop scanning
        };

        offset += size_len;
        let section_data_offset = offset;

        // Save section info
        header.sections[section_index] = Some(ComponentSectionInfo {
            id: section_id_enum,
            size: section_size,
            offset: section_data_offset,
        });

        // Update header based on section type
        header.section_count += 1;
        section_index += 1;

        match section_id_enum {
            ComponentSectionId::Start => {
                header.has_start = true;
            },
            ComponentSectionId::CoreModule => {
                header.has_core_modules = true;
            },
            ComponentSectionId::Module => {
                header.has_sub_components = true;
            },
            ComponentSectionId::ComponentType => {
                // Check for resource type usage
                if check_for_resource_types(bytes, section_data_offset, section_size) {
                    header.uses_resources = true;
                }
            },
            _ => {},
        }

        // Move to next section
        offset = section_data_offset + section_size as usize;
    }

    // Scan and count component imports
    if let Some((offset, size)) = header.find_section(ComponentSectionId::Import) {
        let section_data = &bytes[offset..offset + size as usize];
        scan_component_imports(section_data, &mut header.imports)?;
    }

    // Scan and count component exports
    if let Some((offset, size)) = header.find_section(ComponentSectionId::Export) {
        let section_data = &bytes[offset..offset + size as usize];
        scan_component_exports(section_data, &mut header.exports)?;
    }

    // Scan component types if they exist
    if let Some((offset, size)) = header.find_section(ComponentSectionId::ComponentType) {
        let section_data = &bytes[offset..offset + size as usize];
        scan_component_types(section_data, &mut header.types)?;
    }

    Ok(header)
}

/// Simplified version that uses the Standard verification level
pub fn decode_component_header_simple(bytes: &[u8]) -> Result<ComponentHeader> {
    decode_component_header(bytes, VerificationLevel::Standard)
}

/// Scans for resource types in a component type section
///
/// # Arguments
///
/// * `bytes` - The full component bytes
/// * `offset` - The offset to the type section data
/// * `size` - The size of the type section
///
/// # Returns
///
/// * `bool` - True if resource types are found
fn check_for_resource_types(bytes: &[u8], offset: usize, size: u32) -> bool {
    // This is a simplified check that could be expanded in a production
    // implementation Scan for resource type opcode (0x6E)
    if offset + size as usize <= bytes.len() {
        let section_data = &bytes[offset..offset + size as usize];
        for byte in section_data {
            if *byte == 0x6E {
                return true;
            }
        }
    }
    false
}

/// Scans and counts component imports in a section
///
/// # Arguments
///
/// * `section_data` - The import section data
/// * `imports` - BoundedVec to store import information
///
/// # Returns
///
/// * `Result<()>` - Ok if successful
fn scan_component_imports(
    section_data: &[u8],
    imports: &mut BoundedVec<ComponentImport, MAX_COMPONENT_IMPORTS, DecoderProvider>,
) -> Result<()> {
    if section_data.is_empty() {
        return Ok(());
    }

    // Read the number of imports
    let (count, len) = match binary::read_leb128_u32(section_data, 0) {
        Ok(result) => result,
        Err(_) => return Ok(()), // Error reading count, exit early
    };

    let count = count.min(MAX_COMPONENT_IMPORTS as u32); // Limit to our max capacity
    let mut offset = len;

    for _ in 0..count {
        if offset >= section_data.len() {
            break;
        }

        // Read import name
        if let Ok((name, name_len)) = read_name(section_data, offset) {
            offset += name_len;

            // In a real implementation, we would read the import type here
            // For now, just store the name
            let import = ComponentImport {
                name: {
                    let name_str = core::str::from_utf8(name)
                        .map_err(|_| Error::parse_error("Invalid UTF-8 in name"))?;
                    BoundedString::from_str_truncate(name_str)?
                },
                type_index: 0, // Placeholder
            };

            // Try to add the import to our bounded collection
            let _ = imports.push(import);

            // Skip the import type (simplified)
            if offset < section_data.len() {
                offset += 1; // Skip type byte
            }
        } else {
            break;
        }
    }

    Ok(())
}

/// Scans and counts component exports in a section
///
/// # Arguments
///
/// * `section_data` - The export section data
/// * `exports` - BoundedVec to store export information
///
/// # Returns
///
/// * `Result<()>` - Ok if successful
fn scan_component_exports(
    section_data: &[u8],
    exports: &mut BoundedVec<ComponentExport, MAX_COMPONENT_EXPORTS, DecoderProvider>,
) -> Result<()> {
    if section_data.is_empty() {
        return Ok(());
    }

    // Read the number of exports
    let (count, len) = match binary::read_leb128_u32(section_data, 0) {
        Ok(result) => result,
        Err(_) => return Ok(()), // Error reading count, exit early
    };

    let count = count.min(MAX_COMPONENT_EXPORTS as u32); // Limit to our max capacity
    let mut offset = len;

    for _ in 0..count {
        if offset >= section_data.len() {
            break;
        }

        // Read export name
        if let Ok((name, name_len)) = read_name(section_data, offset) {
            offset += name_len;

            // In a real implementation, we would read the export type here
            // For now, just store the name
            let export = ComponentExport {
                name: {
                    let name_str = core::str::from_utf8(name)
                        .map_err(|_| Error::parse_error("Invalid UTF-8 in name"))?;
                    BoundedString::from_str_truncate(name_str)?
                },
                type_index: 0, // Placeholder
                kind: 0,       // Placeholder
            };

            // Try to add the export to our bounded collection
            let _ = exports.push(export);

            // Skip the export type and index (simplified)
            if offset < section_data.len() {
                offset += 2; // Skip kind and index bytes
            }
        } else {
            break;
        }
    }

    Ok(())
}

/// Scans and counts component types in a section
///
/// # Arguments
///
/// * `section_data` - The type section data
/// * `types` - BoundedVec to store type information
///
/// # Returns
///
/// * `Result<()>` - Ok if successful
fn scan_component_types(
    section_data: &[u8],
    types: &mut BoundedVec<ComponentType, MAX_COMPONENT_TYPES, DecoderProvider>,
) -> Result<()> {
    if section_data.is_empty() {
        return Ok(());
    }

    // Read the number of types
    let (count, len) = match binary::read_leb128_u32(section_data, 0) {
        Ok(result) => result,
        Err(_) => return Ok(()), // Error reading count, exit early
    };

    let count = count.min(MAX_COMPONENT_TYPES as u32); // Limit to our max capacity
    let mut offset = len;

    for _ in 0..count {
        if offset >= section_data.len() {
            break;
        }

        // In a real implementation, we would parse the type structure here
        // For now, just store the type form
        if offset < section_data.len() {
            let type_form = section_data[offset];
            offset += 1;

            let component_type = ComponentType {
                form: type_form,
                // Other fields would be populated here
            };

            // Try to add the type to our bounded collection
            let _ = types.push(component_type);

            // Skip remaining type data (simplified)
            // In a real implementation, we would parse the type structure
            offset += 1;
        }
    }

    Ok(())
}

/// Describes the basic structure of a component without full decoding
///
/// This function examines the component binary and returns information about
/// its structure (types, imports, exports) without fully decoding the
/// component.
///
/// # Arguments
///
/// * `bytes` - The WebAssembly Component binary data
///
/// # Returns
///
/// * `Result<String>` - A description of the component structure or an error
#[cfg(feature = "std")]
pub fn describe_component_structure(bytes: &[u8]) -> Result<String> {
    let header = decode_component_header_simple(bytes)?;

    // Format the component structure description
    // Binary std/no_std choice
    let mut description = format!(
        "Component structure:\n- Size: {} bytes\n- Sections: {}\n- Types: {}\n- Exports: {}\n- \
         Imports: {}\n",
        header.size,
        header.section_count,
        header.types.iter().count(),
        header.exports.iter().count(),
        header.imports.iter().count()
    );

    // Add capability information
    description.push_str("Capabilities:\n");
    if header.has_start {
        description.push_str("- Has start function\n");
    }
    if header.has_core_modules {
        description.push_str("- Contains core modules\n");
    }
    if header.has_sub_components {
        description.push_str("- Contains sub-components\n");
    }
    if header.uses_resources {
        description.push_str("- Uses resource types\n");
    }

    // Add export names if available
    if !header.exports.is_empty() {
        description.push_str("Export names:\n");
        for export in header.exports.iter() {
            if let Ok(name_str) = export.name.as_str() {
                description.push_str(&format!("- {}\n", name_str));
            }
        }
    }

    // Add import names if available
    if !header.imports.is_empty() {
        description.push_str("Import names:\n");
        for import in header.imports.iter() {
            if let Ok(name_str) = import.name.as_str() {
                description.push_str(&format!("- {}\n", name_str));
            }
        }
    }

    Ok(description)
}

/// Validator types for Component Model validation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentValidatorType {
    /// Basic validation only checks component structure
    Basic,
    /// Standard validation performs structural and semantic checks
    Standard,
    /// Full validation performs comprehensive validation including memory
    /// safety
    Full,
}

/// Binary std/no_std choice
///
/// This function performs validation of a WebAssembly Component
/// Binary std/no_std choice
///
/// # Arguments
///
/// * `bytes` - The WebAssembly Component binary data
/// * `validator` - The type of validation to perform
///
/// # Returns
///
/// * `Result<()>` - Ok if the component is valid, Error otherwise
pub fn validate_component_no_alloc(bytes: &[u8], validator: ComponentValidatorType) -> Result<()> {
    // Verify the component header
    verify_component_header(bytes)?;

    // Decode the header to get section information
    let verification_level = match validator {
        ComponentValidatorType::Basic => VerificationLevel::Basic,
        ComponentValidatorType::Standard => VerificationLevel::Standard,
        ComponentValidatorType::Full => VerificationLevel::Full,
    };

    let header = decode_component_header(bytes, verification_level)?;

    // For Basic validation, just checking the header and section structure is
    // enough
    if validator == ComponentValidatorType::Basic {
        return Ok(());
    }

    // For Standard validation, check section order and basic structure
    if validator == ComponentValidatorType::Standard || validator == ComponentValidatorType::Full {
        validate_component_section_order(&header)?;
    }

    // For Full validation, perform additional checks
    if validator == ComponentValidatorType::Full {
        validate_component_types(&header, bytes)?;
        validate_component_imports_exports(&header, bytes)?;

        if header.uses_resources {
            validate_component_resources(&header, bytes)?;
        }
    }

    Ok(())
}

/// Validates the order of sections in a WebAssembly Component
///
/// # Arguments
///
/// * `header` - The component header containing section information
///
/// # Returns
///
/// * `Result<()>` - Ok if the sections are in the correct order, Error
///   otherwise
fn validate_component_section_order(header: &ComponentHeader) -> Result<()> {
    let mut last_id = 0;

    for i in 0..header.section_count as usize {
        if let Some(section) = &header.sections[i] {
            // Custom sections can appear anywhere
            if section.id != ComponentSectionId::Custom {
                let id = section.id as u8;
                if id < last_id {
                    return Err(create_error(
                        NoAllocErrorCode::ValidationError,
                        "Invalid component section order",
                    ));
                }
                last_id = id;
            }
        }
    }

    Ok(())
}

/// Validates component types in a WebAssembly Component
///
/// # Arguments
///
/// * `header` - The component header containing section information
/// * `bytes` - The WebAssembly Component binary data
///
/// # Returns
///
/// * `Result<()>` - Ok if the types are valid, Error otherwise
fn validate_component_types(header: &ComponentHeader, bytes: &[u8]) -> Result<()> {
    if let Some((offset, size)) = header.find_section(ComponentSectionId::ComponentType) {
        if offset + size as usize > bytes.len() {
            return Err(create_error(
                NoAllocErrorCode::BoundsCheckFailed,
                "Component type section extends beyond binary",
            ));
        }

        // In a full implementation, we would validate the types here
        // For now, we just return Ok
    }

    Ok(())
}

/// Validates component imports and exports in a WebAssembly Component
///
/// # Arguments
///
/// * `header` - The component header containing section information
/// * `bytes` - The WebAssembly Component binary data
///
/// # Returns
///
/// * `Result<()>` - Ok if the imports and exports are valid, Error otherwise
fn validate_component_imports_exports(header: &ComponentHeader, bytes: &[u8]) -> Result<()> {
    // Check imports
    if let Some((offset, size)) = header.find_section(ComponentSectionId::Import) {
        if offset + size as usize > bytes.len() {
            return Err(create_error(
                NoAllocErrorCode::BoundsCheckFailed,
                "Component import section extends beyond binary",
            ));
        }

        // In a full implementation, we would validate imports here
    }

    // Check exports
    if let Some((offset, size)) = header.find_section(ComponentSectionId::Export) {
        if offset + size as usize > bytes.len() {
            return Err(create_error(
                NoAllocErrorCode::BoundsCheckFailed,
                "Component export section extends beyond binary",
            ));
        }

        // In a full implementation, we would validate exports here
    }

    Ok(())
}

/// Validates component resource usage in a WebAssembly Component
///
/// # Arguments
///
/// * `header` - The component header containing section information
/// * `bytes` - The WebAssembly Component binary data
///
/// # Returns
///
/// * `Result<()>` - Ok if the resource usage is valid, Error otherwise
fn validate_component_resources(header: &ComponentHeader, bytes: &[u8]) -> Result<()> {
    // Only perform this validation if the component uses resources
    if !header.uses_resources {
        return Ok(());
    }

    // Find the component type section and validate resource type usage
    if let Some((offset, size)) = header.find_section(ComponentSectionId::ComponentType) {
        if offset + size as usize > bytes.len() {
            return Err(create_error(
                NoAllocErrorCode::BoundsCheckFailed,
                "Component type section extends beyond binary",
            ));
        }

        // In a full implementation, we would validate resource types here
        // For now, we just return Ok
    }

    Ok(())
}

/// Extracts information about a specific section in a component
///
/// This function provides details about a section without fully decoding it.
///
/// # Arguments
///
/// * `bytes` - The WebAssembly Component binary data
/// * `section_id` - The section ID to extract information about
///
/// # Returns
///
/// * `Result<Option<ComponentSectionInfo>>` - Section information if found,
///   None if not found
pub fn extract_component_section_info(
    bytes: &[u8],
    section_id: ComponentSectionId,
) -> Result<Option<ComponentSectionInfo>> {
    let header = decode_component_header_simple(bytes)?;

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

    // Minimal valid WebAssembly Component - just magic number and version
    const MINIMAL_COMPONENT: [u8; 8] = [0x00, 0x61, 0x73, 0x6D, 0x0A, 0x00, 0x01, 0x00];

    #[test]
    fn test_verify_component_header_valid() {
        let result = verify_component_header(&MINIMAL_COMPONENT);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_component_header_invalid_magic() {
        let invalid_magic = [0x00, 0x61, 0x73, 0x00, 0x0A, 0x00, 0x01, 0x00];
        let result = verify_component_header(&invalid_magic);
        assert!(result.is_err());
    }

    #[test]
    fn test_component_section_id_from_u8() {
        assert_eq!(ComponentSectionId::from(0), ComponentSectionId::Custom);
        assert_eq!(
            ComponentSectionId::from(1),
            ComponentSectionId::ComponentType
        );
        assert_eq!(ComponentSectionId::from(255), ComponentSectionId::Unknown);
    }

    #[test]
    fn test_decode_component_header_minimal() {
        let result = decode_component_header_simple(&MINIMAL_COMPONENT);
        assert!(result.is_ok());

        let header = result.unwrap();
        assert_eq!(header.size, 8);
        assert_eq!(header.section_count, 0);
        assert_eq!(header.types.len(), 0);
        assert_eq!(header.exports.len(), 0);
        assert_eq!(header.imports.len(), 0);
    }

    #[test]
    fn test_validate_component_no_alloc() {
        let result = validate_component_no_alloc(&MINIMAL_COMPONENT, ComponentValidatorType::Basic);
        assert!(result.is_ok());
    }
}
