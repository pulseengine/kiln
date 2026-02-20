//! Component Model value handling
//!
//! This module provides implementations for Component Model value types,
//! including serialization/deserialization, conversion, and runtime
//! representation.

// Import the various types we need explicitly to avoid confusion
use wrt_format::component::FormatValType;
use wrt_foundation::{
    budget_aware_provider::CrateId,
    component_value::{ComponentValue, ValType as TypesValType, ValTypeRef},
    safe_managed_alloc,
    traits::{ReadStream, WriteStream},
    values::Value,
};

use crate::{bounded_component_infra::ComponentProvider, prelude::*};

// Define type aliases with the component memory provider
type ComponentMemoryProvider = ComponentProvider; // Use the shared provider
type ComponentValType = TypesValType<ComponentMemoryProvider>;
type ComponentComponentValue = ComponentValue<ComponentMemoryProvider>;
type WrtFormatValType = FormatValType;
type WrtTypesValType = TypesValType<ComponentProvider>;

use crate::memory_layout::{MemoryLayout, calculate_layout};

// Use ComponentValType for the canonical ValType
type CanonicalValType = ComponentValType;

/// Convert from CanonicalValType to wrt_format::component::ValType
pub fn convert_common_to_format_valtype(common_type: &CanonicalValType) -> WrtFormatValType {
    match common_type {
        CanonicalValType::Bool => WrtFormatValType::Bool,
        CanonicalValType::S8 => WrtFormatValType::S8,
        CanonicalValType::U8 => WrtFormatValType::U8,
        CanonicalValType::S16 => WrtFormatValType::S16,
        CanonicalValType::U16 => WrtFormatValType::U16,
        CanonicalValType::S32 => WrtFormatValType::S32,
        CanonicalValType::U32 => WrtFormatValType::U32,
        CanonicalValType::S64 => WrtFormatValType::S64,
        CanonicalValType::U64 => WrtFormatValType::U64,
        CanonicalValType::F32 => WrtFormatValType::F32,
        CanonicalValType::F64 => WrtFormatValType::F64,
        CanonicalValType::Char => WrtFormatValType::Char,
        CanonicalValType::String => WrtFormatValType::String,
        CanonicalValType::Ref(idx) => WrtFormatValType::Ref(*idx),
        CanonicalValType::Record(_fields) => {
            // Record uses ValTypeRef and WasmName which need resolution
            // For now, return empty record
            WrtFormatValType::Record(Vec::new())
        },
        CanonicalValType::Variant(_cases) => {
            // Variant uses ValTypeRef and WasmName which need resolution
            // For now, return empty variant
            WrtFormatValType::Variant(Vec::new())
        },
        CanonicalValType::List(_elem_type_ref) => {
            // List uses ValTypeRef which needs resolution via type table
            // For now, return a placeholder
            WrtFormatValType::List(Box::new(WrtFormatValType::Void))
        },
        CanonicalValType::Tuple(_types) => {
            // Tuple uses ValTypeRef which needs resolution via type table
            // For now, return empty tuple
            WrtFormatValType::Tuple(Vec::new())
        },
        CanonicalValType::Flags(names) => {
            // Convert BoundedVec<WasmName> to Vec<String>
            let string_names: Vec<String> = names
                .iter()
                .map(|name| {
                    // WasmName wraps BoundedString - need to convert properly
                    let bytes = name.inner().as_bytes().unwrap_or_else(|_| {
                        // Return empty slice on error
                        match name.inner().slice() {
                            Ok(s) => s,
                            Err(_) => {
                                // Return an empty safe slice - this is safe because we're referencing static empty data
                                wrt_foundation::safe_memory::Slice::new(&[]).unwrap_or_else(|_| {
                                    // If even empty slice creation fails, return a default empty Slice
                                    // This branch should never execute as empty slice creation is always valid
                                    panic!("Failed to create empty slice")
                                })
                            },
                        }
                    });
                    String::from_utf8_lossy(bytes.as_ref()).to_string()
                })
                .collect();
            WrtFormatValType::Flags(string_names)
        },
        CanonicalValType::Enum(variants) => {
            // Convert BoundedVec<WasmName> to Vec<String>
            let string_variants: Vec<String> = variants
                .iter()
                .map(|v| {
                    // WasmName wraps BoundedString - need to convert properly
                    let bytes = v.inner().as_bytes().unwrap_or_else(|_| {
                        // Return empty slice on error
                        match v.inner().slice() {
                            Ok(s) => s,
                            Err(_) => {
                                // Return an empty safe slice - this is safe because we're referencing static empty data
                                wrt_foundation::safe_memory::Slice::new(&[]).unwrap_or_else(|_| {
                                    // If even empty slice creation fails, return a default empty Slice
                                    // This branch should never execute as empty slice creation is always valid
                                    panic!("Failed to create empty slice")
                                })
                            },
                        }
                    });
                    String::from_utf8_lossy(bytes.as_ref()).to_string()
                })
                .collect();
            WrtFormatValType::Enum(string_variants)
        },
        CanonicalValType::Option(_inner_type) => {
            // Option requires ValTypeRef resolution which needs a type table
            // For now, return a placeholder
            WrtFormatValType::Option(Box::new(WrtFormatValType::Void))
        },
        CanonicalValType::Own(idx) => WrtFormatValType::Own(*idx),
        CanonicalValType::Borrow(idx) => WrtFormatValType::Borrow(*idx),
        CanonicalValType::FixedList(_elem_type_ref, size) => {
            // FixedList uses ValTypeRef which needs resolution via type table
            WrtFormatValType::FixedList(Box::new(WrtFormatValType::Void), *size)
        },
        CanonicalValType::Void => {
            // Void doesn't have a direct mapping, convert to a unit tuple
            WrtFormatValType::Tuple(Vec::new())
        },
        CanonicalValType::ErrorContext => WrtFormatValType::ErrorContext,
        CanonicalValType::Result { ok: _, err: _ } => {
            // For WrtFormatValType, we create a Result with a generic type placeholder
            // Since WrtFormatValType::Result requires a concrete type, we'll use Void
            WrtFormatValType::Result(Box::new(WrtFormatValType::Void))
        },
    }
}

/// Convert from wrt_format::component::ValType to CanonicalValType
///
/// NOTE: This conversion is currently limited because CanonicalValType uses
/// BoundedVec and WasmName which require memory providers to instantiate.
/// For complex types (Record, Variant, List, etc.), this returns simplified
/// placeholder types. Full conversion requires a type table and memory provider.
pub fn convert_format_to_common_valtype(format_type: &WrtFormatValType) -> CanonicalValType {
    match format_type {
        WrtFormatValType::Bool => CanonicalValType::Bool,
        WrtFormatValType::S8 => CanonicalValType::S8,
        WrtFormatValType::U8 => CanonicalValType::U8,
        WrtFormatValType::S16 => CanonicalValType::S16,
        WrtFormatValType::U16 => CanonicalValType::U16,
        WrtFormatValType::S32 => CanonicalValType::S32,
        WrtFormatValType::U32 => CanonicalValType::U32,
        WrtFormatValType::S64 => CanonicalValType::S64,
        WrtFormatValType::U64 => CanonicalValType::U64,
        WrtFormatValType::F32 => CanonicalValType::F32,
        WrtFormatValType::F64 => CanonicalValType::F64,
        WrtFormatValType::Char => CanonicalValType::Char,
        WrtFormatValType::String => CanonicalValType::String,
        WrtFormatValType::Ref(idx) => CanonicalValType::Ref(*idx),
        WrtFormatValType::Record(_fields) => {
            // Cannot convert without memory provider for BoundedVec and WasmName
            CanonicalValType::Void
        },
        WrtFormatValType::Variant(_cases) => {
            // Cannot convert without memory provider for BoundedVec and WasmName
            CanonicalValType::Void
        },
        WrtFormatValType::List(_elem_type) => {
            // Cannot convert without type table for ValTypeRef
            CanonicalValType::Void
        },
        WrtFormatValType::Tuple(_types) => {
            // Cannot convert without type table for ValTypeRef
            CanonicalValType::Void
        },
        WrtFormatValType::Flags(_names) => {
            // Cannot convert without memory provider for BoundedVec and WasmName
            CanonicalValType::Void
        },
        WrtFormatValType::Enum(_variants) => {
            // Cannot convert without memory provider for BoundedVec and WasmName
            CanonicalValType::Void
        },
        WrtFormatValType::Option(_inner_type) => {
            // Cannot convert without type table for ValTypeRef
            CanonicalValType::Void
        },
        WrtFormatValType::Result(_result_type) => {
            // Cannot convert without type table for ValTypeRef
            CanonicalValType::Result {
                ok: None,
                err: None,
            }
        },
        WrtFormatValType::Own(idx) => CanonicalValType::Own(*idx),
        WrtFormatValType::Borrow(idx) => CanonicalValType::Borrow(*idx),
        WrtFormatValType::FixedList(_elem_type, size) => {
            // Cannot convert without type table for ValTypeRef
            CanonicalValType::FixedList(ValTypeRef(0), *size)
        },
        WrtFormatValType::ErrorContext => CanonicalValType::ErrorContext,
        // Map any unhandled types to Void
        _ => CanonicalValType::Void,
    }
}

// Serialization and deserialization functions for ComponentValue
pub fn serialize_component_value(value: &ComponentComponentValue) -> Result<Vec<u8>> {
    let common_type = value.get_type();
    let format_type = convert_common_to_format_valtype(&common_type);

    // Serialize the value based on its type
    let mut buffer = Vec::new();

    match value {
        ComponentComponentValue::Bool(b) => {
            buffer.push(if *b { 1 } else { 0 });
        },
        ComponentComponentValue::S8(v) => {
            buffer.push(*v as u8);
        },
        ComponentComponentValue::U8(v) => {
            buffer.push(*v);
        },
        ComponentComponentValue::S16(v) => {
            buffer.extend_from_slice(&v.to_le_bytes());
        },
        ComponentComponentValue::U16(v) => {
            buffer.extend_from_slice(&v.to_le_bytes());
        },
        ComponentComponentValue::S32(v) => {
            buffer.extend_from_slice(&v.to_le_bytes());
        },
        ComponentComponentValue::U32(v) => {
            buffer.extend_from_slice(&v.to_le_bytes());
        },
        ComponentComponentValue::S64(v) => {
            buffer.extend_from_slice(&v.to_le_bytes());
        },
        ComponentComponentValue::U64(v) => {
            buffer.extend_from_slice(&v.to_le_bytes());
        },
        ComponentComponentValue::F32(v) => {
            buffer.extend_from_slice(&v.to_bits().to_le_bytes());
        },
        ComponentComponentValue::F64(v) => {
            buffer.extend_from_slice(&v.to_bits().to_le_bytes());
        },
        ComponentComponentValue::Char(c) => {
            let bytes = [
                (*c as u32 & 0xff) as u8,
                ((*c as u32 >> 8) & 0xff) as u8,
                ((*c as u32 >> 16) & 0xff) as u8,
                ((*c as u32 >> 24) & 0xff) as u8,
            ];
            buffer.extend_from_slice(&bytes);
        },
        ComponentComponentValue::String(s) => {
            // String is encoded as length followed by UTF-8 bytes
            #[cfg(feature = "std")]
            {
                let bytes = s.as_bytes();
                let len = bytes.len() as u32;
                buffer.extend_from_slice(&len.to_le_bytes());
                buffer.extend_from_slice(bytes);
            }
            #[cfg(not(feature = "std"))]
            {
                let bytes = s.as_bytes();
                let len = bytes.len() as u32;
                buffer.extend_from_slice(&len.to_le_bytes());
                buffer.extend_from_slice(bytes);
            }
        },
        ComponentComponentValue::List(items) => {
            // TODO: ComponentValue uses ValueRef (indices) not direct values
            // This requires a ComponentValueStore to resolve the actual values
            // For now, we serialize the count and placeholder data
            let count = items.len() as u32;
            buffer.extend_from_slice(&count.to_le_bytes());

            // Serialize the ValueRef indices
            for item_ref in items.iter() {
                // ValueRef is just a usize index - serialize it as u32
                let index = item_ref.0 as u32;
                buffer.extend_from_slice(&index.to_le_bytes());
            }
        },
        ComponentComponentValue::Record(fields) => {
            // TODO: ComponentValue uses ValueRef (indices) not direct values
            // This requires a ComponentValueStore to resolve the actual values
            let field_count = fields.len() as u32;
            buffer.extend_from_slice(&field_count.to_le_bytes());

            // Serialize each field
            for (name, value_ref) in fields.iter() {
                // Serialize the field name
                let name_bytes = name.inner().as_bytes().map_err(|_| {
                    Error::runtime_execution_error("Failed to get field name bytes")
                })?;
                let name_len = name_bytes.len() as u32;
                buffer.extend_from_slice(&name_len.to_le_bytes());
                buffer.extend_from_slice(name_bytes.as_ref());

                // Serialize the ValueRef index
                let index = value_ref.0 as u32;
                buffer.extend_from_slice(&index.to_le_bytes());
            }
        },
        ComponentComponentValue::Tuple(items) => {
            // TODO: ComponentValue uses ValueRef (indices) not direct values
            // This requires a ComponentValueStore to resolve the actual values
            let count = items.len() as u32;
            buffer.extend_from_slice(&count.to_le_bytes());

            // Serialize the ValueRef indices
            for item_ref in items.iter() {
                let index = item_ref.0 as u32;
                buffer.extend_from_slice(&index.to_le_bytes());
            }
        },
        ComponentComponentValue::Variant(case_name, value_opt) => {
            // Serialize the case name
            let name_bytes = case_name.inner().as_bytes().map_err(|_| {
                Error::runtime_execution_error("Failed to get variant case name bytes")
            })?;
            let name_len = name_bytes.len() as u32;
            buffer.extend_from_slice(&name_len.to_le_bytes());
            buffer.extend_from_slice(name_bytes.as_ref());

            // Serialize presence flag for the value
            buffer.push(if value_opt.is_some() { 1 } else { 0 });

            // TODO: ComponentValue uses ValueRef (indices) not direct values
            // Serialize the ValueRef index if present
            if let Some(value_ref) = value_opt {
                let index = value_ref.0 as u32;
                buffer.extend_from_slice(&index.to_le_bytes());
            }
        },
        ComponentComponentValue::Enum(variant) => {
            // Serialize the enum variant
            let variant_bytes = variant
                .inner()
                .as_bytes()
                .map_err(|_| Error::runtime_execution_error("Failed to get enum variant bytes"))?;
            let variant_len = variant_bytes.len() as u32;
            buffer.extend_from_slice(&variant_len.to_le_bytes());
            buffer.extend_from_slice(variant_bytes.as_ref());
        },
        ComponentComponentValue::Option(value_opt) => {
            // Serialize presence flag for the value
            buffer.push(if value_opt.is_some() { 1 } else { 0 });

            // TODO: ComponentValue uses ValueRef (indices) not direct values
            // Serialize the ValueRef index if present
            if let Some(value_ref) = value_opt {
                let index = value_ref.0 as u32;
                buffer.extend_from_slice(&index.to_le_bytes());
            }
        },
        ComponentComponentValue::Result(result) => {
            // Serialize success flag
            buffer.push(match result {
                Ok(_) => 1,  // Success
                Err(_) => 0, // Error
            });

            // TODO: ComponentValue uses ValueRef (indices) not direct values
            // Serialize the ValueRef index (either Ok or Err)
            match result {
                Ok(value_ref) => {
                    let index = value_ref.0 as u32;
                    buffer.extend_from_slice(&index.to_le_bytes());
                },
                Err(error_ref) => {
                    let index = error_ref.0 as u32;
                    buffer.extend_from_slice(&index.to_le_bytes());
                },
            }
        },
        ComponentComponentValue::Handle(idx) => {
            // Serialize the handle index (from Own)
            buffer.extend_from_slice(&idx.to_le_bytes());
        },
        ComponentComponentValue::Borrow(idx) => {
            // Serialize the borrow index
            buffer.extend_from_slice(&idx.to_le_bytes());
        },
        ComponentComponentValue::Flags(flags) => {
            // Get the number of flags
            let count = flags.len() as u32;
            buffer.extend_from_slice(&count.to_le_bytes());

            // Calculate the flag byte (up to 8 flags in a byte)
            let mut flag_byte: u8 = 0;
            for (i, (_, enabled)) in flags.iter().enumerate().take(8) {
                if enabled {
                    flag_byte |= 1 << i;
                }
            }
            buffer.push(flag_byte);

            // Serialize each flag name
            for (name, _) in flags.iter() {
                let name_bytes = name
                    .inner()
                    .as_bytes()
                    .map_err(|_| Error::runtime_execution_error("Failed to get flag name bytes"))?;
                let name_len = name_bytes.len() as u32;
                buffer.extend_from_slice(&name_len.to_le_bytes());
                buffer.extend_from_slice(name_bytes.as_ref());
            }
        },
        ComponentComponentValue::FixedList(items, size) => {
            // TODO: ComponentValue uses ValueRef (indices) not direct values
            // This requires a ComponentValueStore to resolve the actual values
            let count = items.len() as u32;
            buffer.extend_from_slice(&count.to_le_bytes());
            buffer.extend_from_slice(&size.to_le_bytes()); // Store the size

            // Serialize the ValueRef indices
            for item_ref in items.iter() {
                let index = item_ref.0 as u32;
                buffer.extend_from_slice(&index.to_le_bytes());
            }
        },
        ComponentComponentValue::ErrorContext(ctx) => {
            // TODO: ComponentValue uses ValueRef (indices) not direct values
            // This requires a ComponentValueStore to resolve the actual values
            let count = ctx.len() as u32;
            buffer.extend_from_slice(&count.to_le_bytes());

            // Serialize the ValueRef indices
            for item_ref in ctx.iter() {
                let index = item_ref.0 as u32;
                buffer.extend_from_slice(&index.to_le_bytes());
            }
        },
        ComponentComponentValue::Void => {
            // Just a type marker for void
            buffer.push(0);
        },
        ComponentComponentValue::Unit => {
            // Just a type marker for unit
            buffer.push(255);
        },
        ComponentComponentValue::Own(idx) => {
            // Serialize the own handle index
            buffer.extend_from_slice(&idx.to_le_bytes());
        },
    }

    Ok(buffer)
}

/// Serialize a component value using the WriteStream interface
pub fn serialize_component_value_with_stream<'a, P: wrt_foundation::MemoryProvider>(
    value: &ComponentComponentValue,
    writer: &mut WriteStream<'a>,
    provider: &P,
) -> Result<()> {
    match value {
        ComponentComponentValue::Bool(b) => {
            writer.write_bool(*b)?;
        },
        ComponentComponentValue::S8(v) => {
            writer.write_i8(*v)?;
        },
        ComponentComponentValue::U8(v) => {
            writer.write_u8(*v)?;
        },
        ComponentComponentValue::S16(v) => {
            writer.write_i16_le(*v)?;
        },
        ComponentComponentValue::U16(v) => {
            writer.write_u16_le(*v)?;
        },
        ComponentComponentValue::S32(v) => {
            writer.write_i32_le(*v)?;
        },
        ComponentComponentValue::U32(v) => {
            writer.write_u32_le(*v)?;
        },
        ComponentComponentValue::S64(v) => {
            writer.write_i64_le(*v)?;
        },
        ComponentComponentValue::U64(v) => {
            writer.write_u64_le(*v)?;
        },
        ComponentComponentValue::F32(v) => {
            writer.write_f32_le(v.to_f32())?;
        },
        ComponentComponentValue::F64(v) => {
            writer.write_f64_le(v.to_f64())?;
        },
        ComponentComponentValue::Char(c) => {
            writer.write_u32_le(*c as u32)?;
        },
        ComponentComponentValue::String(s) => {
            // String is encoded as length followed by UTF-8 bytes
            #[cfg(feature = "std")]
            {
                let bytes = s.as_bytes();
                writer.write_u32_le(bytes.len() as u32)?;
                writer.write_all(bytes)?;
            }
            #[cfg(not(feature = "std"))]
            {
                let bytes = s.as_bytes();
                writer.write_u32_le(bytes.len() as u32)?;
                writer.write_all(bytes)?;
            }
        },
        ComponentComponentValue::List(items) => {
            // TODO: ComponentValue uses ValueRef (indices) not direct values
            // This requires a ComponentValueStore to resolve the actual values
            // For now, serialize the count and ValueRef indices
            writer.write_u32_le(items.len() as u32)?;

            // Serialize each ValueRef index
            for item_ref in items.iter() {
                writer.write_u32_le(item_ref.0 as u32)?;
            }
        },
        ComponentComponentValue::Record(fields) => {
            // TODO: ComponentValue uses ValueRef (indices) not direct values
            // This requires a ComponentValueStore to resolve the actual values
            writer.write_u32_le(fields.len() as u32)?;

            // Serialize each field
            for (name, value_ref) in fields.iter() {
                // Serialize the field name
                let name_bytes = name.inner().as_bytes().map_err(|_| {
                    Error::runtime_execution_error("Failed to get field name bytes")
                })?;
                writer.write_u32_le(name_bytes.len() as u32)?;
                writer.write_all(name_bytes.as_ref())?;

                // Serialize the ValueRef index
                writer.write_u32_le(value_ref.0 as u32)?;
            }
        },
        ComponentComponentValue::Tuple(items) => {
            // TODO: ComponentValue uses ValueRef (indices) not direct values
            // This requires a ComponentValueStore to resolve the actual values
            writer.write_u32_le(items.len() as u32)?;

            // Serialize each ValueRef index
            for item_ref in items.iter() {
                writer.write_u32_le(item_ref.0 as u32)?;
            }
        },
        ComponentComponentValue::Variant(case_name, value_opt) => {
            // Serialize the case name
            let name_bytes = case_name.inner().as_bytes().map_err(|_| {
                Error::runtime_execution_error("Failed to get variant case name bytes")
            })?;
            writer.write_u32_le(name_bytes.len() as u32)?;
            writer.write_all(name_bytes.as_ref())?;

            // Serialize presence flag for the value
            writer.write_bool(value_opt.is_some())?;

            // TODO: ComponentValue uses ValueRef (indices) not direct values
            // Serialize the ValueRef index if present
            if let Some(value_ref) = value_opt {
                writer.write_u32_le(value_ref.0 as u32)?;
            }
        },
        ComponentComponentValue::Enum(variant) => {
            // Serialize the enum variant
            let variant_bytes = variant
                .inner()
                .as_bytes()
                .map_err(|_| Error::runtime_execution_error("Failed to get enum variant bytes"))?;
            writer.write_u32_le(variant_bytes.len() as u32)?;
            writer.write_all(variant_bytes.as_ref())?;
        },
        ComponentComponentValue::Option(value_opt) => {
            // Serialize presence flag for the value
            writer.write_bool(value_opt.is_some())?;

            // TODO: ComponentValue uses ValueRef (indices) not direct values
            // Serialize the ValueRef index if present
            if let Some(value_ref) = value_opt {
                writer.write_u32_le(value_ref.0 as u32)?;
            }
        },
        ComponentComponentValue::Result(result) => {
            // Serialize success flag
            let is_ok = result.is_ok();
            writer.write_bool(is_ok)?;

            // TODO: ComponentValue uses ValueRef (indices) not direct values
            // Serialize the ValueRef index (either Ok or Err)
            match result {
                Ok(value_ref) => {
                    writer.write_u32_le(value_ref.0 as u32)?;
                },
                Err(error_ref) => {
                    writer.write_u32_le(error_ref.0 as u32)?;
                },
            }
        },
        ComponentComponentValue::Handle(idx) => {
            // Serialize the handle index (from Own)
            writer.write_u32_le(*idx)?;
        },
        ComponentComponentValue::Borrow(idx) => {
            // Serialize the borrow index
            writer.write_u32_le(*idx)?;
        },
        ComponentComponentValue::Flags(flags) => {
            // Get the number of flags
            writer.write_u32_le(flags.len() as u32)?;

            // Calculate the flag byte (up to 8 flags in a byte)
            let mut flag_byte: u8 = 0;
            for (i, (_, enabled)) in flags.iter().enumerate().take(8) {
                if enabled {
                    flag_byte |= 1 << i;
                }
            }
            writer.write_u8(flag_byte)?;

            // Serialize each flag name
            for (name, _) in flags.iter() {
                let name_bytes = name
                    .inner()
                    .as_bytes()
                    .map_err(|_| Error::runtime_execution_error("Failed to get flag name bytes"))?;
                writer.write_u32_le(name_bytes.len() as u32)?;
                writer.write_all(name_bytes.as_ref())?;
            }
        },
        ComponentComponentValue::FixedList(items, size) => {
            // TODO: ComponentValue uses ValueRef (indices) not direct values
            // This requires a ComponentValueStore to resolve the actual values
            writer.write_u32_le(items.len() as u32)?;
            writer.write_u32_le(*size)?;

            // Serialize each ValueRef index
            for item_ref in items.iter() {
                writer.write_u32_le(item_ref.0 as u32)?;
            }
        },
        ComponentComponentValue::ErrorContext(ctx) => {
            // TODO: ComponentValue uses ValueRef (indices) not direct values
            // This requires a ComponentValueStore to resolve the actual values
            writer.write_u32_le(ctx.len() as u32)?;

            // Serialize each ValueRef index
            for item_ref in ctx.iter() {
                writer.write_u32_le(item_ref.0 as u32)?;
            }
        },
        ComponentComponentValue::Void => {
            // Just a type marker for void
            writer.write_u8(0)?;
        },
        ComponentComponentValue::Unit => {
            // Just a type marker for unit
            writer.write_u8(255)?;
        },
        ComponentComponentValue::Own(idx) => {
            // Serialize the own handle index
            writer.write_u32_le(*idx)?;
        },
    }

    Ok(())
}

// Simplified deserialization function
pub fn deserialize_component_value(
    data: &[u8],
    format_type: &WrtFormatValType,
) -> Result<ComponentComponentValue> {
    let mut offset = 0;
    match format_type {
        WrtFormatValType::Bool => {
            if offset >= data.len() {
                return Err(Error::parse_error("Not enough data to deserialize bool"));
            }
            let value = data[offset] != 0;
            Ok(ComponentComponentValue::Bool(value))
        },
        WrtFormatValType::S8 => {
            if offset >= data.len() {
                return Err(Error::parse_error("Not enough data to deserialize S8"));
            }
            let value = data[offset] as i8;
            Ok(ComponentComponentValue::S8(value))
        },
        WrtFormatValType::U8 => {
            if offset >= data.len() {
                return Err(Error::parse_error("Not enough data to deserialize U8"));
            }
            let value = data[offset];
            Ok(ComponentComponentValue::U8(value))
        },
        WrtFormatValType::S16 => {
            if offset + 2 > data.len() {
                return Err(Error::parse_error("Not enough data to deserialize S16"));
            }
            let value = i16::from_le_bytes([data[offset], data[offset + 1]]);
            Ok(ComponentComponentValue::S16(value))
        },
        WrtFormatValType::U16 => {
            if offset + 2 > data.len() {
                return Err(Error::parse_error("Not enough data to deserialize U16"));
            }
            let value = u16::from_le_bytes([data[offset], data[offset + 1]]);
            Ok(ComponentComponentValue::U16(value))
        },
        WrtFormatValType::S32 => {
            if offset + 4 > data.len() {
                return Err(Error::parse_error("Not enough data to deserialize S32"));
            }
            let value = i32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            Ok(ComponentComponentValue::S32(value))
        },
        WrtFormatValType::U32 => {
            if offset + 4 > data.len() {
                return Err(Error::parse_error("Not enough data to deserialize U32"));
            }
            let value = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            Ok(ComponentComponentValue::U32(value))
        },
        WrtFormatValType::S64 => {
            if offset + 8 > data.len() {
                return Err(Error::parse_error("Not enough data to deserialize S64"));
            }
            let value = i64::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            Ok(ComponentComponentValue::S64(value))
        },
        WrtFormatValType::U64 => {
            if offset + 8 > data.len() {
                return Err(Error::parse_error("Not enough data to deserialize U64"));
            }
            let value = u64::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            Ok(ComponentComponentValue::U64(value))
        },
        WrtFormatValType::F32 => {
            if offset + 4 > data.len() {
                return Err(Error::parse_error("Not enough data to deserialize F32"));
            }
            let bits = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let value = wrt_foundation::float_repr::FloatBits32::from_bits(bits);
            Ok(ComponentComponentValue::F32(value))
        },
        WrtFormatValType::F64 => {
            if offset + 8 > data.len() {
                return Err(Error::parse_error("Not enough data to deserialize F64"));
            }
            let bits = u64::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let value = wrt_foundation::float_repr::FloatBits64::from_bits(bits);
            Ok(ComponentComponentValue::F64(value))
        },
        WrtFormatValType::Char => {
            if offset >= data.len() {
                return Err(Error::parse_error("Not enough data to deserialize Char"));
            }
            let value = data[offset] as char;
            Ok(ComponentComponentValue::Char(value))
        },
        WrtFormatValType::String => {
            if offset + 4 > data.len() {
                return Err(Error::parse_error(
                    "Not enough data to deserialize String length",
                ));
            }
            let len = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            offset += 4;
            if offset + len as usize > data.len() {
                return Err(Error::parse_error("Not enough data to deserialize String"));
            }
            let value = String::from_utf8(data[offset..offset + len as usize].to_vec())
                .map_err(|e| Error::parse_error("Component not found"))?;
            Ok(ComponentComponentValue::String(value))
        },
        WrtFormatValType::List(_elem_type) => {
            // TODO: ComponentValue::List expects BoundedVec<ValueRef> not Vec<ComponentValue>
            // This requires a memory provider and ComponentValueStore
            // Return Void as placeholder since we can't construct proper List without these
            Err(Error::parse_error(
                "List deserialization requires memory provider and value store",
            ))
        },
        WrtFormatValType::Record(_fields) => {
            // TODO: ComponentValue::Record expects BoundedVec<(WasmName, ValueRef)> not Vec<(String, ComponentValue)>
            // This requires a memory provider and ComponentValueStore
            Err(Error::parse_error(
                "Record deserialization requires memory provider and value store",
            ))
        },
        WrtFormatValType::Tuple(_types) => {
            // TODO: ComponentValue::Tuple expects BoundedVec<ValueRef> not Vec<ComponentValue>
            // This requires a memory provider and ComponentValueStore
            Err(Error::parse_error(
                "Tuple deserialization requires memory provider and value store",
            ))
        },
        WrtFormatValType::Variant(_cases) => {
            // TODO: ComponentValue::Variant expects (WasmName, Option<ValueRef>) not (String, Option<Box<ComponentValue>>)
            // This requires a memory provider and ComponentValueStore
            Err(Error::parse_error(
                "Variant deserialization requires memory provider and value store",
            ))
        },
        WrtFormatValType::Enum(_variants) => {
            // TODO: ComponentValue::Enum expects WasmName not String
            // This requires a memory provider
            Err(Error::parse_error(
                "Enum deserialization requires memory provider",
            ))
        },
        WrtFormatValType::Option(_inner_type) => {
            // TODO: ComponentValue::Option expects Option<ValueRef> not Option<Box<ComponentValue>>
            // This requires a ComponentValueStore
            Err(Error::parse_error(
                "Option deserialization requires value store",
            ))
        },
        WrtFormatValType::Result(_result_type) => {
            // TODO: ComponentValue::Result expects Result<ValueRef, ValueRef> not Result<Box<ComponentValue>, Box<ComponentValue>>
            // This requires a ComponentValueStore
            Err(Error::parse_error(
                "Result deserialization requires value store",
            ))
        },
        WrtFormatValType::Own(idx) => {
            if offset >= data.len() {
                return Err(Error::parse_error("Not enough data to deserialize Own"));
            }
            let value = data[offset] as u32;
            // No need to update offset anymore as we return immediately
            Ok(ComponentComponentValue::Handle(value))
        },
        WrtFormatValType::Borrow(idx) => {
            if offset >= data.len() {
                return Err(Error::parse_error("Not enough data to deserialize Borrow"));
            }
            let value = data[offset] as u32;
            // No need to update offset anymore as we return immediately
            Ok(ComponentComponentValue::Borrow(value))
        },
        WrtFormatValType::Flags(_names) => {
            // TODO: ComponentValue::Flags expects BoundedVec<(WasmName, bool)> not Vec<(String, bool)>
            // This requires a memory provider
            Err(Error::parse_error(
                "Flags deserialization requires memory provider",
            ))
        },
        WrtFormatValType::FixedList(_elem_type, _size) => {
            // TODO: ComponentValue::FixedList expects BoundedVec<ValueRef> not Vec<ComponentValue>
            // This requires a memory provider and ComponentValueStore
            Err(Error::parse_error(
                "FixedList deserialization requires memory provider and value store",
            ))
        },
        WrtFormatValType::ErrorContext => {
            // TODO: ComponentValue::ErrorContext expects BoundedVec<ValueRef> not Vec<ComponentValue>
            // This requires a memory provider and ComponentValueStore
            Err(Error::parse_error(
                "ErrorContext deserialization requires memory provider and value store",
            ))
        },
        // Handle any other unimplemented cases
        _ => Err(Error::validation_invalid_input(
            "Type not supported for deserialization",
        )),
    }
}

/// Deserialize a component value using the ReadStream interface
pub fn deserialize_component_value_with_stream<'a, P: wrt_foundation::MemoryProvider>(
    reader: &mut ReadStream<'a>,
    format_type: &WrtFormatValType,
    provider: &P,
) -> Result<ComponentComponentValue> {
    match format_type {
        WrtFormatValType::Bool => {
            let value = reader.read_bool()?;
            Ok(ComponentComponentValue::Bool(value))
        },
        WrtFormatValType::S8 => {
            let value = reader.read_i8()?;
            Ok(ComponentComponentValue::S8(value))
        },
        WrtFormatValType::U8 => {
            let value = reader.read_u8()?;
            Ok(ComponentComponentValue::U8(value))
        },
        WrtFormatValType::S16 => {
            let value = reader.read_i16_le()?;
            Ok(ComponentComponentValue::S16(value))
        },
        WrtFormatValType::U16 => {
            let value = reader.read_u16_le()?;
            Ok(ComponentComponentValue::U16(value))
        },
        WrtFormatValType::S32 => {
            let value = reader.read_i32_le()?;
            Ok(ComponentComponentValue::S32(value))
        },
        WrtFormatValType::U32 => {
            let value = reader.read_u32_le()?;
            Ok(ComponentComponentValue::U32(value))
        },
        WrtFormatValType::S64 => {
            let value = reader.read_i64_le()?;
            Ok(ComponentComponentValue::S64(value))
        },
        WrtFormatValType::U64 => {
            let value = reader.read_u64_le()?;
            Ok(ComponentComponentValue::U64(value))
        },
        WrtFormatValType::F32 => {
            let value = reader.read_f32_le()?;
            Ok(ComponentComponentValue::F32(
                wrt_foundation::float_repr::FloatBits32::from_f32(value),
            ))
        },
        WrtFormatValType::F64 => {
            let value = reader.read_f64_le()?;
            Ok(ComponentComponentValue::F64(
                wrt_foundation::float_repr::FloatBits64::from_f64(value),
            ))
        },
        WrtFormatValType::Char => {
            let value_u32 = reader.read_u32_le()?;
            let value = char::from_u32(value_u32)
                .ok_or_else(|| Error::parse_error("Component not found"))?;
            Ok(ComponentComponentValue::Char(value))
        },
        WrtFormatValType::String => {
            // Read the string length
            let len = reader.read_u32_le()? as usize;

            // Read the string bytes
            let mut bytes = vec![0u8; len];
            reader.read_exact(&mut bytes)?;

            // Convert to a String
            let value =
                String::from_utf8(bytes).map_err(|e| Error::parse_error("Component not found"))?;

            Ok(ComponentComponentValue::String(value))
        },
        WrtFormatValType::List(_elem_type) => {
            // TODO: ComponentValue::List expects BoundedVec<ValueRef> not Vec<ComponentValue>
            // This requires a memory provider and ComponentValueStore
            Err(Error::parse_error(
                "List deserialization requires memory provider and value store",
            ))
        },
        WrtFormatValType::Record(_fields) => {
            // TODO: ComponentValue::Record expects BoundedVec<(WasmName, ValueRef)> not Vec<(String, ComponentValue)>
            // This requires a memory provider and ComponentValueStore
            Err(Error::parse_error(
                "Record deserialization requires memory provider and value store",
            ))
        },
        WrtFormatValType::Tuple(_types) => {
            // TODO: ComponentValue::Tuple expects BoundedVec<ValueRef> not Vec<ComponentValue>
            // This requires a memory provider and ComponentValueStore
            Err(Error::parse_error(
                "Tuple deserialization requires memory provider and value store",
            ))
        },
        WrtFormatValType::Variant(_cases) => {
            // TODO: ComponentValue::Variant expects (WasmName, Option<ValueRef>) not (String, Option<Box<ComponentValue>>)
            // This requires a memory provider and ComponentValueStore
            Err(Error::parse_error(
                "Variant deserialization requires memory provider and value store",
            ))
        },
        WrtFormatValType::Enum(_variants) => {
            // TODO: ComponentValue::Enum expects WasmName not String
            // This requires a memory provider
            Err(Error::parse_error(
                "Enum deserialization requires memory provider",
            ))
        },
        WrtFormatValType::Option(_inner_type) => {
            // TODO: ComponentValue::Option expects Option<ValueRef> not Option<Box<ComponentValue>>
            // This requires a ComponentValueStore
            Err(Error::parse_error(
                "Option deserialization requires value store",
            ))
        },
        WrtFormatValType::Result(_result_type) => {
            // TODO: ComponentValue::Result expects Result<ValueRef, ValueRef> not Result<Box<ComponentValue>, Box<ComponentValue>>
            // This requires a ComponentValueStore
            Err(Error::parse_error(
                "Result deserialization requires value store",
            ))
        },
        WrtFormatValType::Own(idx) => {
            let value = reader.read_u32_le()?;
            Ok(ComponentComponentValue::Handle(value))
        },
        WrtFormatValType::Borrow(idx) => {
            let value = reader.read_u32_le()?;
            Ok(ComponentComponentValue::Borrow(value))
        },
        WrtFormatValType::Flags(_names) => {
            // TODO: ComponentValue::Flags expects BoundedVec<(WasmName, bool)> not Vec<(String, bool)>
            // This requires a memory provider
            Err(Error::parse_error(
                "Flags deserialization requires memory provider",
            ))
        },
        WrtFormatValType::FixedList(_elem_type, _size) => {
            // TODO: ComponentValue::FixedList expects BoundedVec<ValueRef> not Vec<ComponentValue>
            // This requires a memory provider and ComponentValueStore
            Err(Error::parse_error(
                "FixedList deserialization requires memory provider and value store",
            ))
        },
        WrtFormatValType::ErrorContext => {
            // TODO: ComponentValue::ErrorContext expects BoundedVec<ValueRef> not Vec<ComponentValue>
            // This requires a memory provider and ComponentValueStore
            Err(Error::parse_error(
                "ErrorContext deserialization requires memory provider and value store",
            ))
        },
        WrtFormatValType::Void => {
            // Just read a marker byte
            let _ = reader.read_u8()?;
            Ok(ComponentComponentValue::Void)
        },
        // Handle any other unimplemented cases
        _ => Err(Error::validation_invalid_input("Component not found")),
    }
}

/// Serialize multiple component values
pub fn serialize_component_values(values: &[ComponentComponentValue]) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();

    // Write the number of values
    let count = values.len() as u32;
    buffer.extend_from_slice(&count.to_le_bytes());

    // Serialize each value
    for value in values {
        let value_bytes = serialize_component_value(value)?;

        // Write the size of this value's bytes
        let size = value_bytes.len() as u32;
        buffer.extend_from_slice(&size.to_le_bytes());

        // Write the value bytes
        buffer.extend_from_slice(&value_bytes);
    }

    Ok(buffer)
}

/// Serialize multiple component values using a WriteStream
pub fn serialize_component_values_with_stream<'a, P: wrt_foundation::MemoryProvider>(
    values: &[ComponentComponentValue],
    writer: &mut WriteStream<'a>,
    provider: &P,
) -> Result<()> {
    // Write the number of values
    writer.write_u32_le(values.len() as u32)?;

    // Serialize each value directly to the stream
    for value in values {
        serialize_component_value_with_stream(value, writer, provider)?;
    }

    Ok(())
}

/// Deserialize multiple component values
pub fn deserialize_component_values(
    data: &[u8],
    types: &[WrtFormatValType],
) -> Result<Vec<ComponentComponentValue>> {
    // Need at least 4 bytes for the count
    if data.len() < 4 {
        return Err(Error::parse_error("Not enough data to read value count"));
    }

    // Read the count
    let mut offset = 0;
    let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    offset += 4;

    // Validate that we have enough types
    if count > types.len() {
        return Err(Error::validation_error("Validation error"));
    }

    // Read each value
    let mut values = Vec::with_capacity(count);
    for type_idx in 0..count {
        // Need at least 4 more bytes for the size
        if offset + 4 > data.len() {
            return Err(Error::parse_error("Not enough data to read value size"));
        }

        // Read the size
        let size = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;

        // Validate that we have enough data
        if offset + size > data.len() {
            return Err(Error::parse_error("Not enough data to read value"));
        }

        // Read the value
        let value_data = &data[offset..offset + size];
        let value = deserialize_component_value(value_data, &types[type_idx])?;
        values.push(value);

        // Move to the next value
        offset += size;
    }

    Ok(values)
}

/// Deserialize multiple component values using a ReadStream
pub fn deserialize_component_values_with_stream<'a, P: wrt_foundation::MemoryProvider>(
    reader: &mut ReadStream<'a>,
    types: &[WrtFormatValType],
    provider: &P,
) -> Result<Vec<ComponentComponentValue>> {
    // Read the count
    let count = reader.read_u32_le()? as usize;

    // Validate that we have enough types
    if count > types.len() {
        return Err(Error::validation_error("Validation error"));
    }

    // Read each value
    let mut values = Vec::with_capacity(count);
    for type_idx in 0..count {
        let value = deserialize_component_value_with_stream(reader, &types[type_idx], provider)?;
        values.push(value);
    }

    Ok(values)
}

// Core value conversion functions
pub fn core_to_component_value(
    value: &Value,
    ty: &WrtFormatValType,
) -> wrt_error::Result<ComponentComponentValue> {
    use crate::type_conversion::{
        core_value_to_types_componentvalue, format_valtype_to_types_valtype,
    };

    // First, convert the format value type to a types value type
    let types_val_type = format_valtype_to_types_valtype(ty);

    // Then convert the core value to a component value
    let component_value = core_value_to_types_componentvalue(value)?;

    // Check if the types match or provide a conversion error
    match (&component_value, &types_val_type) {
        // Basic type checking for primitive types
        (ComponentComponentValue::S32(_), ComponentValType::S32)
        | (ComponentComponentValue::S64(_), ComponentValType::S64)
        | (ComponentComponentValue::F32(_), ComponentValType::F32)
        | (ComponentComponentValue::F64(_), ComponentValType::F64) => Ok(component_value),

        // Handle boolean conversion from i32
        (ComponentComponentValue::S32(v), ComponentValType::Bool) => {
            Ok(ComponentComponentValue::Bool(*v != 0))
        },

        // Other integer width conversions
        (ComponentComponentValue::S32(v), ComponentValType::S8) => {
            Ok(ComponentComponentValue::S8(*v as i8))
        },
        (ComponentComponentValue::S32(v), ComponentValType::U8) => {
            Ok(ComponentComponentValue::U8(*v as u8))
        },
        (ComponentComponentValue::S32(v), ComponentValType::S16) => {
            Ok(ComponentComponentValue::S16(*v as i16))
        },
        (ComponentComponentValue::S32(v), ComponentValType::U16) => {
            Ok(ComponentComponentValue::U16(*v as u16))
        },
        (ComponentComponentValue::S32(v), ComponentValType::U32) => {
            Ok(ComponentComponentValue::U32(*v as u32))
        },
        (ComponentComponentValue::S64(v), ComponentValType::U64) => {
            Ok(ComponentComponentValue::U64(*v as u64))
        },

        // Error for type mismatch
        _ => Err(Error::runtime_execution_error("Type conversion failed")),
    }
}

pub fn component_to_core_value(value: &ComponentComponentValue) -> wrt_error::Result<Value> {
    use crate::type_conversion::types_componentvalue_to_core_value;

    // Use the centralized conversion function
    types_componentvalue_to_core_value(value)
}

// Size calculation for component values
pub fn size_in_bytes(ty: &WrtFormatValType) -> usize {
    match ty {
        WrtFormatValType::Bool => 1,
        WrtFormatValType::S8 => 1,
        WrtFormatValType::U8 => 1,
        WrtFormatValType::S16 => 2,
        WrtFormatValType::U16 => 2,
        WrtFormatValType::S32 => 4,
        WrtFormatValType::U32 => 4,
        WrtFormatValType::S64 => 8,
        WrtFormatValType::U64 => 8,
        WrtFormatValType::F32 => 4,
        WrtFormatValType::F64 => 8,
        WrtFormatValType::Char => 4,
        WrtFormatValType::String => 8, // Pointer size
        WrtFormatValType::Ref(_) => 4,
        WrtFormatValType::Record(_) => 8,
        WrtFormatValType::Variant(_) => 8,
        WrtFormatValType::List(_) => 8,
        WrtFormatValType::Tuple(_) => 8,
        WrtFormatValType::Flags(_) => 8,
        WrtFormatValType::Enum(_) => 4,
        WrtFormatValType::Option(_) => 8,
        WrtFormatValType::Result(_) => 8,
        WrtFormatValType::Own(_) => 4,
        WrtFormatValType::Borrow(_) => 4,
        WrtFormatValType::FixedList(_, _) => 8,
        WrtFormatValType::ErrorContext => 4,
        // Default size for any unhandled types
        _ => 4,
    }
}
