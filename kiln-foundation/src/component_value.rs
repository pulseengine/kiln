// WRT - wrt-foundation
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! WebAssembly Component Model value types
//!
//! This module defines the runtime value types used in WebAssembly Component
//! Model implementations.

#![allow(clippy::derive_partial_eq_without_eq)]

// ToOwned is now imported from the prelude

use wrt_error::{
    codes,
    Error,
    ErrorCategory,
    Result,
};

use crate::bounded::{
    BoundedString,
    BoundedVec,
    WasmName,
    MAX_WASM_NAME_LENGTH,
}; // Added BoundedString
use crate::{
    component_value_store::ValueRef,
    traits::{
        BytesWriter,
        Checksummable,
        FromBytes,
        ReadStream,
        SerializationError,
        ToBytes,
        WriteStream,
    },
    verification::Checksum,
    ComponentValueStore,
    FloatBits32,
    FloatBits64,
    MemoryProvider,
    Value,
}; // Added import for ValueRef

// no_std is configured at the crate level
#[forbid(clippy::unwrap_used, clippy::expect_used)]
extern crate alloc; // Binary std/no_std choice

// Binary std/no_std choice
use core::{
    fmt,
    hash::{
        Hash,
        Hasher as CoreHasher,
    },
};
#[cfg(feature = "std")]
use std::borrow::ToOwned;

// Use constants from bounded.rs
use crate::bounded::{
    MAX_COMPONENT_ERROR_CONTEXT_ITEMS,
    MAX_COMPONENT_FIXED_LIST_ITEMS,
    MAX_COMPONENT_FLAGS,
    MAX_COMPONENT_LIST_ITEMS,
    MAX_COMPONENT_RECORD_FIELDS,
    MAX_COMPONENT_TUPLE_ITEMS,
    MAX_DESERIALIZED_VALUES,
    MAX_WASM_STRING_LENGTH as MAX_COMPONENT_STRING_LENGTH,
};
#[cfg(feature = "std")]
use crate::prelude::{
    format,
    vec,
    BTreeMap,
    ToString as _,
}; // Removed String, Vec

// Define any component-value specific constants not in bounded.rs
pub const MAX_STORED_COMPONENT_VALUES: usize = 256; // For ComponentValueStore capacity
                                                    // Assuming MemoryProvider P will be passed or accessible where ComponentValue
                                                    // is constructed with these BoundedVecs. For now, the enum definition won't
                                                    // take P directly, but construction sites will need it.

// Define ValTypeRef for recursive type definitions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ValTypeRef(pub u32); // Default derive is for placeholder Box::new(ValType::default())

impl Checksummable for ValTypeRef {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.0.update_checksum(checksum); // u32 is Checksummable
    }
}

impl ToBytes for ValTypeRef {
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        _provider: &PStream, // provider not needed for u32
    ) -> Result<()> {
        writer.write_u32_le(self.0)
    }
}

impl FromBytes for ValTypeRef {
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        _provider: &PStream, // provider not needed for u32
    ) -> Result<Self> {
        let val = reader.read_u32_le()?;
        Ok(ValTypeRef(val))
    }
}

// Use constants from bounded.rs
use crate::bounded::{
    MAX_TYPE_ENUM_NAMES,
    MAX_TYPE_FLAGS_NAMES,
    MAX_TYPE_RECORD_FIELDS,
    MAX_TYPE_TUPLE_ELEMENTS,
    MAX_TYPE_VARIANT_CASES,
};

/// A Component Model value type
#[derive(Debug, Clone, PartialEq, Eq, core::hash::Hash)]
pub enum ValType<P: MemoryProvider + Default + Clone + PartialEq + Eq> {
    // Made Generic over P, removed Default derive
    /// Boolean value
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
    /// Reference to another entity
    Ref(u32),
    /// Record with named fields
    Record(BoundedVec<(WasmName<MAX_WASM_NAME_LENGTH>, ValTypeRef), MAX_TYPE_RECORD_FIELDS, P>),
    /// Variant with cases
    Variant(
        BoundedVec<
            (WasmName<MAX_WASM_NAME_LENGTH>, Option<ValTypeRef>),
            MAX_TYPE_VARIANT_CASES,
            P,
        >,
    ),
    /// List of elements
    List(ValTypeRef), // Replaced Box<ValType>
    /// Fixed-length list of elements with a known length
    FixedList(ValTypeRef, u32), // Replaced Box<ValType>
    /// Tuple of elements
    Tuple(BoundedVec<ValTypeRef, MAX_TYPE_TUPLE_ELEMENTS, P>),
    /// Flags (set of named boolean flags)
    Flags(BoundedVec<WasmName<MAX_WASM_NAME_LENGTH>, MAX_TYPE_FLAGS_NAMES, P>),
    /// Enumeration of variants
    Enum(BoundedVec<WasmName<MAX_WASM_NAME_LENGTH>, MAX_TYPE_ENUM_NAMES, P>),
    /// `Option` type
    Option(ValTypeRef), // Replaced Box<ValType>
    /// `Result` type with both `Ok` and `Err` types (both optional for void)
    Result {
        ok:  Option<ValTypeRef>,
        err: Option<ValTypeRef>,
    }, /* Replaced Result/ResultErr/
        * ResultBoth */
    /// Resource handle (owned)
    Own(u32),
    /// Resource handle (borrowed)
    Borrow(u32),
    /// Void type
    Void,
    /// Error context type
    ErrorContext,
}

impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> Default for ValType<P> {
    fn default() -> Self {
        // A sensible default, e.g. Void or Bool. Let's use Bool as it's simple.
        ValType::Bool
    }
}

impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> Checksummable for ValType<P> {
    fn update_checksum(&self, checksum: &mut Checksum) {
        // Determine a unique u8 discriminant for each variant
        let discriminant: u8 = match self {
            ValType::Bool => 0,
            ValType::S8 => 1,
            ValType::U8 => 2,
            ValType::S16 => 3,
            ValType::U16 => 4,
            ValType::S32 => 5,
            ValType::U32 => 6,
            ValType::S64 => 7,
            ValType::U64 => 8,
            ValType::F32 => 9,
            ValType::F64 => 10,
            ValType::Char => 11,
            ValType::String => 12,
            ValType::Ref(_) => 13,
            ValType::Record(_) => 14,
            ValType::Variant(_) => 15,
            ValType::List(_) => 16,
            ValType::FixedList(_, _) => 17,
            ValType::Tuple(_) => 18,
            ValType::Flags(_) => 19,
            ValType::Enum(_) => 20,
            ValType::Option(_) => 21,
            ValType::Result { .. } => 22,
            ValType::Own(_) => 23,
            ValType::Borrow(_) => 24,
            ValType::Void => 25,
            ValType::ErrorContext => 26,
        };
        discriminant.update_checksum(checksum); // Checksum the discriminant

        // Then checksum the data for variants that have it
        match self {
            ValType::Bool
            | ValType::S8
            | ValType::U8
            | ValType::S16
            | ValType::U16
            | ValType::S32
            | ValType::U32
            | ValType::S64
            | ValType::U64
            | ValType::F32
            | ValType::F64
            | ValType::Char
            | ValType::String
            | ValType::Void
            | ValType::ErrorContext => {}, // No extra data for these simple variants
            ValType::Ref(id) => id.update_checksum(checksum),
            ValType::Record(fields) => fields.update_checksum(checksum),
            ValType::Variant(cases) => cases.update_checksum(checksum),
            ValType::List(element_type_ref) => element_type_ref.update_checksum(checksum),
            ValType::FixedList(element_type_ref, len) => {
                element_type_ref.update_checksum(checksum);
                len.update_checksum(checksum);
            },
            ValType::Tuple(elements) => elements.update_checksum(checksum),
            ValType::Flags(names) => names.update_checksum(checksum),
            ValType::Enum(names) => names.update_checksum(checksum),
            ValType::Option(type_ref) => type_ref.update_checksum(checksum),
            ValType::Result { ok, err } => {
                ok.update_checksum(checksum);
                err.update_checksum(checksum);
            },
            ValType::Own(id) | ValType::Borrow(id) => id.update_checksum(checksum),
        }
    }
}

impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> ToBytes for ValType<P> {
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PStream,
    ) -> Result<()> {
        match self {
            ValType::Bool => writer.write_u8(0)?,
            ValType::S8 => writer.write_u8(1)?,
            ValType::U8 => writer.write_u8(2)?,
            ValType::S16 => writer.write_u8(3)?,
            ValType::U16 => writer.write_u8(4)?,
            ValType::S32 => writer.write_u8(5)?,
            ValType::U32 => writer.write_u8(6)?,
            ValType::S64 => writer.write_u8(7)?,
            ValType::U64 => writer.write_u8(8)?,
            ValType::F32 => writer.write_u8(9)?,
            ValType::F64 => writer.write_u8(10)?,
            ValType::Char => writer.write_u8(11)?,
            ValType::String => writer.write_u8(12)?,
            ValType::Ref(id) => {
                writer.write_u8(13)?;
                writer.write_u32_le(*id)?;
            },
            ValType::Record(fields) => {
                writer.write_u8(14)?;
                fields.to_bytes_with_provider(writer, provider)?;
            },
            ValType::Variant(cases) => {
                writer.write_u8(15)?;
                cases.to_bytes_with_provider(writer, provider)?;
            },
            ValType::List(element_type_ref) => {
                writer.write_u8(16)?;
                element_type_ref.to_bytes_with_provider(writer, provider)?;
            },
            ValType::FixedList(element_type_ref, len) => {
                writer.write_u8(17)?;
                element_type_ref.to_bytes_with_provider(writer, provider)?;
                writer.write_u32_le(*len)?;
            },
            ValType::Tuple(elements) => {
                writer.write_u8(18)?;
                elements.to_bytes_with_provider(writer, provider)?;
            },
            ValType::Flags(names) => {
                writer.write_u8(19)?;
                names.to_bytes_with_provider(writer, provider)?;
            },
            ValType::Enum(names) => {
                writer.write_u8(20)?;
                names.to_bytes_with_provider(writer, provider)?;
            },
            ValType::Option(type_ref) => {
                writer.write_u8(21)?;
                type_ref.to_bytes_with_provider(writer, provider)?;
            },
            ValType::Result { ok, err } => {
                writer.write_u8(22)?;
                match ok {
                    Some(ok_ref) => {
                        writer.write_u8(1)?;
                        ok_ref.to_bytes_with_provider(writer, provider)?;
                    },
                    None => writer.write_u8(0)?,
                }
                match err {
                    Some(err_ref) => {
                        writer.write_u8(1)?;
                        err_ref.to_bytes_with_provider(writer, provider)?;
                    },
                    None => writer.write_u8(0)?,
                }
            },
            ValType::Own(id) => {
                writer.write_u8(23)?;
                writer.write_u32_le(*id)?;
            },
            ValType::Borrow(id) => {
                writer.write_u8(24)?;
                writer.write_u32_le(*id)?;
            },
            ValType::Void => writer.write_u8(25)?,
            ValType::ErrorContext => writer.write_u8(26)?,
        }
        Ok(())
    }
}

impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> FromBytes for ValType<P> {
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream,
    ) -> Result<Self> {
        let discriminant = reader.read_u8()?;
        match discriminant {
            0 => Ok(ValType::Bool),
            1 => Ok(ValType::S8),
            2 => Ok(ValType::U8),
            3 => Ok(ValType::S16),
            4 => Ok(ValType::U16),
            5 => Ok(ValType::S32),
            6 => Ok(ValType::U32),
            7 => Ok(ValType::S64),
            8 => Ok(ValType::U64),
            9 => Ok(ValType::F32),
            10 => Ok(ValType::F64),
            11 => Ok(ValType::Char),
            12 => Ok(ValType::String),
            13 => {
                let id = reader.read_u32_le()?;
                Ok(ValType::Ref(id))
            },
            14 => {
                let fields = BoundedVec::<
                    (WasmName<MAX_WASM_NAME_LENGTH>, ValTypeRef),
                    MAX_TYPE_RECORD_FIELDS,
                    P,
                >::from_bytes_with_provider(reader, provider)?;
                Ok(ValType::Record(fields))
            },
            15 => {
                let cases = BoundedVec::<
                    (WasmName<MAX_WASM_NAME_LENGTH>, Option<ValTypeRef>),
                    MAX_TYPE_VARIANT_CASES,
                    P,
                >::from_bytes_with_provider(reader, provider)?;
                Ok(ValType::Variant(cases))
            },
            16 => {
                let etr = ValTypeRef::from_bytes_with_provider(reader, provider)?;
                Ok(ValType::List(etr))
            },
            17 => {
                let etr = ValTypeRef::from_bytes_with_provider(reader, provider)?;
                let len = reader.read_u32_le()?;
                Ok(ValType::FixedList(etr, len))
            },
            18 => {
                let elements =
                    BoundedVec::<ValTypeRef, MAX_TYPE_TUPLE_ELEMENTS, P>::from_bytes_with_provider(
                        reader, provider,
                    )?;
                Ok(ValType::Tuple(elements))
            },
            19 => {
                let names = BoundedVec::<WasmName<MAX_WASM_NAME_LENGTH>, MAX_TYPE_FLAGS_NAMES, P>::from_bytes_with_provider(reader, provider)?;
                Ok(ValType::Flags(names))
            },
            20 => {
                let names = BoundedVec::<WasmName<MAX_WASM_NAME_LENGTH>, MAX_TYPE_ENUM_NAMES, P>::from_bytes_with_provider(reader, provider)?;
                Ok(ValType::Enum(names))
            },
            21 => {
                let type_ref = ValTypeRef::from_bytes_with_provider(reader, provider)?;
                Ok(ValType::Option(type_ref))
            },
            22 => {
                let ok_present = reader.read_u8()? == 1;
                let ok_ref = if ok_present {
                    Some(ValTypeRef::from_bytes_with_provider(reader, provider)?)
                } else {
                    None
                };
                let err_present = reader.read_u8()? == 1;
                let err_ref = if err_present {
                    Some(ValTypeRef::from_bytes_with_provider(reader, provider)?)
                } else {
                    None
                };
                Ok(ValType::Result {
                    ok:  ok_ref,
                    err: err_ref,
                })
            },
            23 => {
                let id = reader.read_u32_le()?;
                Ok(ValType::Own(id))
            },
            24 => {
                let id = reader.read_u32_le()?;
                Ok(ValType::Borrow(id))
            },
            25 => Ok(ValType::Void),
            26 => Ok(ValType::ErrorContext),
            _ => Err(SerializationError::InvalidFormat.into()),
        }
    }
}

/// WebAssembly component value types
#[derive(Debug, Clone)] // Removed PartialEq, Eq will also need to be manual if floats are involved
pub enum ComponentValue<P: MemoryProvider + Default + Clone + PartialEq + Eq> {
    /// Invalid/uninitialized value
    Void,
    /// Unit value (no data, but meaningful presence - like `()` in Rust)
    Unit,
    /// Boolean value (true/false)
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
    F32(FloatBits32), // Changed from f32
    /// 64-bit floating point
    F64(FloatBits64), // Changed from f64
    /// Unicode character
    Char(char),
    /// UTF-8 string
    #[cfg(feature = "std")]
    String(crate::prelude::String),
    #[cfg(not(any(feature = "std")))]
    String(BoundedString<MAX_COMPONENT_STRING_LENGTH>),
    /// List of component values
    List(BoundedVec<ValueRef, MAX_COMPONENT_LIST_ITEMS, P>),
    /// Fixed-length list of component values with a known length
    FixedList(BoundedVec<ValueRef, MAX_COMPONENT_FIXED_LIST_ITEMS, P>, u32),
    /// Record with named fields
    Record(
        BoundedVec<(WasmName<MAX_WASM_NAME_LENGTH>, ValueRef), MAX_COMPONENT_RECORD_FIELDS, P>,
    ),
    /// Variant with case name and optional value
    Variant(WasmName<MAX_WASM_NAME_LENGTH>, Option<ValueRef>),
    /// Tuple of component values
    Tuple(BoundedVec<ValueRef, MAX_COMPONENT_TUPLE_ITEMS, P>),
    /// Flags with boolean fields
    Flags(BoundedVec<(WasmName<MAX_WASM_NAME_LENGTH>, bool), MAX_COMPONENT_FLAGS, P>),
    /// Enumeration with case name
    Enum(WasmName<MAX_WASM_NAME_LENGTH>),
    /// Optional value (`Some`/`None`)
    Option(Option<ValueRef>),
    /// `Result` value (`Ok`/`Err`)
    Result(core::result::Result<ValueRef, ValueRef>),
    /// Handle to a resource (`u32` representation)
    Own(u32),
    /// Reference to a borrowed resource (`u32` representation)
    Borrow(u32),
    /// Generic handle to a resource
    Handle(u32),
    /// Error context information
    ErrorContext(BoundedVec<ValueRef, MAX_COMPONENT_ERROR_CONTEXT_ITEMS, P>),
}

impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> Default for ComponentValue<P> {
    fn default() -> Self {
        ComponentValue::Void // Void is a safe and simple default
    }
}

impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> Checksummable for ComponentValue<P> {
    fn update_checksum(&self, checksum: &mut Checksum) {
        // Manually write a discriminant byte then checksum the inner value
        match self {
            ComponentValue::Void => checksum.update_slice(&[0]),
            ComponentValue::Unit => checksum.update_slice(&[255]),
            ComponentValue::Bool(v) => {
                checksum.update_slice(&[1]);
                v.update_checksum(checksum);
            },
            ComponentValue::S8(v) => {
                checksum.update_slice(&[2]);
                v.update_checksum(checksum);
            },
            ComponentValue::U8(v) => {
                checksum.update_slice(&[3]);
                v.update_checksum(checksum);
            },
            ComponentValue::S16(v) => {
                checksum.update_slice(&[4]);
                v.update_checksum(checksum);
            },
            ComponentValue::U16(v) => {
                checksum.update_slice(&[5]);
                v.update_checksum(checksum);
            },
            ComponentValue::S32(v) => {
                checksum.update_slice(&[6]);
                v.update_checksum(checksum);
            },
            ComponentValue::U32(v) => {
                checksum.update_slice(&[7]);
                v.update_checksum(checksum);
            },
            ComponentValue::S64(v) => {
                checksum.update_slice(&[8]);
                v.update_checksum(checksum);
            },
            ComponentValue::U64(v) => {
                checksum.update_slice(&[9]);
                v.update_checksum(checksum);
            },
            ComponentValue::F32(v) => {
                checksum.update_slice(&[10]);
                v.update_checksum(checksum);
            }, // v is FloatBits32
            ComponentValue::F64(v) => {
                checksum.update_slice(&[11]);
                v.update_checksum(checksum);
            }, // v is FloatBits64
            ComponentValue::Char(v) => {
                checksum.update_slice(&[12]);
                (*v as u32).update_checksum(checksum);
            }, // Checksum char as u32
            #[cfg(feature = "std")]
            ComponentValue::String(s) => {
                checksum.update_slice(&[13]);
                s.update_checksum(checksum);
            },
            #[cfg(not(any(feature = "std")))]
            ComponentValue::String(s) => {
                checksum.update_slice(&[13]);
                s.update_checksum(checksum);
            }, // BoundedString
            ComponentValue::List(v) => {
                checksum.update_slice(&[14]);
                v.update_checksum(checksum);
            },
            ComponentValue::FixedList(v, len) => {
                checksum.update_slice(&[15]);
                v.update_checksum(checksum);
                len.update_checksum(checksum);
            },
            ComponentValue::Record(v) => {
                checksum.update_slice(&[16]);
                v.update_checksum(checksum);
            },
            ComponentValue::Variant(name, opt_v) => {
                checksum.update_slice(&[17]);
                name.update_checksum(checksum);
                opt_v.update_checksum(checksum);
            },
            ComponentValue::Tuple(v) => {
                checksum.update_slice(&[18]);
                v.update_checksum(checksum);
            },
            ComponentValue::Flags(v) => {
                checksum.update_slice(&[19]);
                v.update_checksum(checksum);
            },
            ComponentValue::Enum(name) => {
                checksum.update_slice(&[20]);
                name.update_checksum(checksum);
            },
            ComponentValue::Option(opt_v) => {
                checksum.update_slice(&[21]);
                opt_v.update_checksum(checksum);
            },
            ComponentValue::Result(res) => {
                checksum.update_slice(&[22]);
                match res {
                    Ok(ok_v) => {
                        checksum.update_slice(&[0]);
                        ok_v.update_checksum(checksum);
                    },
                    Err(err_v) => {
                        checksum.update_slice(&[1]);
                        err_v.update_checksum(checksum);
                    },
                }
            },
            ComponentValue::Own(handle) => {
                checksum.update_slice(&[23]);
                handle.update_checksum(checksum);
            },
            ComponentValue::Borrow(handle) => {
                checksum.update_slice(&[24]);
                handle.update_checksum(checksum);
            },
            ComponentValue::Handle(handle) => {
                checksum.update_slice(&[26]);
                handle.update_checksum(checksum);
            },
            ComponentValue::ErrorContext(v) => {
                checksum.update_slice(&[25]);
                v.update_checksum(checksum);
            },
        }
    }
}

// Manual implementation of PartialEq and Eq for ComponentValue
// This is needed because f32/f64 are not Eq, but FloatBits32/64 are.
impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> PartialEq for ComponentValue<P> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ComponentValue::Void, ComponentValue::Void) => true,
            (ComponentValue::Unit, ComponentValue::Unit) => true,
            (ComponentValue::Bool(a), ComponentValue::Bool(b)) => a == b,
            (ComponentValue::S8(a), ComponentValue::S8(b)) => a == b,
            (ComponentValue::U8(a), ComponentValue::U8(b)) => a == b,
            (ComponentValue::S16(a), ComponentValue::S16(b)) => a == b,
            (ComponentValue::U16(a), ComponentValue::U16(b)) => a == b,
            (ComponentValue::S32(a), ComponentValue::S32(b)) => a == b,
            (ComponentValue::U32(a), ComponentValue::U32(b)) => a == b,
            (ComponentValue::S64(a), ComponentValue::S64(b)) => a == b,
            (ComponentValue::U64(a), ComponentValue::U64(b)) => a == b,
            (ComponentValue::F32(a), ComponentValue::F32(b)) => a == b, // FloatBits32 is Eq
            (ComponentValue::F64(a), ComponentValue::F64(b)) => a == b, // FloatBits64 is Eq
            (ComponentValue::Char(a), ComponentValue::Char(b)) => a == b,
            (ComponentValue::String(a), ComponentValue::String(b)) => a == b,
            (ComponentValue::List(a), ComponentValue::List(b)) => a == b,
            (ComponentValue::FixedList(a_val, a_len), ComponentValue::FixedList(b_val, b_len)) => {
                a_val == b_val && a_len == b_len
            },
            (ComponentValue::Record(a), ComponentValue::Record(b)) => a == b,
            (ComponentValue::Variant(a_name, a_val), ComponentValue::Variant(b_name, b_val)) => {
                a_name == b_name && a_val == b_val
            },
            (ComponentValue::Tuple(a), ComponentValue::Tuple(b)) => a == b,
            (ComponentValue::Flags(a), ComponentValue::Flags(b)) => a == b,
            (ComponentValue::Enum(a), ComponentValue::Enum(b)) => a == b,
            (ComponentValue::Option(a), ComponentValue::Option(b)) => a == b,
            (ComponentValue::Result(a), ComponentValue::Result(b)) => a == b,
            (ComponentValue::Own(a), ComponentValue::Own(b)) => a == b,
            (ComponentValue::Borrow(a), ComponentValue::Borrow(b)) => a == b,
            (ComponentValue::ErrorContext(a), ComponentValue::ErrorContext(b)) => a == b,
            _ => false, // Different variants
        }
    }
}
impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> Eq for ComponentValue<P> {}

// Manual implementation of Hash for ComponentValue
impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> Hash for ComponentValue<P> {
    fn hash<H: CoreHasher>(&self, state: &mut H) {
        // Hash discriminant first
        core::mem::discriminant(self).hash(state);

        // Then hash the data for each variant
        match self {
            ComponentValue::Void | ComponentValue::Unit => {},
            ComponentValue::Bool(v) => v.hash(state),
            ComponentValue::S8(v) => v.hash(state),
            ComponentValue::U8(v) => v.hash(state),
            ComponentValue::S16(v) => v.hash(state),
            ComponentValue::U16(v) => v.hash(state),
            ComponentValue::S32(v) => v.hash(state),
            ComponentValue::U32(v) => v.hash(state),
            ComponentValue::S64(v) => v.hash(state),
            ComponentValue::U64(v) => v.hash(state),
            ComponentValue::F32(v) => v.hash(state),
            ComponentValue::F64(v) => v.hash(state),
            ComponentValue::Char(v) => v.hash(state),
            ComponentValue::String(v) => v.hash(state),
            ComponentValue::List(v) => v.hash(state),
            ComponentValue::FixedList(v, len) => {
                v.hash(state);
                len.hash(state);
            },
            ComponentValue::Record(v) => v.hash(state),
            ComponentValue::Variant(name, val) => {
                name.hash(state);
                val.hash(state);
            },
            ComponentValue::Tuple(v) => v.hash(state),
            ComponentValue::Flags(v) => v.hash(state),
            ComponentValue::Enum(v) => v.hash(state),
            ComponentValue::Option(v) => v.hash(state),
            ComponentValue::Result(v) => v.hash(state),
            ComponentValue::Own(v) => v.hash(state),
            ComponentValue::Borrow(v) => v.hash(state),
            ComponentValue::Handle(v) => v.hash(state),
            ComponentValue::ErrorContext(v) => v.hash(state),
        }
    }
}

impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> ComponentValue<P> {
    /// Get the type of this component value
    ///
    /// NOTE: This returns a ValType with type references (ValTypeRef) that may need
    /// resolution via a type table. For complex types (Record, Variant, List, etc.),
    /// the returned type will contain indices that require lookup in a ComponentValueStore.
    pub fn get_type(&self) -> ValType<P> {
        match self {
            ComponentValue::Void => ValType::Void,
            ComponentValue::Unit => ValType::Tuple(BoundedVec::new(P::default()).unwrap_or_else(|_| panic!("Failed to create empty tuple type"))),
            ComponentValue::Bool(_) => ValType::Bool,
            ComponentValue::S8(_) => ValType::S8,
            ComponentValue::U8(_) => ValType::U8,
            ComponentValue::S16(_) => ValType::S16,
            ComponentValue::U16(_) => ValType::U16,
            ComponentValue::S32(_) => ValType::S32,
            ComponentValue::U32(_) => ValType::U32,
            ComponentValue::S64(_) => ValType::S64,
            ComponentValue::U64(_) => ValType::U64,
            ComponentValue::F32(_) => ValType::F32,
            ComponentValue::F64(_) => ValType::F64,
            ComponentValue::Char(_) => ValType::Char,
            ComponentValue::String(_) => ValType::String,
            // For complex types that use ValueRef, we return placeholder types
            // TODO: These need a type table to properly resolve the ValTypeRef
            ComponentValue::List(_) => ValType::List(ValTypeRef(0)), // Placeholder index
            ComponentValue::FixedList(_, size) => ValType::FixedList(ValTypeRef(0), *size),
            ComponentValue::Record(_) => {
                // Return empty record - proper resolution requires type table
                ValType::Record(BoundedVec::new(P::default()).unwrap_or_else(|_| panic!("Failed to create empty record type")))
            },
            ComponentValue::Variant(_, _) => {
                // Return empty variant - proper resolution requires type table
                ValType::Variant(BoundedVec::new(P::default()).unwrap_or_else(|_| panic!("Failed to create empty variant type")))
            },
            ComponentValue::Tuple(_) => {
                // Return empty tuple - proper resolution requires type table
                ValType::Tuple(BoundedVec::new(P::default()).unwrap_or_else(|_| panic!("Failed to create empty tuple type")))
            },
            ComponentValue::Flags(_) => {
                // Return empty flags - proper resolution requires type table
                ValType::Flags(BoundedVec::new(P::default()).unwrap_or_else(|_| panic!("Failed to create empty flags type")))
            },
            ComponentValue::Enum(_) => {
                // Return empty enum - proper resolution requires type table
                ValType::Enum(BoundedVec::new(P::default()).unwrap_or_else(|_| panic!("Failed to create empty enum type")))
            },
            ComponentValue::Option(_) => ValType::Option(ValTypeRef(0)), // Placeholder index
            ComponentValue::Result(_) => ValType::Result { ok: None, err: None }, // Placeholder
            ComponentValue::Own(idx) => ValType::Own(*idx),
            ComponentValue::Borrow(idx) => ValType::Borrow(*idx),
            ComponentValue::Handle(idx) => ValType::Own(*idx), // Handle maps to Own
            ComponentValue::ErrorContext(_) => ValType::ErrorContext,
        }
    }
}

impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> ToBytes for ComponentValue<P> {
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PStream,
    ) -> Result<()> {
        match self {
            ComponentValue::Void => writer.write_u8(0)?,
            ComponentValue::Unit => writer.write_u8(255)?,
            ComponentValue::Bool(b) => {
                writer.write_u8(1)?;
                writer.write_u8(if *b { 1 } else { 0 })?;
            },
            ComponentValue::S8(val) => {
                writer.write_u8(2)?;
                writer.write_i8(*val)?;
            },
            ComponentValue::U8(val) => {
                writer.write_u8(3)?;
                writer.write_u8(*val)?;
            },
            ComponentValue::S16(val) => {
                writer.write_u8(4)?;
                writer.write_i16_le(*val)?;
            },
            ComponentValue::U16(val) => {
                writer.write_u8(5)?;
                writer.write_u16_le(*val)?;
            },
            ComponentValue::S32(val) => {
                writer.write_u8(6)?;
                writer.write_i32_le(*val)?;
            },
            ComponentValue::U32(val) => {
                writer.write_u8(7)?;
                writer.write_u32_le(*val)?;
            },
            ComponentValue::S64(val) => {
                writer.write_u8(8)?;
                writer.write_i64_le(*val)?;
            },
            ComponentValue::U64(val) => {
                writer.write_u8(9)?;
                writer.write_u64_le(*val)?;
            },
            ComponentValue::F32(val) => {
                writer.write_u8(10)?;
                val.to_bytes_with_provider(writer, provider)?;
            },
            ComponentValue::F64(val) => {
                writer.write_u8(11)?;
                val.to_bytes_with_provider(writer, provider)?;
            },
            ComponentValue::Char(c) => {
                // char is u32
                writer.write_u8(12)?;
                writer.write_u32_le(*c as u32)?;
            },
            ComponentValue::String(s) => {
                writer.write_u8(13)?;
                s.to_bytes_with_provider(writer, provider)?;
            },
            ComponentValue::List(items) => {
                writer.write_u8(14)?;
                items.to_bytes_with_provider(writer, provider)?;
            },
            ComponentValue::FixedList(items, len) => {
                writer.write_u8(15)?;
                items.to_bytes_with_provider(writer, provider)?;
                // len is part of BoundedVec structure, not serialized separately usually
                // but the struct has it explicitly. Let's assume items already handles its
                // count and 'len' here is redundant for BoundedVec
                // serialization, or means something else. Given current
                // BoundedVec likely serializes its own length, this u32 'len' might be extra.
                // For now, let's serialize it as it's in the struct.
                writer.write_u32_le(*len)?;
            },
            ComponentValue::Record(fields) => {
                writer.write_u8(16)?;
                fields.to_bytes_with_provider(writer, provider)?;
            },
            ComponentValue::Variant(name, opt_val_ref) => {
                writer.write_u8(17)?;
                name.to_bytes_with_provider(writer, provider)?;
                match opt_val_ref {
                    Some(val_ref) => {
                        writer.write_u8(1)?;
                        val_ref.to_bytes_with_provider(writer, provider)?;
                    },
                    None => writer.write_u8(0)?,
                }
            },
            ComponentValue::Tuple(items) => {
                writer.write_u8(18)?;
                items.to_bytes_with_provider(writer, provider)?;
            },
            ComponentValue::Flags(flags) => {
                writer.write_u8(19)?;
                flags.to_bytes_with_provider(writer, provider)?;
            },
            ComponentValue::Enum(name) => {
                writer.write_u8(20)?;
                name.to_bytes_with_provider(writer, provider)?;
            },
            ComponentValue::Option(opt_val_ref) => {
                writer.write_u8(21)?;
                match opt_val_ref {
                    Some(val_ref) => {
                        writer.write_u8(1)?;
                        val_ref.to_bytes_with_provider(writer, provider)?;
                    },
                    None => writer.write_u8(0)?,
                }
            },
            ComponentValue::Result(res) => {
                writer.write_u8(22)?;
                match res {
                    Ok(ok_ref) => {
                        writer.write_u8(1)?;
                        ok_ref.to_bytes_with_provider(writer, provider)?;
                    },
                    Err(err_ref) => {
                        writer.write_u8(0)?;
                        err_ref.to_bytes_with_provider(writer, provider)?;
                    },
                }
            },
            ComponentValue::Own(handle) => {
                writer.write_u8(23)?;
                writer.write_u32_le(*handle)?;
            },
            ComponentValue::Borrow(handle) => {
                writer.write_u8(24)?;
                writer.write_u32_le(*handle)?;
            },
            ComponentValue::ErrorContext(items) => {
                writer.write_u8(25)?;
                items.to_bytes_with_provider(writer, provider)?;
            },
            ComponentValue::Handle(handle) => {
                writer.write_u8(26)?;
                writer.write_u32_le(*handle)?;
            },
        }
        Ok(())
    }
}

impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> FromBytes for ComponentValue<P> {
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream,
    ) -> Result<Self> {
        let discriminant = reader.read_u8()?;
        match discriminant {
            0 => Ok(ComponentValue::Void),
            1 => Ok(ComponentValue::Bool(reader.read_u8()? == 1)),
            2 => Ok(ComponentValue::S8(reader.read_i8()?)),
            3 => Ok(ComponentValue::U8(reader.read_u8()?)),
            4 => Ok(ComponentValue::S16(reader.read_i16_le()?)),
            5 => Ok(ComponentValue::U16(reader.read_u16_le()?)),
            6 => Ok(ComponentValue::S32(reader.read_i32_le()?)),
            7 => Ok(ComponentValue::U32(reader.read_u32_le()?)),
            8 => Ok(ComponentValue::S64(reader.read_i64_le()?)),
            9 => Ok(ComponentValue::U64(reader.read_u64_le()?)),
            10 => {
                let val = FloatBits32::from_bytes_with_provider(reader, provider)?;
                Ok(ComponentValue::F32(val))
            },
            11 => {
                let val = FloatBits64::from_bytes_with_provider(reader, provider)?;
                Ok(ComponentValue::F64(val))
            },
            12 => {
                let c_val = reader.read_u32_le()?;
                Ok(ComponentValue::Char(
                    core::char::from_u32(c_val).ok_or_else(|| {
                        Error::new_static(
                            ErrorCategory::Parse,
                            codes::PARSE_ERROR,
                            "Invalid char value",
                        )
                    })?,
                ))
            },
            13 => {
                // Binary std/no_std choice
                #[cfg(feature = "std")]
                {
                    let len = u32::from_bytes_with_provider(reader, provider)? as usize;
                    let mut bytes = vec![0u8; len];
                    reader
                        .read_exact(&mut bytes)
                        .map_err(|_e| Error::parse_error("Failed to read string bytes"))?;
                    let s = crate::prelude::String::from_utf8(bytes)
                        .map_err(|_e| Error::parse_error("Invalid UTF-8 in string"))?;
                    Ok(ComponentValue::String(s))
                }
                #[cfg(not(any(feature = "std")))]
                {
                    let s =
                        BoundedString::<MAX_COMPONENT_STRING_LENGTH, P>::from_bytes_with_provider(
                            reader, provider,
                        )?;
                    Ok(ComponentValue::String(s))
                }
            },
            14 => {
                let items =
                    BoundedVec::<ValueRef, MAX_COMPONENT_LIST_ITEMS, P>::from_bytes_with_provider(
                        reader, provider,
                    )?;
                Ok(ComponentValue::List(items))
            },
            15 => {
                let items = BoundedVec::<ValueRef, MAX_COMPONENT_FIXED_LIST_ITEMS, P>::from_bytes_with_provider(reader, provider)?;
                let len = reader.read_u32_le()?;
                Ok(ComponentValue::FixedList(items, len))
            },
            16 => {
                let fields = BoundedVec::<
                    (WasmName<MAX_WASM_NAME_LENGTH>, ValueRef),
                    MAX_COMPONENT_RECORD_FIELDS,
                    P,
                >::from_bytes_with_provider(reader, provider)?;
                Ok(ComponentValue::Record(fields))
            },
            17 => {
                let name = WasmName::<MAX_WASM_NAME_LENGTH>::from_bytes_with_provider(
                    reader, provider,
                )?;
                let opt_val_ref = if reader.read_u8()? == 1 {
                    Some(ValueRef::from_bytes_with_provider(reader, provider)?)
                } else {
                    None
                };
                Ok(ComponentValue::Variant(name, opt_val_ref))
            },
            18 => {
                let items =
                    BoundedVec::<ValueRef, MAX_COMPONENT_TUPLE_ITEMS, P>::from_bytes_with_provider(
                        reader, provider,
                    )?;
                Ok(ComponentValue::Tuple(items))
            },
            19 => {
                let flags = BoundedVec::<
                    (WasmName<MAX_WASM_NAME_LENGTH>, bool),
                    MAX_COMPONENT_FLAGS,
                    P,
                >::from_bytes_with_provider(reader, provider)?;
                Ok(ComponentValue::Flags(flags))
            },
            20 => {
                let name = WasmName::<MAX_WASM_NAME_LENGTH>::from_bytes_with_provider(
                    reader, provider,
                )?;
                Ok(ComponentValue::Enum(name))
            },
            21 => {
                let opt_val_ref = if reader.read_u8()? == 1 {
                    Some(ValueRef::from_bytes_with_provider(reader, provider)?)
                } else {
                    None
                };
                Ok(ComponentValue::Option(opt_val_ref))
            },
            22 => {
                let is_ok = reader.read_u8()? == 1;
                if is_ok {
                    let ok_ref = ValueRef::from_bytes_with_provider(reader, provider)?;
                    Ok(ComponentValue::Result(Ok(ok_ref)))
                } else {
                    let err_ref = ValueRef::from_bytes_with_provider(reader, provider)?;
                    Ok(ComponentValue::Result(Err(err_ref)))
                }
            },
            23 => Ok(ComponentValue::Own(reader.read_u32_le()?)),
            24 => Ok(ComponentValue::Borrow(reader.read_u32_le()?)),
            25 => {
                let items = BoundedVec::<ValueRef, MAX_COMPONENT_ERROR_CONTEXT_ITEMS, P>::from_bytes_with_provider(reader, provider)?;
                Ok(ComponentValue::ErrorContext(items))
            },
            26 => Ok(ComponentValue::Handle(reader.read_u32_le()?)),
            255 => Ok(ComponentValue::Unit),
            _ => Err(SerializationError::InvalidFormat.into()),
        }
    }
}

/// Simple serialization of component values
pub fn serialize_component_values<
    P: MemoryProvider + Default + Clone + PartialEq + Eq,
    W: BytesWriter,
>(
    values: &[ComponentValue<P>],
    _store: &ComponentValueStore<P>,
    writer: &mut W,
) -> Result<()> {
    // Write the number of values
    writer.write_all(&(values.len() as u32).to_le_bytes()).map_err(|e| e)?;

    // Write each value (very basic implementation)
    for value in values {
        match value {
            ComponentValue::Bool(b) => {
                writer.write_byte(0)?; // Type tag for bool
                writer.write_byte(if *b { 1 } else { 0 })?;
            },
            ComponentValue::U32(v) => {
                writer.write_byte(1)?; // Type tag for u32
                writer.write_all(&v.to_le_bytes())?;
            },
            ComponentValue::S32(v) => {
                writer.write_byte(2)?; // Type tag for s32
                writer.write_all(&v.to_le_bytes())?;
            },
            // Add more types as needed for intercept functionality
            _ => {
                return Err(Error::runtime_execution_error(
                    "Unsupported component value type for serialization",
                ));
            },
        }
    }
    Ok(())
}

#[cfg(feature = "std")]
pub fn deserialize_component_values<P: MemoryProvider>(
    data: &[u8],
    types: &[ValType<P>], // Binary std/no_std choice
) -> Result<BoundedVec<ComponentValue<P>, MAX_DESERIALIZED_VALUES, P>>
// Changed Vec to BoundedVec
where
    P: Default + Clone + PartialEq + Eq, // Added all required trait bounds
{
    if types.is_empty() && !data.is_empty() {
        return Err(Error::runtime_execution_error(
            "Cannot deserialize values: data provided but no types specified",
        ));
    }

    let mut values = BoundedVec::<ComponentValue<P>, MAX_DESERIALIZED_VALUES, P>::new(P::default())
        .map_err(Error::from)?;
    let mut offset = 0;

    for value_type in types {
        if offset >= data.len() {
            // If we run out of data but still have types, it's an error (unless types are
            // all Void?) For simplicity, consider this an error. More nuanced
            // handling might be needed.
            return Err(decoding_error("Insufficient data for all specified types"));
        }

        // Here we'd ideally use ValType to guide deserialization, especially for
        // complex types like lists or records where the structure isn't
        // self-describing in the byte stream alone.
        // For now, ComponentValue::from_bytes is somewhat self-describing via
        // discriminant.
        let slice = crate::safe_memory::Slice::new(&data[offset..]).map_err(|_e| {
            Error::runtime_execution_error("Failed to create slice for deserialization")
        })?;
        let mut reader = crate::traits::ReadStream::new(slice);
        match ComponentValue::<P>::from_bytes_with_provider(&mut reader, &P::default()) {
            Ok(cv) => {
                let bytes_read = reader.position();
                // TODO: Validate `cv` against `value_type` here.
                // This is a crucial step for safety and correctness.
                // Temporarily commenting out until matches_type method is implemented
                // if !cv.matches_type(value_type,
                // &ComponentValueStore::new(P::default()).map_err(Error::from)?) {
                //     return Err(decoding_error("type mismatch")); }

                values.push(cv).map_err(Error::from)?;
                offset += bytes_read;
            },
            Err(e) => {
                // Convert SerializationError to wrt_error::Error
                return Err(Error::runtime_execution_error(
                    "Failed to deserialize component value",
                ));
            },
        }
    }

    if offset != data.len() {
        // If there's leftover data after deserializing all typed values
        return Err(decoding_error(
            "Unexpected extra data after deserializing all values",
        ));
    }

    Ok(values)
}

/// Create a type conversion error with a descriptive message.
///
/// This is a helper function used for type conversion errors within the
/// component model.
#[must_use]
pub fn conversion_error(_message: &str) -> Error {
    // Use static string regardless of feature flags since Error only accepts
    // &'static str
    Error::invalid_type_error("Type conversion error")
}

/// Create a component encoding error with a descriptive message.
///
/// This is a helper function used for encoding errors within the component
/// model.
#[must_use]
pub fn encoding_error(_message: &str) -> Error {
    Error::component_error("Component encoding error")
}

/// Create a component decoding error with a descriptive message.
///
/// This is a helper function used for decoding errors within the component
/// model.
#[must_use]
pub fn decoding_error(_message: &str) -> Error {
    Error::runtime_execution_error("Component decoding error")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use core::f32::consts::PI;

    use super::*;

    #[test]
    fn test_primitive_value_type_matching() {
        // This test needs a ComponentValueStore instance.
        // For primitive types, the store might not be strictly necessary if
        // they don't involve ValueRef. However, matches_type now
        // requires the store. let provider =
        // crate::NoStdProvider::<1024>::new().unwrap()); // Example provider
        // let store = ComponentValueStore::new(provider).unwrap();

        // let bool_value = ComponentValue::Bool(true;
        // let int_value = ComponentValue::S32(42;
        // let float_value = ComponentValue::F32(PI;

        // assert!(bool_value.matches_type(&ValType::Bool, &store);
        // assert!(!bool_value.matches_type(&ValType::S32, &store);

        // assert!(int_value.matches_type(&ValType::S32, &store);
        // assert!(!int_value.matches_type(&ValType::Bool, &store);
    }
}
