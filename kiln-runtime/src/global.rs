//! WebAssembly global value implementation
//!
//! This module provides the implementation for WebAssembly globals.

use std::format;

use kiln_foundation::{
    types::{
        GlobalType as KilnGlobalType,
        ValueType as KilnValueType,
    },
    values::Value as KilnValue,
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
    /// The `initial_value` from `KilnGlobalType` is used to set the runtime
    /// `value` field upon creation.
    ty:    KilnGlobalType,
    /// The current runtime value of the global variable.
    value: KilnValue,
}

impl Global {
    /// Create a new runtime Global instance.
    /// The `initial_value` is used to set the initial runtime `value`.
    pub fn new(value_type: KilnValueType, mutable: bool, initial_value: KilnValue) -> Result<Self> {
        // Construct the KilnGlobalType for storage.
        // The initial_value in KilnGlobalType might seem redundant here if we only use
        // it for the `value` field, but it keeps the `ty` field complete as per
        // its definition.
        let global_ty_descriptor = KilnGlobalType {
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
    pub fn get(&self) -> &KilnValue {
        &self.value
    }

    /// Set the runtime value of the global.
    /// Returns an error if the global is immutable or if the value type
    /// mismatches.
    pub fn set(&mut self, new_value: &KilnValue) -> Result<()> {
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
    pub fn set_initial_value(&mut self, new_value: &KilnValue) -> Result<()> {
        if !new_value.matches_type(&self.ty.value_type) {
            return Err(Error::type_error(
                "Value type does not match global variable type",
            ));
        }

        self.value = new_value.clone();
        Ok(())
    }

    /// Get the `KilnGlobalType` descriptor (`value_type`, mutability, and
    /// original `initial_value`).
    pub fn global_type_descriptor(&self) -> &KilnGlobalType {
        &self.ty
    }
}

impl Default for Global {
    fn default() -> Self {
        use kiln_foundation::{
            types::{
                GlobalType,
                ValueType,
            },
            values::Value,
        };
        Self::new(ValueType::I32, false, Value::I32(0))
            .expect("Critical: Unable to create default global")
    }
}

fn value_type_to_u8(value_type: &KilnValueType) -> u8 {
    match value_type {
        KilnValueType::I32 => 0,
        KilnValueType::I64 => 1,
        KilnValueType::F32 => 2,
        KilnValueType::F64 => 3,
        KilnValueType::V128 => 4,
        KilnValueType::FuncRef => 5,
        KilnValueType::NullFuncRef => 15,
        KilnValueType::ExternRef => 6,
        KilnValueType::I16x8 => 7,
        KilnValueType::StructRef(_) => 8,
        KilnValueType::ArrayRef(_) => 9,
        KilnValueType::ExnRef => 10,
        KilnValueType::I31Ref => 11,
        KilnValueType::AnyRef => 12,
        KilnValueType::EqRef => 13,
        KilnValueType::TypedFuncRef(_, _) => 14,
        KilnValueType::NonNullAbstract(_) => 19,
        KilnValueType::NoneRef => 16,
        KilnValueType::NoExternRef => 17,
        KilnValueType::NoExnRef => 18,
    }
}

impl kiln_foundation::traits::Checksummable for Global {
    fn update_checksum(&self, checksum: &mut kiln_foundation::verification::Checksum) {
        checksum.update_slice(&value_type_to_u8(&self.ty.value_type).to_le_bytes());
        checksum.update_slice(&[u8::from(self.ty.mutable)]);
    }
}

impl kiln_foundation::traits::ToBytes for Global {
    fn serialized_size(&self) -> usize {
        16 // simplified
    }

    fn to_bytes_with_provider<P: kiln_foundation::MemoryProvider>(
        &self,
        writer: &mut kiln_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> Result<()> {
        writer.write_all(&value_type_to_u8(&self.ty.value_type).to_le_bytes())?;
        writer.write_all(&[u8::from(self.ty.mutable)])
    }
}

impl kiln_foundation::traits::FromBytes for Global {
    fn from_bytes_with_provider<P: kiln_foundation::MemoryProvider>(
        reader: &mut kiln_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> Result<Self> {
        let mut bytes = [0u8; 1];
        reader.read_exact(&mut bytes)?;
        let value_type = match bytes[0] {
            0 => kiln_foundation::types::ValueType::I32,
            1 => kiln_foundation::types::ValueType::I64,
            2 => kiln_foundation::types::ValueType::F32,
            3 => kiln_foundation::types::ValueType::F64,
            _ => kiln_foundation::types::ValueType::I32,
        };

        reader.read_exact(&mut bytes)?;
        let mutable = bytes[0] != 0;

        use kiln_foundation::values::Value;
        let initial_value = match value_type {
            kiln_foundation::types::ValueType::I32 => Value::I32(0),
            kiln_foundation::types::ValueType::I64 => Value::I64(0),
            kiln_foundation::types::ValueType::F32 => {
                Value::F32(kiln_foundation::float_repr::FloatBits32::from_float(0.0))
            },
            kiln_foundation::types::ValueType::F64 => {
                Value::F64(kiln_foundation::float_repr::FloatBits64::from_float(0.0))
            },
            _ => Value::I32(0),
        };

        Self::new(value_type, mutable, initial_value)
    }
}

// The local `GlobalType` struct is no longer needed as we use KilnGlobalType
// from kiln_foundation directly. /// Represents a WebAssembly global type
// #[derive(Debug, Clone, PartialEq)]
// pub struct GlobalType { ... } // REMOVED
// impl GlobalType { ... } // REMOVED
