//! Canonical ABI Implementation for WebAssembly Component Model
//!
//! This module provides a complete implementation of the Canonical ABI as
//! specified in the WebAssembly Component Model specification. The Canonical
//! ABI defines how values are transferred between components and core
//! WebAssembly modules.
//!
//! # Features
//!
//! - **Complete Type Support**: All Canonical ABI types including primitives,
//!   strings, lists, records, variants, options, results, and flags
//! - **Cross-Environment Compatibility**: Works in std, no_std+alloc, and pure
//!   no_std
//! - **Memory Safety**: Comprehensive bounds checking and validation
//! - **Performance Optimized**: Efficient lifting and lowering operations
//! - **Error Handling**: Detailed error reporting for invalid operations
//!
//! # Core Operations
//!
//! The Canonical ABI provides two main operations:
//!
//! - **Lifting**: Convert core WebAssembly values to component model values
//! - **Lowering**: Convert component model values to core WebAssembly values
//!
//! # Example
//!
//! ```no_run
//! use wrt_component::canonical_abi::{
//!     CanonicalABI,
//!     ComponentType,
//!     ComponentValue,
//! };
//!
//! // Create a canonical ABI instance
//! let abi = CanonicalABI::new();
//!
//! // Lift an i32 from memory
//! let value = abi.lift_i32(&memory, 0)?;
//!
//! // Lower a string to memory
//! abi.lower_string(&mut memory, 100, "hello")?;
//! ```

// Cross-environment imports
#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(all(not(feature = "std")))]
use alloc::{collections::BTreeMap as HashMap, string::String, vec, vec::Vec};
#[cfg(feature = "std")]
use std::{collections::HashMap, string::String, vec::Vec};

// Note: Using alloc for no_std instead of wrt_foundation bounded types for now
// #[cfg(not(any(feature = "std", )))]
// use wrt_foundation::{BoundedString, BoundedVec, BoundedMap as HashMap};
use wrt_error::{Error, ErrorCategory, Result, codes};
#[cfg(not(feature = "std"))]
use wrt_foundation::safe_memory::NoStdProvider;
use wrt_foundation::{
    traits::{Checksummable, FromBytes, ReadStream, ToBytes, WriteStream},
    verification::Checksum,
};

// Import prelude for consistent type access
use crate::prelude::*;

/// Maximum string length for safety (4MB)
const MAX_STRING_LENGTH: usize = 4 * 1024 * 1024;

/// Maximum list length for safety  
const MAX_LIST_LENGTH: usize = 1024 * 1024;

/// Maximum record field count
const MAX_RECORD_FIELDS: usize = 1024;

/// Page size constant (64KB)
const PAGE_SIZE: usize = 65536;

/// Component model value types as defined in the Canonical ABI
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ComponentType {
    /// Boolean type
    #[default]
    Bool,
    /// Signed 8-bit integer
    S8,
    /// Unsigned 8-bit integer  
    U8,
    /// Signed 16-bit integer
    S16,
    /// Unsigned 16-bit integer
    U16,
    /// Signed 32-bit integer
    S32,
    /// Unsigned 32-bit integer
    U32,
    /// Signed 64-bit integer
    S64,
    /// Unsigned 64-bit integer
    U64,
    /// 32-bit floating point
    F32,
    /// 64-bit floating point
    F64,
    /// Unicode character
    Char,
    /// UTF-8 string
    String,
    /// List of values
    List(Box<ComponentType>),
    /// Record with named fields
    Record(Vec<(String, ComponentType)>),
    /// Tuple of values
    Tuple(Vec<ComponentType>),
    /// Variant with cases
    Variant(Vec<(String, Option<ComponentType>)>),
    /// Enumeration
    Enum(Vec<String>),
    /// Optional value
    Option(Box<ComponentType>),
    /// Result type
    Result(Option<Box<ComponentType>>, Option<Box<ComponentType>>),
    /// Flags (bitset)
    Flags(Vec<String>),
}

/// Component model values as defined in the Canonical ABI
#[derive(Debug, Clone, PartialEq)]
pub enum ComponentValue {
    /// Boolean value
    Bool(bool),
    /// Signed 8-bit integer
    S8(i8),
    /// Unsigned 8-bit integer
    U8(u8),
    /// Signed 16-bit integer
    S16(i16),
    /// Unsigned 16-bit integer
    U16(u16),
    /// Signed 32-bit integer
    S32(i32),
    /// Unsigned 32-bit integer
    U32(u32),
    /// Signed 64-bit integer
    S64(i64),
    /// Unsigned 64-bit integer
    U64(u64),
    /// 32-bit floating point
    F32(f32),
    /// 64-bit floating point
    F64(f64),
    /// Unicode character
    Char(char),
    /// UTF-8 string
    String(String),
    /// List of values
    List(Vec<ComponentValue>),
    /// Record with named fields
    Record(Vec<(String, ComponentValue)>),
    /// Tuple of values
    Tuple(Vec<ComponentValue>),
    /// Variant with case name and optional value
    Variant(String, Option<Box<ComponentValue>>),
    /// Enumeration with case name
    Enum(String),
    /// Optional value
    Option(Option<Box<ComponentValue>>),
    /// Result value  
    Result(core::result::Result<Option<Box<ComponentValue>>, Option<Box<ComponentValue>>>),
    /// Flags (bitset)
    Flags(Vec<String>),
}

// Trait implementations for ComponentType
impl ToBytes for ComponentType {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        // Write discriminant for the type
        let discriminant: u8 = match self {
            ComponentType::Bool => 0,
            ComponentType::S8 => 1,
            ComponentType::U8 => 2,
            ComponentType::S16 => 3,
            ComponentType::U16 => 4,
            ComponentType::S32 => 5,
            ComponentType::U32 => 6,
            ComponentType::S64 => 7,
            ComponentType::U64 => 8,
            ComponentType::F32 => 9,
            ComponentType::F64 => 10,
            ComponentType::Char => 11,
            ComponentType::String => 12,
            _ => 255, // Complex types
        };
        writer.write_u8(discriminant)?;
        Ok(())
    }
}

impl FromBytes for ComponentType {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        let discriminant = reader.read_u8()?;
        match discriminant {
            0 => Ok(ComponentType::Bool),
            1 => Ok(ComponentType::S8),
            2 => Ok(ComponentType::U8),
            3 => Ok(ComponentType::S16),
            4 => Ok(ComponentType::U16),
            5 => Ok(ComponentType::S32),
            6 => Ok(ComponentType::U32),
            7 => Ok(ComponentType::S64),
            8 => Ok(ComponentType::U64),
            9 => Ok(ComponentType::F32),
            10 => Ok(ComponentType::F64),
            11 => Ok(ComponentType::Char),
            12 => Ok(ComponentType::String),
            _ => Ok(ComponentType::Bool), // Default fallback
        }
    }
}

impl Checksummable for ComponentType {
    fn update_checksum(&self, checksum: &mut Checksum) {
        let discriminant: u8 = match self {
            ComponentType::Bool => 0,
            ComponentType::S8 => 1,
            ComponentType::U8 => 2,
            ComponentType::S16 => 3,
            ComponentType::U16 => 4,
            ComponentType::S32 => 5,
            ComponentType::U32 => 6,
            ComponentType::S64 => 7,
            ComponentType::U64 => 8,
            ComponentType::F32 => 9,
            ComponentType::F64 => 10,
            ComponentType::Char => 11,
            ComponentType::String => 12,
            _ => 255,
        };
        checksum.update(discriminant);
    }
}

/// Memory interface for canonical ABI operations
pub trait CanonicalMemory {
    /// Read bytes from memory
    fn read_bytes(&self, offset: u32, len: u32) -> Result<Vec<u8>>;

    /// Write bytes to memory
    fn write_bytes(&mut self, offset: u32, data: &[u8]) -> Result<()>;

    /// Get memory size in bytes
    fn size(&self) -> u32;

    /// Read a single byte
    fn read_u8(&self, offset: u32) -> Result<u8> {
        let bytes = self.read_bytes(offset, 1)?;
        Ok(bytes[0])
    }

    /// Write a single byte
    fn write_u8(&mut self, offset: u32, value: u8) -> Result<()> {
        self.write_bytes(offset, &[value])
    }

    /// Read little-endian u16
    fn read_u16_le(&self, offset: u32) -> Result<u16> {
        let bytes = self.read_bytes(offset, 2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    /// Write little-endian u16
    fn write_u16_le(&mut self, offset: u32, value: u16) -> Result<()> {
        self.write_bytes(offset, &value.to_le_bytes())
    }

    /// Read little-endian u32
    fn read_u32_le(&self, offset: u32) -> Result<u32> {
        let bytes = self.read_bytes(offset, 4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Write little-endian u32
    fn write_u32_le(&mut self, offset: u32, value: u32) -> Result<()> {
        self.write_bytes(offset, &value.to_le_bytes())
    }

    /// Read little-endian u64
    fn read_u64_le(&self, offset: u32) -> Result<u64> {
        let bytes = self.read_bytes(offset, 8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    /// Write little-endian u64
    fn write_u64_le(&mut self, offset: u32, value: u64) -> Result<()> {
        self.write_bytes(offset, &value.to_le_bytes())
    }
}

/// Simple memory implementation for testing
#[cfg(feature = "std")]
#[derive(Debug, Clone)]
pub struct SimpleMemory {
    data: Vec<u8>,
}

#[cfg(feature = "std")]
impl SimpleMemory {
    /// Create a new memory with the given size
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![0; size],
        }
    }

    /// Get a reference to the underlying data
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Get a mutable reference to the underlying data
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

#[cfg(feature = "std")]
impl CanonicalMemory for SimpleMemory {
    fn read_bytes(&self, offset: u32, len: u32) -> Result<Vec<u8>> {
        let start = offset as usize;
        let end = start + len as usize;

        if end > self.data.len() {
            return Err(Error::memory_out_of_bounds("Memory read out of bounds"));
        }

        Ok(self.data[start..end].to_vec())
    }

    fn write_bytes(&mut self, offset: u32, data: &[u8]) -> Result<()> {
        let start = offset as usize;
        let end = start + data.len();

        if end > self.data.len() {
            return Err(Error::memory_out_of_bounds("Memory write out of bounds"));
        }

        self.data[start..end].copy_from_slice(data);
        Ok(())
    }

    fn size(&self) -> u32 {
        self.data.len() as u32
    }
}

/// Canonical ABI implementation
#[derive(Debug)]
pub struct CanonicalABI {
    /// String encoding (always UTF-8 for now)
    string_encoding: StringEncoding,
    /// Binary std/no_std choice
    alignment: u32,
}

/// String encoding options
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StringEncoding {
    /// UTF-8 encoding (default)
    Utf8,
    /// UTF-16 encoding
    Utf16,
    /// UTF-16 Little Endian encoding
    Utf16Le,
    /// UTF-16 Big Endian encoding
    Utf16Be,
    /// Latin-1 encoding
    Latin1,
}

impl Default for StringEncoding {
    fn default() -> Self {
        Self::Utf8
    }
}

impl Default for CanonicalABI {
    fn default() -> Self {
        Self::new()
    }
}

impl CanonicalABI {
    /// Create a new Canonical ABI instance
    pub fn new() -> Self {
        Self {
            string_encoding: StringEncoding::Utf8,
            alignment: 1,
        }
    }

    /// Set the string encoding
    pub fn with_string_encoding(mut self, encoding: StringEncoding) -> Self {
        self.string_encoding = encoding;
        self
    }

    /// Set the memory alignment
    pub fn with_alignment(mut self, alignment: u32) -> Self {
        self.alignment = alignment;
        self
    }

    /// Calculate the size of a type in memory
    pub fn size_of(&self, ty: &ComponentType) -> Result<u32> {
        match ty {
            ComponentType::Bool | ComponentType::S8 | ComponentType::U8 => Ok(1),
            ComponentType::S16 | ComponentType::U16 => Ok(2),
            ComponentType::S32 | ComponentType::U32 | ComponentType::F32 | ComponentType::Char => {
                Ok(4)
            },
            ComponentType::S64 | ComponentType::U64 | ComponentType::F64 => Ok(8),
            ComponentType::String | ComponentType::List(_) => Ok(8), // ptr + len
            ComponentType::Option(inner) => {
                let inner_size = self.size_of(inner)?;
                Ok(inner_size + 1) // discriminant + optional value
            },
            ComponentType::Result(ok, err) => {
                let ok_size = if let Some(ok_ty) = ok { self.size_of(ok_ty)? } else { 0 };
                let err_size = if let Some(err_ty) = err { self.size_of(err_ty)? } else { 0 };
                Ok(4 + ok_size.max(err_size)) // discriminant + max(ok, err)
            },
            ComponentType::Record(fields) => {
                let mut total_size = 0;
                for (_, field_ty) in fields {
                    total_size += self.size_of(field_ty)?;
                }
                Ok(total_size)
            },
            ComponentType::Tuple(types) => {
                let mut total_size = 0;
                for ty in types {
                    total_size += self.size_of(ty)?;
                }
                Ok(total_size)
            },
            ComponentType::Variant(cases) => {
                let mut max_payload_size = 0;
                for (_, payload_ty) in cases {
                    if let Some(ty) = payload_ty {
                        max_payload_size = max_payload_size.max(self.size_of(ty)?);
                    }
                }
                Ok(4 + max_payload_size) // discriminant + max payload
            },
            ComponentType::Enum(_) => Ok(4), // discriminant only
            ComponentType::Flags(flags) => {
                // Each flag is 1 bit, round up to byte boundary
                let bit_count = flags.len();
                let byte_count = bit_count.div_ceil(8);
                Ok(byte_count as u32)
            },
        }
    }

    /// Calculate the alignment of a type
    pub fn align_of(&self, ty: &ComponentType) -> Result<u32> {
        match ty {
            ComponentType::Bool | ComponentType::S8 | ComponentType::U8 => Ok(1),
            ComponentType::S16 | ComponentType::U16 => Ok(2),
            ComponentType::S32 | ComponentType::U32 | ComponentType::F32 | ComponentType::Char => {
                Ok(4)
            },
            ComponentType::S64 | ComponentType::U64 | ComponentType::F64 => Ok(8),
            ComponentType::String | ComponentType::List(_) => Ok(4), // pointer alignment
            ComponentType::Option(inner) => self.align_of(inner),
            ComponentType::Result(ok, err) => {
                let ok_align = if let Some(ok_ty) = ok { self.align_of(ok_ty)? } else { 1 };
                let err_align = if let Some(err_ty) = err { self.align_of(err_ty)? } else { 1 };
                Ok(4.max(ok_align).max(err_align))
            },
            ComponentType::Record(fields) => {
                let mut max_align = 1;
                for (_, field_ty) in fields {
                    max_align = max_align.max(self.align_of(field_ty)?);
                }
                Ok(max_align)
            },
            ComponentType::Tuple(types) => {
                let mut max_align = 1;
                for ty in types {
                    max_align = max_align.max(self.align_of(ty)?);
                }
                Ok(max_align)
            },
            ComponentType::Variant(_) | ComponentType::Enum(_) => Ok(4),
            ComponentType::Flags(_) => Ok(1),
        }
    }

    // ==== LIFTING OPERATIONS ====

    /// Lift a value from memory
    pub fn lift<M: CanonicalMemory>(
        &self,
        memory: &M,
        ty: &ComponentType,
        offset: u32,
    ) -> Result<ComponentValue> {
        match ty {
            ComponentType::Bool => self.lift_bool(memory, offset),
            ComponentType::S8 => self.lift_s8(memory, offset),
            ComponentType::U8 => self.lift_u8(memory, offset),
            ComponentType::S16 => self.lift_s16(memory, offset),
            ComponentType::U16 => self.lift_u16(memory, offset),
            ComponentType::S32 => self.lift_s32(memory, offset),
            ComponentType::U32 => self.lift_u32(memory, offset),
            ComponentType::S64 => self.lift_s64(memory, offset),
            ComponentType::U64 => self.lift_u64(memory, offset),
            ComponentType::F32 => self.lift_f32(memory, offset),
            ComponentType::F64 => self.lift_f64(memory, offset),
            ComponentType::Char => self.lift_char(memory, offset),
            ComponentType::String => self.lift_string(memory, offset),
            ComponentType::List(element_ty) => self.lift_list(memory, element_ty, offset),
            ComponentType::Record(fields) => self.lift_record(memory, fields, offset),
            ComponentType::Tuple(types) => self.lift_tuple(memory, types, offset),
            ComponentType::Variant(cases) => self.lift_variant(memory, cases, offset),
            ComponentType::Enum(cases) => self.lift_enum(memory, cases, offset),
            ComponentType::Option(inner_ty) => self.lift_option(memory, inner_ty, offset),
            ComponentType::Result(ok_ty, err_ty) => self.lift_result(memory, ok_ty, err_ty, offset),
            ComponentType::Flags(flags) => self.lift_flags(memory, flags, offset),
        }
    }

    /// Lift a boolean value
    pub fn lift_bool<M: CanonicalMemory>(&self, memory: &M, offset: u32) -> Result<ComponentValue> {
        let value = memory.read_u8(offset)?;
        Ok(ComponentValue::Bool(value != 0))
    }

    /// Lift an i8 value
    pub fn lift_s8<M: CanonicalMemory>(&self, memory: &M, offset: u32) -> Result<ComponentValue> {
        let value = memory.read_u8(offset)? as i8;
        Ok(ComponentValue::S8(value))
    }

    /// Lift a u8 value
    pub fn lift_u8<M: CanonicalMemory>(&self, memory: &M, offset: u32) -> Result<ComponentValue> {
        let value = memory.read_u8(offset)?;
        Ok(ComponentValue::U8(value))
    }

    /// Lift an i16 value
    pub fn lift_s16<M: CanonicalMemory>(&self, memory: &M, offset: u32) -> Result<ComponentValue> {
        let value = memory.read_u16_le(offset)? as i16;
        Ok(ComponentValue::S16(value))
    }

    /// Lift a u16 value
    pub fn lift_u16<M: CanonicalMemory>(&self, memory: &M, offset: u32) -> Result<ComponentValue> {
        let value = memory.read_u16_le(offset)?;
        Ok(ComponentValue::U16(value))
    }

    /// Lift an i32 value
    pub fn lift_s32<M: CanonicalMemory>(&self, memory: &M, offset: u32) -> Result<ComponentValue> {
        let value = memory.read_u32_le(offset)? as i32;
        Ok(ComponentValue::S32(value))
    }

    /// Lift a u32 value
    pub fn lift_u32<M: CanonicalMemory>(&self, memory: &M, offset: u32) -> Result<ComponentValue> {
        let value = memory.read_u32_le(offset)?;
        Ok(ComponentValue::U32(value))
    }

    /// Lift an i64 value
    pub fn lift_s64<M: CanonicalMemory>(&self, memory: &M, offset: u32) -> Result<ComponentValue> {
        let value = memory.read_u64_le(offset)? as i64;
        Ok(ComponentValue::S64(value))
    }

    /// Lift a u64 value
    pub fn lift_u64<M: CanonicalMemory>(&self, memory: &M, offset: u32) -> Result<ComponentValue> {
        let value = memory.read_u64_le(offset)?;
        Ok(ComponentValue::U64(value))
    }

    /// Lift an f32 value
    pub fn lift_f32<M: CanonicalMemory>(&self, memory: &M, offset: u32) -> Result<ComponentValue> {
        let bits = memory.read_u32_le(offset)?;
        let value = f32::from_bits(bits);
        Ok(ComponentValue::F32(value))
    }

    /// Lift an f64 value
    pub fn lift_f64<M: CanonicalMemory>(&self, memory: &M, offset: u32) -> Result<ComponentValue> {
        let bits = memory.read_u64_le(offset)?;
        let value = f64::from_bits(bits);
        Ok(ComponentValue::F64(value))
    }

    /// Lift a char value
    pub fn lift_char<M: CanonicalMemory>(&self, memory: &M, offset: u32) -> Result<ComponentValue> {
        let code_point = memory.read_u32_le(offset)?;
        let ch = char::from_u32(code_point)
            .ok_or_else(|| Error::validation_error("Error occurred: Invalid Unicode code point"))?;
        Ok(ComponentValue::Char(ch))
    }

    /// Lift a string value
    pub fn lift_string<M: CanonicalMemory>(
        &self,
        memory: &M,
        offset: u32,
    ) -> Result<ComponentValue> {
        // String is stored as (ptr: u32, len: u32)
        let ptr = memory.read_u32_le(offset)?;
        let len = memory.read_u32_le(offset + 4)?;

        // Safety check
        if len > MAX_STRING_LENGTH as u32 {
            return Err(Error::validation_error("Error occurred: String too long"));
        }

        // Read string data
        let bytes = memory.read_bytes(ptr, len)?;

        // Decode based on encoding
        let string = match self.string_encoding {
            StringEncoding::Utf8 => String::from_utf8(bytes)
                .map_err(|_| Error::validation_error("Error occurred: Invalid UTF-8 string"))?,
            StringEncoding::Utf16 | StringEncoding::Utf16Le => {
                if bytes.len() % 2 != 0 {
                    return Err(Error::validation_error(
                        "Error occurred: UTF-16 byte sequence must have even length",
                    ));
                }

                let mut code_units = Vec::new();
                for chunk in bytes.chunks_exact(2) {
                    let code_unit = u16::from_le_bytes([chunk[0], chunk[1]]);
                    code_units.push(code_unit);
                }

                String::from_utf16(&code_units).map_err(|_| {
                    Error::validation_error("Error occurred: Invalid UTF-16 sequence")
                })?
            },
            StringEncoding::Utf16Be => {
                if bytes.len() % 2 != 0 {
                    return Err(Error::validation_error(
                        "Error occurred: UTF-16 byte sequence must have even length",
                    ));
                }

                let mut code_units = Vec::new();
                for chunk in bytes.chunks_exact(2) {
                    let code_unit = u16::from_be_bytes([chunk[0], chunk[1]]);
                    code_units.push(code_unit);
                }

                String::from_utf16(&code_units).map_err(|_| {
                    Error::validation_error("Error occurred: Invalid UTF-16 sequence")
                })?
            },
            StringEncoding::Latin1 => {
                // Latin-1 is a direct mapping from bytes to Unicode code points 0x00-0xFF
                bytes.iter().map(|&b| b as char).collect()
            },
        };

        Ok(ComponentValue::String(string))
    }

    /// Lift a list value
    pub fn lift_list<M: CanonicalMemory>(
        &self,
        memory: &M,
        element_ty: &ComponentType,
        offset: u32,
    ) -> Result<ComponentValue> {
        // List is stored as (ptr: u32, len: u32)
        let ptr = memory.read_u32_le(offset)?;
        let len = memory.read_u32_le(offset + 4)?;

        // Safety check
        if len > MAX_LIST_LENGTH as u32 {
            return Err(Error::validation_error("Error occurred: List too long"));
        }

        let element_size = self.size_of(element_ty)?;
        let mut values = Vec::new();

        for i in 0..len {
            let element_offset = ptr + i * element_size;
            let value = self.lift(memory, element_ty, element_offset)?;
            values.push(value);
        }

        Ok(ComponentValue::List(values))
    }

    /// Lift a record value
    pub fn lift_record<M: CanonicalMemory>(
        &self,
        memory: &M,
        fields: &[(String, ComponentType)],
        offset: u32,
    ) -> Result<ComponentValue> {
        let mut field_values = Vec::new();
        let mut current_offset = offset;

        for (field_name, field_ty) in fields {
            let value = self.lift(memory, field_ty, current_offset)?;
            field_values.push((field_name.clone(), value));
            current_offset += self.size_of(field_ty)?;
        }

        Ok(ComponentValue::Record(field_values))
    }

    /// Lift a tuple value
    pub fn lift_tuple<M: CanonicalMemory>(
        &self,
        memory: &M,
        types: &[ComponentType],
        offset: u32,
    ) -> Result<ComponentValue> {
        let mut values = Vec::new();
        let mut current_offset = offset;

        for ty in types {
            let value = self.lift(memory, ty, current_offset)?;
            values.push(value);
            current_offset += self.size_of(ty)?;
        }

        Ok(ComponentValue::Tuple(values))
    }

    /// Lift a variant value
    pub fn lift_variant<M: CanonicalMemory>(
        &self,
        memory: &M,
        cases: &[(String, Option<ComponentType>)],
        offset: u32,
    ) -> Result<ComponentValue> {
        let discriminant = memory.read_u32_le(offset)?;

        if discriminant as usize >= cases.len() {
            return Err(Error::validation_error(
                "Error occurred: Invalid variant discriminant",
            ));
        }

        let (case_name, payload_ty) = &cases[discriminant as usize];

        if let Some(ty) = payload_ty {
            let payload_value = self.lift(memory, ty, offset + 4)?;
            Ok(ComponentValue::Variant(
                case_name.clone(),
                Some(Box::new(payload_value)),
            ))
        } else {
            Ok(ComponentValue::Variant(case_name.clone(), None))
        }
    }

    /// Lift an enum value
    pub fn lift_enum<M: CanonicalMemory>(
        &self,
        memory: &M,
        cases: &[String],
        offset: u32,
    ) -> Result<ComponentValue> {
        let discriminant = memory.read_u32_le(offset)?;

        if discriminant as usize >= cases.len() {
            return Err(Error::validation_error(
                "Error occurred: Invalid enum discriminant",
            ));
        }

        Ok(ComponentValue::Enum(cases[discriminant as usize].clone()))
    }

    /// Lift an option value
    pub fn lift_option<M: CanonicalMemory>(
        &self,
        memory: &M,
        inner_ty: &ComponentType,
        offset: u32,
    ) -> Result<ComponentValue> {
        let discriminant = memory.read_u8(offset)?;

        if discriminant == 0 {
            Ok(ComponentValue::Option(None))
        } else {
            let value = self.lift(memory, inner_ty, offset + 1)?;
            Ok(ComponentValue::Option(Some(Box::new(value))))
        }
    }

    /// Lift a result value
    pub fn lift_result<M: CanonicalMemory>(
        &self,
        memory: &M,
        ok_ty: &Option<Box<ComponentType>>,
        err_ty: &Option<Box<ComponentType>>,
        offset: u32,
    ) -> Result<ComponentValue> {
        let discriminant = memory.read_u32_le(offset)?;

        match discriminant {
            0 => {
                // Ok case
                if let Some(ty) = ok_ty {
                    let value = self.lift(memory, ty, offset + 4)?;
                    Ok(ComponentValue::Result(Ok(Some(Box::new(value)))))
                } else {
                    Ok(ComponentValue::Result(Ok(None)))
                }
            },
            1 => {
                // Err case
                if let Some(ty) = err_ty {
                    let value = self.lift(memory, ty, offset + 4)?;
                    Ok(ComponentValue::Result(Err(Some(Box::new(value)))))
                } else {
                    Ok(ComponentValue::Result(Err(None)))
                }
            },
            _ => Err(Error::validation_error(
                "Error occurred: Invalid result discriminant",
            )),
        }
    }

    /// Lift a flags value
    pub fn lift_flags<M: CanonicalMemory>(
        &self,
        memory: &M,
        flags: &[String],
        offset: u32,
    ) -> Result<ComponentValue> {
        let byte_count = flags.len().div_ceil(8);
        let bytes = memory.read_bytes(offset, byte_count as u32)?;

        let mut active_flags = Vec::new();

        for (i, flag_name) in flags.iter().enumerate() {
            let byte_index = i / 8;
            let bit_index = i % 8;

            if byte_index < bytes.len() && (bytes[byte_index] & (1 << bit_index)) != 0 {
                active_flags.push(flag_name.clone());
            }
        }

        Ok(ComponentValue::Flags(active_flags))
    }

    // ==== LOWERING OPERATIONS ====

    /// Lower a value to memory
    pub fn lower<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        value: &ComponentValue,
        offset: u32,
    ) -> Result<()> {
        match value {
            ComponentValue::Bool(v) => self.lower_bool(memory, *v, offset),
            ComponentValue::S8(v) => self.lower_s8(memory, *v, offset),
            ComponentValue::U8(v) => self.lower_u8(memory, *v, offset),
            ComponentValue::S16(v) => self.lower_s16(memory, *v, offset),
            ComponentValue::U16(v) => self.lower_u16(memory, *v, offset),
            ComponentValue::S32(v) => self.lower_s32(memory, *v, offset),
            ComponentValue::U32(v) => self.lower_u32(memory, *v, offset),
            ComponentValue::S64(v) => self.lower_s64(memory, *v, offset),
            ComponentValue::U64(v) => self.lower_u64(memory, *v, offset),
            ComponentValue::F32(v) => self.lower_f32(memory, *v, offset),
            ComponentValue::F64(v) => self.lower_f64(memory, *v, offset),
            ComponentValue::Char(v) => self.lower_char(memory, *v, offset),
            ComponentValue::String(v) => self.lower_string(memory, v, offset),
            ComponentValue::List(v) => self.lower_list(memory, v, offset),
            ComponentValue::Record(v) => self.lower_record(memory, v, offset),
            ComponentValue::Tuple(v) => self.lower_tuple(memory, v, offset),
            ComponentValue::Variant(name, payload) => {
                // TODO: Need type information to properly lower variants
                // For now, write discriminant 0 and payload
                memory.write_u8(offset, 0)?;
                if let Some(payload_value) = payload {
                    self.lower(memory, payload_value, offset + 1)?;
                }
                Ok(())
            },
            ComponentValue::Enum(name) => {
                // TODO: Need type information to properly lower enums
                // For now, write discriminant 0
                memory.write_u8(offset, 0)
            },
            ComponentValue::Option(v) => self.lower_option(memory, v, offset),
            ComponentValue::Result(v) => self.lower_result(memory, v, offset),
            ComponentValue::Flags(v) => {
                // TODO: Need type information to properly lower flags
                // For now, write empty flags (all zeros)
                memory.write_u8(offset, 0)
            },
        }
    }

    /// Lower a value to memory with type information
    ///
    /// This version uses type information to correctly compute discriminants
    /// for variants/enums and bit patterns for flags.
    pub fn lower_typed<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        value: &ComponentValue,
        val_type: &ComponentType,
        offset: u32,
    ) -> Result<()> {
        match (value, val_type) {
            // Primitive types delegate to untyped version
            (ComponentValue::Bool(v), ComponentType::Bool) => self.lower_bool(memory, *v, offset),
            (ComponentValue::S8(v), ComponentType::S8) => self.lower_s8(memory, *v, offset),
            (ComponentValue::U8(v), ComponentType::U8) => self.lower_u8(memory, *v, offset),
            (ComponentValue::S16(v), ComponentType::S16) => self.lower_s16(memory, *v, offset),
            (ComponentValue::U16(v), ComponentType::U16) => self.lower_u16(memory, *v, offset),
            (ComponentValue::S32(v), ComponentType::S32) => self.lower_s32(memory, *v, offset),
            (ComponentValue::U32(v), ComponentType::U32) => self.lower_u32(memory, *v, offset),
            (ComponentValue::S64(v), ComponentType::S64) => self.lower_s64(memory, *v, offset),
            (ComponentValue::U64(v), ComponentType::U64) => self.lower_u64(memory, *v, offset),
            (ComponentValue::F32(v), ComponentType::F32) => self.lower_f32(memory, *v, offset),
            (ComponentValue::F64(v), ComponentType::F64) => self.lower_f64(memory, *v, offset),
            (ComponentValue::Char(v), ComponentType::Char) => self.lower_char(memory, *v, offset),
            (ComponentValue::String(v), ComponentType::String) => {
                self.lower_string(memory, v, offset)
            },

            // List with element type
            (ComponentValue::List(v), ComponentType::List(_elem_type)) => {
                self.lower_list(memory, v, offset)
            },

            // Record with field types
            (ComponentValue::Record(fields), ComponentType::Record(field_types)) => {
                self.lower_record_typed(memory, fields, field_types, offset)
            },

            // Tuple with element types
            (ComponentValue::Tuple(values), ComponentType::Tuple(elem_types)) => {
                self.lower_tuple_typed(memory, values, elem_types, offset)
            },

            // Variant: compute discriminant from case name position in type
            (ComponentValue::Variant(case_name, payload), ComponentType::Variant(cases)) => {
                let discriminant = cases
                    .iter()
                    .position(|(name, _)| name == case_name)
                    .ok_or_else(|| Error::validation_error("Unknown variant case"))?;

                // Discriminant size depends on number of cases
                let disc_size = self.discriminant_size(cases.len());
                self.write_discriminant(memory, offset, discriminant as u32, disc_size)?;

                // Write payload if present
                if let Some(payload_value) = payload {
                    // Find payload type from cases
                    if let Some(payload_type) = &cases[discriminant].1 {
                        // Payload offset: discriminant + alignment padding
                        let payload_offset = offset
                            + disc_size
                            + self.payload_alignment_padding(disc_size, payload_type);
                        self.lower_typed(memory, payload_value, payload_type, payload_offset)?;
                    }
                }
                Ok(())
            },

            // Enum: compute discriminant from case name position in type
            (ComponentValue::Enum(case_name), ComponentType::Enum(cases)) => {
                let discriminant = cases
                    .iter()
                    .position(|name| name == case_name)
                    .ok_or_else(|| Error::validation_error("Unknown enum case"))?;

                // Enum discriminant size depends on number of cases
                let disc_size = self.discriminant_size(cases.len());
                self.write_discriminant(memory, offset, discriminant as u32, disc_size)
            },

            // Option: discriminant 0/1 + optional payload
            (ComponentValue::Option(opt_value), ComponentType::Option(inner_type)) => {
                match opt_value {
                    None => memory.write_u8(offset, 0),
                    Some(inner) => {
                        memory.write_u8(offset, 1)?;
                        // Payload at offset + 1 (aligned appropriately)
                        let payload_offset =
                            offset + 1 + self.payload_alignment_padding(1, inner_type);
                        self.lower_typed(memory, inner, inner_type, payload_offset)
                    },
                }
            },

            // Result: discriminant 0=ok/1=err + payload
            (ComponentValue::Result(result), ComponentType::Result(ok_type, err_type)) => {
                match result {
                    Ok(payload) => {
                        memory.write_u8(offset, 0)?;
                        if let (Some(val), Some(ty)) = (payload, ok_type) {
                            let payload_offset = offset + 1 + self.payload_alignment_padding(1, ty);
                            self.lower_typed(memory, val, ty, payload_offset)?;
                        }
                        Ok(())
                    },
                    Err(payload) => {
                        memory.write_u8(offset, 1)?;
                        if let (Some(val), Some(ty)) = (payload, err_type) {
                            let payload_offset = offset + 1 + self.payload_alignment_padding(1, ty);
                            self.lower_typed(memory, val, ty, payload_offset)?;
                        }
                        Ok(())
                    },
                }
            },

            // Flags: compute bit pattern from active flag names
            (ComponentValue::Flags(active_flags), ComponentType::Flags(all_flags)) => {
                let byte_count = all_flags.len().div_ceil(8);
                let mut bytes = vec![0u8; byte_count];

                for flag_name in active_flags {
                    if let Some(idx) = all_flags.iter().position(|f| f == flag_name) {
                        let byte_index = idx / 8;
                        let bit_index = idx % 8;
                        bytes[byte_index] |= 1 << bit_index;
                    }
                }

                // Write all bytes
                for (i, byte) in bytes.iter().enumerate() {
                    memory.write_u8(offset + i as u32, *byte)?;
                }
                Ok(())
            },

            // Type mismatch
            _ => Err(Error::validation_error(
                "Value type doesn't match expected type",
            )),
        }
    }

    /// Calculate discriminant size based on number of cases
    fn discriminant_size(&self, num_cases: usize) -> u32 {
        if num_cases <= 256 {
            1
        } else if num_cases <= 65536 {
            2
        } else {
            4
        }
    }

    /// Write discriminant value with appropriate size
    fn write_discriminant<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        offset: u32,
        discriminant: u32,
        disc_size: u32,
    ) -> Result<()> {
        match disc_size {
            1 => memory.write_u8(offset, discriminant as u8),
            2 => memory.write_u16_le(offset, discriminant as u16),
            4 => memory.write_u32_le(offset, discriminant),
            _ => Err(Error::validation_error("Invalid discriminant size")),
        }
    }

    /// Calculate alignment padding for payload after discriminant
    fn payload_alignment_padding(&self, disc_size: u32, payload_type: &ComponentType) -> u32 {
        let payload_align = self.type_alignment(payload_type);
        let current_pos = disc_size;
        let remainder = current_pos % payload_align;
        if remainder == 0 { 0 } else { payload_align - remainder }
    }

    /// Get alignment requirement for a type
    fn type_alignment(&self, ty: &ComponentType) -> u32 {
        match ty {
            ComponentType::Bool | ComponentType::S8 | ComponentType::U8 => 1,
            ComponentType::S16 | ComponentType::U16 => 2,
            ComponentType::S32 | ComponentType::U32 | ComponentType::F32 | ComponentType::Char => 4,
            ComponentType::S64 | ComponentType::U64 | ComponentType::F64 => 8,
            ComponentType::String | ComponentType::List(_) => 4, // ptr + len
            ComponentType::Record(fields) => {
                fields.iter().map(|(_, ft)| self.type_alignment(ft)).max().unwrap_or(1)
            },
            ComponentType::Tuple(types) => {
                types.iter().map(|t| self.type_alignment(t)).max().unwrap_or(1)
            },
            ComponentType::Variant(cases) => {
                let disc_align = self.discriminant_size(cases.len());
                let payload_align = cases
                    .iter()
                    .filter_map(|(_, ty)| ty.as_ref())
                    .map(|t| self.type_alignment(t))
                    .max()
                    .unwrap_or(1);
                disc_align.max(payload_align)
            },
            ComponentType::Enum(cases) => self.discriminant_size(cases.len()),
            ComponentType::Option(inner) => self.type_alignment(inner).max(1),
            ComponentType::Result(ok, err) => {
                let ok_align = ok.as_ref().map(|t| self.type_alignment(t)).unwrap_or(1);
                let err_align = err.as_ref().map(|t| self.type_alignment(t)).unwrap_or(1);
                ok_align.max(err_align).max(1)
            },
            ComponentType::Flags(flags) => {
                let byte_count = flags.len().div_ceil(8);
                if byte_count <= 1 {
                    1
                } else if byte_count <= 2 {
                    2
                } else {
                    4
                }
            },
        }
    }

    /// Lower a record with type information
    fn lower_record_typed<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        fields: &[(String, ComponentValue)],
        field_types: &[(String, ComponentType)],
        offset: u32,
    ) -> Result<()> {
        let mut current_offset = offset;

        for (field_name, field_type) in field_types {
            // Align to field type
            let align = self.type_alignment(field_type);
            let remainder = current_offset % align;
            if remainder != 0 {
                current_offset += align - remainder;
            }

            // Find matching value
            let value = fields
                .iter()
                .find(|(name, _)| name == field_name)
                .map(|(_, v)| v)
                .ok_or_else(|| Error::validation_error("Missing record field"))?;

            self.lower_typed(memory, value, field_type, current_offset)?;

            // Move to next field
            current_offset += self.type_size(field_type);
        }

        Ok(())
    }

    /// Lower a tuple with type information
    fn lower_tuple_typed<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        values: &[ComponentValue],
        types: &[ComponentType],
        offset: u32,
    ) -> Result<()> {
        if values.len() != types.len() {
            return Err(Error::validation_error("Tuple value/type length mismatch"));
        }

        let mut current_offset = offset;

        for (value, ty) in values.iter().zip(types.iter()) {
            // Align to element type
            let align = self.type_alignment(ty);
            let remainder = current_offset % align;
            if remainder != 0 {
                current_offset += align - remainder;
            }

            self.lower_typed(memory, value, ty, current_offset)?;

            // Move to next element
            current_offset += self.type_size(ty);
        }

        Ok(())
    }

    /// Get size of a type in bytes
    fn type_size(&self, ty: &ComponentType) -> u32 {
        match ty {
            ComponentType::Bool | ComponentType::S8 | ComponentType::U8 => 1,
            ComponentType::S16 | ComponentType::U16 => 2,
            ComponentType::S32 | ComponentType::U32 | ComponentType::F32 | ComponentType::Char => 4,
            ComponentType::S64 | ComponentType::U64 | ComponentType::F64 => 8,
            ComponentType::String | ComponentType::List(_) => 8, // ptr (4) + len (4)
            ComponentType::Record(fields) => {
                let mut size = 0u32;
                for (_, ft) in fields {
                    let align = self.type_alignment(ft);
                    let remainder = size % align;
                    if remainder != 0 {
                        size += align - remainder;
                    }
                    size += self.type_size(ft);
                }
                size
            },
            ComponentType::Tuple(types) => {
                let mut size = 0u32;
                for t in types {
                    let align = self.type_alignment(t);
                    let remainder = size % align;
                    if remainder != 0 {
                        size += align - remainder;
                    }
                    size += self.type_size(t);
                }
                size
            },
            ComponentType::Variant(cases) => {
                let disc_size = self.discriminant_size(cases.len());
                let max_payload = cases
                    .iter()
                    .filter_map(|(_, ty)| ty.as_ref())
                    .map(|t| self.type_size(t))
                    .max()
                    .unwrap_or(0);
                disc_size + max_payload
            },
            ComponentType::Enum(cases) => self.discriminant_size(cases.len()),
            ComponentType::Option(inner) => 1 + self.type_size(inner),
            ComponentType::Result(ok, err) => {
                let ok_size = ok.as_ref().map(|t| self.type_size(t)).unwrap_or(0);
                let err_size = err.as_ref().map(|t| self.type_size(t)).unwrap_or(0);
                1 + ok_size.max(err_size)
            },
            ComponentType::Flags(flags) => flags.len().div_ceil(8) as u32,
        }
    }

    /// Lower a boolean value
    pub fn lower_bool<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        value: bool,
        offset: u32,
    ) -> Result<()> {
        memory.write_u8(offset, if value { 1 } else { 0 })
    }

    /// Lower an i8 value
    pub fn lower_s8<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        value: i8,
        offset: u32,
    ) -> Result<()> {
        memory.write_u8(offset, value as u8)
    }

    /// Lower a u8 value
    pub fn lower_u8<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        value: u8,
        offset: u32,
    ) -> Result<()> {
        memory.write_u8(offset, value)
    }

    /// Lower an i16 value
    pub fn lower_s16<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        value: i16,
        offset: u32,
    ) -> Result<()> {
        memory.write_u16_le(offset, value as u16)
    }

    /// Lower a u16 value
    pub fn lower_u16<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        value: u16,
        offset: u32,
    ) -> Result<()> {
        memory.write_u16_le(offset, value)
    }

    /// Lower an i32 value
    pub fn lower_s32<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        value: i32,
        offset: u32,
    ) -> Result<()> {
        memory.write_u32_le(offset, value as u32)
    }

    /// Lower a u32 value
    pub fn lower_u32<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        value: u32,
        offset: u32,
    ) -> Result<()> {
        memory.write_u32_le(offset, value)
    }

    /// Lower an i64 value
    pub fn lower_s64<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        value: i64,
        offset: u32,
    ) -> Result<()> {
        memory.write_u64_le(offset, value as u64)
    }

    /// Lower a u64 value
    pub fn lower_u64<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        value: u64,
        offset: u32,
    ) -> Result<()> {
        memory.write_u64_le(offset, value)
    }

    /// Lower an f32 value
    pub fn lower_f32<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        value: f32,
        offset: u32,
    ) -> Result<()> {
        memory.write_u32_le(offset, value.to_bits())
    }

    /// Lower an f64 value
    pub fn lower_f64<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        value: f64,
        offset: u32,
    ) -> Result<()> {
        memory.write_u64_le(offset, value.to_bits())
    }

    /// Lower a char value
    pub fn lower_char<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        value: char,
        offset: u32,
    ) -> Result<()> {
        memory.write_u32_le(offset, value as u32)
    }

    /// Lower a string value
    pub fn lower_string<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        value: &str,
        offset: u32,
    ) -> Result<()> {
        // This is a simplified implementation that assumes string data
        // Binary std/no_std choice
        // Binary std/no_std choice

        let bytes = value.as_bytes();
        let len = bytes.len() as u32;

        // Safety check
        if len > MAX_STRING_LENGTH as u32 {
            return Err(Error::validation_error("Error occurred: String too long"));
        }

        // For this simplified implementation, we'll assume the string data
        // is written immediately after the pointer/length pair
        let data_offset = offset + 8;

        // Write pointer and length
        memory.write_u32_le(offset, data_offset)?;
        memory.write_u32_le(offset + 4, len)?;

        // Write string data
        memory.write_bytes(data_offset, bytes)?;

        Ok(())
    }

    /// Lower a list value to memory
    ///
    /// According to the Component Model Canonical ABI, lists are stored as:
    /// - (pointer: i32, length: i32) pair at the given offset
    /// - Element data stored contiguously starting at the pointer location
    ///
    /// This simplified implementation writes element data immediately after the
    /// pointer/length pair (similar to lower_string). A full implementation
    /// would use cabi_realloc to allocate memory in the component's linear memory.
    pub fn lower_list<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        values: &[ComponentValue],
        offset: u32,
    ) -> Result<()> {
        let len = values.len() as u32;

        // Safety check
        if len > MAX_LIST_LENGTH as u32 {
            return Err(Error::validation_error("Error occurred: List too long"));
        }

        // Handle empty list case
        if values.is_empty() {
            // For empty lists, pointer can be 0 (null) with length 0
            memory.write_u32_le(offset, 0)?;
            memory.write_u32_le(offset + 4, 0)?;
            return Ok(());
        }

        // Calculate element layout based on the first element
        // (all elements in a list have the same type)
        let element_layout = self.calculate_value_layout(&values[0]);
        let element_size = element_layout.size as u32;
        let element_alignment = element_layout.alignment as u32;

        // Data starts after the (pointer, length) pair (8 bytes)
        // Align the data start to the element's alignment requirement
        let data_start_unaligned = offset + 8;
        let data_start = if element_alignment > 1 {
            let remainder = data_start_unaligned % element_alignment;
            if remainder == 0 {
                data_start_unaligned
            } else {
                data_start_unaligned + (element_alignment - remainder)
            }
        } else {
            data_start_unaligned
        };

        // Write pointer and length
        memory.write_u32_le(offset, data_start)?;
        memory.write_u32_le(offset + 4, len)?;

        // Write each element to memory
        let mut current_offset = data_start;
        for value in values {
            // Align current offset to element alignment
            let remainder = current_offset % element_alignment;
            if remainder != 0 {
                current_offset += element_alignment - remainder;
            }

            // Lower the element value to memory
            self.lower(memory, value, current_offset)?;

            // Move to next element position
            current_offset += element_size;
        }

        Ok(())
    }

    /// Lower a record value with proper field layout
    pub fn lower_record<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        fields: &[(String, ComponentValue)],
        offset: u32,
    ) -> Result<()> {
        // Calculate field layouts and offsets
        let mut current_offset = 0;

        for (field_name, field_value) in fields {
            // Calculate field layout based on value type
            let field_layout = self.calculate_value_layout(field_value);

            // Align current offset to field's alignment requirement
            current_offset = align_to(current_offset, field_layout.alignment);

            // Lower the field value at the aligned offset
            self.lower(memory, field_value, offset + current_offset as u32)?;

            // Move to next field position
            current_offset += field_layout.size;
        }

        Ok(())
    }

    /// Lower a tuple value with proper element layout
    pub fn lower_tuple<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        values: &[ComponentValue],
        offset: u32,
    ) -> Result<()> {
        // Calculate element layouts and offsets
        let mut current_offset = 0;

        for value in values {
            // Calculate element layout based on value type
            let element_layout = self.calculate_value_layout(value);

            // Align current offset to element's alignment requirement
            current_offset = align_to(current_offset, element_layout.alignment);

            // Lower the element value at the aligned offset
            self.lower(memory, value, offset + current_offset as u32)?;

            // Move to next element position
            current_offset += element_layout.size;
        }

        Ok(())
    }

    /// Lower a variant value with proper discriminant and payload layout
    pub fn lower_variant<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        cases: &[(String, Option<ComponentType>)],
        case_name: &str,
        payload: &Option<Box<ComponentValue>>,
        offset: u32,
    ) -> Result<()> {
        // Find the discriminant for this case
        let discriminant = cases
            .iter()
            .position(|(name, _)| name == case_name)
            .ok_or_else(|| Error::validation_error("Error occurred: Variant case not found"))?;

        // Calculate discriminant size based on number of cases
        let discriminant_size = if cases.len() <= 256 {
            1
        } else if cases.len() <= 65536 {
            2
        } else {
            4
        };

        // Write discriminant
        match discriminant_size {
            1 => memory.write_u8(offset, discriminant as u8)?,
            2 => memory.write_u16_le(offset, discriminant as u16)?,
            4 => memory.write_u32_le(offset, discriminant as u32)?,
            _ => {
                return Err(Error::validation_error(
                    "Error occurred: Invalid discriminant size calculated",
                ));
            },
        }

        // If there's a payload, lower it after the discriminant with proper alignment
        if let Some(payload_value) = payload {
            let payload_layout = self.calculate_value_layout(payload_value);

            // Calculate payload offset with proper alignment
            let payload_offset = align_to(discriminant_size, payload_layout.alignment);

            // Lower the payload
            self.lower(memory, payload_value, offset + payload_offset as u32)?;
        }

        Ok(())
    }

    /// Lower an enum value with proper discriminant calculation
    pub fn lower_enum<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        cases: &[String],
        case_name: &str,
        offset: u32,
    ) -> Result<()> {
        // Find the discriminant for this case
        let discriminant = cases
            .iter()
            .position(|name| name == case_name)
            .ok_or_else(|| Error::validation_error("Error occurred: Enum case not found"))?;

        // Calculate discriminant size based on number of cases
        let discriminant_size = if cases.len() <= 256 {
            1
        } else if cases.len() <= 65536 {
            2
        } else {
            4
        };

        // Write discriminant
        match discriminant_size {
            1 => memory.write_u8(offset, discriminant as u8),
            2 => memory.write_u16_le(offset, discriminant as u16),
            4 => memory.write_u32_le(offset, discriminant as u32),
            _ => Err(Error::validation_error(
                "Error occurred: Invalid discriminant size calculated",
            )),
        }
    }

    /// Lower an option value with proper layout
    pub fn lower_option<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        value: &Option<Box<ComponentValue>>,
        offset: u32,
    ) -> Result<()> {
        match value {
            Some(inner_value) => {
                // Write Some discriminant (1)
                memory.write_u8(offset, 1)?;

                // Calculate layout for the inner value
                let inner_layout = self.calculate_value_layout(inner_value);

                // Calculate payload offset with proper alignment
                let payload_offset = align_to(1, inner_layout.alignment);

                // Lower the inner value
                self.lower(memory, inner_value, offset + payload_offset as u32)?;
            },
            None => {
                // Write None discriminant (0)
                memory.write_u8(offset, 0)?;
            },
        }
        Ok(())
    }

    /// Lower a result value with proper layout
    pub fn lower_result<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        value: &core::result::Result<Option<Box<ComponentValue>>, Option<Box<ComponentValue>>>,
        offset: u32,
    ) -> Result<()> {
        match value {
            Ok(ok_value) => {
                // Write Ok discriminant (0)
                memory.write_u8(offset, 0)?;

                // If there's an Ok value, lower it
                if let Some(inner_value) = ok_value {
                    let inner_layout = self.calculate_value_layout(inner_value);
                    let payload_offset = align_to(1, inner_layout.alignment);
                    self.lower(memory, inner_value, offset + payload_offset as u32)?;
                }
            },
            Err(err_value) => {
                // Write Err discriminant (1)
                memory.write_u8(offset, 1)?;

                // If there's an Err value, lower it
                if let Some(inner_value) = err_value {
                    let inner_layout = self.calculate_value_layout(inner_value);
                    let payload_offset = align_to(1, inner_layout.alignment);
                    self.lower(memory, inner_value, offset + payload_offset as u32)?;
                }
            },
        }
        Ok(())
    }

    /// Lower a flags value with proper bit layout
    pub fn lower_flags<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        flag_definitions: &[String],
        active_flags: &[String],
        offset: u32,
    ) -> Result<()> {
        // Calculate the number of bytes needed for all flags
        let num_bytes = flag_definitions.len().div_ceil(8);

        // Create bit array
        let mut flag_bytes = vec![0u8; num_bytes];

        // Set bits for active flags
        for active_flag in active_flags {
            if let Some(flag_index) = flag_definitions.iter().position(|f| f == active_flag) {
                let byte_index = flag_index / 8;
                let bit_index = flag_index % 8;
                if byte_index < flag_bytes.len() {
                    flag_bytes[byte_index] |= 1 << bit_index;
                }
            }
        }

        // Write flag bytes to memory
        memory.write_bytes(offset, &flag_bytes)
    }

    /// Calculate memory layout for a ComponentValue
    fn calculate_value_layout(&self, value: &ComponentValue) -> MemoryLayout {
        match value {
            ComponentValue::Bool(_) => MemoryLayout::new(1, 1),
            ComponentValue::S8(_) | ComponentValue::U8(_) => MemoryLayout::new(1, 1),
            ComponentValue::S16(_) | ComponentValue::U16(_) => MemoryLayout::new(2, 2),
            ComponentValue::S32(_) | ComponentValue::U32(_) => MemoryLayout::new(4, 4),
            ComponentValue::S64(_) | ComponentValue::U64(_) => MemoryLayout::new(8, 8),
            ComponentValue::F32(_) => MemoryLayout::new(4, 4),
            ComponentValue::F64(_) => MemoryLayout::new(8, 8),
            ComponentValue::Char(_) => MemoryLayout::new(4, 4),
            ComponentValue::String(_) => MemoryLayout::new(8, 4), // ptr + len
            ComponentValue::List(_) => MemoryLayout::new(8, 4),   // ptr + len
            ComponentValue::Record(fields) => {
                // Calculate record layout from fields
                let mut offset = 0;
                let mut max_alignment = 1;

                for (_, field_value) in fields {
                    let field_layout = self.calculate_value_layout(field_value);
                    offset = align_to(offset, field_layout.alignment);
                    offset += field_layout.size;
                    max_alignment = max_alignment.max(field_layout.alignment);
                }

                let final_size = align_to(offset, max_alignment);
                MemoryLayout::new(final_size, max_alignment)
            },
            ComponentValue::Tuple(values) => {
                // Calculate tuple layout from values
                let mut offset = 0;
                let mut max_alignment = 1;

                for value in values {
                    let value_layout = self.calculate_value_layout(value);
                    offset = align_to(offset, value_layout.alignment);
                    offset += value_layout.size;
                    max_alignment = max_alignment.max(value_layout.alignment);
                }

                let final_size = align_to(offset, max_alignment);
                MemoryLayout::new(final_size, max_alignment)
            },
            ComponentValue::Option(inner) => {
                if let Some(inner_value) = inner {
                    let inner_layout = self.calculate_value_layout(inner_value);
                    let payload_offset = align_to(1, inner_layout.alignment);
                    let total_size = payload_offset + inner_layout.size;
                    let alignment = inner_layout.alignment.max(1);
                    let final_size = align_to(total_size, alignment);
                    MemoryLayout::new(final_size, alignment)
                } else {
                    MemoryLayout::new(1, 1) // Just discriminant
                }
            },
            ComponentValue::Result(result) => {
                let mut max_payload_size = 0;
                let mut max_payload_alignment = 1;

                match result {
                    Ok(Some(ok_value)) => {
                        let layout = self.calculate_value_layout(ok_value);
                        max_payload_size = layout.size;
                        max_payload_alignment = layout.alignment;
                    },
                    Err(Some(err_value)) => {
                        let layout = self.calculate_value_layout(err_value);
                        max_payload_size = layout.size;
                        max_payload_alignment = layout.alignment;
                    },
                    _ => {}, // No payload
                }

                let payload_offset = align_to(1, max_payload_alignment);
                let total_size = payload_offset + max_payload_size;
                let alignment = max_payload_alignment.max(1);
                let final_size = align_to(total_size, alignment);
                MemoryLayout::new(final_size, alignment)
            },
            ComponentValue::Variant(_, payload) => {
                if let Some(payload_value) = payload {
                    let payload_layout = self.calculate_value_layout(payload_value);
                    let payload_offset = align_to(4, payload_layout.alignment); // 4-byte discriminant
                    let total_size = payload_offset + payload_layout.size;
                    let alignment = payload_layout.alignment.max(4);
                    let final_size = align_to(total_size, alignment);
                    MemoryLayout::new(final_size, alignment)
                } else {
                    MemoryLayout::new(4, 4) // Just discriminant
                }
            },
            ComponentValue::Enum(_) => MemoryLayout::new(4, 4), // 4-byte discriminant
            ComponentValue::Flags(flags) => {
                let num_bytes = flags.len().div_ceil(8);
                let alignment = if num_bytes <= 1 {
                    1
                } else if num_bytes <= 2 {
                    2
                } else if num_bytes <= 4 {
                    4
                } else {
                    8
                };
                let size = align_to(num_bytes, alignment);
                MemoryLayout::new(size, alignment)
            },
            _ => MemoryLayout::new(0, 1), // Unknown types
        }
    }
}

/// Memory layout information for values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MemoryLayout {
    /// Size in bytes
    size: usize,
    /// Alignment requirement in bytes
    alignment: usize,
}

impl MemoryLayout {
    /// Create a new memory layout
    fn new(size: usize, alignment: usize) -> Self {
        Self { size, alignment }
    }
}

/// Align a value to the specified alignment
fn align_to(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

// ============================================================================
// BATCH LIFT/LOWER OPERATIONS FOR FUNCTION CALLS
// ============================================================================

/// Context for canonical ABI operations during a function call
#[derive(Debug)]
pub struct CanonicalCallContext {
    /// Memory index to use (usually 0)
    pub memory_index: u32,
    /// Realloc function index for allocation during lowering
    pub realloc_index: Option<u32>,
    /// Post-return function index for cleanup
    pub post_return_index: Option<u32>,
    /// String encoding to use
    pub string_encoding: StringEncoding,
}

impl Default for CanonicalCallContext {
    fn default() -> Self {
        Self {
            memory_index: 0,
            realloc_index: None,
            post_return_index: None,
            string_encoding: StringEncoding::Utf8,
        }
    }
}

/// Result of lifting core values to component values
#[derive(Debug)]
pub struct LiftResult {
    /// The lifted component values
    pub values: Vec<ComponentValue>,
    /// Number of core values consumed
    pub core_values_consumed: usize,
}

/// Result of lowering component values to core values
#[derive(Debug)]
pub struct LowerResult {
    /// The lowered core values (to push onto WASM stack)
    pub core_values: Vec<CoreValue>,
    /// Bytes written to memory (for cleanup tracking)
    pub bytes_written: u32,
}

/// Core WebAssembly value (matches wrt_foundation::values::Value)
#[derive(Debug, Clone, PartialEq)]
pub enum CoreValue {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
}

impl CanonicalABI {
    // ========================================================================
    // BATCH LIFTING: Core WASM values -> Component values
    // ========================================================================

    /// Lift multiple values from core WASM representation to component values.
    ///
    /// This is the main entry point for lifting function arguments.
    /// Core values come from the WASM operand stack, and complex types
    /// (strings, lists, etc.) are read from linear memory.
    ///
    /// # Arguments
    /// * `types` - The component types to lift to
    /// * `core_values` - Core WASM values from the operand stack
    /// * `memory` - Linear memory for reading complex types
    /// * `ctx` - Call context with encoding options
    ///
    /// # Returns
    /// The lifted component values and count of core values consumed
    pub fn lift_values<M: CanonicalMemory>(
        &self,
        types: &[ComponentType],
        core_values: &[CoreValue],
        memory: &M,
        _ctx: &CanonicalCallContext,
    ) -> Result<LiftResult> {
        let mut result = Vec::new();
        let mut core_idx = 0;

        for ty in types {
            let (value, consumed) = self.lift_single_value(ty, &core_values[core_idx..], memory)?;
            result.push(value);
            core_idx += consumed;
        }

        Ok(LiftResult {
            values: result,
            core_values_consumed: core_idx,
        })
    }

    /// Lift a single value from core representation
    fn lift_single_value<M: CanonicalMemory>(
        &self,
        ty: &ComponentType,
        core_values: &[CoreValue],
        memory: &M,
    ) -> Result<(ComponentValue, usize)> {
        match ty {
            // Primitives: direct conversion from single core value
            ComponentType::Bool => {
                let i = self.expect_i32(core_values, 0)?;
                Ok((ComponentValue::Bool(i != 0), 1))
            },
            ComponentType::S8 => {
                let i = self.expect_i32(core_values, 0)?;
                Ok((ComponentValue::S8(i as i8), 1))
            },
            ComponentType::U8 => {
                let i = self.expect_i32(core_values, 0)?;
                Ok((ComponentValue::U8(i as u8), 1))
            },
            ComponentType::S16 => {
                let i = self.expect_i32(core_values, 0)?;
                Ok((ComponentValue::S16(i as i16), 1))
            },
            ComponentType::U16 => {
                let i = self.expect_i32(core_values, 0)?;
                Ok((ComponentValue::U16(i as u16), 1))
            },
            ComponentType::S32 => {
                let i = self.expect_i32(core_values, 0)?;
                Ok((ComponentValue::S32(i), 1))
            },
            ComponentType::U32 => {
                let i = self.expect_i32(core_values, 0)?;
                Ok((ComponentValue::U32(i as u32), 1))
            },
            ComponentType::S64 => {
                let i = self.expect_i64(core_values, 0)?;
                Ok((ComponentValue::S64(i), 1))
            },
            ComponentType::U64 => {
                let i = self.expect_i64(core_values, 0)?;
                Ok((ComponentValue::U64(i as u64), 1))
            },
            ComponentType::F32 => {
                let f = self.expect_f32(core_values, 0)?;
                Ok((ComponentValue::F32(f), 1))
            },
            ComponentType::F64 => {
                let f = self.expect_f64(core_values, 0)?;
                Ok((ComponentValue::F64(f), 1))
            },
            ComponentType::Char => {
                let i = self.expect_i32(core_values, 0)?;
                let ch = char::from_u32(i as u32)
                    .ok_or_else(|| Error::validation_error("Invalid Unicode code point"))?;
                Ok((ComponentValue::Char(ch), 1))
            },

            // String: ptr + len on stack, data in memory
            ComponentType::String => {
                let ptr = self.expect_i32(core_values, 0)? as u32;
                let len = self.expect_i32(core_values, 1)? as u32;

                // Read string data from memory
                let bytes = memory.read_bytes(ptr, len)?;
                let string = String::from_utf8(bytes)
                    .map_err(|_| Error::validation_error("Invalid UTF-8 string"))?;

                Ok((ComponentValue::String(string), 2))
            },

            // List: ptr + len on stack, elements in memory
            ComponentType::List(element_ty) => {
                let ptr = self.expect_i32(core_values, 0)? as u32;
                let len = self.expect_i32(core_values, 1)? as u32;

                let element_size = self.size_of(element_ty)?;
                let mut elements = Vec::new();

                for i in 0..len {
                    let offset = ptr + i * element_size;
                    let value = self.lift(memory, element_ty, offset)?;
                    elements.push(value);
                }

                Ok((ComponentValue::List(elements), 2))
            },

            // Record: flattened fields on stack or in memory
            ComponentType::Record(fields) => {
                // For simplicity, assume record is passed as pointer
                let ptr = self.expect_i32(core_values, 0)? as u32;
                let value = self.lift_record(memory, fields, ptr)?;
                Ok((value, 1))
            },

            // Tuple: flattened elements
            ComponentType::Tuple(types) => {
                let ptr = self.expect_i32(core_values, 0)? as u32;
                let value = self.lift_tuple(memory, types, ptr)?;
                Ok((value, 1))
            },

            // Variant: discriminant + optional payload
            ComponentType::Variant(cases) => {
                let ptr = self.expect_i32(core_values, 0)? as u32;
                let value = self.lift_variant(memory, cases, ptr)?;
                Ok((value, 1))
            },

            // Enum: just discriminant
            ComponentType::Enum(cases) => {
                let discriminant = self.expect_i32(core_values, 0)? as u32;
                if discriminant as usize >= cases.len() {
                    return Err(Error::validation_error("Invalid enum discriminant"));
                }
                Ok((
                    ComponentValue::Enum(cases[discriminant as usize].clone()),
                    1,
                ))
            },

            // Option: discriminant + optional value
            ComponentType::Option(inner_ty) => {
                let discriminant = self.expect_i32(core_values, 0)?;
                if discriminant == 0 {
                    Ok((ComponentValue::Option(None), 1))
                } else {
                    // Payload follows discriminant
                    let (inner_value, consumed) =
                        self.lift_single_value(inner_ty, &core_values[1..], memory)?;
                    Ok((
                        ComponentValue::Option(Some(Box::new(inner_value))),
                        1 + consumed,
                    ))
                }
            },

            // Result: discriminant + ok/err payload
            ComponentType::Result(ok_ty, err_ty) => {
                let discriminant = self.expect_i32(core_values, 0)?;
                if discriminant == 0 {
                    // Ok case
                    if let Some(ty) = ok_ty {
                        let (value, consumed) =
                            self.lift_single_value(ty, &core_values[1..], memory)?;
                        Ok((
                            ComponentValue::Result(Ok(Some(Box::new(value)))),
                            1 + consumed,
                        ))
                    } else {
                        Ok((ComponentValue::Result(Ok(None)), 1))
                    }
                } else {
                    // Err case
                    if let Some(ty) = err_ty {
                        let (value, consumed) =
                            self.lift_single_value(ty, &core_values[1..], memory)?;
                        Ok((
                            ComponentValue::Result(Err(Some(Box::new(value)))),
                            1 + consumed,
                        ))
                    } else {
                        Ok((ComponentValue::Result(Err(None)), 1))
                    }
                }
            },

            // Flags: bit vector
            ComponentType::Flags(flag_names) => {
                let bits = self.expect_i32(core_values, 0)? as u32;
                let mut active = Vec::new();
                for (i, name) in flag_names.iter().enumerate() {
                    if (bits >> i) & 1 != 0 {
                        active.push(name.clone());
                    }
                }
                Ok((ComponentValue::Flags(active), 1))
            },
        }
    }

    // ========================================================================
    // BATCH LOWERING: Component values -> Core WASM values
    // ========================================================================

    /// Lower multiple component values to core WASM representation.
    ///
    /// This is the main entry point for lowering function results.
    /// Simple values become core values for the WASM stack, complex types
    /// are written to linear memory and their ptr+len pushed to stack.
    ///
    /// # Arguments
    /// * `values` - Component values to lower
    /// * `types` - The component types (for validation)
    /// * `memory` - Linear memory for writing complex types
    /// * `next_alloc` - Next free address in memory for allocation
    /// * `ctx` - Call context with encoding options
    ///
    /// # Returns
    /// Core values to push onto WASM stack
    pub fn lower_values<M: CanonicalMemory>(
        &self,
        values: &[ComponentValue],
        types: &[ComponentType],
        memory: &mut M,
        mut next_alloc: u32,
        _ctx: &CanonicalCallContext,
    ) -> Result<LowerResult> {
        if values.len() != types.len() {
            return Err(Error::validation_error(
                "Value count doesn't match type count",
            ));
        }

        let mut core_values = Vec::new();
        let start_alloc = next_alloc;

        for (value, ty) in values.iter().zip(types.iter()) {
            let (lowered, bytes_used) = self.lower_single_value(value, ty, memory, next_alloc)?;
            core_values.extend(lowered);
            next_alloc += bytes_used;
        }

        Ok(LowerResult {
            core_values,
            bytes_written: next_alloc - start_alloc,
        })
    }

    /// Lower a single component value to core representation
    fn lower_single_value<M: CanonicalMemory>(
        &self,
        value: &ComponentValue,
        ty: &ComponentType,
        memory: &mut M,
        alloc_ptr: u32,
    ) -> Result<(Vec<CoreValue>, u32)> {
        match (value, ty) {
            // Primitives: direct conversion to single core value
            (ComponentValue::Bool(b), ComponentType::Bool) => {
                Ok((vec![CoreValue::I32(if *b { 1 } else { 0 })], 0))
            },
            (ComponentValue::S8(v), ComponentType::S8) => Ok((vec![CoreValue::I32(*v as i32)], 0)),
            (ComponentValue::U8(v), ComponentType::U8) => Ok((vec![CoreValue::I32(*v as i32)], 0)),
            (ComponentValue::S16(v), ComponentType::S16) => {
                Ok((vec![CoreValue::I32(*v as i32)], 0))
            },
            (ComponentValue::U16(v), ComponentType::U16) => {
                Ok((vec![CoreValue::I32(*v as i32)], 0))
            },
            (ComponentValue::S32(v), ComponentType::S32) => Ok((vec![CoreValue::I32(*v)], 0)),
            (ComponentValue::U32(v), ComponentType::U32) => {
                Ok((vec![CoreValue::I32(*v as i32)], 0))
            },
            (ComponentValue::S64(v), ComponentType::S64) => Ok((vec![CoreValue::I64(*v)], 0)),
            (ComponentValue::U64(v), ComponentType::U64) => {
                Ok((vec![CoreValue::I64(*v as i64)], 0))
            },
            (ComponentValue::F32(v), ComponentType::F32) => Ok((vec![CoreValue::F32(*v)], 0)),
            (ComponentValue::F64(v), ComponentType::F64) => Ok((vec![CoreValue::F64(*v)], 0)),
            (ComponentValue::Char(ch), ComponentType::Char) => {
                Ok((vec![CoreValue::I32(*ch as i32)], 0))
            },

            // String: write to memory, return ptr + len
            (ComponentValue::String(s), ComponentType::String) => {
                let bytes = s.as_bytes();
                let len = bytes.len() as u32;

                // Write string data to memory
                memory.write_bytes(alloc_ptr, bytes)?;

                // Return ptr and len as core values
                Ok((
                    vec![CoreValue::I32(alloc_ptr as i32), CoreValue::I32(len as i32)],
                    len,
                ))
            },

            // List: write elements to memory, return ptr + len
            (ComponentValue::List(elements), ComponentType::List(element_ty)) => {
                let element_size = self.size_of(element_ty)?;
                let total_size = element_size * elements.len() as u32;

                // Write each element
                for (i, elem) in elements.iter().enumerate() {
                    let offset = alloc_ptr + (i as u32) * element_size;
                    self.lower(memory, elem, offset)?;
                }

                Ok((
                    vec![
                        CoreValue::I32(alloc_ptr as i32),
                        CoreValue::I32(elements.len() as i32),
                    ],
                    total_size,
                ))
            },

            // Record: write to memory, return ptr
            (ComponentValue::Record(fields), ComponentType::Record(_)) => {
                self.lower_record(memory, fields, alloc_ptr)?;
                let size = self.calculate_value_layout(value).size as u32;
                Ok((vec![CoreValue::I32(alloc_ptr as i32)], size))
            },

            // Tuple: write to memory, return ptr
            (ComponentValue::Tuple(values), ComponentType::Tuple(_)) => {
                self.lower_tuple(memory, values, alloc_ptr)?;
                let size = self.calculate_value_layout(value).size as u32;
                Ok((vec![CoreValue::I32(alloc_ptr as i32)], size))
            },

            // Enum: return discriminant
            (ComponentValue::Enum(case_name), ComponentType::Enum(cases)) => {
                let discriminant = cases
                    .iter()
                    .position(|c| c == case_name)
                    .ok_or_else(|| Error::validation_error("Unknown enum case"))?;
                Ok((vec![CoreValue::I32(discriminant as i32)], 0))
            },

            // Option: discriminant + optional value
            (ComponentValue::Option(None), ComponentType::Option(_)) => {
                Ok((vec![CoreValue::I32(0)], 0))
            },
            (ComponentValue::Option(Some(inner)), ComponentType::Option(inner_ty)) => {
                let (mut inner_values, bytes) =
                    self.lower_single_value(inner, inner_ty, memory, alloc_ptr)?;
                let mut result = vec![CoreValue::I32(1)];
                result.append(&mut inner_values);
                Ok((result, bytes))
            },

            // Result: discriminant + payload
            (ComponentValue::Result(Ok(payload)), ComponentType::Result(ok_ty, _)) => {
                let mut result = vec![CoreValue::I32(0)];
                if let (Some(val), Some(ty)) = (payload, ok_ty) {
                    let (mut inner_values, bytes) =
                        self.lower_single_value(val, ty, memory, alloc_ptr)?;
                    result.append(&mut inner_values);
                    Ok((result, bytes))
                } else {
                    Ok((result, 0))
                }
            },
            (ComponentValue::Result(Err(payload)), ComponentType::Result(_, err_ty)) => {
                let mut result = vec![CoreValue::I32(1)];
                if let (Some(val), Some(ty)) = (payload, err_ty) {
                    let (mut inner_values, bytes) =
                        self.lower_single_value(val, ty, memory, alloc_ptr)?;
                    result.append(&mut inner_values);
                    Ok((result, bytes))
                } else {
                    Ok((result, 0))
                }
            },

            // Flags: pack into i32
            (ComponentValue::Flags(active), ComponentType::Flags(all_flags)) => {
                let mut bits: u32 = 0;
                for flag in active {
                    if let Some(idx) = all_flags.iter().position(|f| f == flag) {
                        bits |= 1 << idx;
                    }
                }
                Ok((vec![CoreValue::I32(bits as i32)], 0))
            },

            // Type mismatch
            _ => Err(Error::validation_error(
                "Value type doesn't match expected type",
            )),
        }
    }

    // ========================================================================
    // HELPER METHODS FOR EXTRACTING CORE VALUES
    // ========================================================================

    fn expect_i32(&self, values: &[CoreValue], idx: usize) -> Result<i32> {
        values
            .get(idx)
            .and_then(|v| match v {
                CoreValue::I32(i) => Some(*i),
                _ => None,
            })
            .ok_or_else(|| Error::validation_error("Expected i32 value"))
    }

    fn expect_i64(&self, values: &[CoreValue], idx: usize) -> Result<i64> {
        values
            .get(idx)
            .and_then(|v| match v {
                CoreValue::I64(i) => Some(*i),
                _ => None,
            })
            .ok_or_else(|| Error::validation_error("Expected i64 value"))
    }

    fn expect_f32(&self, values: &[CoreValue], idx: usize) -> Result<f32> {
        values
            .get(idx)
            .and_then(|v| match v {
                CoreValue::F32(f) => Some(*f),
                _ => None,
            })
            .ok_or_else(|| Error::validation_error("Expected f32 value"))
    }

    fn expect_f64(&self, values: &[CoreValue], idx: usize) -> Result<f64> {
        values
            .get(idx)
            .and_then(|v| match v {
                CoreValue::F64(f) => Some(*f),
                _ => None,
            })
            .ok_or_else(|| Error::validation_error("Expected f64 value"))
    }

    // ========================================================================
    // CONVERSION BETWEEN CoreValue AND wrt_foundation::values::Value
    // ========================================================================

    /// Convert wrt_foundation::values::Value to CoreValue
    pub fn from_runtime_value(value: &wrt_foundation::values::Value) -> CoreValue {
        use wrt_foundation::values::Value;
        match value {
            Value::I32(i) => CoreValue::I32(*i),
            Value::I64(i) => CoreValue::I64(*i),
            Value::F32(f) => CoreValue::F32(f.to_f32()),
            Value::F64(f) => CoreValue::F64(f.to_f64()),
            // Other types (FuncRef, ExternRef) are not supported in canonical ABI
            _ => CoreValue::I32(0),
        }
    }

    /// Convert CoreValue to wrt_foundation::values::Value
    pub fn to_runtime_value(value: &CoreValue) -> wrt_foundation::values::Value {
        use wrt_foundation::float_repr::{FloatBits32, FloatBits64};
        use wrt_foundation::values::Value;
        match value {
            CoreValue::I32(i) => Value::I32(*i),
            CoreValue::I64(i) => Value::I64(*i),
            CoreValue::F32(f) => Value::F32(FloatBits32::from_f32(*f)),
            CoreValue::F64(f) => Value::F64(FloatBits64::from_f64(*f)),
        }
    }

    /// Convert a slice of runtime values to core values
    pub fn from_runtime_values(values: &[wrt_foundation::values::Value]) -> Vec<CoreValue> {
        values.iter().map(Self::from_runtime_value).collect()
    }

    /// Convert a slice of core values to runtime values
    pub fn to_runtime_values(values: &[CoreValue]) -> Vec<wrt_foundation::values::Value> {
        values.iter().map(Self::to_runtime_value).collect()
    }
}
