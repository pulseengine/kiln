//! WebAssembly global value implementation
//!
//! This module provides the implementation for WebAssembly globals.

// Use WrtGlobalType directly from wrt_foundation, and WrtValueType, WrtValue
// alloc is imported in lib.rs with proper feature gates

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::format;
// Import format! macro for string formatting
#[cfg(feature = "std")]
use std::format;

use wrt_foundation::{
    types::{
        GlobalType as WrtGlobalType,
        ValueType as WrtValueType,
    },
    values::Value as WrtValue,
};

use crate::prelude::{
    Debug,
    Eq,
    Error,
    ErrorCategory,
    PartialEq,
    Result,
};

/// Represents a WebAssembly global variable in the runtime
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Global {
    /// The global type (`value_type` and mutability).
    /// The `initial_value` from `WrtGlobalType` is used to set the runtime
    /// `value` field upon creation.
    ty:    WrtGlobalType,
    /// The current runtime value of the global variable.
    value: WrtValue,
}

impl Global {
    /// Create a new runtime Global instance.
    /// The `initial_value` is used to set the initial runtime `value`.
    pub fn new(value_type: WrtValueType, mutable: bool, initial_value: WrtValue) -> Result<Self> {
        // Construct the WrtGlobalType for storage.
        // The initial_value in WrtGlobalType might seem redundant here if we only use
        // it for the `value` field, but it keeps the `ty` field complete as per
        // its definition.
        let global_ty_descriptor = WrtGlobalType {
            value_type,
            mutable,
        };

        // The runtime `value` starts as the provided `initial_value`.
        Ok(Self {
            ty:    global_ty_descriptor,
            value: initial_value,
        })
    }

    /// Get the current runtime value of the global.
    pub fn get(&self) -> &WrtValue {
        &self.value
    }

    /// Set the runtime value of the global.
    /// Returns an error if the global is immutable or if the value type
    /// mismatches.
    pub fn set(&mut self, new_value: &WrtValue) -> Result<()> {
        if !self.ty.mutable {
            return Err(Error::runtime_execution_error(
                "Cannot set immutable global variable",
            ));
        }

        if !new_value.matches_type(&self.ty.value_type) {
            return Err(Error::type_error(
                "Value type does not match global variable type",
            ));
        }

        self.value = new_value.clone();
        Ok(())
    }

    /// Set the initial value of the global during instantiation.
    /// Unlike `set()`, this method does not check mutability since
    /// immutable globals can still be initialized once with computed values
    /// (e.g., via `global.get` of imported globals).
    ///
    /// This should only be called during module instantiation, not at runtime.
    pub fn set_initial_value(&mut self, new_value: &WrtValue) -> Result<()> {
        if !new_value.matches_type(&self.ty.value_type) {
            return Err(Error::type_error(
                "Value type does not match global variable type",
            ));
        }

        self.value = new_value.clone();
        Ok(())
    }

    /// Get the `WrtGlobalType` descriptor (`value_type`, mutability, and
    /// original `initial_value`).
    pub fn global_type_descriptor(&self) -> &WrtGlobalType {
        &self.ty
    }
}

impl Default for Global {
    fn default() -> Self {
        use wrt_foundation::{
            types::{
                GlobalType,
                ValueType,
            },
            values::Value,
        };
        Self::new(ValueType::I32, false, Value::I32(0)).unwrap_or_else(|e| {
            // If we can't create default global, panic as this is a critical failure
            panic!("Critical: Unable to create default global: {}", e)
        })
    }
}

fn value_type_to_u8(value_type: &WrtValueType) -> u8 {
    match value_type {
        WrtValueType::I32 => 0,
        WrtValueType::I64 => 1,
        WrtValueType::F32 => 2,
        WrtValueType::F64 => 3,
        WrtValueType::V128 => 4,
        WrtValueType::FuncRef => 5,
        WrtValueType::NullFuncRef => 15,
        WrtValueType::ExternRef => 6,
        WrtValueType::I16x8 => 7,
        WrtValueType::StructRef(_) => 8,
        WrtValueType::ArrayRef(_) => 9,
        WrtValueType::ExnRef => 10,
        WrtValueType::I31Ref => 11,
        WrtValueType::AnyRef => 12,
        WrtValueType::EqRef => 13,
        WrtValueType::TypedFuncRef(_, _) => 14,
    }
}

impl wrt_foundation::traits::Checksummable for Global {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        checksum.update_slice(&value_type_to_u8(&self.ty.value_type).to_le_bytes());
        checksum.update_slice(&[u8::from(self.ty.mutable)]);
    }
}

impl wrt_foundation::traits::ToBytes for Global {
    fn serialized_size(&self) -> usize {
        16 // simplified
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> Result<()> {
        writer.write_all(&value_type_to_u8(&self.ty.value_type).to_le_bytes())?;
        writer.write_all(&[u8::from(self.ty.mutable)])
    }
}

impl wrt_foundation::traits::FromBytes for Global {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> Result<Self> {
        let mut bytes = [0u8; 1];
        reader.read_exact(&mut bytes)?;
        let value_type = match bytes[0] {
            0 => wrt_foundation::types::ValueType::I32,
            1 => wrt_foundation::types::ValueType::I64,
            2 => wrt_foundation::types::ValueType::F32,
            3 => wrt_foundation::types::ValueType::F64,
            _ => wrt_foundation::types::ValueType::I32,
        };

        reader.read_exact(&mut bytes)?;
        let mutable = bytes[0] != 0;

        use wrt_foundation::values::Value;
        let initial_value = match value_type {
            wrt_foundation::types::ValueType::I32 => Value::I32(0),
            wrt_foundation::types::ValueType::I64 => Value::I64(0),
            wrt_foundation::types::ValueType::F32 => {
                Value::F32(wrt_foundation::float_repr::FloatBits32::from_float(0.0))
            },
            wrt_foundation::types::ValueType::F64 => {
                Value::F64(wrt_foundation::float_repr::FloatBits64::from_float(0.0))
            },
            _ => Value::I32(0),
        };

        Self::new(value_type, mutable, initial_value)
    }
}

// The local `GlobalType` struct is no longer needed as we use WrtGlobalType
// from wrt_foundation directly. /// Represents a WebAssembly global type
// #[derive(Debug, Clone, PartialEq)]
// pub struct GlobalType { ... } // REMOVED
// impl GlobalType { ... } // REMOVED
