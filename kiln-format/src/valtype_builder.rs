//! Helper module for building ValType instances with proper BoundedVec
//! initialization
//!
//! This module provides utilities to convert from parsed Vec-based structures
//! to the proper BoundedVec-based ValType structures.

#[cfg(not(feature = "std"))]
use std::{
    string::String,
    vec::Vec,
};
#[cfg(feature = "std")]
use std::{
    string::String,
    vec::Vec,
};

use wrt_error::{
    codes,
    Error,
    ErrorCategory,
};
use wrt_foundation::{
    bounded::{
        BoundedVec,
        WasmName,
    },
    component_value::{
        ValType,
        ValTypeRef,
    },
    MemoryProvider,
};

/// Type store for managing ValType instances and references
pub struct TypeStore<P: MemoryProvider + Default + Clone + PartialEq + Eq> {
    types: Vec<ValType<P>>,
}

impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> TypeStore<P> {
    pub fn new() -> Self {
        Self { types: Vec::new() }
    }

    /// Store a ValType and return its reference
    pub fn store_type(&mut self, val_type: ValType<P>) -> ValTypeRef {
        let index = self.types.len() as u32;
        self.types.push(val_type);
        ValTypeRef(index)
    }

    /// Get a ValType by reference
    pub fn get_type(&self, type_ref: ValTypeRef) -> Option<&ValType<P>> {
        self.types.get(type_ref.0 as usize)
    }
}

/// Helper to build ValType::Record from Vec
pub fn build_record<P: MemoryProvider + Default + Clone + PartialEq + Eq>(
    fields: Vec<(String, ValType<P>)>,
    provider: P,
    type_store: &mut TypeStore<P>,
) -> Result<ValType<P>, Error> {
    let mut bounded_fields = BoundedVec::new(provider.clone())?;

    for (name, val_type) in fields {
        // Convert String to WasmName
        let wasm_name = WasmName::try_from_str(&name)?;

        // Store the ValType and get a reference
        let type_ref = type_store.store_type(val_type);

        // Push to bounded vec
        bounded_fields
            .push((wasm_name, type_ref))
            .map_err(|_| Error::memory_error("Failed to push to bounded fields"))?;
    }

    Ok(ValType::Record(bounded_fields))
}

/// Helper to build ValType::Variant from Vec
pub fn build_variant<P: MemoryProvider + Default + Clone + PartialEq + Eq>(
    cases: Vec<(String, Option<ValType<P>>)>,
    provider: P,
    type_store: &mut TypeStore<P>,
) -> Result<ValType<P>, Error> {
    let mut bounded_cases = BoundedVec::new(provider.clone())?;

    for (name, maybe_val_type) in cases {
        let wasm_name = WasmName::try_from_str(&name)?;
        let maybe_type_ref = maybe_val_type.map(|vt| type_store.store_type(vt));

        bounded_cases
            .push((wasm_name, maybe_type_ref))
            .map_err(|_| Error::memory_error("Failed to push to bounded cases"))?;
    }

    Ok(ValType::Variant(bounded_cases))
}

/// Helper to build ValType::Tuple from Vec
pub fn build_tuple<P: MemoryProvider + Default + Clone + PartialEq + Eq>(
    types: Vec<ValType<P>>,
    provider: P,
    type_store: &mut TypeStore<P>,
) -> Result<ValType<P>, Error> {
    let mut bounded_types = BoundedVec::new(provider)?;

    for val_type in types {
        let type_ref = type_store.store_type(val_type);
        bounded_types
            .push(type_ref)
            .map_err(|_| Error::memory_error("Failed to push to bounded tuple"))?;
    }

    Ok(ValType::Tuple(bounded_types))
}

/// Helper to build ValType::Flags from Vec
pub fn build_flags<P: MemoryProvider + Default + Clone + PartialEq + Eq>(
    names: Vec<String>,
    provider: P,
) -> Result<ValType<P>, Error> {
    let mut bounded_names = BoundedVec::new(provider.clone())?;

    for name in names {
        let wasm_name = WasmName::try_from_str(&name)?;
        bounded_names
            .push(wasm_name)
            .map_err(|_| Error::memory_error("Failed to push to bounded flags"))?;
    }

    Ok(ValType::Flags(bounded_names))
}

/// Helper to build ValType::Enum from Vec
pub fn build_enum<P: MemoryProvider + Default + Clone + PartialEq + Eq>(
    names: Vec<String>,
    provider: P,
) -> Result<ValType<P>, Error> {
    let mut bounded_names = BoundedVec::new(provider.clone())?;

    for name in names {
        let wasm_name = WasmName::try_from_str(&name)?;
        bounded_names
            .push(wasm_name)
            .map_err(|_| Error::memory_error("Failed to push to bounded enum"))?;
    }

    Ok(ValType::Enum(bounded_names))
}
