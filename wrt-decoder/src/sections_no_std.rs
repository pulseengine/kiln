// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Section parsers for WebAssembly binary format (no_std version)
//!
//! This module contains parsers for various sections in WebAssembly modules
//! using bounded collections for static memory allocation.

use wrt_error::{Error, ErrorCategory, Result, codes};
use wrt_format::{
    DataSegment as WrtDataSegment, ElementSegment as WrtElementSegment, WasmString,
    binary::{self},
    module::{ElementInit, Export as WrtExport, ExportKind},
    pure_format_types::{PureDataMode, PureDataSegment, PureElementMode, PureElementSegment},
    types::{RefType, parse_value_type},
};
use wrt_foundation::{
    bounded::{BoundedVec, WasmName},
    budget_aware_provider::CrateId,
    safe_managed_alloc,
    safe_memory::NoStdProvider,
    traits::BoundedCapacity,
    types::{
        FuncType as WrtFuncType, GlobalType as WrtGlobalType, Import as WrtImport,
        ImportDesc as WrtImportDesc, MemoryType as WrtMemoryType, TableType as WrtTableType,
    },
};

// Import bounded infrastructure
use crate::{
    bounded_decoder_infra::*,
    memory_optimized::{check_bounds_u32, safe_usize_conversion},
    optimized_string::parse_utf8_string_inplace,
};
use wrt_foundation::collections::StaticVec;

/// WebAssembly section representation for no_std environments
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Section {
    /// Type section containing function signatures
    Type(BoundedTypeVec<WrtFuncType>),
    /// Import section
    Import(BoundedImportVec<WrtImport<DecoderProvider>>),
    /// Function section (function indices)
    Function(BoundedFunctionVec<u32>),
    /// Table section
    Table(BoundedTableVec<WrtTableType>),
    /// Memory section
    Memory(BoundedMemoryVec<WrtMemoryType>),
    /// Global section
    Global(BoundedGlobalVec<WrtGlobalType>),
    /// Export section
    Export(BoundedExportVec<WrtExport>),
    /// Start section
    Start(u32),
    /// Element section
    Element(BoundedElementVec<WrtElementSegment>),
    /// Code section (function bodies)
    Code(BoundedFunctionVec<BoundedCustomData>),
    /// Data section
    Data(BoundedDataVec<WrtDataSegment>),
    /// Data count section (for memory.init/data.drop validation)
    DataCount(u32),
    /// Custom section
    Custom {
        name: WasmString,
        data: BoundedCustomData,
    },
    /// Empty section (default)
    #[default]
    Empty,
}

// Implement required traits for Section
impl wrt_foundation::traits::Checksummable for Section {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        // Simple checksum based on variant
        let discriminant = match self {
            Section::Type(_) => 0u8,
            Section::Import(_) => 1,
            Section::Function(_) => 2,
            Section::Table(_) => 3,
            Section::Memory(_) => 4,
            Section::Global(_) => 5,
            Section::Export(_) => 6,
            Section::Start(_) => 7,
            Section::Element(_) => 8,
            Section::Code(_) => 9,
            Section::Data(_) => 10,
            Section::DataCount(_) => 11,
            Section::Custom { .. } => 12,
            Section::Empty => 13,
        };
        checksum.update_slice(&[discriminant]);
    }
}

impl wrt_foundation::traits::ToBytes for Section {
    fn serialized_size(&self) -> usize {
        1 // Just discriminant for simplicity
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        let discriminant = match self {
            Section::Type(_) => 0u8,
            Section::Import(_) => 1,
            Section::Function(_) => 2,
            Section::Table(_) => 3,
            Section::Memory(_) => 4,
            Section::Global(_) => 5,
            Section::Export(_) => 6,
            Section::Start(_) => 7,
            Section::Element(_) => 8,
            Section::Code(_) => 9,
            Section::Data(_) => 10,
            Section::DataCount(_) => 11,
            Section::Custom { .. } => 12,
            Section::Empty => 13,
        };
        writer.write_u8(discriminant)
    }
}

impl wrt_foundation::traits::FromBytes for Section {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        let discriminant = reader.read_u8()?;
        match discriminant {
            13 => Ok(Section::Empty),
            _ => Ok(Section::Empty), // Default fallback
        }
    }
}

// Helper function stubs (same as original)
fn parse_element_segment(
    bytes: &[u8],
    offset: usize,
) -> Result<(wrt_format::pure_format_types::PureElementSegment, usize)> {
    // For both std and no_std, implement basic element parsing
    // Create empty vecs using the standard library Vec type that's available in
    // no_std via alloc
    #[cfg(not(feature = "std"))]
    use wrt_foundation::prelude::*;

    let pure_element = PureElementSegment {
        element_type: wrt_format::types::RefType::Funcref,
        mode: PureElementMode::Passive,
        offset_expr_bytes: Default::default(),
        init_data: wrt_format::pure_format_types::PureElementInit::FunctionIndices(
            Default::default(),
        ),
    };
    Ok((pure_element, offset + 1))
}

fn parse_data(
    bytes: &[u8],
    offset: usize,
) -> Result<(wrt_format::pure_format_types::PureDataSegment, usize)> {
    // For both std and no_std, implement basic data parsing
    // Create empty vecs using the standard library Vec type that's available in
    // no_std via alloc
    #[cfg(not(feature = "std"))]
    use wrt_foundation::prelude::*;

    let pure_data = PureDataSegment {
        mode: PureDataMode::Passive,
        offset_expr_bytes: Default::default(),
        data_bytes: Default::default(),
    };
    Ok((pure_data, offset + 1))
}

fn parse_limits(bytes: &[u8], offset: usize) -> Result<(wrt_format::types::Limits, usize)> {
    if offset >= bytes.len() {
        return Err(Error::parse_error("Unexpected end while parsing limits"));
    }

    let flags = bytes[offset];
    let mut new_offset = offset + 1;

    // Check memory64 flag (bit 2)
    let memory64 = flags & 0x04 != 0;

    // Read minimum - memory64 uses u64, regular uses u32
    let (min, min_offset) = if memory64 {
        binary::read_leb128_u64(bytes, new_offset)?
    } else {
        let (val, off) = binary::read_leb128_u32(bytes, new_offset)?;
        (val as u64, off)
    };
    new_offset = min_offset;

    let max = if flags & 0x01 != 0 {
        let (max_val, max_offset) = if memory64 {
            binary::read_leb128_u64(bytes, new_offset)?
        } else {
            let (val, off) = binary::read_leb128_u32(bytes, new_offset)?;
            (val as u64, off)
        };
        new_offset = max_offset;
        Some(max_val)
    } else {
        None
    };

    let shared = flags & 0x02 != 0;

    Ok((
        wrt_format::types::Limits {
            min,
            max,
            shared,
            memory64,
        },
        new_offset,
    ))
}

/// Parsers implementation with bounded collections
pub mod parsers {
    use super::*;

    /// Parse a type section with bounded memory
    pub fn parse_type_section(bytes: &[u8]) -> Result<BoundedTypeVec<WrtFuncType>> {
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;

        check_bounds_u32(count, MAX_TYPES as u32, "type count")?;
        let count_usize = safe_usize_conversion(count, "type count")?;

        let mut format_func_types = new_type_vec();

        for _ in 0..count {
            // Read function type tag (0x60)
            if offset >= bytes.len() {
                return Err(Error::parse_error(
                    "Unexpected end while parsing function type",
                ));
            }

            if bytes[offset] != 0x60 {
                return Err(Error::parse_error("Invalid function type tag"));
            }
            offset += 1;

            // Parse parameters
            let (param_count, param_offset) = binary::read_leb128_u32(bytes, offset)?;
            offset = param_offset;

            check_bounds_u32(param_count, MAX_FUNCTION_PARAMS as u32, "parameter count")?;
            let param_count_usize = safe_usize_conversion(param_count, "parameter count")?;

            let mut params = new_params_vec();

            for _ in 0..param_count {
                if offset >= bytes.len() {
                    return Err(Error::parse_error(
                        "Unexpected end while parsing parameter type",
                    ));
                }

                let val_type = parse_value_type(bytes[offset])?;
                offset += 1;

                params
                    .push(val_type)
                    .map_err(|_| Error::memory_error("Too many parameters in function type"))?;
            }

            // Parse results
            let (result_count, result_offset) = binary::read_leb128_u32(bytes, offset)?;
            offset = result_offset;

            check_bounds_u32(result_count, MAX_FUNCTION_RESULTS as u32, "result count")?;
            let result_count_usize = safe_usize_conversion(result_count, "result count")?;

            let mut results = new_results_vec();

            for _ in 0..result_count {
                if offset >= bytes.len() {
                    return Err(Error::parse_error(
                        "Unexpected end while parsing result type",
                    ));
                }

                let val_type = parse_value_type(bytes[offset])?;
                offset += 1;

                results
                    .push(val_type)
                    .map_err(|_| Error::memory_error("Too many results in function type"))?;
            }

            // Create FuncType (no longer needs provider after StaticVec migration)
            let func_type = WrtFuncType::new(params.iter().copied(), results.iter().copied())?;

            format_func_types
                .push(func_type)
                .map_err(|_| Error::memory_error("Too many function types"))?;
        }

        Ok(format_func_types)
    }

    /// Parse a function section with bounded memory
    pub fn parse_function_section(bytes: &[u8]) -> Result<BoundedFunctionVec<u32>> {
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;

        check_bounds_u32(count, MAX_FUNCTIONS as u32, "function count")?;

        let mut func_indices = new_function_vec();

        for _ in 0..count {
            let (func_idx, new_offset) = binary::read_leb128_u32(bytes, offset)?;
            offset = new_offset;

            func_indices
                .push(func_idx)
                .map_err(|_| Error::memory_error("Too many functions"))?;
        }

        Ok(func_indices)
    }

    /// Parse an import section with bounded memory
    pub fn parse_import_section(
        bytes: &[u8],
    ) -> Result<BoundedImportVec<WrtImport<DecoderProvider>>> {
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;

        check_bounds_u32(count, MAX_IMPORTS as u32, "import count")?;

        let mut format_imports = new_import_vec();

        for _ in 0..count {
            // Parse module name
            let (module_string, new_offset) = parse_utf8_string_inplace(bytes, offset)?;
            offset = new_offset;

            // Parse field name
            let (field_string, new_offset) = parse_utf8_string_inplace(bytes, offset)?;
            offset = new_offset;

            // Parse import descriptor type
            if offset >= bytes.len() {
                return Err(Error::parse_error(
                    "Unexpected end while parsing import descriptor",
                ));
            }

            let desc_type = bytes[offset];
            offset += 1;

            // Parse import descriptor based on type
            let import_desc = match desc_type {
                0x00 => {
                    // Function import
                    let (type_idx, new_offset) = binary::read_leb128_u32(bytes, offset)?;
                    offset = new_offset;
                    WrtImportDesc::Function(type_idx)
                },
                0x01 => {
                    // Table import
                    let (table_type, new_offset) =
                        (wrt_foundation::TableType::default(), offset + 1); // TODO: Implement table type parsing
                    offset = new_offset;
                    // Convert to WrtTableType - needs implementation
                    WrtImportDesc::Table(table_type)
                },
                0x02 => {
                    // Memory import
                    let (mem_limits, new_offset) = parse_limits(bytes, offset)?;
                    offset = new_offset;
                    // Convert to MemoryType
                    let memory_type = WrtMemoryType {
                        limits: wrt_foundation::types::Limits {
                            min: mem_limits.min as u32,
                            max: mem_limits.max.map(|v| v as u32),
                        },
                        shared: mem_limits.shared,
                        memory64: mem_limits.memory64,
                    };
                    WrtImportDesc::Memory(memory_type)
                },
                0x03 => {
                    // Global import
                    let (global_type, new_offset) =
                        (wrt_foundation::GlobalType::default(), offset + 1); // TODO: Implement global type parsing
                    offset = new_offset;
                    // Convert to WrtGlobalType - needs implementation
                    WrtImportDesc::Global(global_type)
                },
                _ => {
                    return Err(Error::parse_error("Invalid import descriptor type"));
                },
            };

            // Convert to WrtImport (use DecoderProvider size for consistency)
            let provider = safe_managed_alloc!(65536, CrateId::Decoder)?;
            let module_str = module_string
                .as_str()
                .map_err(|_| Error::parse_error("Invalid module string"))?;
            let field_str =
                field_string.as_str().map_err(|_| Error::parse_error("Invalid field string"))?;
            let module_name = WasmName::try_from_str(module_str)
                .map_err(|_| Error::memory_error("Module name too long"))?;
            let item_name = WasmName::try_from_str(field_str)
                .map_err(|_| Error::memory_error("Item name too long"))?;

            let wrt_import = WrtImport {
                module_name,
                item_name,
                desc: import_desc,
            };

            format_imports
                .push(wrt_import)
                .map_err(|_| Error::memory_error("Too many imports"))?;
        }

        Ok(format_imports)
    }

    /// Parse a table section with bounded memory
    pub fn parse_table_section(bytes: &[u8]) -> Result<BoundedTableVec<WrtTableType>> {
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;

        check_bounds_u32(count, MAX_TABLE_ENTRIES as u32, "table count")?;

        let mut tables = new_table_vec();

        for _ in 0..count {
            let (table_type, new_offset) = (wrt_foundation::TableType::default(), offset + 1); // TODO: Implement table type parsing
            offset = new_offset;

            // Convert to WrtTableType - needs implementation
            let wrt_table_type = WrtTableType {
                element_type: table_type.element_type,
                limits: table_type.limits,
            };

            tables
                .push(wrt_table_type)
                .map_err(|_| Error::memory_error("Too many tables"))?;
        }

        Ok(tables)
    }

    /// Parse a memory section with bounded memory
    pub fn parse_memory_section(bytes: &[u8]) -> Result<BoundedMemoryVec<WrtMemoryType>> {
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;

        check_bounds_u32(count, MAX_MEMORIES as u32, "memory count")?;

        let mut memories = new_memory_vec();

        for _ in 0..count {
            let (limits, new_offset) = parse_limits(bytes, offset)?;
            offset = new_offset;

            // Convert wrt_format::Limits to wrt_foundation::Limits
            let wrt_limits = wrt_foundation::types::Limits {
                min: limits.min as u32,
                max: limits.max.map(|v| v as u32),
            };
            let memory_type = WrtMemoryType {
                limits: wrt_limits,
                shared: limits.shared,
                memory64: limits.memory64,
            };

            memories
                .push(memory_type)
                .map_err(|_| Error::memory_error("Too many memories"))?;
        }

        Ok(memories)
    }

    /// Parse a global section with bounded memory
    pub fn parse_global_section(bytes: &[u8]) -> Result<BoundedGlobalVec<WrtGlobalType>> {
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;

        check_bounds_u32(count, MAX_GLOBALS as u32, "global count")?;

        let mut globals = new_global_vec();

        for _ in 0..count {
            let (global_type, new_offset) = (wrt_foundation::GlobalType::default(), offset + 1); // TODO: Implement global type parsing
            offset = new_offset;

            // Skip init expression parsing for now
            // In real implementation, we'd parse the init expression here

            // Convert to WrtGlobalType
            let wrt_global_type = WrtGlobalType {
                value_type: global_type.value_type,
                mutable: global_type.mutable,
            };

            globals
                .push(wrt_global_type)
                .map_err(|_| Error::memory_error("Too many globals"))?;
        }

        Ok(globals)
    }

    /// Parse an export section with bounded memory
    pub fn parse_export_section(bytes: &[u8]) -> Result<BoundedExportVec<WrtExport>> {
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;

        check_bounds_u32(count, MAX_EXPORTS as u32, "export count")?;

        let mut exports = new_export_vec();

        for _ in 0..count {
            // Parse export name
            let (name_string, new_offset) = parse_utf8_string_inplace(bytes, offset)?;
            offset = new_offset;

            // Parse export kind
            if offset >= bytes.len() {
                return Err(Error::parse_error(
                    "Unexpected end while parsing export kind",
                ));
            }

            let kind = match bytes[offset] {
                0x00 => ExportKind::Function,
                0x01 => ExportKind::Table,
                0x02 => ExportKind::Memory,
                0x03 => ExportKind::Global,
                _ => {
                    return Err(Error::parse_error("Invalid export kind"));
                },
            };
            offset += 1;

            // Parse export index
            let (index, new_offset) = binary::read_leb128_u32(bytes, offset)?;
            offset = new_offset;

            // Convert string to WasmString with proper provider size
            let string_provider = wrt_foundation::safe_managed_alloc!(
                1024,
                wrt_foundation::budget_aware_provider::CrateId::Decoder
            )?;
            let name_str = name_string
                .as_str()
                .map_err(|_| Error::parse_error("Invalid export name string"))?;
            let export_name = WasmString::try_from_str(name_str)
                .map_err(|_| Error::memory_error("Export name too long"))?;

            let export = WrtExport {
                name: export_name,
                kind,
                index,
            };

            exports.push(export).map_err(|_| Error::memory_error("Too many exports"))?;
        }

        Ok(exports)
    }

    /// Parse an element section with bounded memory
    pub fn parse_element_section(
        bytes: &[u8],
    ) -> Result<BoundedElementVec<wrt_format::pure_format_types::PureElementSegment>> {
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;

        check_bounds_u32(count, MAX_ELEMENTS as u32, "element count")?;

        let mut elements =
            StaticVec::<wrt_format::pure_format_types::PureElementSegment, MAX_ELEMENTS>::new();

        for _ in 0..count {
            let (pure_element, new_offset) = parse_element_segment(bytes, offset)?;
            offset = new_offset;

            elements
                .push(pure_element)
                .map_err(|_| Error::memory_error("Too many element segments"))?;
        }

        Ok(elements)
    }

    /// Parse a code section with bounded memory
    pub fn parse_code_section(bytes: &[u8]) -> Result<BoundedFunctionVec<BoundedCustomData>> {
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;

        check_bounds_u32(count, MAX_FUNCTIONS as u32, "code count")?;

        let mut bodies = new_code_bodies_vec();

        for _ in 0..count {
            // Parse body size
            let (body_size, new_offset) = binary::read_leb128_u32(bytes, offset)?;
            offset = new_offset;

            check_bounds_u32(
                body_size,
                MAX_CUSTOM_SECTION_SIZE as u32,
                "function body size",
            )?;
            let body_size_usize = safe_usize_conversion(body_size, "body size")?;

            // Check bounds
            if offset + body_size_usize > bytes.len() {
                return Err(Error::parse_error(
                    "Function body extends beyond section bounds",
                ));
            }

            // Copy body data
            let mut body = StaticVec::<u8, MAX_CUSTOM_SECTION_SIZE>::new();

            let body_slice = &bytes[offset..offset + body_size_usize];
            for &byte in body_slice {
                body.push(byte).map_err(|_| Error::memory_error("Function body too large"))?;
            }

            offset += body_size_usize;

            bodies.push(body).map_err(|_| Error::memory_error("Too many function bodies"))?;
        }

        Ok(bodies)
    }

    /// Parse a data section with bounded memory
    pub fn parse_data_section(
        bytes: &[u8],
    ) -> Result<BoundedDataVec<wrt_format::pure_format_types::PureDataSegment>> {
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;

        check_bounds_u32(count, MAX_DATA_SEGMENTS as u32, "data count")?;

        let mut data_segments =
            StaticVec::<wrt_format::pure_format_types::PureDataSegment, MAX_DATA_SEGMENTS>::new();

        for _ in 0..count {
            let (pure_data, new_offset) = parse_data(bytes, offset)?;
            offset = new_offset;

            // Use the pure data segment directly (it's already the right type)
            data_segments
                .push(pure_data)
                .map_err(|_| Error::memory_error("Too many data segments"))?;
        }

        Ok(data_segments)
    }
}

// Helper functions removed - using concrete implementations from
// bounded_decoder_infra.rs

// All duplicate helper functions removed - using implementations from
// bounded_decoder_infra.rs

// new_results_vec also removed - using implementation from
// bounded_decoder_infra.rs
