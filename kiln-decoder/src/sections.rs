// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Section parsers for WebAssembly binary format
//!
//! This module contains parsers for various sections in WebAssembly modules.

use wrt_error::{Error, ErrorCategory, ErrorSource, Result, codes};
use wrt_format::{
    binary::{self},
    types::ValueType as FormatValueType,
};
// Note: These functions should be available if they're exported by wrt_format
// If not, we'll need to implement alternatives or define them locally
use wrt_foundation::NoStdProvider;
use wrt_foundation::types::{
    FuncType, GlobalType as WrtGlobalType, Import as WrtImport, MemoryType as WrtMemoryType,
    TableType as WrtTableType,
};

// Type aliases with result types for error propagation
type WrtFuncType = FuncType;
type WrtFoundationImport = WrtImport<wrt_foundation::NoStdProvider<65536>>;

// Import segment types from wrt-format
use wrt_format::{
    DataSegment as WrtDataSegment, ElementSegment as WrtElementSegment, module::Export as WrtExport,
};

use crate::{
    memory_optimized::{check_bounds_u32, safe_usize_conversion},
    optimized_string::parse_utf8_string_inplace,
};
// Type aliases to make Vec/String usage explicit
#[cfg(feature = "std")]
type SectionVec<T> = alloc::vec::Vec<T>;
#[cfg(not(feature = "std"))]
type SectionVec<T> = wrt_foundation::BoundedVec<T, 256, wrt_foundation::NoStdProvider<4096>>;

#[cfg(feature = "std")]
type SectionString = alloc::string::String;
#[cfg(not(feature = "std"))]
type SectionString = wrt_foundation::BoundedString<256>;

/// WebAssembly section representation
#[derive(Debug, Clone)]
pub enum Section {
    /// Type section containing function signatures
    Type(SectionVec<WrtFuncType>),
    /// Import section
    Import(SectionVec<WrtFoundationImport>),
    /// Function section (function indices)
    Function(SectionVec<u32>),
    /// Table section
    Table(SectionVec<WrtTableType>),
    /// Memory section
    Memory(SectionVec<WrtMemoryType>),
    /// Global section
    Global(SectionVec<WrtGlobalType>),
    /// Export section
    Export(SectionVec<WrtExport>),
    /// Start section (function index)
    Start(u32),
    /// Element section
    Element(SectionVec<WrtElementSegment>),
    /// Code section (function bodies)
    Code(
        SectionVec<
            wrt_foundation::bounded::BoundedVec<u8, 65536, wrt_foundation::NoStdProvider<65536>>,
        >,
    ),
    /// Data section
    Data(SectionVec<WrtDataSegment>),
    /// Data count section
    DataCount(u32),
    /// Custom section
    Custom {
        /// Section name
        name: SectionString,
        /// Section data
        data: wrt_foundation::bounded::BoundedVec<u8, 65536, wrt_foundation::NoStdProvider<65536>>,
    },
}

// Helper functions for missing imports
fn parse_element_segment(
    bytes: &[u8],
    offset: usize,
) -> Result<(wrt_format::pure_format_types::PureElementSegment, usize)> {
    // For both std and no_std, implement basic element parsing
    // This is a simplified version that creates passive elements
    let pure_element = wrt_format::pure_format_types::PureElementSegment {
        element_type: wrt_format::types::RefType::Funcref,
        mode: wrt_format::pure_format_types::PureElementMode::Passive,
        offset_expr_bytes: Vec::new(),
        init_data: wrt_format::pure_format_types::PureElementInit::FunctionIndices(Vec::new()),
    };
    Ok((pure_element, offset + 1))
}

fn parse_data(
    bytes: &[u8],
    offset: usize,
) -> Result<(wrt_format::pure_format_types::PureDataSegment, usize)> {
    use wrt_format::pure_format_types::{PureDataMode, PureDataSegment};

    if offset >= bytes.len() {
        return Err(Error::parse_error(
            "Unexpected end while parsing data segment",
        ));
    }

    let tag = bytes[offset];
    let mut current_offset = offset + 1;

    match tag {
        // Active data segment with implicit memory 0
        0x00 => {
            // Parse offset expression - find the end (0x0B terminator)
            let expr_start = current_offset;
            let mut depth = 1u32; // Track block depth for nested blocks
            while current_offset < bytes.len() {
                let opcode = bytes[current_offset];
                current_offset += 1;

                match opcode {
                    0x02..=0x04 => depth += 1, // block, loop, if
                    0x0B => {
                        // end
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            break;
                        }
                    },
                    0x41 => {
                        // i32.const
                        // Skip LEB128 i32
                        while current_offset < bytes.len() && bytes[current_offset] & 0x80 != 0 {
                            current_offset += 1;
                        }
                        if current_offset < bytes.len() {
                            current_offset += 1; // Final byte
                        }
                    },
                    0x42 => {
                        // i64.const
                        // Skip LEB128 i64
                        while current_offset < bytes.len() && bytes[current_offset] & 0x80 != 0 {
                            current_offset += 1;
                        }
                        if current_offset < bytes.len() {
                            current_offset += 1; // Final byte
                        }
                    },
                    0x23 => {
                        // global.get
                        // Skip LEB128 global index
                        while current_offset < bytes.len() && bytes[current_offset] & 0x80 != 0 {
                            current_offset += 1;
                        }
                        if current_offset < bytes.len() {
                            current_offset += 1;
                        }
                    },
                    _ => {}, // Other opcodes we skip
                }
            }
            let offset_expr_bytes = bytes[expr_start..current_offset].to_vec();

            // Parse data byte count and data
            let (data_len, new_offset) = binary::read_leb128_u32(bytes, current_offset)?;
            current_offset = new_offset;

            if current_offset + data_len as usize > bytes.len() {
                return Err(Error::parse_error("Data segment data exceeds bounds"));
            }

            let data_bytes = bytes[current_offset..current_offset + data_len as usize].to_vec();
            current_offset += data_len as usize;

            Ok((
                PureDataSegment {
                    mode: PureDataMode::Active {
                        memory_index: 0,
                        offset_expr_len: offset_expr_bytes.len() as u32,
                    },
                    offset_expr_bytes,
                    data_bytes,
                },
                current_offset,
            ))
        },
        // Passive data segment
        0x01 => {
            // Parse data byte count and data
            let (data_len, new_offset) = binary::read_leb128_u32(bytes, current_offset)?;
            current_offset = new_offset;

            if current_offset + data_len as usize > bytes.len() {
                return Err(Error::parse_error("Data segment data exceeds bounds"));
            }

            let data_bytes = bytes[current_offset..current_offset + data_len as usize].to_vec();
            current_offset += data_len as usize;

            Ok((
                PureDataSegment {
                    mode: PureDataMode::Passive,
                    offset_expr_bytes: Vec::new(),
                    data_bytes,
                },
                current_offset,
            ))
        },
        // Active data segment with explicit memory index
        0x02 => {
            // Parse memory index
            let (memory_index, new_offset) = binary::read_leb128_u32(bytes, current_offset)?;
            current_offset = new_offset;

            // Parse offset expression - find the end (0x0B terminator)
            let expr_start = current_offset;
            let mut depth = 1u32;
            while current_offset < bytes.len() {
                let opcode = bytes[current_offset];
                current_offset += 1;

                match opcode {
                    0x02..=0x04 => depth += 1,
                    0x0B => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            break;
                        }
                    },
                    0x41 => {
                        while current_offset < bytes.len() && bytes[current_offset] & 0x80 != 0 {
                            current_offset += 1;
                        }
                        if current_offset < bytes.len() {
                            current_offset += 1;
                        }
                    },
                    0x42 => {
                        while current_offset < bytes.len() && bytes[current_offset] & 0x80 != 0 {
                            current_offset += 1;
                        }
                        if current_offset < bytes.len() {
                            current_offset += 1;
                        }
                    },
                    0x23 => {
                        while current_offset < bytes.len() && bytes[current_offset] & 0x80 != 0 {
                            current_offset += 1;
                        }
                        if current_offset < bytes.len() {
                            current_offset += 1;
                        }
                    },
                    _ => {},
                }
            }
            let offset_expr_bytes = bytes[expr_start..current_offset].to_vec();

            // Parse data byte count and data
            let (data_len, new_offset) = binary::read_leb128_u32(bytes, current_offset)?;
            current_offset = new_offset;

            if current_offset + data_len as usize > bytes.len() {
                return Err(Error::parse_error("Data segment data exceeds bounds"));
            }

            let data_bytes = bytes[current_offset..current_offset + data_len as usize].to_vec();
            current_offset += data_len as usize;

            Ok((
                PureDataSegment {
                    mode: PureDataMode::Active {
                        memory_index,
                        offset_expr_len: offset_expr_bytes.len() as u32,
                    },
                    offset_expr_bytes,
                    data_bytes,
                },
                current_offset,
            ))
        },
        _ => Err(Error::parse_error("Invalid data segment tag")),
    }
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
        let (val, off) = binary::read_leb128_u64(bytes, new_offset)?;
        (val, off)
    } else {
        let (val, off) = binary::read_leb128_u32(bytes, new_offset)?;
        (val as u64, off)
    };
    new_offset = min_offset;

    // Check if maximum is present (flag bit 0)
    let max = if flags & 0x01 != 0 {
        let (max_val, max_offset) = if memory64 {
            let (val, off) = binary::read_leb128_u64(bytes, new_offset)?;
            (val, off)
        } else {
            let (val, off) = binary::read_leb128_u32(bytes, new_offset)?;
            (val as u64, off)
        };
        new_offset = max_offset;
        Some(max_val)
    } else {
        None
    };

    // Check shared flag (flag bit 1)
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

/// Parsers implementation
pub mod parsers {
    use super::*;

    /// Parse a type section with memory optimization
    pub fn parse_type_section(bytes: &[u8]) -> Result<Vec<WrtFuncType>> {
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;

        // Binary std/no_std choice
        check_bounds_u32(count, 10000, "type count")?;
        let count_usize = safe_usize_conversion(count, "type count")?;

        let mut format_func_types = Vec::with_capacity(count_usize.min(1024));

        for _ in 0..count {
            // Function type indicator (0x60)
            if offset >= bytes.len() || bytes[offset] != 0x60 {
                return Err(Error::parse_error(
                    "Expected function type indicator (0x60)",
                ));
            }
            offset += 1;

            // Parse param types
            let (param_count, new_offset) = binary::read_leb128_u32(bytes, offset)?;
            offset = new_offset;

            // Bounds check param count
            check_bounds_u32(param_count, 1000, "param count")?;
            let param_count_usize = safe_usize_conversion(param_count, "param count")?;

            let mut params = Vec::with_capacity(param_count_usize.min(256));
            for _ in 0..param_count {
                if offset >= bytes.len() {
                    return Err(Error::parse_error("Unexpected end of param types"));
                }

                let val_type_byte = bytes[offset];
                let format_val_type = wrt_format::conversion::parse_value_type(val_type_byte)
                    .map_err(|_e: wrt_error::Error| {
                        Error::runtime_execution_error("Parse error")
                    })?;
                params.push(format_val_type);
                offset += 1;
            }

            // Parse result types
            let (result_count, new_offset) = binary::read_leb128_u32(bytes, offset)?;
            offset = new_offset;

            // Bounds check result count
            check_bounds_u32(result_count, 1000, "result count")?;
            let result_count_usize = safe_usize_conversion(result_count, "result count")?;

            let mut results = Vec::with_capacity(result_count_usize.min(256));
            for _ in 0..result_count {
                if offset >= bytes.len() {
                    return Err(Error::parse_error("Unexpected end of result types"));
                }

                let val_type_byte = bytes[offset];
                let format_val_type = wrt_format::conversion::parse_value_type(val_type_byte)
                    .map_err(|_e: wrt_error::Error| {
                        Error::runtime_execution_error("Parse error")
                    })?;
                results.push(format_val_type);
                offset += 1;
            }

            let provider = wrt_foundation::safe_managed_alloc!(
                65536,
                wrt_foundation::budget_aware_provider::CrateId::Decoder
            )?;
            format_func_types.push(wrt_format::types::FuncType::new(params, results)?);
        }

        // Since wrt_format::types::FuncType is re-exported from wrt_foundation,
        // we can return the vector directly
        Ok(format_func_types)
    }

    /// Parse a function section
    pub fn parse_function_section(bytes: &[u8]) -> Result<Vec<u32>> {
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;
        let mut indices = Vec::with_capacity(count as usize);

        for _ in 0..count {
            let (index, new_offset) = binary::read_leb128_u32(bytes, offset)?;
            offset = new_offset;
            indices.push(index);
        }

        Ok(indices)
    }

    /// Parse an import section with memory optimization
    pub fn parse_import_section(bytes: &[u8]) -> Result<Vec<WrtFoundationImport>> {
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;

        // Binary std/no_std choice
        check_bounds_u32(count, 10000, "import count")?;
        let count_usize = safe_usize_conversion(count, "import count")?;

        let mut format_imports = Vec::with_capacity(count_usize.min(1024));

        for _ in 0..count {
            // Parse module name using optimized string processing
            let (module_string, new_offset) = parse_utf8_string_inplace(bytes, offset)?;
            offset = new_offset;

            // Parse field name using optimized string processing
            let (field_string, new_offset) = parse_utf8_string_inplace(bytes, offset)?;
            offset = new_offset;

            if offset >= bytes.len() {
                return Err(Error::parse_error("Unexpected end of import description"));
            }

            // Parse import description kind byte
            let kind_byte = bytes[offset];
            offset += 1;

            // Parse into wrt_format::module::ImportDesc first
            let format_desc = match kind_byte {
                0x00 => {
                    // Function import
                    let (type_idx, new_offset) = binary::read_leb128_u32(bytes, offset)?;
                    offset = new_offset;
                    wrt_format::module::ImportDesc::Function(type_idx)
                },
                0x01 => {
                    // Table import
                    let (format_table, new_offset) = parse_format_module_table(bytes, offset)?;
                    offset = new_offset;
                    wrt_format::module::ImportDesc::Table(format_table)
                },
                0x02 => {
                    // Memory import
                    let (format_memory, new_offset) = parse_format_module_memory(bytes, offset)?;
                    offset = new_offset;
                    wrt_format::module::ImportDesc::Memory(format_memory)
                },
                0x03 => {
                    // Global import
                    let (format_global_type, new_offset) = parse_format_global_type(bytes, offset)?;
                    offset = new_offset;
                    wrt_format::module::ImportDesc::Global(format_global_type)
                },
                // TODO: Handle 0x04 Tag import if/when supported by wrt_format
                _ => {
                    return Err(Error::parse_error("Invalid import description kind"));
                },
            };

            format_imports.push(wrt_format::module::Import {
                module: module_string,
                name: field_string,
                desc: format_desc,
            });
        }

        // Convert wrt_format::Import to wrt_foundation::Import
        // Since Table and Memory are now type aliases to foundation types, this should
        // work directly
        let mut wrt_imports = Vec::with_capacity(format_imports.len());
        let provider = wrt_foundation::safe_managed_alloc!(
            65536,
            wrt_foundation::budget_aware_provider::CrateId::Decoder
        )?;

        for format_import in format_imports {
            let module_name =
                wrt_foundation::bounded::WasmName::try_from_str(&format_import.module)
                    .map_err(|_| Error::parse_error("Module name too long for bounded string"))?;

            let item_name = wrt_foundation::bounded::WasmName::try_from_str(&format_import.name)
                .map_err(|_| Error::parse_error("Item name too long for bounded string"))?;

            let wrt_desc = match format_import.desc {
                wrt_format::module::ImportDesc::Function(type_idx) => {
                    wrt_foundation::types::ImportDesc::Function(type_idx)
                },
                wrt_format::module::ImportDesc::Table(table) => {
                    wrt_foundation::types::ImportDesc::Table(table)
                },
                wrt_format::module::ImportDesc::Memory(memory) => {
                    wrt_foundation::types::ImportDesc::Memory(memory)
                },
                wrt_format::module::ImportDesc::Global(format_global) => {
                    // Convert FormatGlobalType to wrt_foundation::GlobalType
                    let global_type = wrt_foundation::GlobalType::new(
                        format_global.value_type,
                        format_global.mutable,
                    );
                    wrt_foundation::types::ImportDesc::Global(global_type)
                },
                wrt_format::module::ImportDesc::Tag(type_idx) => {
                    // Tag is not available in ImportDesc, map to Function for now
                    wrt_foundation::types::ImportDesc::Function(type_idx)
                },
            };

            wrt_imports.push(WrtFoundationImport {
                module_name,
                item_name,
                desc: wrt_desc,
            });
        }

        Ok(wrt_imports)
    }

    /// Parse a table section
    pub fn parse_table_section(bytes: &[u8]) -> Result<Vec<WrtTableType>> {
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;
        let mut wrt_tables = Vec::with_capacity(count as usize);

        // Parse format tables and convert directly to foundation types
        for _ in 0..count {
            let (format_table, new_offset) = parse_format_module_table(bytes, offset)?;
            offset = new_offset;

            // Since wrt_format::module::Table is now a type alias to
            // wrt_foundation::TableType, we can use it directly
            wrt_tables.push(format_table);
        }

        Ok(wrt_tables)
    }

    fn parse_format_module_table(
        bytes: &[u8],
        mut offset: usize,
    ) -> Result<(wrt_foundation::TableType, usize)> {
        if offset >= bytes.len() {
            return Err(Error::parse_error(
                "Unexpected end of table entry (element type byte)",
            ));
        }
        let element_type_byte = bytes[offset];
        offset += 1;

        let element_type = wrt_format::conversion::parse_value_type(element_type_byte)
            .map_err(|_e| Error::runtime_execution_error("Parse error"))?;

        if element_type != FormatValueType::FuncRef && element_type != FormatValueType::ExternRef {
            return Err(Error::new(
                ErrorCategory::Parse,
                codes::INVALID_TYPE,
                "Invalid table element type",
            ));
        }

        let (limits, new_offset) = parse_limits(bytes, offset)?;
        offset = new_offset;

        // Convert ValueType to RefType for table element_type
        let ref_type = match element_type {
            FormatValueType::FuncRef => wrt_foundation::RefType::Funcref,
            FormatValueType::ExternRef => wrt_foundation::RefType::Externref,
            _ => {
                return Err(Error::runtime_execution_error("Invalid table element type"));
            },
        };

        // Convert wrt_format::Limits to wrt_foundation::Limits
        let foundation_limits = wrt_foundation::Limits::new(
            limits.min as u32, // Convert u64 to u32
            limits.max.map(|m| m as u32),
        );

        Ok((
            wrt_foundation::TableType {
                element_type: ref_type,
                limits: foundation_limits,
            },
            offset,
        ))
    }

    /// Parse a memory section
    pub fn parse_memory_section(bytes: &[u8]) -> Result<Vec<WrtMemoryType>> {
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;
        let mut wrt_memories = Vec::with_capacity(count as usize);

        // Parse format memories and convert directly to foundation types
        for _ in 0..count {
            let (format_memory, new_offset) = parse_format_module_memory(bytes, offset)?;
            offset = new_offset;

            // Since wrt_format::module::Memory is now a type alias to
            // wrt_foundation::MemoryType, we can use it directly
            wrt_memories.push(format_memory);
        }

        Ok(wrt_memories)
    }

    fn parse_format_module_memory(
        bytes: &[u8],
        offset: usize,
    ) -> Result<(wrt_foundation::MemoryType, usize)> {
        let (limits, new_offset) = parse_limits(bytes, offset)?;

        // Convert wrt_format::Limits to wrt_foundation::Limits
        let foundation_limits = wrt_foundation::Limits::new(
            limits.min as u32, // Convert u64 to u32
            limits.max.map(|m| m as u32),
        );

        Ok((
            wrt_foundation::MemoryType::new_with_memory64(foundation_limits, limits.shared, limits.memory64),
            new_offset,
        ))
    }

    fn parse_format_global_type(
        bytes: &[u8],
        mut offset: usize,
    ) -> Result<(wrt_format::types::FormatGlobalType, usize)> {
        if offset + 1 >= bytes.len() {
            // Need valtype + mutability byte
            return Err(Error::parse_error("Unexpected end of global type"));
        }
        let val_type_byte = bytes[offset];
        offset += 1;
        let mutability_byte = bytes[offset];
        offset += 1;

        let value_type = wrt_format::conversion::parse_value_type(val_type_byte)
            .map_err(|_e| Error::runtime_execution_error("Parse error"))?;

        let mutable = match mutability_byte {
            0x00 => false,
            0x01 => true,
            _ => return Err(Error::parse_error("Invalid mutability byte")),
        };

        Ok((
            wrt_format::types::FormatGlobalType {
                value_type,
                mutable,
            },
            offset,
        ))
    }

    /// Parse a global section
    pub fn parse_global_section(bytes: &[u8]) -> Result<Vec<WrtGlobalType>> {
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;
        let mut wrt_globals = Vec::with_capacity(count as usize);

        for _ in 0..count {
            let (format_global_type, new_offset) = parse_format_global_type(bytes, offset)?;
            offset = new_offset;

            let init_expr_start = offset;
            #[allow(unused_assignments)]
            let mut end_opcode_idx = None;
            // Find the END opcode for the init_expr
            // Correctly iterate through the slice from the current offset
            let mut temp_offset = init_expr_start;
            loop {
                if temp_offset >= bytes.len() {
                    return Err(Error::parse_error(
                        "Global init expression unterminated or extends beyond section bounds",
                    ));
                }
                // A simple way to find END is to try parsing instructions one by one,
                // but that requires a full instruction parser here or assumptions.
                // A more robust way is to scan for END, but must be careful about nested blocks
                // if allowed in init_expr (they are not). MVP: init_expr is a
                // single const instruction, or global.get, followed by END. So,
                // we can scan for the END opcode.
                if bytes[temp_offset] == binary::END {
                    // binary::END should be 0x0B
                    end_opcode_idx = Some(temp_offset);
                    break;
                }
                // To avoid infinite loop on malformed input without END, check reasonable
                // length or instruction. For now, simple scan for END. Max
                // init_expr length is small.
                if temp_offset > init_expr_start + 20 {
                    // Heuristic limit to prevent runaway scan
                    return Err(Error::parse_error(
                        "Global init expression too long or END opcode not found within \
                         reasonable limit",
                    ));
                }
                temp_offset += 1;
            }

            let end_idx = end_opcode_idx
                .ok_or_else(|| Error::parse_error("Global init expression missing END opcode"))?;

            let init_expr_bytes = &bytes[init_expr_start..end_idx + 1]; // Slice includes the END opcode
            offset = end_idx + 1; // Update main offset to after the init_expr

            let _format_global = wrt_format::module::Global {
                global_type: format_global_type,
                init: init_expr_bytes.to_vec(),
            };

            // Convert FormatGlobalType to wrt_foundation::GlobalType
            // Both types have the same structure (value_type and mutable)
            let wrt_global = WrtGlobalType {
                value_type: format_global_type.value_type,
                mutable: format_global_type.mutable,
            };

            wrt_globals.push(wrt_global);
        }
        Ok(wrt_globals)
    }

    /// Parse an export section with memory optimization
    pub fn parse_export_section(bytes: &[u8]) -> Result<Vec<WrtExport>> {
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;

        // Binary std/no_std choice
        check_bounds_u32(count, 10000, "export count")?;
        let count_usize = safe_usize_conversion(count, "export count")?;

        let mut format_exports = Vec::with_capacity(count_usize.min(1024));

        for _ in 0..count {
            // Parse export name using optimized string processing
            let (export_name, new_offset) = parse_utf8_string_inplace(bytes, offset)?;
            offset = new_offset;

            if offset >= bytes.len() {
                return Err(Error::parse_error("Unexpected end of export kind"));
            }
            let kind_byte = bytes[offset];
            offset += 1;

            let format_kind = match kind_byte {
                0x00 => wrt_format::module::ExportKind::Function,
                0x01 => wrt_format::module::ExportKind::Table,
                0x02 => wrt_format::module::ExportKind::Memory,
                0x03 => wrt_format::module::ExportKind::Global,
                // TODO: Handle 0x04 Tag if/when supported by wrt_format
                _ => return Err(Error::parse_error("Invalid export kind byte")),
            };

            let (index, new_offset) = binary::read_leb128_u32(bytes, offset)?;
            offset = new_offset;

            format_exports.push(wrt_format::module::Export {
                name: export_name,
                kind: format_kind,
                index,
            });
        }
        // Return the format_exports since wrt_format::module::Export is what's expected
        Ok(format_exports)
    }

    /// Parse an element section
    pub fn parse_element_section(bytes: &[u8]) -> Result<Vec<WrtElementSegment>> {
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;
        let mut wrt_elements = Vec::with_capacity(count as usize);

        for _ in 0..count {
            // Parse using pure format type
            let (pure_element, new_offset) = parse_element_segment(bytes, offset)
                .map_err(|e| Error::new(e.category(), e.code(), "Failed to parse element entry"))?;
            offset = new_offset;

            // Use the pure element segment directly (it's already the right type)
            wrt_elements.push(pure_element);
        }
        Ok(wrt_elements)
    }

    /// Parse a code section with memory optimization
    pub fn parse_code_section(bytes: &[u8]) -> Result<Vec<Vec<u8>>> {
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;

        // Binary std/no_std choice
        check_bounds_u32(count, 100000, "function count")?;
        let count_usize = safe_usize_conversion(count, "function count")?;

        let mut bodies = Vec::new();
        bodies.reserve(count_usize.min(10000)); // Conservative reservation

        for _ in 0..count {
            // Get body size
            let (body_size, new_offset) = binary::read_leb128_u32(bytes, offset)?;
            offset = new_offset;

            // Bounds check body size
            check_bounds_u32(body_size, 1_000_000, "function body size")?;
            let body_size_usize = safe_usize_conversion(body_size, "function body size")?;

            if offset + body_size_usize > bytes.len() {
                return Err(Error::parse_error("Unexpected end of code body"));
            }

            // Binary std/no_std choice
            let mut body = Vec::new();
            body.reserve_exact(body_size_usize);
            body.extend_from_slice(&bytes[offset..offset + body_size_usize]);
            offset += body_size_usize;

            bodies.push(body);
        }

        Ok(bodies)
    }

    /// Parse a data section
    pub fn parse_data_section(bytes: &[u8]) -> Result<Vec<WrtDataSegment>> {
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;
        let mut wrt_data_segments = Vec::with_capacity(count as usize);

        for _ in 0..count {
            // Parse using pure format type
            let (pure_data_segment, new_offset) = parse_data(bytes, offset).map_err(|e| {
                Error::new(e.category(), e.code(), "Failed to parse data segment entry")
            })?;
            offset = new_offset;

            // Use the pure data segment directly (it's already the right type)
            wrt_data_segments.push(pure_data_segment);
        }
        Ok(wrt_data_segments)
    }
}
