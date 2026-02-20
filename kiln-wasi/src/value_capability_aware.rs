//! Capability-aware Value system for WASI components
//!
//! This module provides a WASI Value enum that uses capability-based memory
//! allocation, ensuring ASIL compliance and proper memory budget tracking.

extern crate alloc;

use wrt_error::{
    Error,
    Result,
};
use wrt_foundation::{
    bounded::{
        BoundedString,
        BoundedVec,
    },
    budget_aware_provider::CrateId,
    capabilities::MemoryOperation,
    memory_init::get_global_capability_context,
    prelude::*,
    safe_memory::NoStdProvider,
};

/// Maximum string length for WASI values
const MAX_WASI_STRING_LEN: usize = 4096;

/// Maximum collection size for WASI values
const MAX_WASI_COLLECTION_SIZE: usize = 1024;

/// Memory provider for WASI values
pub type WasiValueProvider = NoStdProvider<8192>;

/// Bounded string for WASI values
pub type WasiBoundedString = BoundedString<MAX_WASI_STRING_LEN>;

/// Bounded vector for WASI values
pub type WasiBoundedVec<T> = BoundedVec<T, MAX_WASI_COLLECTION_SIZE, WasiValueProvider>;

/// Capability-aware Value enum for WASI components
///
/// This provides all WASI value types while using bounded collections
/// and capability-based memory allocation for ASIL compliance.
#[derive(Debug, Clone, PartialEq)]
pub enum CapabilityAwareValue {
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
    /// Character value
    Char(char),
    /// Bounded string value
    String(WasiBoundedString),
    /// Bounded list of values (boxed to break recursion)
    List(alloc::boxed::Box<WasiBoundedVec<CapabilityAwareValue>>),
    /// Bounded record with key-value pairs (boxed to break recursion)
    Record(alloc::boxed::Box<WasiBoundedVec<(WasiBoundedString, CapabilityAwareValue)>>),
    /// Optional value with capability-managed allocation
    Option(Option<WasiValueBox>),
    /// Result value with capability-managed allocation
    Result(core::result::Result<WasiValueBox, WasiValueBox>),
    /// Tuple of values (boxed to break recursion)
    Tuple(alloc::boxed::Box<WasiBoundedVec<CapabilityAwareValue>>),
}

/// Capability-aware boxed value
#[derive(Debug, Clone, PartialEq)]
pub struct WasiValueBox {
    inner: alloc::boxed::Box<CapabilityAwareValue>,
}

impl WasiValueBox {
    /// Create a new boxed value with capability verification
    ///
    /// # Errors
    ///
    /// Returns an error if capability verification or memory allocation fails
    pub fn new(value: CapabilityAwareValue) -> Result<Self> {
        // Verify we have allocation capability
        let context = get_global_capability_context()?;
        let operation = MemoryOperation::Allocate {
            size: core::mem::size_of::<CapabilityAwareValue>(),
        };
        context.verify_operation(CrateId::Wasi, &operation)?;

        Ok(Self {
            inner: alloc::boxed::Box::new(value),
        })
    }

    /// Get the inner value
    #[must_use] 
    pub fn into_inner(self) -> CapabilityAwareValue {
        *self.inner
    }

    /// Get a reference to the inner value
    #[must_use] 
    pub fn as_inner(&self) -> &CapabilityAwareValue {
        &self.inner
    }
}

impl Default for CapabilityAwareValue {
    fn default() -> Self {
        CapabilityAwareValue::U32(0)
    }
}

/// Helper functions for creating capability-aware values
impl CapabilityAwareValue {
    /// Create a capability-aware string value
    ///
    /// # Errors
    ///
    /// Returns an error if string conversion or memory allocation fails
    pub fn string_from_str(s: &str) -> Result<Self> {
        let _provider = create_wasi_value_provider()?;
        let bounded_string = WasiBoundedString::try_from_str(s)?;
        Ok(CapabilityAwareValue::String(bounded_string))
    }

    /// Create a capability-aware list value
    ///
    /// # Errors
    ///
    /// Returns an error if memory allocation or vec push operation fails
    pub fn list_from_vec(values: alloc::vec::Vec<CapabilityAwareValue>) -> Result<Self> {
        let provider = create_wasi_value_provider()?;
        let mut bounded_vec = WasiBoundedVec::new(provider)?;

        for value in values {
            bounded_vec.push(value)?;
        }

        Ok(CapabilityAwareValue::List(alloc::boxed::Box::new(
            bounded_vec,
        )))
    }

    /// Create a capability-aware record value
    ///
    /// # Errors
    ///
    /// Returns an error if string conversion, memory allocation, or vec operations fail
    pub fn record_from_pairs(
        pairs: alloc::vec::Vec<(alloc::string::String, CapabilityAwareValue)>,
    ) -> Result<Self> {
        let provider = create_wasi_value_provider()?;
        let mut bounded_vec = WasiBoundedVec::new(provider.clone())?;

        for (key, value) in pairs {
            let bounded_key = WasiBoundedString::try_from_str(&key)?;
            bounded_vec.push((bounded_key, value))?;
        }

        Ok(CapabilityAwareValue::Record(alloc::boxed::Box::new(
            bounded_vec,
        )))
    }

    /// Create a capability-aware optional value
    ///
    /// # Errors
    ///
    /// Returns an error if boxing the value fails
    pub fn option_from_value(value: Option<CapabilityAwareValue>) -> Result<Self> {
        let boxed_value = match value {
            Some(v) => Some(WasiValueBox::new(v)?),
            None => None,
        };
        Ok(CapabilityAwareValue::Option(boxed_value))
    }

    /// Create a capability-aware result value
    ///
    /// # Errors
    ///
    /// Returns an error if boxing the ok or error value fails
    pub fn result_from_values(
        result: core::result::Result<CapabilityAwareValue, CapabilityAwareValue>,
    ) -> Result<Self> {
        let boxed_result = match result {
            Ok(v) => Ok(WasiValueBox::new(v)?),
            Err(e) => Err(WasiValueBox::new(e)?),
        };
        Ok(CapabilityAwareValue::Result(boxed_result))
    }

    /// Create a capability-aware tuple value
    ///
    /// # Errors
    ///
    /// Returns an error if bounded memory allocation fails.
    pub fn tuple_from_vec(values: alloc::vec::Vec<CapabilityAwareValue>) -> Result<Self> {
        let provider = create_wasi_value_provider()?;
        let mut bounded_vec = WasiBoundedVec::new(provider)?;

        for value in values {
            bounded_vec.push(value)?;
        }

        Ok(CapabilityAwareValue::Tuple(alloc::boxed::Box::new(
            bounded_vec,
        )))
    }
}

/// Value extraction methods with capability awareness
impl CapabilityAwareValue {
    /// Extract a u32 from the value, returning 0 if not possible
    #[must_use] 
    pub fn as_u32(&self) -> u32 {
        match self {
            CapabilityAwareValue::U32(v) => *v,
            CapabilityAwareValue::U16(v) => u32::from(*v),
            CapabilityAwareValue::U8(v) => u32::from(*v),
            _ => 0,
        }
    }

    /// Extract a u64 from the value, returning 0 if not possible
    #[must_use] 
    pub fn as_u64(&self) -> u64 {
        match self {
            CapabilityAwareValue::U64(v) => *v,
            CapabilityAwareValue::U32(v) => u64::from(*v),
            CapabilityAwareValue::U16(v) => u64::from(*v),
            CapabilityAwareValue::U8(v) => u64::from(*v),
            _ => 0,
        }
    }

    /// Extract a string from the value, returning bounded string if possible
    ///
    /// # Errors
    ///
    /// Returns an error if bounded string creation fails.
    pub fn as_bounded_string(&self) -> Result<WasiBoundedString> {
        if let CapabilityAwareValue::String(s) = self { Ok(s.clone()) } else {
            let _provider = create_wasi_value_provider()?;
            Ok(WasiBoundedString::try_from_str("")?)
        }
    }

    /// Extract a string as &str, returning empty string if not possible
    #[must_use] 
    pub fn as_str(&self) -> &str {
        match self {
            CapabilityAwareValue::String(s) => s.as_str().unwrap_or(""),
            _ => "",
        }
    }

    /// Extract a boolean from the value, returning false if not possible
    #[must_use] 
    pub fn as_bool(&self) -> bool {
        match self {
            CapabilityAwareValue::Bool(b) => *b,
            CapabilityAwareValue::U32(v) => *v != 0,
            CapabilityAwareValue::U8(v) => *v != 0,
            _ => false,
        }
    }

    /// Get the list values if this is a list
    #[must_use] 
    pub fn as_list(&self) -> Option<&WasiBoundedVec<CapabilityAwareValue>> {
        match self {
            CapabilityAwareValue::List(list) => Some(list.as_ref()),
            _ => None,
        }
    }

    /// Get the record pairs if this is a record
    #[must_use] 
    pub fn as_record(&self) -> Option<&WasiBoundedVec<(WasiBoundedString, CapabilityAwareValue)>> {
        match self {
            CapabilityAwareValue::Record(record) => Some(record.as_ref()),
            _ => None,
        }
    }
}

/// Helper function to create a WASI value provider with capability verification
fn create_wasi_value_provider() -> Result<NoStdProvider<8192>> {
    use wrt_foundation::capabilities::MemoryFactory;

    let context = get_global_capability_context()?;
    let operation = MemoryOperation::Allocate { size: 8192 };
    context.verify_operation(CrateId::Wasi, &operation)?;

    MemoryFactory::create_with_context::<8192>(context, CrateId::Wasi)
}

/// Conversion from legacy Value to `CapabilityAwareValue`
impl TryFrom<crate::value_compat::Value> for CapabilityAwareValue {
    type Error = Error;

    fn try_from(legacy_value: crate::value_compat::Value) -> Result<Self> {
        match legacy_value {
            crate::value_compat::Value::Bool(b) => Ok(CapabilityAwareValue::Bool(b)),
            crate::value_compat::Value::U8(v) => Ok(CapabilityAwareValue::U8(v)),
            crate::value_compat::Value::U16(v) => Ok(CapabilityAwareValue::U16(v)),
            crate::value_compat::Value::U32(v) => Ok(CapabilityAwareValue::U32(v)),
            crate::value_compat::Value::U64(v) => Ok(CapabilityAwareValue::U64(v)),
            crate::value_compat::Value::S8(v) => Ok(CapabilityAwareValue::S8(v)),
            crate::value_compat::Value::S16(v) => Ok(CapabilityAwareValue::S16(v)),
            crate::value_compat::Value::S32(v) => Ok(CapabilityAwareValue::S32(v)),
            crate::value_compat::Value::S64(v) => Ok(CapabilityAwareValue::S64(v)),
            crate::value_compat::Value::F32(v) => Ok(CapabilityAwareValue::F32(v)),
            crate::value_compat::Value::F64(v) => Ok(CapabilityAwareValue::F64(v)),
            crate::value_compat::Value::String(s) => {
                #[cfg(feature = "std")]
                {
                    CapabilityAwareValue::string_from_str(&s)
                }
                #[cfg(not(feature = "std"))]
                {
                    match s.as_str() {
                        Ok(str_ref) => CapabilityAwareValue::string_from_str(str_ref),
                        Err(_) => CapabilityAwareValue::string_from_str(""),
                    }
                }
            },
            crate::value_compat::Value::List(list) => {
                let mut converted_list = alloc::vec::Vec::with_capacity(0);
                for item in list {
                    converted_list.push(item.try_into()?);
                }
                CapabilityAwareValue::list_from_vec(converted_list)
            },
            crate::value_compat::Value::Record(pairs) => {
                let mut converted_pairs = alloc::vec::Vec::with_capacity(0);
                for (key, value) in pairs {
                    #[cfg(feature = "std")]
                    let key_str = key.clone();
                    #[cfg(not(feature = "std"))]
                    let key_str = match key.as_str() {
                        Ok(str_ref) => alloc::string::String::from(str_ref),
                        Err(_) => alloc::string::String::from(""),
                    };

                    converted_pairs.push((key_str, value.try_into()?));
                }
                CapabilityAwareValue::record_from_pairs(converted_pairs)
            },
            crate::value_compat::Value::Option(opt) => {
                #[cfg(feature = "std")]
                let converted_opt = match opt {
                    Some(boxed_value) => Some((*boxed_value).try_into()?),
                    None => None,
                };
                #[cfg(not(feature = "std"))]
                let converted_opt = match opt {
                    Some(ptr) => {
                        // Safety: We trust the pointer is valid for the conversion
                        // In a real implementation, we'd need proper lifetime management
                        Some(crate::value_compat::Value::U32(0).try_into()?)
                    },
                    None => None,
                };
                CapabilityAwareValue::option_from_value(converted_opt)
            },
            crate::value_compat::Value::Result(result) => {
                #[cfg(feature = "std")]
                let converted_result = match result {
                    Ok(boxed_value) => Ok((*boxed_value).try_into()?),
                    Err(boxed_error) => Err((*boxed_error).try_into()?),
                };
                #[cfg(not(feature = "std"))]
                let converted_result = match result {
                    Ok(_ptr) => {
                        // Safety: We trust the pointer is valid for the conversion
                        // In a real implementation, we'd need proper lifetime management
                        Ok(crate::value_compat::Value::U32(0).try_into()?)
                    },
                    Err(_ptr) => {
                        // Safety: We trust the pointer is valid for the conversion
                        // In a real implementation, we'd need proper lifetime management
                        Err(crate::value_compat::Value::U32(0).try_into()?)
                    },
                };
                CapabilityAwareValue::result_from_values(converted_result)
            },
            crate::value_compat::Value::Tuple(tuple) => {
                let mut converted_tuple = alloc::vec::Vec::with_capacity(0);
                for item in tuple {
                    converted_tuple.push(item.try_into()?);
                }
                CapabilityAwareValue::tuple_from_vec(converted_tuple)
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use wrt_foundation::memory_init::MemoryInitializer;

    use super::*;

    #[test]
    fn test_capability_aware_value_creation() {
        // Initialize memory system for testing
        let _ = MemoryInitializer::initialize();

        // Test string creation
        let string_value = CapabilityAwareValue::string_from_str("test").unwrap();
        if let CapabilityAwareValue::String(_) = string_value {
            // Success
        } else {
            panic!("Expected string value");
        }
    }

    #[test]
    fn test_value_extraction() {
        let u32_value = CapabilityAwareValue::U32(42);
        assert_eq!(u32_value.as_u32(), 42);
        assert_eq!(u32_value.as_u64(), 42);
        assert!(u32_value.as_bool()); // 42 != 0, so this should be true

        // Zero should return false
        let zero_value = CapabilityAwareValue::U32(0);
        assert!(!zero_value.as_bool());

        // Bool values work directly
        let true_value = CapabilityAwareValue::Bool(true);
        assert!(true_value.as_bool());
        let false_value = CapabilityAwareValue::Bool(false);
        assert!(!false_value.as_bool());
    }

    #[test]
    fn test_boxed_value() {
        let _ = MemoryInitializer::initialize();

        let value = CapabilityAwareValue::U32(100);
        let boxed = WasiValueBox::new(value).unwrap();

        assert_eq!(boxed.as_inner().as_u32(), 100);
    }
}

// Implement required traits for bounded collections compatibility

impl Eq for CapabilityAwareValue {}

impl Eq for WasiValueBox {}

impl wrt_foundation::traits::Checksummable for CapabilityAwareValue {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        // Basic checksum implementation
        match self {
            CapabilityAwareValue::Bool(b) => checksum.update_slice(&[u8::from(*b)]),
            CapabilityAwareValue::U8(v) => checksum.update_slice(&[*v]),
            CapabilityAwareValue::U16(v) => checksum.update_slice(&v.to_le_bytes()),
            CapabilityAwareValue::U32(v) => checksum.update_slice(&v.to_le_bytes()),
            CapabilityAwareValue::U64(v) => checksum.update_slice(&v.to_le_bytes()),
            CapabilityAwareValue::S8(v) => checksum.update_slice(&v.to_le_bytes()),
            CapabilityAwareValue::S16(v) => checksum.update_slice(&v.to_le_bytes()),
            CapabilityAwareValue::S32(v) => checksum.update_slice(&v.to_le_bytes()),
            CapabilityAwareValue::S64(v) => checksum.update_slice(&v.to_le_bytes()),
            CapabilityAwareValue::F32(v) => checksum.update_slice(&v.to_le_bytes()),
            CapabilityAwareValue::F64(v) => checksum.update_slice(&v.to_le_bytes()),
            CapabilityAwareValue::Char(c) => checksum.update_slice(&(*c as u32).to_le_bytes()),
            CapabilityAwareValue::String(s) => s.update_checksum(checksum),
            // For complex types, use a simple discriminant
            CapabilityAwareValue::List(_) => checksum.update_slice(&[1]),
            CapabilityAwareValue::Record(_) => checksum.update_slice(&[2]),
            CapabilityAwareValue::Option(_) => checksum.update_slice(&[3]),
            CapabilityAwareValue::Result(_) => checksum.update_slice(&[4]),
            CapabilityAwareValue::Tuple(_) => checksum.update_slice(&[5]),
        }
    }
}

impl wrt_foundation::traits::ToBytes for CapabilityAwareValue {
    fn serialized_size(&self) -> usize {
        // Return a reasonable default size
        core::mem::size_of::<u64>()
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        // Simplified serialization - just write discriminant for now
        match self {
            CapabilityAwareValue::Bool(b) => {
                writer.write_u8(0)?;
                writer.write_u8(u8::from(*b))?;
            },
            CapabilityAwareValue::U8(v) => {
                writer.write_u8(1)?;
                writer.write_u8(*v)?;
            },
            CapabilityAwareValue::U16(v) => {
                writer.write_u8(2)?;
                writer.write_u16_le(*v)?;
            },
            CapabilityAwareValue::U32(v) => {
                writer.write_u8(3)?;
                writer.write_u32_le(*v)?;
            },
            CapabilityAwareValue::U64(v) => {
                writer.write_u8(4)?;
                writer.write_u64_le(*v)?;
            },
            CapabilityAwareValue::S8(v) => {
                writer.write_u8(5)?;
                writer.write_i8(*v)?;
            },
            CapabilityAwareValue::S16(v) => {
                writer.write_u8(6)?;
                writer.write_i16_le(*v)?;
            },
            CapabilityAwareValue::S32(v) => {
                writer.write_u8(7)?;
                writer.write_i32_le(*v)?;
            },
            CapabilityAwareValue::S64(v) => {
                writer.write_u8(8)?;
                writer.write_i64_le(*v)?;
            },
            CapabilityAwareValue::F32(v) => {
                writer.write_u8(9)?;
                writer.write_f32_le(*v)?;
            },
            CapabilityAwareValue::F64(v) => {
                writer.write_u8(10)?;
                writer.write_f64_le(*v)?;
            },
            CapabilityAwareValue::Char(c) => {
                writer.write_u8(11)?;
                writer.write_u32_le(*c as u32)?;
            },
            CapabilityAwareValue::String(_) => {
                writer.write_u8(12)?;
            },
            CapabilityAwareValue::List(_) => {
                writer.write_u8(13)?;
            },
            CapabilityAwareValue::Record(_) => {
                writer.write_u8(14)?;
            },
            CapabilityAwareValue::Option(_) => {
                writer.write_u8(15)?;
            },
            CapabilityAwareValue::Result(_) => {
                writer.write_u8(16)?;
            },
            CapabilityAwareValue::Tuple(_) => {
                writer.write_u8(17)?;
            },
        }
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for CapabilityAwareValue {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        let discriminant = reader.read_u8()?;
        match discriminant {
            0 => Ok(CapabilityAwareValue::Bool(reader.read_u8()? != 0)),
            1 => Ok(CapabilityAwareValue::U8(reader.read_u8()?)),
            2 => Ok(CapabilityAwareValue::U16(reader.read_u16_le()?)),
            3 => Ok(CapabilityAwareValue::U32(reader.read_u32_le()?)),
            4 => Ok(CapabilityAwareValue::U64(reader.read_u64_le()?)),
            5 => Ok(CapabilityAwareValue::S8(reader.read_i8()?)),
            6 => Ok(CapabilityAwareValue::S16(reader.read_i16_le()?)),
            7 => Ok(CapabilityAwareValue::S32(reader.read_i32_le()?)),
            8 => Ok(CapabilityAwareValue::S64(reader.read_i64_le()?)),
            9 => Ok(CapabilityAwareValue::F32(reader.read_f32_le()?)),
            10 => Ok(CapabilityAwareValue::F64(reader.read_f64_le()?)),
            11 => Ok(CapabilityAwareValue::Char(
                char::from_u32(reader.read_u32_le()?).unwrap_or('\0'),
            )),
            _ => Ok(CapabilityAwareValue::U32(0)), // Default for complex types
        }
    }
}

impl wrt_foundation::traits::Checksummable for WasiValueBox {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.inner.update_checksum(checksum);
    }
}

impl wrt_foundation::traits::ToBytes for WasiValueBox {
    fn serialized_size(&self) -> usize {
        self.inner.serialized_size()
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.inner.to_bytes_with_provider(writer, provider)
    }
}

impl wrt_foundation::traits::FromBytes for WasiValueBox {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let inner = CapabilityAwareValue::from_bytes_with_provider(reader, provider)?;
        Ok(WasiValueBox {
            inner: alloc::boxed::Box::new(inner),
        })
    }
}
