//! Streaming decoder for WebAssembly binaries with minimal memory usage
//!
//! This module provides a streaming API for decoding WebAssembly modules
//! that processes sections one at a time without loading the entire binary
//! into memory.

#[cfg(not(feature = "std"))]
extern crate alloc;

use alloc::vec::Vec;

#[cfg(feature = "tracing")]
use wrt_foundation::tracing::trace;

// Platform-specific limits for bounded allocation
use wrt_foundation::limits;

// Allocation tracing for understanding memory patterns
#[cfg(feature = "allocation-tracing")]
use wrt_foundation::{AllocationPhase, trace_alloc};

use wrt_format::module::{Function, Module as WrtModule};
use wrt_foundation::{bounded::BoundedVec, safe_memory::NoStdProvider, types::TagType};

use crate::{
    prelude::*,
    streaming_validator::{ComprehensivePlatformLimits, StreamingWasmValidator},
};

/// Skip an LEB128-encoded unsigned integer and return the number of bytes consumed
fn skip_leb128_u32(data: &[u8], offset: usize) -> usize {
    let mut bytes = 0;
    while offset + bytes < data.len() {
        let byte = data[offset + bytes];
        bytes += 1;
        if byte & 0x80 == 0 {
            break;
        }
        // Safety limit to prevent infinite loops
        if bytes > 5 {
            break;
        }
    }
    bytes
}

/// Skip an LEB128-encoded signed integer (i32) and return the number of bytes consumed
fn skip_leb128_i32(data: &[u8], offset: usize) -> usize {
    let mut bytes = 0;
    while offset + bytes < data.len() {
        let byte = data[offset + bytes];
        bytes += 1;
        if byte & 0x80 == 0 {
            break;
        }
        // Safety limit to prevent infinite loops
        if bytes > 5 {
            break;
        }
    }
    bytes
}

/// Skip an LEB128-encoded signed integer (i64) and return the number of bytes consumed
fn skip_leb128_i64(data: &[u8], offset: usize) -> usize {
    let mut bytes = 0;
    while offset + bytes < data.len() {
        let byte = data[offset + bytes];
        bytes += 1;
        if byte & 0x80 == 0 {
            break;
        }
        // Safety limit to prevent infinite loops
        if bytes > 10 {
            break;
        }
    }
    bytes
}

/// Find the end of an expression by properly parsing instructions.
/// Returns the position AFTER the end opcode (0x0B).
///
/// This is necessary because 0x0B can appear as a LEB128 value (e.g., the value 11),
/// so we can't just scan for the first 0x0B.
fn find_expression_end(data: &[u8], start: usize) -> Result<usize> {
    let mut offset = start;

    while offset < data.len() {
        let opcode = data[offset];
        offset += 1;

        match opcode {
            0x0B => {
                // end opcode - we found the end of the expression
                return Ok(offset);
            },
            0x41 => {
                // i32.const - skip LEB128 i32 value
                offset += skip_leb128_i32(data, offset);
            },
            0x42 => {
                // i64.const - skip LEB128 i64 value
                offset += skip_leb128_i64(data, offset);
            },
            0x43 => {
                // f32.const - skip 4 bytes
                offset += 4;
            },
            0x44 => {
                // f64.const - skip 8 bytes
                offset += 8;
            },
            0x23 => {
                // global.get - skip LEB128 global index
                offset += skip_leb128_u32(data, offset);
            },
            0xD0 => {
                // ref.null - skip LEB128 heap type
                offset += skip_leb128_u32(data, offset);
            },
            0xD2 => {
                // ref.func - skip LEB128 function index
                offset += skip_leb128_u32(data, offset);
            },
            _ => {
                // Unknown opcode in expression - this shouldn't happen in valid WASM
                // but we'll continue and hope to find an end
                #[cfg(feature = "tracing")]
                trace!(
                    opcode = opcode,
                    offset = offset - 1,
                    "Unknown opcode in expression"
                );
            },
        }
    }

    Err(Error::parse_error("Expression did not end with 0x0B"))
}

/// Streaming decoder that processes WebAssembly modules section by section
pub struct StreamingDecoder<'a> {
    /// The WebAssembly binary data
    binary: &'a [u8],
    /// Current offset in the binary
    offset: usize,
    /// Platform limits for validation
    platform_limits: ComprehensivePlatformLimits,
    /// Number of function imports (for proper function indexing)
    num_function_imports: usize,
    /// Number of memory imports (for multiple memory validation)
    num_memory_imports: usize,
    /// Count from function section (module-defined functions, not imports)
    function_count: Option<u32>,
    /// Count from code section
    code_count: Option<u32>,
    /// Value from data count section
    data_count_value: Option<u32>,
    /// Count from data section
    data_section_count: Option<u32>,
    /// The module being built (std version)
    #[cfg(feature = "std")]
    module: WrtModule,
    /// The module being built (no_std version)
    #[cfg(not(feature = "std"))]
    module: WrtModule<NoStdProvider<8192>>,
}

impl<'a> StreamingDecoder<'a> {
    /// Create a new streaming decoder (std version)
    #[cfg(feature = "std")]
    pub fn new(binary: &'a [u8]) -> Result<Self> {
        let module = WrtModule::default();

        Ok(Self {
            binary,
            offset: 0,
            platform_limits: ComprehensivePlatformLimits::default(),
            num_function_imports: 0,
            num_memory_imports: 0,
            function_count: None,
            code_count: None,
            data_count_value: None,
            data_section_count: None,
            module,
        })
    }

    /// Create a new streaming decoder (no_std version)
    #[cfg(not(feature = "std"))]
    pub fn new(binary: &'a [u8]) -> Result<Self> {
        let provider = wrt_foundation::safe_managed_alloc!(
            8192,
            wrt_foundation::budget_aware_provider::CrateId::Decoder
        )?;
        let module = WrtModule::default();

        Ok(Self {
            binary,
            offset: 0,
            platform_limits: ComprehensivePlatformLimits::default(),
            num_function_imports: 0,
            num_memory_imports: 0,
            function_count: None,
            code_count: None,
            data_count_value: None,
            data_section_count: None,
            module,
        })
    }

    /// Decode the module header
    pub fn decode_header(&mut self) -> Result<()> {
        // Validate magic number and version
        if self.binary.len() < 8 {
            return Err(Error::parse_error(
                "Binary too small for WebAssembly header",
            ));
        }

        // Check magic number
        if &self.binary[0..4] != b"\0asm" {
            return Err(Error::parse_error("Invalid WebAssembly magic number"));
        }

        // Check version
        if self.binary[4..8] != [0x01, 0x00, 0x00, 0x00] {
            return Err(Error::parse_error("Unsupported WebAssembly version"));
        }

        self.offset = 8;
        Ok(())
    }

    /// Process the next section in the stream
    pub fn process_next_section(&mut self) -> Result<bool> {
        if self.offset >= self.binary.len() {
            return Ok(false); // No more sections
        }

        // Read section ID
        let section_id = self.binary[self.offset];
        self.offset += 1;

        // Read section size
        let (section_size, bytes_read) = read_leb128_u32(self.binary, self.offset)?;
        self.offset += bytes_read;

        let section_end = self.offset + section_size as usize;
        if section_end > self.binary.len() {
            return Err(Error::parse_error("Section extends beyond binary"));
        }

        // Process section data without loading it all into memory
        let section_data = &self.binary[self.offset..section_end];
        self.process_section(section_id, section_data)?;

        self.offset = section_end;
        Ok(true)
    }

    /// Process a specific section
    /// Returns the number of bytes consumed from the section data.
    fn process_section(&mut self, section_id: u8, data: &[u8]) -> Result<usize> {
        let bytes_consumed = match section_id {
            1 => self.process_type_section(data)?,
            2 => self.process_import_section(data)?,
            3 => self.process_function_section(data)?,
            4 => self.process_table_section(data)?,
            5 => self.process_memory_section(data)?,
            6 => self.process_global_section(data)?,
            7 => self.process_export_section(data)?,
            8 => self.process_start_section(data)?,
            9 => self.process_element_section(data)?,
            10 => self.process_code_section(data)?,
            11 => self.process_data_section(data)?,
            12 => self.process_data_count_section(data)?,
            13 => self.process_tag_section(data)?,
            _ => self.process_custom_section(data)?,
        };

        // Validate that the section content exactly matches the declared size
        if bytes_consumed != data.len() {
            return Err(Error::parse_error("section size mismatch"));
        }

        Ok(bytes_consumed)
    }

    /// Process type section
    ///
    /// Handles both MVP function types (0x60) and GC proposal types:
    /// - 0x60 = func (function type)
    /// - 0x5F = struct (struct type) - parsed but stored separately
    /// - 0x5E = array (array type) - parsed but stored separately
    /// - 0x4E = rec (recursive type group)
    /// - 0x50 = sub (subtype declaration)
    /// - 0x4F = sub final (final subtype declaration)
    fn process_type_section(&mut self, data: &[u8]) -> Result<usize> {
        use wrt_format::binary::{
            COMPOSITE_TYPE_ARRAY, COMPOSITE_TYPE_FUNC, COMPOSITE_TYPE_REC, COMPOSITE_TYPE_STRUCT,
            COMPOSITE_TYPE_SUB, COMPOSITE_TYPE_SUB_FINAL, read_leb128_u32,
        };
        use wrt_foundation::types::ValueType;

        let mut offset = 0;
        let (count, bytes_read) = read_leb128_u32(data, offset)?;
        offset += bytes_read;

        // Validate type count against platform limits
        if count as usize > limits::MAX_TYPES {
            return Err(Error::parse_error(
                "Module exceeds maximum type count for platform",
            ));
        }

        #[cfg(feature = "tracing")]
        trace!(count = count, data_len = data.len(), "process_type_section");

        // Process each type entry one at a time
        // Note: A type entry can be a single composite type, a subtype, or a rec group
        let mut i = 0u32;
        while i < count {
            if offset >= data.len() {
                return Err(Error::parse_error("Unexpected end of type section"));
            }

            let type_marker = data[offset];

            match type_marker {
                COMPOSITE_TYPE_REC => {
                    // rec group: 0x4E count subtype*
                    offset += 1;
                    let (rec_count, bytes_read) = read_leb128_u32(data, offset)?;
                    offset += bytes_read;

                    #[cfg(feature = "tracing")]
                    trace!(rec_count = rec_count, "process_type_section: rec group");

                    // Process each subtype in the recursive group
                    for _j in 0..rec_count {
                        offset = self.parse_subtype_entry(data, offset)?;
                    }
                    // A rec group with N types counts as N type entries
                    // But the loop already counted as 1, so we need to account for the rest
                    // Actually, for type indexing, the rec group entries each get their own index
                    // The outer count counts rec groups as single entries, but we need to adjust
                    // For now, treat rec as consuming one entry (the spec says rec is one type entry
                    // that defines multiple types with consecutive indices)
                    i += 1;
                },
                COMPOSITE_TYPE_SUB | COMPOSITE_TYPE_SUB_FINAL => {
                    // subtype: 0x50/0x4F supertype* comptype
                    offset = self.parse_subtype_entry(data, offset)?;
                    i += 1;
                },
                COMPOSITE_TYPE_FUNC | COMPOSITE_TYPE_STRUCT | COMPOSITE_TYPE_ARRAY => {
                    // Direct composite type without subtype wrapper
                    offset = self.parse_composite_type(data, offset)?;
                    i += 1;
                },
                _ => {
                    return Err(Error::parse_error("Invalid type section marker"));
                },
            }

            #[cfg(feature = "tracing")]
            trace!(type_index = i - 1, "process_type_section: parsed type");
        }

        #[cfg(feature = "tracing")]
        trace!(
            types_count = self.module.types.len(),
            "process_type_section: complete"
        );

        Ok(offset)
    }

    /// Parse a subtype entry (sub, sub final, or bare composite type)
    fn parse_subtype_entry(&mut self, data: &[u8], mut offset: usize) -> Result<usize> {
        use wrt_format::binary::{
            COMPOSITE_TYPE_ARRAY, COMPOSITE_TYPE_FUNC, COMPOSITE_TYPE_STRUCT, COMPOSITE_TYPE_SUB,
            COMPOSITE_TYPE_SUB_FINAL, read_leb128_u32,
        };

        if offset >= data.len() {
            return Err(Error::parse_error("Unexpected end of subtype entry"));
        }

        let marker = data[offset];

        match marker {
            COMPOSITE_TYPE_SUB | COMPOSITE_TYPE_SUB_FINAL => {
                // sub/sub_final: marker supertype_count supertype* comptype
                offset += 1;
                let (supertype_count, bytes_read) = read_leb128_u32(data, offset)?;
                offset += bytes_read;

                // Skip supertype indices
                for _ in 0..supertype_count {
                    let (_supertype_idx, bytes_read) = read_leb128_u32(data, offset)?;
                    offset += bytes_read;
                }

                // Parse the composite type
                offset = self.parse_composite_type(data, offset)?;
            },
            COMPOSITE_TYPE_FUNC | COMPOSITE_TYPE_STRUCT | COMPOSITE_TYPE_ARRAY => {
                // Direct composite type (implicitly final with no supertypes)
                offset = self.parse_composite_type(data, offset)?;
            },
            _ => {
                return Err(Error::parse_error("Invalid subtype marker"));
            },
        }

        Ok(offset)
    }

    /// Parse a composite type (func, struct, or array)
    fn parse_composite_type(&mut self, data: &[u8], mut offset: usize) -> Result<usize> {
        use wrt_format::binary::{
            COMPOSITE_TYPE_ARRAY, COMPOSITE_TYPE_FUNC, COMPOSITE_TYPE_STRUCT, read_leb128_u32,
        };
        use wrt_foundation::types::ValueType;

        if offset >= data.len() {
            return Err(Error::parse_error("Unexpected end of composite type"));
        }

        let type_marker = data[offset];
        offset += 1;

        match type_marker {
            COMPOSITE_TYPE_FUNC => {
                // Parse function type: param_count param* result_count result*
                let (param_count, bytes_read) = read_leb128_u32(data, offset)?;
                offset += bytes_read;

                // Validate param count against platform limits before allocation
                if param_count as usize > limits::MAX_FUNCTION_PARAMS {
                    return Err(Error::parse_error(
                        "Function type exceeds maximum parameter count for platform",
                    ));
                }

                #[cfg(feature = "allocation-tracing")]
                trace_alloc!(
                    AllocationPhase::Decode,
                    "streaming_decoder:func_type_params",
                    "params",
                    param_count as usize
                );

                #[cfg(feature = "std")]
                let mut params = Vec::with_capacity(param_count as usize);
                #[cfg(not(feature = "std"))]
                let mut params = alloc::vec::Vec::with_capacity(param_count as usize);

                for _ in 0..param_count {
                    let (vt, new_offset) = self.parse_value_type(data, offset)?;
                    offset = new_offset;
                    params.push(vt);
                }

                let (result_count, bytes_read) = read_leb128_u32(data, offset)?;
                offset += bytes_read;

                // Validate result count against platform limits before allocation
                if result_count as usize > limits::MAX_FUNCTION_RESULTS {
                    return Err(Error::parse_error(
                        "Function type exceeds maximum result count for platform",
                    ));
                }

                #[cfg(feature = "allocation-tracing")]
                trace_alloc!(
                    AllocationPhase::Decode,
                    "streaming_decoder:func_type_results",
                    "results",
                    result_count as usize
                );

                #[cfg(feature = "std")]
                let mut results = Vec::with_capacity(result_count as usize);
                #[cfg(not(feature = "std"))]
                let mut results = alloc::vec::Vec::with_capacity(result_count as usize);

                for _ in 0..result_count {
                    let (vt, new_offset) = self.parse_value_type(data, offset)?;
                    offset = new_offset;
                    results.push(vt);
                }

                // Store function type
                #[cfg(feature = "std")]
                {
                    use wrt_foundation::CleanCoreFuncType;
                    let func_type = CleanCoreFuncType { params, results };
                    self.module.types.push(func_type);
                }

                #[cfg(not(feature = "std"))]
                {
                    use wrt_foundation::types::FuncType;
                    let func_type = FuncType::new(params.into_iter(), results.into_iter())?;
                    let _ = self.module.types.push(func_type);
                }
            },
            COMPOSITE_TYPE_STRUCT => {
                // Parse struct type: field_count field*
                // field = storage_type mutability
                let (field_count, bytes_read) = read_leb128_u32(data, offset)?;
                offset += bytes_read;

                #[cfg(feature = "tracing")]
                trace!(field_count = field_count, "parse_composite_type: struct");

                for _ in 0..field_count {
                    // Parse storage type (value type or packed type)
                    let (_, new_offset) = self.parse_storage_type(data, offset)?;
                    offset = new_offset;

                    // Parse mutability flag
                    if offset >= data.len() {
                        return Err(Error::parse_error("Unexpected end of struct field"));
                    }
                    offset += 1; // mut flag
                }

                // TODO: Store struct type when we have proper GC type storage
                // For now, we add a placeholder func type to maintain index alignment
                #[cfg(feature = "std")]
                {
                    use wrt_foundation::CleanCoreFuncType;
                    let placeholder = CleanCoreFuncType {
                        params: Vec::new(),
                        results: Vec::new(),
                    };
                    self.module.types.push(placeholder);
                }

                #[cfg(not(feature = "std"))]
                {
                    use wrt_foundation::types::FuncType;
                    let placeholder = FuncType::new(core::iter::empty(), core::iter::empty())?;
                    let _ = self.module.types.push(placeholder);
                }
            },
            COMPOSITE_TYPE_ARRAY => {
                // Parse array type: storage_type mutability
                let (_, new_offset) = self.parse_storage_type(data, offset)?;
                offset = new_offset;

                // Parse mutability flag
                if offset >= data.len() {
                    return Err(Error::parse_error("Unexpected end of array type"));
                }
                offset += 1; // mut flag

                #[cfg(feature = "tracing")]
                trace!("parse_composite_type: array");

                // TODO: Store array type when we have proper GC type storage
                // For now, we add a placeholder func type to maintain index alignment
                #[cfg(feature = "std")]
                {
                    use wrt_foundation::CleanCoreFuncType;
                    let placeholder = CleanCoreFuncType {
                        params: Vec::new(),
                        results: Vec::new(),
                    };
                    self.module.types.push(placeholder);
                }

                #[cfg(not(feature = "std"))]
                {
                    use wrt_foundation::types::FuncType;
                    let placeholder = FuncType::new(core::iter::empty(), core::iter::empty())?;
                    let _ = self.module.types.push(placeholder);
                }
            },
            _ => {
                return Err(Error::parse_error("Invalid composite type marker"));
            },
        }

        Ok(offset)
    }

    /// Parse a storage type (value type or packed type)
    fn parse_storage_type(&self, data: &[u8], mut offset: usize) -> Result<(u8, usize)> {
        use wrt_format::binary::read_leb128_u32;

        if offset >= data.len() {
            return Err(Error::parse_error("Unexpected end of storage type"));
        }

        let byte = data[offset];

        // Check for packed types first (i8 = 0x78, i16 = 0x77)
        if byte == 0x78 || byte == 0x77 {
            return Ok((byte, offset + 1));
        }

        // Otherwise parse as value type
        let (_, new_offset) = self.parse_value_type(data, offset)?;
        Ok((byte, new_offset))
    }

    /// Parse a value type (may include GC reference types)
    fn parse_value_type(
        &self,
        data: &[u8],
        mut offset: usize,
    ) -> Result<(wrt_foundation::types::ValueType, usize)> {
        use wrt_format::binary::{REF_TYPE_NON_NULLABLE, REF_TYPE_NULLABLE};
        use wrt_foundation::types::ValueType;

        if offset >= data.len() {
            return Err(Error::parse_error("Unexpected end of value type"));
        }

        let byte = data[offset];

        match byte {
            // Standard value types
            0x7F => Ok((ValueType::I32, offset + 1)),
            0x7E => Ok((ValueType::I64, offset + 1)),
            0x7D => Ok((ValueType::F32, offset + 1)),
            0x7C => Ok((ValueType::F64, offset + 1)),
            0x7B => Ok((ValueType::V128, offset + 1)),
            // Reference types
            0x70 => Ok((ValueType::FuncRef, offset + 1)),
            0x6F => Ok((ValueType::ExternRef, offset + 1)),
            0x69 => Ok((ValueType::ExnRef, offset + 1)),
            // GC abstract heap type references (shorthand form)
            0x6E => Ok((ValueType::AnyRef, offset + 1)), // anyref
            0x6D => Ok((ValueType::EqRef, offset + 1)),  // eqref
            0x6C => Ok((ValueType::I31Ref, offset + 1)), // i31ref
            0x6B => Ok((ValueType::StructRef(0), offset + 1)), // structref (abstract)
            0x6A => Ok((ValueType::ArrayRef(0), offset + 1)), // arrayref (abstract)
            0x73 => Ok((ValueType::NullFuncRef, offset + 1)), // nofunc (bottom for func)
            0x72 => Ok((ValueType::ExternRef, offset + 1)), // noextern (bottom for extern)
            0x71 => Ok((ValueType::AnyRef, offset + 1)), // none (bottom for any)
            0x74 => Ok((ValueType::ExnRef, offset + 1)), // noexn (bottom for exn)
            // GC typed references: (ref null? ht)
            REF_TYPE_NULLABLE | REF_TYPE_NON_NULLABLE => {
                let nullable = byte == REF_TYPE_NULLABLE;
                offset += 1;
                // Parse heap type as s33 (signed 33-bit LEB128)
                let (heap_type_idx, new_offset) = self.parse_heap_type(data, offset)?;

                // Abstract heap types are encoded as negative s33 values:
                // - 0x70 (func) -> single-byte s33 = -16
                // - 0x6F (extern) -> -17
                // - 0x6E (any) -> -18
                // - 0x6D (eq) -> -19
                // - 0x6C (i31) -> -20
                // - 0x6B (struct) -> -21
                // - 0x6A (array) -> -22
                // - 0x69 (exn) -> -23
                // - 0x73 (nofunc) -> -13
                // - 0x72 (noextern) -> -14
                // - 0x71 (none) -> -15
                // Concrete type indices are non-negative.

                if heap_type_idx < 0 {
                    // Abstract heap type
                    match heap_type_idx {
                        -16 => Ok((ValueType::FuncRef, new_offset)), // func (0x70)
                        -17 => Ok((ValueType::ExternRef, new_offset)), // extern (0x6F)
                        -18 => Ok((ValueType::AnyRef, new_offset)),  // any (0x6E)
                        -19 => Ok((ValueType::EqRef, new_offset)),   // eq (0x6D)
                        -20 => Ok((ValueType::I31Ref, new_offset)),  // i31 (0x6C)
                        -21 => Ok((ValueType::StructRef(0), new_offset)), // struct (0x6B)
                        -22 => Ok((ValueType::ArrayRef(0), new_offset)), // array (0x6A)
                        -23 => Ok((ValueType::ExnRef, new_offset)),  // exn (0x69)
                        -13 => Ok((ValueType::NullFuncRef, new_offset)), // nofunc (0x73) - bottom for func
                        -14 => Ok((ValueType::ExternRef, new_offset)),   // noextern (0x72)
                        -15 => Ok((ValueType::AnyRef, new_offset)), // none (0x71) - bottom for any
                        -12 => Ok((ValueType::ExnRef, new_offset)), // noexn (0x74) - bottom for exn
                        _ => Ok((ValueType::AnyRef, new_offset)),   // fallback for unknown
                    }
                } else {
                    // Concrete type index - reference to a defined type
                    // Use TypedFuncRef to preserve nullability and type index
                    // Subtype checking during validation will determine if this is
                    // compatible with funcref, structref, etc.
                    Ok((
                        ValueType::TypedFuncRef(heap_type_idx as u32, nullable),
                        new_offset,
                    ))
                }
            },
            _ => {
                // Try to parse as ValueType using existing method
                let vt = ValueType::from_binary(byte)?;
                Ok((vt, offset + 1))
            },
        }
    }

    /// Parse a heap type (for GC reference types)
    fn parse_heap_type(&self, data: &[u8], offset: usize) -> Result<(i64, usize)> {
        use wrt_format::binary::read_leb128_i64;

        // Heap type is encoded as s33 (signed 33-bit LEB128)
        // We use i64 reading since it can handle the s33 range.
        // Abstract heap types are encoded as negative values (0x6E-0x73 range)
        // Concrete type indices are non-negative
        let (value, bytes_read) = read_leb128_i64(data, offset)?;
        Ok((value, offset + bytes_read))
    }

    /// Process import section
    fn process_import_section(&mut self, data: &[u8]) -> Result<usize> {
        use crate::optimized_string::validate_utf8_name;

        let mut offset = 0;
        let (count, bytes_read) = read_leb128_u32(data, offset)?;
        offset += bytes_read;

        // Validate import count against platform limits
        if count as usize > limits::MAX_IMPORTS {
            return Err(Error::parse_error(
                "Module exceeds maximum import count for platform",
            ));
        }

        #[cfg(feature = "tracing")]
        trace!(
            count = count,
            data_len = data.len(),
            "process_import_section"
        );

        // Process each import one at a time
        for i in 0..count {
            // Parse module name
            let (module_name, new_offset) = validate_utf8_name(data, offset)?;
            offset = new_offset;

            // Parse field name
            let (field_name, new_offset) = validate_utf8_name(data, offset)?;
            offset = new_offset;

            // Parse import kind
            if offset >= data.len() {
                return Err(Error::parse_error("Unexpected end of import kind"));
            }
            let kind = data[offset];
            offset += 1;

            #[cfg(feature = "tracing")]
            trace!(
                import_index = i,
                module = module_name,
                field = field_name,
                kind = kind,
                "import parsed"
            );

            // Parse import description and handle based on kind
            match kind {
                0x00 => {
                    // Function import
                    let (type_idx, bytes_read) = read_leb128_u32(data, offset)?;
                    offset += bytes_read;

                    #[cfg(feature = "tracing")]
                    trace!(import_index = i, type_idx = type_idx, "import: function");

                    // Create placeholder function for imported function
                    // This ensures function index space includes imports
                    let func = Function {
                        type_idx,
                        locals: alloc::vec::Vec::new(),
                        code: alloc::vec::Vec::new(),
                    };
                    self.module.functions.push(func);

                    // Track function imports for proper function indexing
                    self.num_function_imports += 1;

                    // Store the import in module.imports for runtime resolution
                    #[cfg(feature = "std")]
                    {
                        use wrt_format::module::{Import, ImportDesc};
                        let import = Import {
                            module: module_name.to_string(),
                            name: field_name.to_string(),
                            desc: ImportDesc::Function(type_idx),
                        };
                        self.module.imports.push(import);
                    }

                    #[cfg(feature = "tracing")]
                    trace!(
                        module = module_name,
                        name = field_name,
                        func_index = self.num_function_imports - 1,
                        "recorded import"
                    );
                },
                0x01 => {
                    // Table import - need to parse table type
                    // ref_type (1 byte) + limits (flags + min, optional max)
                    if offset >= data.len() {
                        return Err(Error::parse_error("Unexpected end of table import"));
                    }
                    let ref_type_byte = data[offset];
                    offset += 1;

                    // Parse limits
                    if offset >= data.len() {
                        return Err(Error::parse_error("Unexpected end of table limits"));
                    }
                    let flags = data[offset];
                    offset += 1;
                    let (min, bytes_read) = read_leb128_u32(data, offset)?;
                    offset += bytes_read;
                    let max = if flags & 0x01 != 0 {
                        let (max, bytes_read) = read_leb128_u32(data, offset)?;
                        offset += bytes_read;
                        Some(max)
                    } else {
                        None
                    };
                    #[cfg(feature = "tracing")]
                    trace!(import_index = i, min = min, max = ?max, "import: table");

                    // Store the table import in module.imports for runtime resolution
                    #[cfg(feature = "std")]
                    {
                        use wrt_format::module::{Import, ImportDesc};
                        use wrt_foundation::types::{Limits, RefType, TableType};

                        // Convert ref_type byte to RefType
                        let ref_type = match ref_type_byte {
                            0x70 => RefType::Funcref,
                            0x6F => RefType::Externref,
                            _ => RefType::Funcref, // Default for unknown
                        };

                        let limits = Limits { min, max };

                        let table_type = TableType {
                            element_type: ref_type,
                            limits,
                        };

                        let import = Import {
                            module: module_name.to_string(),
                            name: field_name.to_string(),
                            desc: ImportDesc::Table(table_type),
                        };
                        self.module.imports.push(import);
                    }
                },
                0x02 => {
                    // Memory import - need to parse limits
                    use wrt_format::binary::read_leb128_u64;

                    if offset >= data.len() {
                        return Err(Error::parse_error("Unexpected end of memory import"));
                    }
                    let flags = data[offset];
                    offset += 1;

                    // Check for memory64 flag (bit 2)
                    let is_memory64 = (flags & 0x04) != 0;

                    // WebAssembly spec: memory size must be at most 65536 pages (4GB)
                    const MAX_MEMORY_PAGES: u32 = 65536;

                    // Parse limits - memory64 uses u64, regular memory uses u32
                    let (min, max) = if is_memory64 {
                        let (min64, bytes_read) = read_leb128_u64(data, offset)?;
                        offset += bytes_read;

                        let max64 = if flags & 0x01 != 0 {
                            let (max_val, bytes_read) = read_leb128_u64(data, offset)?;
                            offset += bytes_read;
                            Some(max_val)
                        } else {
                            None
                        };

                        // Validate memory64 limits
                        if min64 > MAX_MEMORY_PAGES as u64 {
                            return Err(Error::validation_error(
                                "memory size must be at most 65536 pages (4 GiB)",
                            ));
                        }
                        if let Some(max64) = max64 {
                            if max64 > MAX_MEMORY_PAGES as u64 {
                                return Err(Error::validation_error(
                                    "memory size must be at most 65536 pages (4 GiB)",
                                ));
                            }
                        }

                        (min64 as u32, max64.map(|v| v as u32))
                    } else {
                        let (min, bytes_read) = read_leb128_u32(data, offset)?;
                        offset += bytes_read;
                        let max = if flags & 0x01 != 0 {
                            let (max, bytes_read) = read_leb128_u32(data, offset)?;
                            offset += bytes_read;
                            Some(max)
                        } else {
                            None
                        };

                        // Validate regular memory limits
                        if min > MAX_MEMORY_PAGES {
                            return Err(Error::validation_error(
                                "memory size must be at most 65536 pages (4 GiB)",
                            ));
                        }
                        if let Some(max_val) = max {
                            if max_val > MAX_MEMORY_PAGES {
                                return Err(Error::validation_error(
                                    "memory size must be at most 65536 pages (4 GiB)",
                                ));
                            }
                        }

                        (min, max)
                    };

                    #[cfg(feature = "tracing")]
                    trace!(import_index = i, min_pages = min, max_pages = ?max, "import: memory");

                    // Store the memory import in module.imports for runtime resolution
                    #[cfg(feature = "std")]
                    {
                        use wrt_format::module::{Import, ImportDesc};
                        use wrt_foundation::types::{Limits, MemoryType};

                        let limits = Limits { min, max };

                        let memory_type = MemoryType {
                            limits,
                            shared: flags & 0x02 != 0, // bit 1 = shared
                            memory64: flags & 0x04 != 0, // bit 2 = memory64
                        };

                        let import = Import {
                            module: module_name.to_string(),
                            name: field_name.to_string(),
                            desc: ImportDesc::Memory(memory_type),
                        };
                        self.module.imports.push(import);
                    }

                    // Track memory imports for multiple memory validation
                    self.num_memory_imports += 1;
                },
                0x03 => {
                    // Global import - need to parse global type
                    // value_type (1 byte) + mutability (1 byte)
                    if offset + 1 >= data.len() {
                        return Err(Error::parse_error("Unexpected end of global import"));
                    }
                    let value_type_byte = data[offset];
                    offset += 1;
                    let mutability_byte = data[offset];
                    offset += 1;

                    // WebAssembly spec: only 0x00 (immutable) and 0x01 (mutable) are valid
                    if mutability_byte != 0x00 && mutability_byte != 0x01 {
                        return Err(Error::parse_error("malformed mutability"));
                    }

                    // Parse value type
                    let value_type = match value_type_byte {
                        0x7F => wrt_foundation::ValueType::I32,
                        0x7E => wrt_foundation::ValueType::I64,
                        0x7D => wrt_foundation::ValueType::F32,
                        0x7C => wrt_foundation::ValueType::F64,
                        0x7B => wrt_foundation::ValueType::V128,
                        0x70 => wrt_foundation::ValueType::FuncRef,
                        0x6F => wrt_foundation::ValueType::ExternRef,
                        0x69 => wrt_foundation::ValueType::ExnRef,
                        _ => return Err(Error::parse_error("Invalid global import value type")),
                    };

                    #[cfg(feature = "tracing")]
                    trace!(import_index = i, value_type = ?value_type, mutable = (mutability_byte != 0), "import: global");

                    // Store the global import in module.imports for runtime resolution
                    #[cfg(feature = "std")]
                    {
                        use wrt_format::module::{Import, ImportDesc};
                        use wrt_format::types::FormatGlobalType;
                        let global_type = FormatGlobalType {
                            value_type,
                            mutable: mutability_byte != 0,
                        };
                        let import = Import {
                            module: module_name.to_string(),
                            name: field_name.to_string(),
                            desc: ImportDesc::Global(global_type),
                        };
                        self.module.imports.push(import);
                    }
                },
                0x04 => {
                    // Tag import: attribute byte + type_idx
                    // Per WebAssembly spec, tag type is: attribute (must be 0) + type_idx
                    if offset >= data.len() {
                        return Err(Error::parse_error("Unexpected end of tag import"));
                    }
                    let attribute = data[offset];
                    offset += 1;

                    // Validate attribute - must be 0 (exception)
                    if attribute != 0 {
                        return Err(Error::validation_error("Invalid tag attribute"));
                    }

                    let (type_idx, bytes_read) = read_leb128_u32(data, offset)?;
                    offset += bytes_read;

                    #[cfg(feature = "tracing")]
                    trace!(
                        import_index = i,
                        attribute = attribute,
                        type_idx = type_idx,
                        "import: tag"
                    );

                    // Store tag import in module.imports for runtime resolution
                    #[cfg(feature = "std")]
                    {
                        use wrt_format::module::{Import, ImportDesc};

                        let import = Import {
                            module: module_name.to_string(),
                            name: field_name.to_string(),
                            desc: ImportDesc::Tag(type_idx),
                        };
                        self.module.imports.push(import);
                    }
                },
                _ => {
                    return Err(Error::parse_error("Invalid import kind"));
                },
            }
        }

        #[cfg(feature = "tracing")]
        trace!(
            functions_count = self.module.functions.len(),
            "process_import_section: complete"
        );

        Ok(offset)
    }

    /// Process function section
    fn process_function_section(&mut self, data: &[u8]) -> Result<usize> {
        let mut offset = 0;
        let (count, bytes_read) = read_leb128_u32(data, offset)?;
        offset += bytes_read;

        // Store function count for cross-section validation
        self.function_count = Some(count);

        // Validate function count against platform limits
        // Note: This is module-defined functions only, not including imports
        let total_funcs = self.num_function_imports + count as usize;
        if total_funcs > limits::MAX_FUNCTIONS {
            return Err(Error::parse_error(
                "Module exceeds maximum function count for platform",
            ));
        }

        #[cfg(feature = "tracing")]
        trace!(
            count = count,
            data_len = data.len(),
            "process_function_section"
        );

        // Reserve space for functions
        for i in 0..count {
            let (type_idx, bytes_read) = read_leb128_u32(data, offset)?;
            offset += bytes_read;

            #[cfg(feature = "tracing")]
            trace!(
                func_index = i,
                type_idx = type_idx,
                "process_function_section: function parsed"
            );

            // Create function with empty body for now
            let func = Function {
                type_idx,
                locals: alloc::vec::Vec::new(),
                code: alloc::vec::Vec::new(),
            };

            self.module.functions.push(func);
        }

        Ok(offset)
    }

    /// Process table section
    fn process_table_section(&mut self, data: &[u8]) -> Result<usize> {
        use wrt_format::binary::read_leb128_u32;
        use wrt_foundation::types::{Limits, RefType, TableType};

        let mut offset = 0;
        let (count, bytes_read) = read_leb128_u32(data, offset)?;
        offset += bytes_read;

        #[cfg(feature = "tracing")]
        trace!(count = count, "process_table_section");

        // Process each table one at a time
        for i in 0..count {
            // Parse ref_type (element type)
            // WebAssembly 2.0 tables can have init expressions with 0x40 0x00 prefix
            if offset >= data.len() {
                return Err(Error::parse_error("Unexpected end of table section"));
            }
            let first_byte = data[offset];

            // Check for table with init expression (0x40 0x00 prefix)
            let has_init_expr = first_byte == 0x40;
            if has_init_expr {
                offset += 1;
                // Read the reserved 0x00 byte
                if offset >= data.len() || data[offset] != 0x00 {
                    return Err(Error::parse_error(
                        "Expected 0x00 after 0x40 in table with init expr",
                    ));
                }
                offset += 1;
            }

            // Now parse the ref_type
            if offset >= data.len() {
                return Err(Error::parse_error(
                    "Unexpected end of table section (ref_type)",
                ));
            }
            let ref_type_byte = data[offset];
            offset += 1;

            let element_type = match ref_type_byte {
                0x70 => RefType::Funcref,   // funcref
                0x6F => RefType::Externref, // externref
                _ => {
                    return Err(Error::parse_error("Unknown table element type"));
                },
            };

            // Parse limits (flags + min, optional max)
            if offset >= data.len() {
                return Err(Error::parse_error("Unexpected end of table limits"));
            }
            let flags = data[offset];
            offset += 1;

            let (min, bytes_read) = read_leb128_u32(data, offset)?;
            offset += bytes_read;

            let max = if flags & 0x01 != 0 {
                let (max_val, bytes_read) = read_leb128_u32(data, offset)?;
                offset += bytes_read;
                Some(max_val)
            } else {
                None
            };

            // Parse and validate init expression if present (ends with 0x0B)
            if has_init_expr {
                // Count global imports - at table section time, only imported globals exist
                use wrt_format::module::ImportDesc;
                let num_global_imports = self
                    .module
                    .imports
                    .iter()
                    .filter(|imp| matches!(imp.desc, ImportDesc::Global(..)))
                    .count();

                // Scan for the end opcode (0x0B), handling nested blocks
                let mut block_depth = 0u32;
                loop {
                    if offset >= data.len() {
                        return Err(Error::parse_error(
                            "Unexpected end of table init expression",
                        ));
                    }
                    let opcode = data[offset];
                    offset += 1;

                    match opcode {
                        // Block-starting opcodes
                        0x02 | 0x03 | 0x04 | 0x05 | 0x06 | 0x11 => {
                            block_depth += 1;
                        },
                        // End opcode
                        0x0B => {
                            if block_depth == 0 {
                                break;
                            }
                            block_depth -= 1;
                        },
                        // global.get - validate global index
                        0x23 => {
                            // Read global index (LEB128)
                            let (global_idx, bytes_read) = read_leb128_u32(data, offset)?;
                            offset += bytes_read;
                            // At table section time, only imported globals are available
                            // Defined globals (section 9) haven't been parsed yet
                            if global_idx as usize >= num_global_imports {
                                return Err(Error::validation_error("unknown global"));
                            }
                        },
                        // Skip LEB128 immediates for common opcodes
                        0x41 => {
                            // i32.const - skip LEB128
                            while offset < data.len() && (data[offset] & 0x80) != 0 {
                                offset += 1;
                            }
                            if offset < data.len() {
                                offset += 1;
                            }
                        },
                        0x42 => {
                            // i64.const - skip LEB128
                            while offset < data.len() && (data[offset] & 0x80) != 0 {
                                offset += 1;
                            }
                            if offset < data.len() {
                                offset += 1;
                            }
                        },
                        0xD0 => {
                            // ref.null - skip heap type byte
                            if offset < data.len() {
                                offset += 1;
                            }
                        },
                        _ => {
                            // Other opcodes - continue scanning
                        },
                    }
                }
                #[cfg(feature = "tracing")]
                trace!(table_index = i, "table has init expression (validated)");
            }

            #[cfg(feature = "tracing")]
            trace!(table_index = i, element_type = ?element_type, min = min, max = ?max, "table parsed");

            // Create table type and add to module
            let table_type = TableType::new(element_type, Limits { min, max });
            self.module.tables.push(table_type);

            #[cfg(feature = "tracing")]
            trace!(
                table_index = i,
                total_tables = self.module.tables.len(),
                "table added"
            );
        }

        Ok(offset)
    }

    /// Process memory section
    fn process_memory_section(&mut self, data: &[u8]) -> Result<usize> {
        use wrt_format::{read_leb128_u32, read_leb128_u64};

        let mut offset = 0;
        let (count, bytes_read) = read_leb128_u32(data, offset)?;
        offset += bytes_read;

        #[cfg(feature = "tracing")]
        trace!(count = count, "process_memory_section");

        // WebAssembly core spec: only one memory is allowed without the multi-memory proposal
        // Total memories = imported memories + defined memories
        let total_memories = self.num_memory_imports + count as usize;
        if total_memories > 1 {
            return Err(Error::validation_error("multiple memories"));
        }

        // Process each memory one at a time
        for i in 0..count {
            // Parse limits flag (0x00 = min only, 0x01 = min and max, 0x03 = min/max/shared)
            if offset >= data.len() {
                return Err(Error::parse_error(
                    "Memory section truncated: missing limits flag",
                ));
            }
            let flags = data[offset];
            offset += 1;

            // Check for memory64 flag (bit 2)
            let is_memory64 = (flags & 0x04) != 0;

            // WebAssembly spec: memory size must be at most 65536 pages (4GB)
            // for non-memory64 memories (and memory64 has its own limit)
            const MAX_MEMORY_PAGES: u32 = 65536;

            // Parse limits - memory64 uses u64, regular memory uses u32
            let (min, max) = if is_memory64 {
                let (min64, bytes_read) = read_leb128_u64(data, offset)?;
                offset += bytes_read;

                let max64 = if flags & 0x01 != 0 {
                    let (max_val, bytes_read) = read_leb128_u64(data, offset)?;
                    offset += bytes_read;
                    Some(max_val)
                } else {
                    None
                };

                // Validate memory64 limits (still have a limit, though higher)
                // For non-memory64 tests, values > 65536 pages should fail
                if min64 > MAX_MEMORY_PAGES as u64 {
                    return Err(Error::validation_error(
                        "memory size must be at most 65536 pages (4 GiB)",
                    ));
                }
                if let Some(max64) = max64 {
                    if max64 > MAX_MEMORY_PAGES as u64 {
                        return Err(Error::validation_error(
                            "memory size must be at most 65536 pages (4 GiB)",
                        ));
                    }
                }

                (min64 as u32, max64.map(|v| v as u32))
            } else {
                let (min, bytes_read) = read_leb128_u32(data, offset)?;
                offset += bytes_read;

                let max = if flags & 0x01 != 0 {
                    let (max_val, bytes_read) = read_leb128_u32(data, offset)?;
                    offset += bytes_read;
                    Some(max_val)
                } else {
                    None
                };
                (min, max)
            };

            let shared = (flags & 0x02) != 0;

            // WebAssembly threads proposal: shared memory must have a maximum
            if shared && max.is_none() {
                return Err(Error::validation_error("shared memory must have maximum"));
            }

            if min > MAX_MEMORY_PAGES {
                return Err(Error::validation_error(
                    "memory size must be at most 65536 pages (4 GiB)",
                ));
            }
            if let Some(max_val) = max {
                if max_val > MAX_MEMORY_PAGES {
                    return Err(Error::validation_error(
                        "memory size must be at most 65536 pages (4 GiB)",
                    ));
                }
            }

            // Create memory type
            let memory_type = wrt_foundation::types::MemoryType {
                limits: wrt_foundation::types::Limits { min, max },
                shared,
                memory64: is_memory64,
            };

            // Add to module
            self.module.memories.push(memory_type);

            #[cfg(feature = "tracing")]
            trace!(
                memory_index = i,
                total_memories = self.module.memories.len(),
                "memory added"
            );
        }

        Ok(offset)
    }

    /// Process global section
    fn process_global_section(&mut self, data: &[u8]) -> Result<usize> {
        use wrt_format::binary::read_leb128_u32;
        use wrt_format::module::Global;
        use wrt_format::types::FormatGlobalType;
        use wrt_foundation::types::ValueType;

        let (count, mut offset) = read_leb128_u32(data, 0)?;

        #[cfg(feature = "tracing")]
        trace!(
            count = count,
            data_len = data.len(),
            offset = offset,
            "process_global_section"
        );

        for i in 0..count {
            #[cfg(feature = "tracing")]
            trace!(global_index = i, offset = offset, "parsing global");

            // Parse global type: value_type + mutability
            if offset >= data.len() {
                return Err(Error::parse_error("Unexpected end of global type"));
            }

            // Parse value type using full GC-aware parser
            let (value_type, new_offset) = self.parse_value_type(data, offset)?;
            offset = new_offset;

            // Parse mutability (0x00 = const, 0x01 = var)
            if offset >= data.len() {
                return Err(Error::parse_error("Unexpected end of global mutability"));
            }
            let mutability_byte = data[offset];
            // WebAssembly spec: only 0x00 (immutable) and 0x01 (mutable) are valid
            if mutability_byte != 0x00 && mutability_byte != 0x01 {
                return Err(Error::parse_error("malformed mutability"));
            }
            let mutable = mutability_byte == 0x01;
            offset += 1;

            // Parse init expression - must properly parse opcodes and their arguments
            // Cannot just scan for 0x0b because it can appear as a value (e.g., i32.const 11)
            let init_start = offset;

            // Parse init expression by understanding opcodes
            loop {
                if offset >= data.len() {
                    return Err(Error::parse_error("Init expression missing end marker"));
                }

                let opcode = data[offset];
                offset += 1;

                match opcode {
                    0x0b => {
                        // End marker - we're done
                        break;
                    },
                    0x41 => {
                        // i32.const - followed by LEB128 i32
                        let (_, bytes_read) = wrt_format::binary::read_leb128_i32(data, offset)?;
                        offset += bytes_read;
                    },
                    0x42 => {
                        // i64.const - followed by LEB128 i64
                        let (_, bytes_read) = wrt_format::binary::read_leb128_i64(data, offset)?;
                        offset += bytes_read;
                    },
                    0x43 => {
                        // f32.const - followed by 4 bytes
                        offset += 4;
                    },
                    0x44 => {
                        // f64.const - followed by 8 bytes
                        offset += 8;
                    },
                    0x23 => {
                        // global.get - followed by LEB128 global index
                        let (_, bytes_read) = wrt_format::binary::read_leb128_u32(data, offset)?;
                        offset += bytes_read;
                    },
                    0xd0 => {
                        // ref.null - followed by heap type (single byte for common types)
                        if offset < data.len() {
                            offset += 1; // Skip the heap type byte
                        }
                    },
                    0xd2 => {
                        // ref.func - followed by LEB128 func index
                        let (_, bytes_read) = wrt_format::binary::read_leb128_u32(data, offset)?;
                        offset += bytes_read;
                    },
                    _ => {
                        // Unknown opcode in init expression - skip to find 0x0b
                        // This is a fallback for any opcodes we don't handle
                        #[cfg(feature = "tracing")]
                        trace!(
                            opcode = opcode,
                            offset = offset - 1,
                            "global: unknown init opcode"
                        );
                        // Continue to next byte
                    },
                }
            }

            // Extract init expression bytes (including the 0x0b end marker)
            #[cfg(feature = "std")]
            let init_bytes = data[init_start..offset].to_vec();
            #[cfg(not(feature = "std"))]
            let init_bytes = {
                use wrt_foundation::safe_memory::NoStdProvider;
                let mut bounded = wrt_foundation::BoundedVec::<u8, 1024, NoStdProvider<8192>>::new(
                    NoStdProvider::default(),
                )
                .map_err(|_| Error::parse_error("Failed to allocate init expression"))?;
                for &byte in &data[init_start..offset] {
                    bounded
                        .push(byte)
                        .map_err(|_| Error::parse_error("Init expression too large"))?;
                }
                bounded
            };

            let global_type = FormatGlobalType {
                value_type,
                mutable,
            };

            let global = Global {
                global_type,
                init: init_bytes,
            };

            self.module.globals.push(global);

            #[cfg(feature = "tracing")]
            trace!(global_index = i, value_type = ?value_type, mutable = mutable, init_len = offset - init_start, "global parsed");
        }

        Ok(offset)
    }

    /// Process export section
    fn process_export_section(&mut self, data: &[u8]) -> Result<usize> {
        use wrt_format::binary::read_leb128_u32;

        use crate::optimized_string::validate_utf8_name;

        let (count, mut offset) = read_leb128_u32(data, 0)?;

        // Validate export count against platform limits
        if count as usize > limits::MAX_EXPORTS {
            return Err(Error::parse_error(
                "Module exceeds maximum export count for platform",
            ));
        }

        #[cfg(feature = "tracing")]
        trace!(
            count = count,
            offset = offset,
            data_len = data.len(),
            "process_export_section"
        );

        for i in 0..count {
            // Parse export name - use validate_utf8_name for std builds to avoid
            // BoundedString issues
            #[cfg(feature = "tracing")]
            trace!(export_index = i, offset = offset, "parsing export name");
            let (export_name_str, new_offset) = validate_utf8_name(data, offset)?;
            #[cfg(feature = "tracing")]
            trace!(
                export_index = i,
                name = export_name_str,
                new_offset = new_offset,
                "export name parsed"
            );
            offset = new_offset;

            if offset >= data.len() {
                return Err(Error::parse_error("Unexpected end of export kind"));
            }

            // Parse export kind
            let kind_byte = data[offset];
            offset += 1;

            #[cfg(feature = "tracing")]
            trace!(
                export_index = i,
                kind_byte = kind_byte,
                offset = offset,
                "export kind parsed"
            );
            let kind = match kind_byte {
                0x00 => wrt_format::module::ExportKind::Function,
                0x01 => wrt_format::module::ExportKind::Table,
                0x02 => wrt_format::module::ExportKind::Memory,
                0x03 => wrt_format::module::ExportKind::Global,
                0x04 => wrt_format::module::ExportKind::Tag,
                _ => {
                    #[cfg(feature = "tracing")]
                    trace!(kind_byte = kind_byte, "invalid export kind");
                    return Err(Error::parse_error("Invalid export kind"));
                },
            };

            // Parse export index
            let (index, bytes_consumed) = read_leb128_u32(data, offset)?;
            offset += bytes_consumed;

            #[cfg(feature = "tracing")]
            trace!(
                export_index = i,
                index = index,
                offset = offset,
                "export index parsed"
            );

            // Add export to module
            #[cfg(feature = "std")]
            {
                self.module.exports.push(wrt_format::module::Export {
                    name: String::from(export_name_str),
                    kind,
                    index,
                });
            }
            #[cfg(not(feature = "std"))]
            {
                use wrt_foundation::BoundedString;

                let name = BoundedString::<1024>::try_from_str(export_name_str)
                    .map_err(|_| wrt_error::Error::parse_error("Export name too long"))?;

                let _ = self.module.exports.push(wrt_format::module::Export { name, kind, index });
            }
        }

        Ok(offset)
    }

    /// Process start section
    fn process_start_section(&mut self, data: &[u8]) -> Result<usize> {
        let (start_idx, bytes_read) = read_leb128_u32(data, 0)?;
        self.module.start = Some(start_idx);
        Ok(bytes_read)
    }

    /// Process element section
    fn process_element_section(&mut self, data: &[u8]) -> Result<usize> {
        use wrt_format::pure_format_types::{PureElementInit, PureElementMode, PureElementSegment};

        let mut offset = 0;
        let (count, bytes_read) = read_leb128_u32(data, offset)?;
        offset += bytes_read;

        #[cfg(feature = "tracing")]
        trace!(
            count = count,
            data_len = data.len(),
            "process_element_section"
        );

        for elem_idx in 0..count {
            // Parse element segment flags (see WebAssembly spec 5.5.10)
            // Flags determine the mode and encoding:
            // 0: Active, implicit table 0, offset expr, funcidx vec
            // 1: Passive, reftype, expression vec
            // 2: Active, explicit table, offset expr, elemkind=0x00, funcidx vec
            // 3: Declarative, reftype, expression vec
            // 4: Active, explicit table, offset expr, funcidx vec
            // 5: Passive, reftype, expression vec
            // 6: Active, explicit table, offset expr, reftype, expression vec
            // 7: Declarative, reftype, expression vec
            let (flags, bytes_read) = read_leb128_u32(data, offset)?;
            offset += bytes_read;

            #[cfg(feature = "tracing")]
            trace!(elem_idx = elem_idx, flags = flags, "element segment flags");

            let (mode, offset_expr_bytes, element_type) = match flags {
                0 => {
                    // Active, table 0, funcref, func indices
                    // Parse offset expression properly (can't just scan for 0x0B as it may appear as a value)
                    let expr_start = offset;
                    offset = find_expression_end(data, offset)?;
                    let offset_expr_bytes: Vec<u8> = data[expr_start..offset].to_vec();
                    #[cfg(feature = "tracing")]
                    trace!(
                        elem_idx = elem_idx,
                        offset_expr_len = offset_expr_bytes.len(),
                        "element: active table 0"
                    );

                    (
                        PureElementMode::Active {
                            table_index: 0,
                            offset_expr_len: offset_expr_bytes.len() as u32,
                        },
                        offset_expr_bytes,
                        wrt_format::types::RefType::Funcref,
                    )
                },
                1 => {
                    // Passive, element type, expressions
                    let elem_type = data[offset];
                    offset += 1;
                    let ref_type = match elem_type {
                        0x70 => wrt_format::types::RefType::Funcref,
                        0x6F => wrt_format::types::RefType::Externref,
                        _ => wrt_format::types::RefType::Funcref,
                    };
                    #[cfg(feature = "tracing")]
                    trace!(elem_idx = elem_idx, ref_type = ?ref_type, "element: passive");
                    (PureElementMode::Passive, Vec::new(), ref_type)
                },
                2 => {
                    // Active, explicit table index, offset expr, elemkind=0x00, funcidx vec
                    // Per WebAssembly spec: flags=2 is "2:flags x:tableidx e:expr 0x00:elemkind y*:vec(funcidx)"

                    // Parse table index
                    let (table_index, bytes_read) = read_leb128_u32(data, offset)?;
                    offset += bytes_read;

                    // Parse offset expression properly (can't just scan for 0x0B as it may appear as a value)
                    let expr_start = offset;
                    offset = find_expression_end(data, offset)?;
                    let offset_expr_bytes: Vec<u8> = data[expr_start..offset].to_vec();

                    // Parse elemkind (must be 0x00 = funcref)
                    let _elemkind = data[offset];
                    offset += 1;

                    #[cfg(feature = "tracing")]
                    trace!(
                        elem_idx = elem_idx,
                        table_index = table_index,
                        offset_expr_len = offset_expr_bytes.len(),
                        "element: active legacy funcidx"
                    );

                    (
                        PureElementMode::Active {
                            table_index,
                            offset_expr_len: offset_expr_bytes.len() as u32,
                        },
                        offset_expr_bytes,
                        wrt_format::types::RefType::Funcref,
                    )
                },
                3 => {
                    // Declarative, element type, expressions
                    let elem_type = data[offset];
                    offset += 1;
                    let ref_type = match elem_type {
                        0x70 => wrt_format::types::RefType::Funcref,
                        0x6F => wrt_format::types::RefType::Externref,
                        _ => wrt_format::types::RefType::Funcref,
                    };
                    #[cfg(feature = "tracing")]
                    trace!(elem_idx = elem_idx, ref_type = ?ref_type, "element: declarative");
                    (PureElementMode::Declared, Vec::new(), ref_type)
                },
                4 => {
                    // Active, table 0 (implicit), offset expr, vec<expr>
                    // Per WebAssembly spec: flags=4 has NO explicit table index (table 0 implicit)
                    // Parse offset expression properly (can't just scan for 0x0B as it may appear as a value)
                    let expr_start = offset;
                    offset = find_expression_end(data, offset)?;
                    let offset_expr_bytes: Vec<u8> = data[expr_start..offset].to_vec();
                    #[cfg(feature = "tracing")]
                    trace!(
                        elem_idx = elem_idx,
                        offset_expr_len = offset_expr_bytes.len(),
                        "element: active expressions table 0"
                    );

                    (
                        PureElementMode::Active {
                            table_index: 0,
                            offset_expr_len: offset_expr_bytes.len() as u32,
                        },
                        offset_expr_bytes,
                        wrt_format::types::RefType::Funcref,
                    )
                },
                5 => {
                    // Passive, expressions with type
                    let ref_type_byte = data[offset];
                    offset += 1;
                    let ref_type = match ref_type_byte {
                        0x70 => wrt_format::types::RefType::Funcref,
                        0x6F => wrt_format::types::RefType::Externref,
                        _ => wrt_format::types::RefType::Funcref,
                    };
                    #[cfg(feature = "tracing")]
                    trace!(elem_idx = elem_idx, ref_type = ?ref_type, "element: passive with type");
                    (PureElementMode::Passive, Vec::new(), ref_type)
                },
                6 => {
                    // Active explicit table, expressions with type
                    // Format: table_idx offset_expr ref_type vec(expr)
                    let (table_index, bytes_read) = read_leb128_u32(data, offset)?;
                    offset += bytes_read;

                    // Parse offset expression properly (can't just scan for 0x0B as it may appear as a value)
                    let expr_start = offset;
                    offset = find_expression_end(data, offset)?;
                    let offset_expr_bytes: Vec<u8> = data[expr_start..offset].to_vec();

                    // Parse ref_type (comes after offset expression, before items)
                    if offset >= data.len() {
                        return Err(Error::parse_error(
                            "Unexpected end of element segment (ref_type)",
                        ));
                    }
                    let ref_type_byte = data[offset];
                    offset += 1;
                    let ref_type = match ref_type_byte {
                        0x70 => wrt_format::types::RefType::Funcref,
                        0x6F => wrt_format::types::RefType::Externref,
                        _ => wrt_format::types::RefType::Funcref,
                    };

                    #[cfg(feature = "tracing")]
                    trace!(elem_idx = elem_idx, table_index = table_index, offset_expr_len = offset_expr_bytes.len(), ref_type = ?ref_type, "element: active with expressions");

                    (
                        PureElementMode::Active {
                            table_index,
                            offset_expr_len: offset_expr_bytes.len() as u32,
                        },
                        offset_expr_bytes,
                        ref_type,
                    )
                },
                7 => {
                    // Declarative, expressions with type
                    let ref_type_byte = data[offset];
                    offset += 1;
                    let ref_type = match ref_type_byte {
                        0x70 => wrt_format::types::RefType::Funcref,
                        0x6F => wrt_format::types::RefType::Externref,
                        _ => wrt_format::types::RefType::Funcref,
                    };
                    #[cfg(feature = "tracing")]
                    trace!(elem_idx = elem_idx, ref_type = ?ref_type, "element: declarative with type");
                    (PureElementMode::Declared, Vec::new(), ref_type)
                },
                _ => {
                    return Err(Error::parse_error("Unknown element segment flags"));
                },
            };

            // Parse element items
            let (item_count, bytes_read) = read_leb128_u32(data, offset)?;
            offset += bytes_read;

            // Validate element count against platform limits before allocation
            if item_count as usize > limits::MAX_ELEMENT_ITEMS {
                return Err(Error::parse_error(
                    "Element segment exceeds maximum item count for platform",
                ));
            }

            #[cfg(feature = "tracing")]
            trace!(
                elem_idx = elem_idx,
                item_count = item_count,
                "element items"
            );

            let init_data = if flags == 0 || flags == 1 || flags == 2 || flags == 3 {
                // Function indices format (flags 0, 1, 2, 3 use elemkind + funcidx)
                #[cfg(feature = "allocation-tracing")]
                trace_alloc!(
                    AllocationPhase::Decode,
                    "streaming_decoder:elem_func_indices",
                    "func_indices",
                    item_count as usize
                );

                let mut func_indices = Vec::with_capacity(item_count as usize);
                for i in 0..item_count {
                    let (func_idx, bytes_read) = read_leb128_u32(data, offset)?;
                    offset += bytes_read;
                    func_indices.push(func_idx);
                    #[cfg(feature = "tracing")]
                    if i < 5 || i == item_count - 1 {
                        trace!(
                            item_index = i,
                            func_idx = func_idx,
                            "element item: func index"
                        );
                    }
                }
                PureElementInit::FunctionIndices(func_indices)
            } else {
                // Expression format (flags 4, 5, 6, 7 use reftype + expressions)
                #[cfg(feature = "allocation-tracing")]
                trace_alloc!(
                    AllocationPhase::Decode,
                    "streaming_decoder:elem_expressions",
                    "expr_bytes",
                    item_count as usize
                );

                let mut expr_bytes = Vec::with_capacity(item_count as usize);
                for i in 0..item_count {
                    // Parse item expression properly (can't just scan for 0x0B as it may appear as a value)
                    let expr_start = offset;
                    offset = find_expression_end(data, offset)?;
                    let expr_data: Vec<u8> = data[expr_start..offset].to_vec();
                    #[cfg(feature = "tracing")]
                    if i < 5 {
                        trace!(
                            item_index = i,
                            expr_len = expr_data.len(),
                            "element item: expression"
                        );
                    }
                    expr_bytes.push(expr_data);
                }
                PureElementInit::ExpressionBytes(expr_bytes)
            };

            let elem_segment = PureElementSegment {
                mode,
                element_type,
                offset_expr_bytes,
                init_data,
            };

            self.module.elements.push(elem_segment);
            #[cfg(feature = "tracing")]
            trace!(
                elem_idx = elem_idx,
                total_elements = self.module.elements.len(),
                "element added"
            );
        }

        #[cfg(feature = "tracing")]
        trace!(count = count, "process_element_section: complete");

        Ok(offset)
    }

    /// Process code section
    fn process_code_section(&mut self, data: &[u8]) -> Result<usize> {
        let mut offset = 0;
        let (count, bytes_read) = read_leb128_u32(data, offset)?;
        offset += bytes_read;

        // Store code count for cross-section validation
        self.code_count = Some(count);

        // Validate function and code section counts match
        if let Some(func_count) = self.function_count {
            if func_count != count {
                return Err(Error::parse_error(
                    "function and code section have inconsistent lengths",
                ));
            }
        }

        #[cfg(feature = "tracing")]
        trace!(count = count, "process_code_section");

        // Code bodies are for module-defined functions only (not imports)
        // So code[i] goes to function[num_imports + i]
        let num_imports = self.num_function_imports;

        #[cfg(feature = "tracing")]
        trace!(
            num_imports = num_imports,
            total_functions = self.module.functions.len(),
            "code section function mapping"
        );

        // Process each function body one at a time
        for i in 0..count {
            let (body_size, bytes_read) = read_leb128_u32(data, offset)?;
            offset += bytes_read;

            // Validate function body size against platform limits
            if body_size as usize > limits::MAX_FUNCTION_CODE_SIZE {
                return Err(Error::parse_error(
                    "Function body exceeds maximum code size for platform",
                ));
            }

            let body_start = offset;
            let body_end = offset + body_size as usize;
            if body_end > data.len() {
                return Err(Error::parse_error("Function body extends beyond section"));
            }

            // Parse locals first (they come before instructions in the function body)
            let mut body_offset = 0;
            let (local_count, local_bytes) =
                read_leb128_u32(&data[body_start..body_end], body_offset)?;
            body_offset += local_bytes;

            #[cfg(feature = "tracing")]
            trace!(
                func_index = i,
                local_groups = local_count,
                "code section: function locals"
            );

            // Code section index i corresponds to module-defined function at index (num_imports + i)
            let func_index = num_imports + i as usize;
            if let Some(func) = self.module.functions.get_mut(func_index) {
                // Parse local variable declarations
                for _ in 0..local_count {
                    let (count, bytes) = read_leb128_u32(&data[body_start..body_end], body_offset)?;
                    body_offset += bytes;

                    if body_offset >= body_size as usize {
                        return Err(Error::parse_error("Unexpected end of function body"));
                    }

                    let value_type = data[body_start + body_offset];
                    body_offset += 1;

                    // Convert to ValueType and add to locals
                    let vt = match value_type {
                        0x7F => wrt_foundation::types::ValueType::I32,
                        0x7E => wrt_foundation::types::ValueType::I64,
                        0x7D => wrt_foundation::types::ValueType::F32,
                        0x7C => wrt_foundation::types::ValueType::F64,
                        0x7B => wrt_foundation::types::ValueType::V128,
                        0x70 => wrt_foundation::types::ValueType::FuncRef,
                        0x6F => wrt_foundation::types::ValueType::ExternRef,
                        0x69 => wrt_foundation::types::ValueType::ExnRef,
                        _ => return Err(Error::parse_error("Invalid local type")),
                    };

                    // Validate total locals against platform limits before allocation
                    let new_total = func.locals.len() + count as usize;
                    if new_total > limits::MAX_FUNCTION_LOCALS {
                        return Err(Error::parse_error(
                            "Function exceeds maximum local count for platform",
                        ));
                    }

                    #[cfg(feature = "allocation-tracing")]
                    trace_alloc!(
                        AllocationPhase::Decode,
                        "streaming_decoder:func_locals",
                        "locals",
                        count as usize
                    );

                    // Add 'count' locals of this type
                    for _ in 0..count {
                        func.locals.push(vt);
                    }
                }

                // Now copy only the instruction bytes (after locals, before the implicit 'end')
                let instructions_start = body_start + body_offset;
                let instructions_data = &data[instructions_start..body_end];

                #[cfg(feature = "allocation-tracing")]
                trace_alloc!(
                    AllocationPhase::Decode,
                    "streaming_decoder:func_code",
                    "code_bytes",
                    instructions_data.len()
                );

                func.code.extend_from_slice(instructions_data);

                #[cfg(feature = "tracing")]
                trace!(
                    func_index = i,
                    locals_count = func.locals.len(),
                    instruction_bytes = func.code.len(),
                    "code section: function parsed"
                );
            }

            offset = body_end;
        }

        Ok(offset)
    }

    /// Process data section
    fn process_data_section(&mut self, data: &[u8]) -> Result<usize> {
        use wrt_format::pure_format_types::{PureDataMode, PureDataSegment};

        let mut offset = 0;
        let (count, bytes_read) = read_leb128_u32(data, offset)?;
        offset += bytes_read;

        // Store data section count for cross-section validation
        self.data_section_count = Some(count);

        // Validate data count section matches data section if present
        if let Some(data_count) = self.data_count_value {
            if data_count != count {
                return Err(Error::parse_error(
                    "data count and data section have inconsistent lengths",
                ));
            }
        }

        // Validate data segment count against platform limits
        if count as usize > limits::MAX_DATA_SEGMENTS {
            return Err(Error::parse_error(
                "Module exceeds maximum data segment count for platform",
            ));
        }

        #[cfg(feature = "tracing")]
        trace!(count = count, data_len = data.len(), "process_data_section");

        for i in 0..count {
            if offset >= data.len() {
                return Err(Error::parse_error("Unexpected end of data section"));
            }

            let tag = data[offset];
            offset += 1;

            let segment = match tag {
                // Active data segment with implicit memory 0
                0x00 => {
                    // Parse offset expression - find the end (0x0B terminator)
                    let expr_start = offset;
                    let mut depth = 1u32;
                    while offset < data.len() {
                        let opcode = data[offset];
                        offset += 1;

                        match opcode {
                            0x02..=0x04 => depth += 1,
                            0x0B => {
                                depth = depth.saturating_sub(1);
                                if depth == 0 {
                                    break;
                                }
                            },
                            0x41 | 0x42 | 0x23 => {
                                // i32.const, i64.const, global.get - skip LEB128
                                while offset < data.len() && data[offset] & 0x80 != 0 {
                                    offset += 1;
                                }
                                if offset < data.len() {
                                    offset += 1;
                                }
                            },
                            _ => {},
                        }
                    }
                    let offset_expr_bytes = data[expr_start..offset].to_vec();

                    // Parse data byte count and data
                    let (data_len, bytes_read) = read_leb128_u32(data, offset)?;
                    offset += bytes_read;

                    if offset + data_len as usize > data.len() {
                        return Err(Error::parse_error("Data segment data exceeds bounds"));
                    }

                    let data_bytes = data[offset..offset + data_len as usize].to_vec();
                    offset += data_len as usize;

                    #[cfg(feature = "tracing")]
                    trace!(
                        segment_index = i,
                        memory_index = 0,
                        offset_expr_len = offset_expr_bytes.len(),
                        data_len = data_bytes.len(),
                        "data segment: active"
                    );

                    PureDataSegment {
                        mode: PureDataMode::Active {
                            memory_index: 0,
                            offset_expr_len: offset_expr_bytes.len() as u32,
                        },
                        offset_expr_bytes,
                        data_bytes,
                    }
                },
                // Passive data segment
                0x01 => {
                    // Parse data byte count and data
                    let (data_len, bytes_read) = read_leb128_u32(data, offset)?;
                    offset += bytes_read;

                    if offset + data_len as usize > data.len() {
                        return Err(Error::parse_error("Data segment data exceeds bounds"));
                    }

                    let data_bytes = data[offset..offset + data_len as usize].to_vec();
                    offset += data_len as usize;

                    #[cfg(feature = "tracing")]
                    trace!(
                        segment_index = i,
                        data_len = data_bytes.len(),
                        "data segment: passive"
                    );

                    PureDataSegment {
                        mode: PureDataMode::Passive,
                        offset_expr_bytes: Vec::new(),
                        data_bytes,
                    }
                },
                // Active data segment with explicit memory index
                0x02 => {
                    // Parse memory index
                    let (memory_index, bytes_read) = read_leb128_u32(data, offset)?;
                    offset += bytes_read;

                    // Parse offset expression
                    let expr_start = offset;
                    let mut depth = 1u32;
                    while offset < data.len() {
                        let opcode = data[offset];
                        offset += 1;

                        match opcode {
                            0x02..=0x04 => depth += 1,
                            0x0B => {
                                depth = depth.saturating_sub(1);
                                if depth == 0 {
                                    break;
                                }
                            },
                            0x41 | 0x42 | 0x23 => {
                                while offset < data.len() && data[offset] & 0x80 != 0 {
                                    offset += 1;
                                }
                                if offset < data.len() {
                                    offset += 1;
                                }
                            },
                            _ => {},
                        }
                    }
                    let offset_expr_bytes = data[expr_start..offset].to_vec();

                    // Parse data byte count and data
                    let (data_len, bytes_read) = read_leb128_u32(data, offset)?;
                    offset += bytes_read;

                    if offset + data_len as usize > data.len() {
                        return Err(Error::parse_error("Data segment data exceeds bounds"));
                    }

                    let data_bytes = data[offset..offset + data_len as usize].to_vec();
                    offset += data_len as usize;

                    #[cfg(feature = "tracing")]
                    trace!(
                        segment_index = i,
                        memory_index = memory_index,
                        offset_expr_len = offset_expr_bytes.len(),
                        data_len = data_bytes.len(),
                        "data segment: active explicit"
                    );

                    PureDataSegment {
                        mode: PureDataMode::Active {
                            memory_index,
                            offset_expr_len: offset_expr_bytes.len() as u32,
                        },
                        offset_expr_bytes,
                        data_bytes,
                    }
                },
                _ => {
                    return Err(Error::parse_error("Invalid data segment tag"));
                },
            };

            // Add segment to module
            #[cfg(feature = "std")]
            self.module.data.push(segment);

            #[cfg(not(feature = "std"))]
            {
                let _ = self.module.data.push(segment);
            }
        }

        #[cfg(feature = "tracing")]
        trace!(
            count = count,
            total_segments = self.module.data.len(),
            "process_data_section: complete"
        );

        Ok(offset)
    }

    /// Process tag section (exception handling proposal)
    /// Tag section ID is 13 (0x0D)
    fn process_tag_section(&mut self, data: &[u8]) -> Result<usize> {
        use wrt_format::binary::read_leb128_u32;

        let mut offset = 0;
        let (count, bytes_read) = read_leb128_u32(data, offset)?;
        offset += bytes_read;

        #[cfg(feature = "tracing")]
        trace!(count = count, "process_tag_section");

        for _i in 0..count {
            if offset >= data.len() {
                return Err(Error::parse_error("Unexpected end of tag section"));
            }

            // Each tag has: attribute (u8, must be 0) + type_idx (LEB128 u32)
            let attribute = data[offset];
            offset += 1;

            // Validate attribute - must be 0 (exception)
            if attribute != 0 {
                return Err(Error::validation_error("Invalid tag attribute"));
            }

            let (type_idx, bytes_read) = read_leb128_u32(data, offset)?;
            offset += bytes_read;

            // Validate type index
            if type_idx as usize >= self.module.types.len() {
                return Err(Error::validation_error("Invalid tag type index"));
            }

            let tag = TagType {
                attribute,
                type_idx,
            };

            #[cfg(feature = "std")]
            self.module.tags.push(tag);
            #[cfg(not(feature = "std"))]
            self.module
                .tags
                .push(tag)
                .map_err(|_| Error::resource_exhausted("Too many tags"))?;

            #[cfg(feature = "tracing")]
            trace!(tag_idx = _i, type_idx = type_idx, "tag");
        }

        Ok(offset)
    }

    /// Process data count section
    fn process_data_count_section(&mut self, data: &[u8]) -> Result<usize> {
        // Parse and store the data count value for cross-section validation
        let (count, bytes_read) = read_leb128_u32(data, 0)?;
        self.data_count_value = Some(count);
        Ok(bytes_read)
    }

    /// Process custom section
    /// Returns the number of bytes consumed (entire section for custom sections).
    fn process_custom_section(&mut self, data: &[u8]) -> Result<usize> {
        // Custom sections are skipped - consume all bytes
        Ok(data.len())
    }

    /// Perform cross-section validation at end of decoding
    fn validate_cross_section_counts(&self) -> Result<()> {
        // Validate function and code section counts match
        // Both must be present and equal, or both must be absent
        match (self.function_count, self.code_count) {
            (Some(func), Some(code)) if func != code => {
                return Err(Error::parse_error(
                    "function and code section have inconsistent lengths",
                ));
            }
            (Some(_), None) => {
                // Function section exists but no code section - this is invalid
                return Err(Error::parse_error(
                    "function and code section have inconsistent lengths",
                ));
            }
            (None, Some(_)) => {
                // Code section exists but no function section - this is invalid
                return Err(Error::parse_error(
                    "function and code section have inconsistent lengths",
                ));
            }
            _ => {}
        }

        // Validate data count section matches data section if present
        if let Some(data_count) = self.data_count_value {
            match self.data_section_count {
                Some(data_section) if data_count != data_section => {
                    return Err(Error::parse_error(
                        "data count and data section have inconsistent lengths",
                    ));
                }
                None => {
                    // Data count section present but no data section
                    // This is valid if data_count is 0
                    if data_count != 0 {
                        return Err(Error::parse_error(
                            "data count and data section have inconsistent lengths",
                        ));
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Finish decoding and return the module
    /// Finish decoding and return the module (std version)
    #[cfg(feature = "std")]
    pub fn finish(self) -> Result<WrtModule> {
        // Perform cross-section validation
        self.validate_cross_section_counts()?;

        #[cfg(feature = "tracing")]
        trace!(
            imports_count = self.module.imports.len(),
            "StreamingDecoder::finish"
        );
        Ok(self.module)
    }

    /// Finish decoding and return the module (no_std version)
    #[cfg(not(feature = "std"))]
    pub fn finish(self) -> Result<WrtModule<NoStdProvider<8192>>> {
        // Perform cross-section validation
        self.validate_cross_section_counts()?;

        Ok(self.module)
    }
}

/// Decode a WebAssembly module using streaming processing (std version)
#[cfg(feature = "std")]
pub fn decode_module_streaming(binary: &[u8]) -> Result<WrtModule> {
    // Enter module scope for bump allocator - all Vec allocations will be tracked
    let _scope = wrt_foundation::capabilities::MemoryFactory::enter_module_scope(
        wrt_foundation::budget_aware_provider::CrateId::Decoder,
    )?;

    let mut decoder = StreamingDecoder::new(binary)?;
    decoder.decode_header()?;

    // Process all sections
    while decoder.process_next_section()? {
        // Process sections one at a time
    }

    decoder.finish()
    // Scope drops here, memory available for reuse
}

/// Decode a WebAssembly module using streaming processing (no_std version)
#[cfg(not(feature = "std"))]
pub fn decode_module_streaming(binary: &[u8]) -> Result<WrtModule<NoStdProvider<8192>>> {
    let mut decoder = StreamingDecoder::new(binary)?;

    // First validate and decode the header
    decoder.decode_header()?;

    // Process sections one at a time
    while decoder.process_next_section()? {
        // Each section is processed with minimal memory usage
    }

    // Return the completed module
    decoder.finish()
}
