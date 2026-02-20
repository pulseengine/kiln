//! Unified loading API for WebAssembly modules and components
//!
//! This module provides a single entry point for loading WASM binaries that can
//! return both module and component information efficiently, avoiding redundant
//! parsing operations.

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, collections::BTreeMap as HashMap, string::String, vec::Vec};
#[cfg(feature = "std")]
extern crate alloc;
#[cfg(feature = "std")]
use alloc::{boxed::Box, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::collections::HashMap;

use wrt_error::Result;
use wrt_format::module::Module as WrtModule;

use crate::prelude::*;

/// WebAssembly format type detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmFormat {
    /// Core WebAssembly module
    CoreModule,
    /// WebAssembly Component Model
    Component,
    /// Unknown or invalid format
    Unknown,
}

/// Module information extracted from WASM binary
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    /// Function type signatures
    pub function_types: Vec<String>,
    /// Import requirements
    pub imports: Vec<ImportInfo>,
    /// Export declarations
    pub exports: Vec<ExportInfo>,
    /// Memory requirements
    pub memory_pages: Option<(u32, Option<u32>)>, // (min, max)
    /// Start function index
    pub start_function: Option<u32>,
}

/// Component information extracted from WASM binary
#[derive(Debug, Clone)]
pub struct ComponentInfo {
    /// Component type definitions
    pub component_types: Vec<String>,
    /// Interface imports
    pub interface_imports: Vec<String>,
    /// Interface exports
    pub interface_exports: Vec<String>,
    /// Instance declarations
    pub instances: Vec<String>,
}

/// Import information
#[derive(Debug, Clone)]
pub struct ImportInfo {
    /// Module name
    pub module: String,
    /// Import name
    pub name: String,
    /// Import type
    pub import_type: ImportType,
}

/// Export information
#[derive(Debug, Clone)]
pub struct ExportInfo {
    /// Export name
    pub name: String,
    /// Export type
    pub export_type: ExportType,
    /// Index in respective namespace
    pub index: u32,
}

/// Import type classification
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportType {
    /// Function import
    Function(u32), // type index
    /// Table import
    Table,
    /// Memory import
    Memory,
    /// Global import
    Global,
}

/// Export type classification
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportType {
    /// Function export
    Function,
    /// Table export
    Table,
    /// Memory export
    Memory,
    /// Global export
    Global,
}

/// Unified WASM information structure
#[derive(Debug, Clone)]
pub struct WasmInfo {
    /// Detected format type
    pub format_type: WasmFormat,
    /// Module information (if core module) - Boxed to avoid stack overflow with large modules
    pub module_info: Option<Box<ModuleInfo>>,
    /// Component information (if component)
    pub component_info: Option<ComponentInfo>,
    /// Built-in imports found in the binary
    pub builtin_imports: Vec<String>,
    /// Raw binary size for memory planning
    pub binary_size: usize,
}

impl WasmInfo {
    /// Create a new empty WasmInfo
    pub fn new(format_type: WasmFormat, binary_size: usize) -> Self {
        Self {
            format_type,
            module_info: None,
            component_info: None,
            builtin_imports: Vec::new(),
            binary_size,
        }
    }

    /// Check if this is a core module
    pub fn is_core_module(&self) -> bool {
        matches!(self.format_type, WasmFormat::CoreModule)
    }

    /// Check if this is a component
    pub fn is_component(&self) -> bool {
        matches!(self.format_type, WasmFormat::Component)
    }

    /// Get module info, returning error if not a module
    pub fn require_module_info(&self) -> Result<&ModuleInfo> {
        self.module_info
            .as_ref()
            .map(|boxed| boxed.as_ref())  // Dereference the Box
            .ok_or_else(|| Error::validation_type_mismatch("WASM binary is not a core module"))
    }

    /// Get component info, returning error if not a component
    pub fn require_component_info(&self) -> Result<&ComponentInfo> {
        self.component_info
            .as_ref()
            .ok_or_else(|| Error::validation_type_mismatch("WASM binary is not a component"))
    }
}

/// Unified WASM loading function
///
/// This is the main entry point for loading WASM binaries. It automatically
/// detects the format and extracts both module and component information
/// efficiently.
///
/// # Arguments
/// * `binary` - The WASM binary data
///
/// # Returns
/// * `WasmInfo` containing all extracted information
pub fn load_wasm_unified(binary: &[u8]) -> Result<WasmInfo> {
    // Validate basic WASM header
    if binary.len() < 8 {
        return Err(Error::parse_error("Binary too small to be valid WASM"));
    }

    // Check magic number
    if &binary[0..4] != b"\0asm" {
        return Err(Error::parse_error("Invalid WASM magic number"));
    }

    // Check version (1.0 for core modules, different for components)
    let version = u32::from_le_bytes([binary[4], binary[5], binary[6], binary[7]]);
    let format_type = match version {
        1 => WasmFormat::CoreModule,
        _ => {
            // Check for component-specific indicators
            if detect_component_format(binary)? {
                WasmFormat::Component
            } else {
                WasmFormat::Unknown
            }
        },
    };

    let mut info = WasmInfo::new(format_type, binary.len());

    match format_type {
        WasmFormat::CoreModule => {
            // Parse as core module
            let module_info = extract_module_info(binary)?;
            info.module_info = Some(Box::new(module_info)); // Box to avoid stack overflow
            info.builtin_imports = extract_builtin_imports(binary)?;
        },
        WasmFormat::Component => {
            // Parse as component
            info.component_info = Some(extract_component_info(binary)?);
        },
        WasmFormat::Unknown => {
            return Err(Error::runtime_execution_error("Unknown WebAssembly format"));
        },
    }

    Ok(info)
}

/// Detect if binary is a WebAssembly component
fn detect_component_format(binary: &[u8]) -> Result<bool> {
    // Simple heuristic: look for component-specific section IDs
    // Component sections typically start at ID 13 and above
    let mut offset = 8; // Skip header

    while offset < binary.len() {
        if offset + 1 >= binary.len() {
            break;
        }

        let section_id = binary[offset];

        // Component-specific section IDs (13+)
        if section_id >= 13 {
            return Ok(true);
        }

        // Skip this section
        offset += 1;

        // Read section size
        let (section_size, bytes_read) = read_leb128_u32(binary, offset)
            .map_err(|_| Error::parse_error("Failed to read section size"))?;
        offset += bytes_read;
        offset += section_size as usize;
    }

    Ok(false)
}

/// Extract module information from core WASM binary
fn extract_module_info(binary: &[u8]) -> Result<ModuleInfo> {
    let mut info = ModuleInfo {
        function_types: Vec::new(),
        imports: Vec::new(),
        exports: Vec::new(),
        memory_pages: None,
        start_function: None,
    };

    let mut offset = 8; // Skip header

    // Parse sections to extract information
    while offset < binary.len() {
        if offset + 1 >= binary.len() {
            break;
        }

        let section_id = binary[offset];
        offset += 1;

        let (section_size, bytes_read) = read_leb128_u32(binary, offset)
            .map_err(|_| Error::parse_error("Failed to read section size"))?;
        offset += bytes_read;

        let section_end = offset + section_size as usize;
        if section_end > binary.len() {
            return Err(Error::parse_error("Section extends beyond binary"));
        }

        let section_data = &binary[offset..section_end];

        match section_id {
            1 => parse_type_section_info(section_data, &mut info)?,
            2 => parse_import_section_info(section_data, &mut info)?,
            5 => parse_memory_section_info(section_data, &mut info)?,
            7 => parse_export_section_info(section_data, &mut info)?,
            8 => parse_start_section_info(section_data, &mut info)?,
            _ => {}, // Skip other sections for basic info
        }

        offset = section_end;
    }

    Ok(info)
}

/// Parse type section for function signatures
fn parse_type_section_info(data: &[u8], info: &mut ModuleInfo) -> Result<()> {
    let mut offset = 0;
    let (count, bytes_read) = read_leb128_u32(data, offset)?;
    offset += bytes_read;

    for _ in 0..count {
        // For now, just add placeholder type info
        // In a full implementation, we'd parse the actual function types
        #[cfg(feature = "std")]
        info.function_types.push("func".to_string());

        #[cfg(not(feature = "std"))]
        {
            let func_str = DecoderString::try_from_str("func")
                .map_err(|_| Error::runtime_execution_error("Failed to create string"))?;
            // For now, just use a hardcoded string as we can't easily convert BoundedString
            // to String in no_std
            info.function_types.push(alloc::string::String::from("func"));
        }

        // Skip the actual type parsing for now
        if offset < data.len() {
            offset += 1; // Skip type form
        }
    }

    Ok(())
}

/// Parse import section for import information
pub(crate) fn parse_import_section_info(data: &[u8], info: &mut ModuleInfo) -> Result<()> {
    let mut offset = 0;
    let (count, bytes_read) = read_leb128_u32(data, offset)?;
    offset += bytes_read;

    for _ in 0..count {
        // Parse module name
        let (module_len, bytes_read) = read_leb128_u32(data, offset)?;
        offset += bytes_read;

        if offset + module_len as usize > data.len() {
            return Err(Error::parse_error(
                "Import module name extends beyond section",
            ));
        }

        let module_name = core::str::from_utf8(&data[offset..offset + module_len as usize])
            .map_err(|_| Error::parse_error("Invalid UTF-8 in import module name"))?;
        offset += module_len as usize;

        // Parse import name
        let (name_len, bytes_read) = read_leb128_u32(data, offset)?;
        offset += bytes_read;

        if offset + name_len as usize > data.len() {
            return Err(Error::parse_error("Import name extends beyond section"));
        }

        let import_name = core::str::from_utf8(&data[offset..offset + name_len as usize])
            .map_err(|_| Error::parse_error("Invalid UTF-8 in import name"))?;
        offset += name_len as usize;

        // Parse import kind
        if offset >= data.len() {
            return Err(Error::parse_error("Missing import kind"));
        }

        let import_kind = data[offset];
        offset += 1;

        let import_type = match import_kind {
            0 => {
                // Function import - read type index
                let (type_idx, bytes_read) = read_leb128_u32(data, offset)?;
                offset += bytes_read;
                ImportType::Function(type_idx)
            },
            1 => {
                // Table import - skip table type
                offset += 2; // Skip element type and limits flag
                ImportType::Table
            },
            2 => {
                // Memory import - skip memory type
                offset += 1; // Skip limits flag
                ImportType::Memory
            },
            3 => {
                // Global import - skip global type
                offset += 2; // Skip value type and mutability
                ImportType::Global
            },
            _ => {
                return Err(Error::parse_error("Invalid import kind"));
            },
        };

        #[cfg(feature = "std")]
        {
            info.imports.push(ImportInfo {
                module: module_name.to_string(),
                name: import_name.to_string(),
                import_type,
            });
        }
        #[cfg(not(feature = "std"))]
        {
            // Use the original string values directly
            info.imports.push(ImportInfo {
                module: alloc::string::String::from(module_name),
                name: alloc::string::String::from(import_name),
                import_type,
            });
        }
    }

    Ok(())
}

/// Parse memory section for memory requirements
fn parse_memory_section_info(data: &[u8], info: &mut ModuleInfo) -> Result<()> {
    let mut offset = 0;
    let (count, bytes_read) = read_leb128_u32(data, offset)?;
    offset += bytes_read;

    if count > 0 {
        // Read first memory's limits
        let limits_flag = data[offset];
        offset += 1;

        let (min, bytes_read) = read_leb128_u32(data, offset)?;
        offset += bytes_read;

        let max = if limits_flag & 0x01 != 0 {
            let (max_val, _) = read_leb128_u32(data, offset)?;
            Some(max_val)
        } else {
            None
        };

        info.memory_pages = Some((min, max));
    }

    Ok(())
}

/// Parse export section for export information
pub(crate) fn parse_export_section_info(data: &[u8], info: &mut ModuleInfo) -> Result<()> {
    let mut offset = 0;
    let (count, bytes_read) = read_leb128_u32(data, offset)?;
    offset += bytes_read;

    for _ in 0..count {
        // Parse export name
        let (name_len, bytes_read) = read_leb128_u32(data, offset)?;
        offset += bytes_read;

        if offset + name_len as usize > data.len() {
            return Err(Error::parse_error("Export name extends beyond section"));
        }

        let export_name = core::str::from_utf8(&data[offset..offset + name_len as usize])
            .map_err(|_| Error::parse_error("Invalid UTF-8 in export name"))?;
        offset += name_len as usize;

        // Parse export kind
        if offset >= data.len() {
            return Err(Error::parse_error("Missing export kind"));
        }

        let export_kind = data[offset];
        offset += 1;

        // Parse export index
        let (index, bytes_read) = read_leb128_u32(data, offset)?;
        offset += bytes_read;

        let export_type = match export_kind {
            0 => ExportType::Function,
            1 => ExportType::Table,
            2 => ExportType::Memory,
            3 => ExportType::Global,
            _ => {
                return Err(Error::parse_error("Invalid export kind"));
            },
        };

        #[cfg(feature = "std")]
        info.exports.push(ExportInfo {
            name: export_name.to_string(),
            export_type,
            index,
        });

        #[cfg(not(feature = "std"))]
        {
            // Use the original string value directly
            info.exports.push(ExportInfo {
                name: alloc::string::String::from(export_name),
                export_type,
                index,
            });
        }
    }

    Ok(())
}

/// Parse start section for start function
fn parse_start_section_info(data: &[u8], info: &mut ModuleInfo) -> Result<()> {
    let (start_idx, _) = read_leb128_u32(data, 0)?;
    info.start_function = Some(start_idx);
    Ok(())
}

/// Extract component information from component WASM binary
fn extract_component_info(_binary: &[u8]) -> Result<ComponentInfo> {
    // Placeholder implementation for component parsing
    // In a full implementation, this would parse component-specific sections
    Ok(ComponentInfo {
        component_types: Vec::new(),
        interface_imports: Vec::new(),
        interface_exports: Vec::new(),
        instances: Vec::new(),
    })
}

/// Extract built-in imports from WASM binary
fn extract_builtin_imports(binary: &[u8]) -> Result<Vec<String>> {
    let mut builtin_imports = Vec::new();
    let mut offset = 8; // Skip header

    // Find and parse import section
    while offset < binary.len() {
        if offset + 1 >= binary.len() {
            break;
        }

        let section_id = binary[offset];
        offset += 1;

        let (section_size, bytes_read) = read_leb128_u32(binary, offset)
            .map_err(|_| Error::parse_error("Failed to read section size"))?;
        offset += bytes_read;

        let section_end = offset + section_size as usize;

        // Check if this is the import section (ID = 2)
        if section_id == 2 {
            let section_data = &binary[offset..section_end];
            let mut inner_offset = 0;

            let (count, bytes_read) = read_leb128_u32(section_data, inner_offset)?;
            inner_offset += bytes_read;

            for _ in 0..count {
                // Parse module name
                let (module_len, bytes_read) = read_leb128_u32(section_data, inner_offset)?;
                inner_offset += bytes_read;

                if inner_offset + module_len as usize > section_data.len() {
                    break;
                }

                let module_name = core::str::from_utf8(
                    &section_data[inner_offset..inner_offset + module_len as usize],
                )
                .unwrap_or("");
                inner_offset += module_len as usize;

                // Parse import name
                let (name_len, bytes_read) = read_leb128_u32(section_data, inner_offset)?;
                inner_offset += bytes_read;

                if inner_offset + name_len as usize > section_data.len() {
                    break;
                }

                let import_name = core::str::from_utf8(
                    &section_data[inner_offset..inner_offset + name_len as usize],
                )
                .unwrap_or("");
                inner_offset += name_len as usize;

                // Check if this is a wasi_builtin import
                if module_name == "wasi_builtin" {
                    #[cfg(feature = "std")]
                    builtin_imports.push(import_name.to_string());

                    #[cfg(not(feature = "std"))]
                    {
                        // Use the original string value directly
                        builtin_imports.push(alloc::string::String::from(import_name));
                    }
                }

                // Skip import kind and type info
                if inner_offset < section_data.len() {
                    inner_offset += 1; // Skip import kind
                    // Skip additional type-specific data (simplified)
                    if inner_offset < section_data.len() {
                        inner_offset += 1;
                    }
                }
            }
            break; // Found import section, no need to continue
        }

        offset = section_end;
    }

    Ok(builtin_imports)
}

/// Helper function to read LEB128 unsigned 32-bit integer
fn read_leb128_u32(data: &[u8], offset: usize) -> Result<(u32, usize)> {
    let mut result = 0u32;
    let mut shift = 0;
    let mut bytes_read = 0;

    for i in 0..5 {
        // Max 5 bytes for u32
        if offset + i >= data.len() {
            return Err(Error::parse_error("Truncated LEB128 value"));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_core_module() {
        // Simple core module header
        let module = [0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
        let info = load_wasm_unified(&module).unwrap();
        assert_eq!(info.format_type, WasmFormat::CoreModule);
        assert!(info.is_core_module());
        assert!(!info.is_component());
    }

    #[test]
    fn test_invalid_magic() {
        let invalid = [0x00, 0x61, 0x73, 0x6E, 0x01, 0x00, 0x00, 0x00]; // Wrong magic
        assert!(load_wasm_unified(&invalid).is_err());
    }

    #[test]
    fn test_too_small() {
        let too_small = [0x00, 0x61, 0x73]; // Too small
        assert!(load_wasm_unified(&too_small).is_err());
    }

    #[test]
    fn test_empty_module_info() {
        let module = [0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
        let info = load_wasm_unified(&module).unwrap();
        let module_info = info.require_module_info().unwrap();
        assert!(module_info.imports.is_empty());
        assert!(module_info.exports.is_empty());
        assert!(module_info.start_function.is_none());
    }
}
