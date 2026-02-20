//! Value compatibility layer for WASI components
//!
//! This module provides a simplified Value enum that matches the interface
//! expected by WASI components while being compatible with wrt-foundation's
//! component value system.

#[cfg(all(not(feature = "std"), feature = "alloc"))]
use alloc::boxed::Box;
#[cfg(feature = "std")]
use std::boxed::Box;
#[cfg(feature = "std")]
use std::string::String;
#[cfg(feature = "std")]
use std::vec::Vec;

use wrt_foundation::prelude::*;
#[cfg(all(not(feature = "std"), not(feature = "alloc")))]
type Box<T> = T; // Simple workaround for no_std without alloc
#[cfg(not(feature = "std"))]
type WasiProvider = wrt_foundation::safe_memory::NoStdProvider<1024>;
#[cfg(not(feature = "std"))]
type WasiString = BoundedString<256, WasiProvider>;
#[cfg(not(feature = "std"))]
type WasiVec<T> = BoundedVec<T, 32, WasiProvider>;

/// Simplified Value enum for WASI component interface
///
/// This provides the variants that WASI code expects while being
/// compatible with wrt-foundation's type system.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Boolean value
    Bool(bool),
    /// Unsigned 8-bit integer
    U8(u8),
    /// Unsigned 16-bit integer  
    U16(u16),
    /// Unsigned 32-bit integer
    U32(u32),
    /// Unsigned 64-bit integer
    U64(u64),
    /// Signed 8-bit integer
    S8(i8),
    /// Signed 16-bit integer
    S16(i16),
    /// Signed 32-bit integer
    S32(i32),
    /// Signed 64-bit integer
    S64(i64),
    /// 32-bit floating point
    F32(f32),
    /// 64-bit floating point
    F64(f64),
    /// String value
    #[cfg(feature = "std")]
    String(String),
    #[cfg(not(feature = "std"))]
    String(WasiString),
    /// List of values
    #[cfg(feature = "std")]
    List(Vec<Value>),
    #[cfg(not(feature = "std"))]
    List(WasiVec<Value>),
    /// Record with key-value pairs
    #[cfg(feature = "std")]
    Record(Vec<(String, Value)>),
    #[cfg(not(feature = "std"))]
    Record(WasiVec<(WasiString, Value)>),
    /// Optional value
    #[cfg(feature = "std")]
    Option(Option<Box<Value>>),
    #[cfg(not(feature = "std"))]
    Option(Option<*const Value>), // Use raw pointer for no_std
    /// Result value
    #[cfg(feature = "std")]
    Result(core::result::Result<Box<Value>, Box<Value>>),
    #[cfg(not(feature = "std"))]
    Result(core::result::Result<*const Value, *const Value>), // Use raw pointers for no_std
    /// Tuple of values
    #[cfg(feature = "std")]
    Tuple(Vec<Value>),
    #[cfg(not(feature = "std"))]
    Tuple(WasiVec<Value>),
}

// Implement Default for Value
impl Default for Value {
    fn default() -> Self {
        Value::U32(0)
    }
}

// Implement Eq for Value (required for BoundedVec)
impl Eq for Value {}

// Implement required traits for BoundedVec compatibility
impl wrt_foundation::traits::Checksummable for Value {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        // Simple checksum based on variant
        let discriminant = match self {
            Value::Bool(_) => 0u8,
            Value::U8(_) => 1,
            Value::U16(_) => 2,
            Value::U32(_) => 3,
            Value::U64(_) => 4,
            Value::S8(_) => 5,
            Value::S16(_) => 6,
            Value::S32(_) => 7,
            Value::S64(_) => 8,
            Value::F32(_) => 9,
            Value::F64(_) => 10,
            Value::String(_) => 11,
            Value::List(_) => 12,
            Value::Record(_) => 13,
            Value::Option(_) => 14,
            Value::Result(_) => 15,
            Value::Tuple(_) => 16,
        };
        checksum.update_slice(&[discriminant]);
    }
}

impl wrt_foundation::traits::ToBytes for Value {
    fn serialized_size(&self) -> usize {
        // Simplified size calculation
        1 + match self {
            Value::Bool(_) | Value::U8(_) | Value::S8(_) => 1,
            Value::U16(_) | Value::S16(_) => 2,
            Value::U32(_) | Value::S32(_) | Value::F32(_) => 4,
            _ => 8, // 8-byte types (U64, S64, F64) and complex types
        }
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        // Simplified serialization
        let discriminant = match self {
            Value::Bool(_) => 0u8,
            Value::U8(_) => 1,
            Value::U16(_) => 2,
            Value::U32(_) => 3,
            Value::U64(_) => 4,
            Value::S8(_) => 5,
            Value::S16(_) => 6,
            Value::S32(_) => 7,
            Value::S64(_) => 8,
            Value::F32(_) => 9,
            Value::F64(_) => 10,
            Value::String(_) => 11,
            Value::List(_) => 12,
            Value::Record(_) => 13,
            Value::Option(_) => 14,
            Value::Result(_) => 15,
            Value::Tuple(_) => 16,
        };
        writer.write_u8(discriminant)?;
        match self {
            Value::Bool(v) => writer.write_u8(u8::from(*v)),
            Value::U8(v) => writer.write_u8(*v),
            // Per CLAUDE.md: fail loud and early for unsupported types
            _ => Err(wrt_error::Error::not_implemented_error(
                "ToBytes not implemented for this Value variant"
            )),
        }
    }
}

impl wrt_foundation::traits::FromBytes for Value {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        // Simplified deserialization
        let discriminant = reader.read_u8()?;
        match discriminant {
            0 => Ok(Value::Bool(reader.read_u8()? != 0)),
            1 => Ok(Value::U8(reader.read_u8()?)),
            3 => {
                // Read u32 manually (4 bytes, little-endian)
                let b0 = u32::from(reader.read_u8()?);
                let b1 = u32::from(reader.read_u8()?);
                let b2 = u32::from(reader.read_u8()?);
                let b3 = u32::from(reader.read_u8()?);
                Ok(Value::U32(b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)))
            },
            // Per CLAUDE.md: fail loud and early for unknown discriminants
            _ => Err(wrt_error::Error::parse_error(
                "Unknown Value discriminant in FromBytes deserialization"
            )),
        }
    }
}

impl Value {
    /// Extract a u32 from the value, returning 0 if not possible
    #[must_use] 
    pub fn as_u32(&self) -> u32 {
        match self {
            Value::U32(v) => *v,
            Value::U16(v) => u32::from(*v),
            Value::U8(v) => u32::from(*v),
            _ => 0,
        }
    }

    /// Extract a u64 from the value, returning 0 if not possible
    #[must_use] 
    pub fn as_u64(&self) -> u64 {
        match self {
            Value::U64(v) => *v,
            Value::U32(v) => u64::from(*v),
            Value::U16(v) => u64::from(*v),
            Value::U8(v) => u64::from(*v),
            _ => 0,
        }
    }

    /// Extract a string from the value, returning empty string if not possible
    #[cfg(feature = "std")]
    #[must_use] 
    pub fn as_string(&self) -> String {
        match self {
            Value::String(s) => s.clone(),
            _ => String::new(),
        }
    }

    /// Extract a string from the value, returning empty string if not possible
    #[cfg(not(feature = "std"))]
    pub fn as_string(&self) -> WasiString {
        match self {
            Value::String(s) => s.clone(),
            _ => {
                // Return empty string for non-string values
                // NO FALLBACK CHAIN - use a single allocation attempt
                let provider = safe_managed_alloc!(1024, CrateId::Wasi)
                    .expect("Failed to allocate memory for empty string - memory exhausted");
                BoundedString::try_from_str("", provider)
                    .expect("Failed to create empty string - should never fail")
            },
        }
    }

    /// Extract a boolean from the value, returning false if not possible
    #[must_use] 
    pub fn as_bool(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::U32(v) => *v != 0,
            Value::U8(v) => *v != 0,
            _ => false,
        }
    }
}
