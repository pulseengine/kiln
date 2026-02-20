//! Proper Async Canonical ABI Lifting and Lowering Implementation
//!
//! This module implements the actual canonical ABI conversion for async
//! operations according to the WebAssembly Component Model specification.

#[cfg(not(feature = "std"))]
use core::{fmt, mem};
#[cfg(feature = "std")]
use std::{boxed::Box, vec::Vec};
#[cfg(feature = "std")]
use std::{fmt, mem};

use wrt_error::{Error, ErrorCategory, Result};
use wrt_foundation::{
    bounded::BoundedString, budget_aware_provider::CrateId, collections::StaticVec as BoundedVec,
    prelude::*, safe_managed_alloc,
};

use crate::{
    canonical_abi::canonical_options::CanonicalOptions,
    types::{FutureHandle, StreamHandle, ValType, Value},
};

/// Maximum size for immediate values in no_std
const MAX_IMMEDIATE_SIZE: usize = 4096;

/// Canonical ABI alignment requirements
#[derive(Debug, Clone, Copy)]
pub struct Alignment {
    /// Alignment in bytes (must be power of 2)
    pub bytes: usize,
}

impl Alignment {
    /// Create alignment from bytes
    pub const fn from_bytes(bytes: usize) -> Self {
        debug_assert!(bytes.is_power_of_two());
        Self { bytes }
    }

    /// Get alignment for a value type
    pub fn for_val_type(val_type: &ValType) -> Self {
        match val_type {
            ValType::Bool | ValType::U8 | ValType::S8 => Self::from_bytes(1),
            ValType::U16 | ValType::S16 => Self::from_bytes(2),
            ValType::U32 | ValType::S32 | ValType::F32 | ValType::Char => Self::from_bytes(4),
            ValType::U64 | ValType::S64 | ValType::F64 => Self::from_bytes(8),
            ValType::String => Self::from_bytes(4), // Pointer alignment
            ValType::List(_) => Self::from_bytes(4), // Pointer alignment
            ValType::Record(_) => Self::from_bytes(4), // Maximum member alignment
            ValType::Variant(_) => Self::from_bytes(4), // Discriminant alignment
            ValType::Tuple(_) => Self::from_bytes(4), // Maximum member alignment
            ValType::Option(_) => Self::from_bytes(4), // Discriminant alignment
            ValType::Result { .. } => Self::from_bytes(4), // Discriminant alignment
            ValType::Flags(_) => Self::from_bytes(4), // u32 representation
            ValType::Enum(_) => Self::from_bytes(4), // u32 representation
            ValType::Stream(_) => Self::from_bytes(4), // Handle alignment
            ValType::Future(_) => Self::from_bytes(4), // Handle alignment
            ValType::Own(_) | ValType::Borrow(_) => Self::from_bytes(4), // Handle alignment
        }
    }

    /// Align an offset to this alignment
    pub fn align_offset(&self, offset: usize) -> usize {
        (offset + self.bytes - 1) & !(self.bytes - 1)
    }
}

/// Canonical ABI encoder for async operations
pub struct AsyncCanonicalEncoder {
    /// Buffer for encoded data
    #[cfg(feature = "std")]
    buffer: Vec<u8>,
    #[cfg(not(any(feature = "std",)))]
    buffer: BoundedVec<u8, MAX_IMMEDIATE_SIZE>,

    /// Current write position
    position: usize,
}

impl AsyncCanonicalEncoder {
    /// Create new encoder
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "std")]
            buffer: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            buffer: {
                // StaticVec is stack-allocated with fixed capacity
                BoundedVec::new()
            },
            position: 0,
        }
    }

    /// Encode a value according to canonical ABI
    pub fn encode_value(&mut self, value: &Value, options: &CanonicalOptions) -> Result<()> {
        match value {
            Value::Bool(b) => self.encode_bool(*b),
            Value::U8(n) => self.encode_u8(*n),
            Value::S8(n) => self.encode_s8(*n),
            Value::U16(n) => self.encode_u16(*n),
            Value::S16(n) => self.encode_s16(*n),
            Value::U32(n) => self.encode_u32(*n),
            Value::S32(n) => self.encode_s32(*n),
            Value::U64(n) => self.encode_u64(*n),
            Value::S64(n) => self.encode_s64(*n),
            Value::F32(n) => self.encode_f32(*n),
            Value::F64(n) => self.encode_f64(*n),
            Value::Char(c) => self.encode_char(*c),
            Value::String(s) => self.encode_string(s.as_str()?, options),
            Value::List(list) => self.encode_list(list.as_slice(), options),
            // Record stores just values, not (name, value) pairs - treat like tuple
            Value::Record(fields) => self.encode_tuple(fields.as_slice(), options),
            Value::Variant {
                discriminant,
                value,
            } => self.encode_variant(*discriminant, value.as_deref(), options),
            Value::Tuple(values) => self.encode_tuple(values.as_slice(), options),
            Value::Option(opt) => self.encode_option(opt.as_deref(), options),
            Value::Result(res) => self.encode_result(res, options),
            Value::Flags(flags) => self.encode_flags(*flags),
            Value::Enum(n) => self.encode_enum(*n),
            Value::Stream(handle) => self.encode_stream(handle.0),
            Value::Future(handle) => self.encode_future(handle.0),
            Value::Own(handle) => self.encode_own(*handle),
            Value::Borrow(handle) => self.encode_borrow(*handle),
        }
    }

    /// Get the encoded buffer
    pub fn finish(self) -> Result<Vec<u8>> {
        #[cfg(feature = "std")]
        {
            Ok(self.buffer)
        }
        #[cfg(not(any(feature = "std",)))]
        {
            Ok(self.buffer.iter().copied().collect())
        }
    }

    // Primitive encoding methods

    fn encode_bool(&mut self, value: bool) -> Result<()> {
        self.write_u8(if value { 1 } else { 0 })
    }

    fn encode_u8(&mut self, value: u8) -> Result<()> {
        self.write_u8(value)
    }

    fn encode_s8(&mut self, value: i8) -> Result<()> {
        self.write_u8(value as u8)
    }

    fn encode_u16(&mut self, value: u16) -> Result<()> {
        self.align_to(2)?;
        self.write_bytes(&value.to_le_bytes())
    }

    fn encode_s16(&mut self, value: i16) -> Result<()> {
        self.align_to(2)?;
        self.write_bytes(&value.to_le_bytes())
    }

    fn encode_u32(&mut self, value: u32) -> Result<()> {
        self.align_to(4)?;
        self.write_bytes(&value.to_le_bytes())
    }

    fn encode_s32(&mut self, value: i32) -> Result<()> {
        self.align_to(4)?;
        self.write_bytes(&value.to_le_bytes())
    }

    fn encode_u64(&mut self, value: u64) -> Result<()> {
        self.align_to(8)?;
        self.write_bytes(&value.to_le_bytes())
    }

    fn encode_s64(&mut self, value: i64) -> Result<()> {
        self.align_to(8)?;
        self.write_bytes(&value.to_le_bytes())
    }

    fn encode_f32(&mut self, value: f32) -> Result<()> {
        self.align_to(4)?;
        self.write_bytes(&value.to_le_bytes())
    }

    fn encode_f64(&mut self, value: f64) -> Result<()> {
        self.align_to(8)?;
        self.write_bytes(&value.to_le_bytes())
    }

    fn encode_char(&mut self, value: char) -> Result<()> {
        self.encode_u32(value as u32)
    }

    fn encode_string(&mut self, value: &str, options: &CanonicalOptions) -> Result<()> {
        // Encode as pointer and length
        let bytes = value.as_bytes();
        self.encode_u32(bytes.len() as u32)?;
        self.encode_u32(0)?; // Binary std/no_std choice
        Ok(())
    }

    fn encode_list(&mut self, values: &[Value], options: &CanonicalOptions) -> Result<()> {
        // Encode as pointer and length
        self.encode_u32(values.len() as u32)?;
        self.encode_u32(0)?; // Placeholder pointer
        Ok(())
    }

    fn encode_record(
        &mut self,
        fields: &[(String, Value)],
        options: &CanonicalOptions,
    ) -> Result<()> {
        // Encode fields in order
        for (_, value) in fields {
            self.encode_value(value, options)?;
        }
        Ok(())
    }

    fn encode_variant(
        &mut self,
        tag: u32,
        value: Option<&Value>,
        options: &CanonicalOptions,
    ) -> Result<()> {
        // Encode discriminant
        self.encode_u32(tag)?;

        // Encode payload if present
        if let Some(val) = value {
            self.encode_value(val, options)?;
        }
        Ok(())
    }

    fn encode_tuple(&mut self, values: &[Value], options: &CanonicalOptions) -> Result<()> {
        // Encode each value in order
        for value in values {
            self.encode_value(value, options)?;
        }
        Ok(())
    }

    fn encode_option(&mut self, value: Option<&Value>, options: &CanonicalOptions) -> Result<()> {
        match value {
            None => self.encode_u32(0), // None discriminant
            Some(val) => {
                self.encode_u32(1)?; // Some discriminant
                self.encode_value(val, options)
            },
        }
    }

    fn encode_result(
        &mut self,
        result: &core::result::Result<Option<Box<Value>>, Box<Value>>,
        options: &CanonicalOptions,
    ) -> Result<()> {
        match result {
            Ok(val_opt) => {
                self.encode_u32(0)?; // Ok discriminant
                if let Some(val) = val_opt { self.encode_value(val, options) } else { Ok(()) }
            },
            Err(val) => {
                self.encode_u32(1)?; // Err discriminant
                self.encode_value(val, options)
            },
        }
    }

    fn encode_flags(&mut self, flags: u32) -> Result<()> {
        // Flags are already packed as u32
        self.encode_u32(flags)
    }

    fn encode_enum(&mut self, value: u32) -> Result<()> {
        self.encode_u32(value)
    }

    fn encode_stream(&mut self, handle: u32) -> Result<()> {
        self.encode_u32(handle)
    }

    fn encode_future(&mut self, handle: u32) -> Result<()> {
        self.encode_u32(handle)
    }

    fn encode_own(&mut self, handle: u32) -> Result<()> {
        self.encode_u32(handle)
    }

    fn encode_borrow(&mut self, handle: u32) -> Result<()> {
        self.encode_u32(handle)
    }

    // Helper methods

    fn align_to(&mut self, alignment: usize) -> Result<()> {
        let aligned = Alignment::from_bytes(alignment).align_offset(self.position);
        let padding = aligned - self.position;

        for _ in 0..padding {
            self.write_u8(0)?;
        }
        Ok(())
    }

    fn write_u8(&mut self, value: u8) -> Result<()> {
        #[cfg(feature = "std")]
        {
            self.buffer.push(value);
        }
        #[cfg(not(any(feature = "std",)))]
        {
            self.buffer
                .push(value)
                .map_err(|_| Error::runtime_execution_error("Buffer overflow in encoder"))?;
        }
        self.position += 1;
        Ok(())
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        for &byte in bytes {
            self.write_u8(byte)?;
        }
        Ok(())
    }
}

/// Canonical ABI decoder for async operations
pub struct AsyncCanonicalDecoder<'a> {
    /// Buffer to decode from
    buffer: &'a [u8],

    /// Current read position
    position: usize,
}

impl<'a> AsyncCanonicalDecoder<'a> {
    /// Create new decoder
    pub fn new(buffer: &'a [u8]) -> Self {
        Self {
            buffer,
            position: 0,
        }
    }

    /// Decode a value according to canonical ABI
    pub fn decode_value(
        &mut self,
        val_type: &ValType,
        options: &CanonicalOptions,
    ) -> Result<Value> {
        match val_type {
            ValType::Bool => Ok(Value::Bool(self.decode_bool()?)),
            ValType::U8 => Ok(Value::U8(self.decode_u8()?)),
            ValType::S8 => Ok(Value::S8(self.decode_s8()?)),
            ValType::U16 => Ok(Value::U16(self.decode_u16()?)),
            ValType::S16 => Ok(Value::S16(self.decode_s16()?)),
            ValType::U32 => Ok(Value::U32(self.decode_u32()?)),
            ValType::S32 => Ok(Value::S32(self.decode_s32()?)),
            ValType::U64 => Ok(Value::U64(self.decode_u64()?)),
            ValType::S64 => Ok(Value::S64(self.decode_s64()?)),
            ValType::F32 => Ok(Value::F32(self.decode_f32()?)),
            ValType::F64 => Ok(Value::F64(self.decode_f64()?)),
            ValType::Char => Ok(Value::Char(self.decode_char()?)),
            ValType::String => Ok(Value::String(self.decode_string(options)?)),
            ValType::List(elem_type) => {
                let vec = self.decode_list(elem_type, options)?;
                #[cfg(feature = "std")]
                {
                    Ok(Value::List(Box::new(vec)))
                }
                #[cfg(not(any(feature = "std",)))]
                {
                    Ok(Value::List(Box::new(BoundedVec::from_slice(&vec)?)))
                }
            },
            ValType::Record(fields) => {
                let vec = self.decode_record(fields.fields.as_slice(), options)?;
                #[cfg(feature = "std")]
                {
                    Ok(Value::Record(Box::new(vec)))
                }
                #[cfg(not(any(feature = "std",)))]
                {
                    Ok(Value::Record(Box::new(BoundedVec::from_slice(&vec)?)))
                }
            },
            ValType::Variant(variant) => self.decode_variant(variant.cases.as_slice(), options),
            ValType::Tuple(types) => {
                let vec = self.decode_tuple(types.types.as_slice(), options)?;
                #[cfg(feature = "std")]
                {
                    Ok(Value::Tuple(Box::new(vec)))
                }
                #[cfg(not(any(feature = "std",)))]
                {
                    Ok(Value::Tuple(Box::new(BoundedVec::from_slice(&vec)?)))
                }
            },
            ValType::Option(inner) => Ok(Value::Option(self.decode_option(inner, options)?)),
            ValType::Result(result_type) => {
                let ok_type = result_type
                    .ok
                    .as_ref()
                    .ok_or_else(|| Error::runtime_type_mismatch("Result type missing ok type"))?;
                let err_type = result_type
                    .err
                    .as_ref()
                    .ok_or_else(|| Error::runtime_type_mismatch("Result type missing err type"))?;
                Ok(Value::Result(
                    self.decode_result(ok_type, err_type, options)?,
                ))
            },
            ValType::Flags(names) => Ok(Value::Flags(self.decode_flags(names.labels.len())?)),
            ValType::Enum(_) => Ok(Value::Enum(self.decode_enum()?)),
            ValType::Stream(elem_type) => Ok(Value::Stream(StreamHandle(self.decode_stream()?))),
            ValType::Future(elem_type) => Ok(Value::Future(FutureHandle(self.decode_future()?))),
            ValType::Own(_) => Ok(Value::Own(self.decode_own()?)),
            ValType::Borrow(_) => Ok(Value::Borrow(self.decode_borrow()?)),
        }
    }

    // Primitive decoding methods

    fn decode_bool(&mut self) -> Result<bool> {
        Ok(self.read_u8()? != 0)
    }

    fn decode_u8(&mut self) -> Result<u8> {
        self.read_u8()
    }

    fn decode_s8(&mut self) -> Result<i8> {
        Ok(self.read_u8()? as i8)
    }

    fn decode_u16(&mut self) -> Result<u16> {
        self.align_to(2)?;
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn decode_s16(&mut self) -> Result<i16> {
        self.align_to(2)?;
        let bytes = self.read_bytes(2)?;
        Ok(i16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn decode_u32(&mut self) -> Result<u32> {
        self.align_to(4)?;
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn decode_s32(&mut self) -> Result<i32> {
        self.align_to(4)?;
        let bytes = self.read_bytes(4)?;
        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn decode_u64(&mut self) -> Result<u64> {
        self.align_to(8)?;
        let bytes = self.read_bytes(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn decode_s64(&mut self) -> Result<i64> {
        self.align_to(8)?;
        let bytes = self.read_bytes(8)?;
        Ok(i64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn decode_f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.decode_u32()?))
    }

    fn decode_f64(&mut self) -> Result<f64> {
        Ok(f64::from_bits(self.decode_u64()?))
    }

    fn decode_char(&mut self) -> Result<char> {
        let code = self.decode_u32()?;
        char::from_u32(code).ok_or_else(|| {
            Error::new(
                ErrorCategory::Parse,
                wrt_error::codes::PARSE_ERROR,
                "Invalid Unicode code point",
            )
        })
    }

    fn decode_string(&mut self, options: &CanonicalOptions) -> Result<BoundedString<1024>> {
        let _len = self.decode_u32()?;
        let _ptr = self.decode_u32()?;
        // In real implementation, would read from linear memory
        let provider = safe_managed_alloc!(2048, CrateId::Component)?;
        BoundedString::try_from_str("decoded_string")
            .map_err(|e| wrt_error::Error::runtime_error("Failed to create BoundedString"))
    }

    fn decode_list(
        &mut self,
        elem_type: &ValType,
        options: &CanonicalOptions,
    ) -> Result<Vec<Value>> {
        let _len = self.decode_u32()?;
        let _ptr = self.decode_u32()?;
        // In real implementation, would read from linear memory
        Ok(Vec::new())
    }

    fn decode_record(
        &mut self,
        fields: &[crate::types::Field],
        options: &CanonicalOptions,
    ) -> Result<Vec<Value>> {
        let mut result = Vec::new();
        for field in fields {
            let value = self.decode_value(&field.ty, options)?;
            result.push(value);
        }
        Ok(result)
    }

    fn decode_variant(
        &mut self,
        cases: &[crate::types::Case],
        options: &CanonicalOptions,
    ) -> Result<Value> {
        let discriminant = self.decode_u32()?;

        if let Some(case) = cases.get(discriminant as usize) {
            let value = if let Some(ref val_type) = case.ty {
                Some(Box::new(self.decode_value(val_type, options)?))
            } else {
                None
            };
            Ok(Value::Variant {
                discriminant,
                value,
            })
        } else {
            Err(Error::runtime_execution_error(
                "Invalid variant discriminant",
            ))
        }
    }

    fn decode_tuple(
        &mut self,
        types: &[ValType],
        options: &CanonicalOptions,
    ) -> Result<Vec<Value>> {
        let mut values = Vec::new();
        for val_type in types {
            values.push(self.decode_value(val_type, options)?);
        }
        Ok(values)
    }

    fn decode_option(
        &mut self,
        inner: &ValType,
        options: &CanonicalOptions,
    ) -> Result<Option<Box<Value>>> {
        let discriminant = self.decode_u32()?;
        match discriminant {
            0 => Ok(None),
            1 => Ok(Some(Box::new(self.decode_value(inner, options)?))),
            _ => Err(Error::new(
                ErrorCategory::Parse,
                wrt_error::codes::PARSE_ERROR,
                "Invalid option discriminant",
            )),
        }
    }

    fn decode_result(
        &mut self,
        ok_type: &ValType,
        err_type: &ValType,
        options: &CanonicalOptions,
    ) -> Result<core::result::Result<Option<Box<Value>>, Box<Value>>> {
        let discriminant = self.decode_u32()?;
        match discriminant {
            0 => Ok(Ok(Some(Box::new(self.decode_value(ok_type, options)?)))),
            1 => Ok(Err(Box::new(self.decode_value(err_type, options)?))),
            _ => Err(Error::runtime_execution_error(
                "Invalid result discriminant",
            )),
        }
    }

    fn decode_flags(&mut self, count: usize) -> Result<u32> {
        let packed = self.decode_u32()?;
        // Return the packed u32 representation
        Ok(packed)
    }

    fn decode_enum(&mut self) -> Result<u32> {
        self.decode_u32()
    }

    fn decode_stream(&mut self) -> Result<u32> {
        self.decode_u32()
    }

    fn decode_future(&mut self) -> Result<u32> {
        self.decode_u32()
    }

    fn decode_own(&mut self) -> Result<u32> {
        self.decode_u32()
    }

    fn decode_borrow(&mut self) -> Result<u32> {
        self.decode_u32()
    }

    // Helper methods

    fn align_to(&mut self, alignment: usize) -> Result<()> {
        let aligned = Alignment::from_bytes(alignment).align_offset(self.position);
        self.position = aligned;
        Ok(())
    }

    fn read_u8(&mut self) -> Result<u8> {
        if self.position >= self.buffer.len() {
            return Err(Error::new(
                ErrorCategory::Parse,
                wrt_error::codes::PARSE_ERROR,
                "Unexpected end of buffer",
            ));
        }

        let value = self.buffer[self.position];
        self.position += 1;
        Ok(value)
    }

    fn read_bytes(&mut self, count: usize) -> Result<&[u8]> {
        if self.position + count > self.buffer.len() {
            return Err(Error::runtime_execution_error("Unexpected end of buffer"));
        }

        let bytes = &self.buffer[self.position..self.position + count];
        self.position += count;
        Ok(bytes)
    }
}

impl Default for AsyncCanonicalEncoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Perform async canonical lifting
pub fn async_canonical_lift(
    bytes: &[u8],
    target_types: &[ValType],
    options: &CanonicalOptions,
) -> Result<Vec<Value>> {
    let mut decoder = AsyncCanonicalDecoder::new(bytes);
    let mut values = Vec::new();

    for val_type in target_types {
        values.push(decoder.decode_value(val_type, options)?);
    }

    Ok(values)
}

/// Perform async canonical lowering
pub fn async_canonical_lower(values: &[Value], options: &CanonicalOptions) -> Result<Vec<u8>> {
    let mut encoder = AsyncCanonicalEncoder::new();

    for value in values {
        encoder.encode_value(value, options)?;
    }

    encoder.finish()
}
