// WRT - wrt-foundation
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Provides a store for component model values that require allocation,
//! such as strings, lists, and records. This helps in managing the lifetime
//! and storage of these complex types, especially when interfacing with
//! WebAssembly components.

#![allow(dead_code, unused_variables)] // Allow unused for stub

#[cfg(feature = "std")]
extern crate alloc;
#[cfg(feature = "std")]
use std::format;

// External crate imports
use wrt_error::{
    ErrorCategory,
    Result,
};

#[cfg(feature = "std")]
use crate::prelude::BTreeMap;
// Internal imports organized by module
use crate::{
    // Bounded collections
    bounded::{
        BoundedError,
        BoundedString,
        BoundedVec,
        WasmName,
        MAX_COMPONENT_ERROR_CONTEXT_ITEMS,
        MAX_COMPONENT_FLAGS,
        MAX_COMPONENT_TUPLE_ITEMS,
        MAX_WASM_NAME_LENGTH,
        MAX_WASM_STRING_LENGTH,
    },

    // Re-exported items
    codes,
    // Component value types
    component_value::{
        ComponentValue,
        ValType,
        ValTypeRef,
        MAX_STORED_COMPONENT_VALUES,
    },

    // Other types
    prelude::Debug,
    // Traits
    traits::{
        BytesWriter,
        Checksummable,
        FromBytes,
        ReadStream,
        SerializationError,
        ToBytes,
        WriteStream,
    },

    types::{
        Limits,
        ValueType,
    },
    values::Value,

    verification::Checksum,
    Error,
    MemoryProvider,
    SafeMemoryHandler,
};
use crate::{
    prelude::*,
    traits::BoundedCapacity,
}; // Added import

/// An opaque reference (index) to a `ComponentValue` within the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ValueRef(pub usize);

impl ValueRef {
    /// Creates a new `ValueRef`.
    pub fn new(idx: usize) -> Self {
        ValueRef(idx)
    }

    /// Returns the underlying index.
    pub fn index(&self) -> usize {
        self.0
    }
}

impl ToBytes for ValueRef {
    fn to_bytes_with_provider<PStream: MemoryProvider>(
        &self,
        writer: &mut WriteStream,
        provider: &PStream,
    ) -> Result<()> {
        self.0.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for ValueRef {
    fn from_bytes_with_provider<PStream: MemoryProvider>(
        reader: &mut ReadStream,
        provider: &PStream,
    ) -> Result<Self> {
        let val = usize::from_bytes_with_provider(reader, provider)?;
        Ok(ValueRef(val))
    }
}

impl Checksummable for ValueRef {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.0.update_checksum(checksum);
    }
}

/// Maximum number of values in a store
pub const MAX_STORE_VALUES: usize = 1024; // Example capacity
/// Maximum number of types in a store
pub const MAX_STORE_TYPES: usize = 256; // Example capacity for types

// Capacity for the type_to_ref_map, should be related to MAX_STORE_TYPES
/// Maximum number of entries in the type-to-reference map
#[cfg(feature = "std")]
pub const MAX_TYPE_TO_REF_MAP_ENTRIES: usize = MAX_STORE_TYPES;
/// Binary std/no_std choice
#[cfg(not(feature = "std"))]
pub const MAX_TYPE_TO_REF_MAP_ENTRIES: usize = MAX_STORE_TYPES; // Binary std/no_std choice

/// Stores component values and their types, managing references between them.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentValueStore<P: MemoryProvider + Default + Clone + PartialEq + Eq> {
    provider:        P,
    values:          BoundedVec<ComponentValue<P>, MAX_STORE_VALUES, P>,
    // ValType is also P-generic, if we store ValTypes here, P needs to be consistent.
    types:           BoundedVec<ValType<P>, MAX_STORE_TYPES, P>,
    // TODO: Implement BoundedHashMap - using a vec pair for now
    type_to_ref_map: BoundedVec<(ValType<P>, ValTypeRef), MAX_TYPE_TO_REF_MAP_ENTRIES, P>,
}

impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> ComponentValueStore<P> {
    /// Creates a new, empty `ComponentValueStore` with the given memory
    /// provider.
    pub fn new(provider: P) -> Result<Self> {
        let values = BoundedVec::new(provider.clone()).map_err(|_e| {
            Error::runtime_execution_error("Failed to create BoundedVec for component values")
        })?;
        let types = BoundedVec::new(provider.clone()).map_err(|_e| {
            Error::new(
                wrt_error::ErrorCategory::Memory,
                codes::MEMORY_ALLOCATION_ERROR,
                "Failed to allocate memory for value types",
            )
        })?;
        let type_map = BoundedVec::new(provider.clone()).map_err(|_e| {
            Error::runtime_execution_error("Failed to create BoundedVec for type-to-ref map")
        })?;
        Ok(Self {
            provider,
            values,
            types,
            type_to_ref_map: type_map,
        })
    }

    /// Returns a reference to the memory provider used by this store.
    pub fn get_provider(&self) -> &P {
        &self.provider
    }

    /// Adds a component value to the store and returns a reference to it.
    ///
    /// # Errors
    /// Binary std/no_std choice
    pub fn add_value(&mut self, value: ComponentValue<P>) -> Result<ValueRef> {
        let index = self.values.len() as u32;
        self.values.push(value).map_err(|_e| {
            Error::new(
                wrt_error::ErrorCategory::Resource,
                codes::RESOURCE_LIMIT_EXCEEDED,
                "Component value store capacity exceeded",
            )
        })?;
        Ok(ValueRef(index as usize))
    }

    /// Resolves a `ValueRef` to a reference to the `ComponentValue` in the
    /// store.
    ///
    /// Returns `None` if the `ValueRef` is invalid (e.g., out of bounds).
    pub fn resolve_value(&self, value_ref: ValueRef) -> Option<ComponentValue<P>> {
        self.values.get(value_ref.index()).ok()
    }

    /// Retrieves a reference to a core `Value` by its handle.
    /// Note: This is a placeholder and currently always returns `None`.
    #[must_use]
    pub fn get_ref(&self, handle: u32) -> Option<&Value> {
        // This is a placeholder. Actual implementation would depend on how
        // various reference types are stored and what `handle` refers to.
        // For now, assume handle might be an index into a generic list of refs,
        // or it might be specific to the type of ref (string, list, etc.)
        // Returning None to indicate not found or not implemented properly yet.
        None
    }

    /// Retrieves a string slice from the store by its handle.
    ///
    /// # Errors
    /// Returns an error if the handle is invalid or the value is not a string.
    pub fn get_string<'a>(&'a self, val_ref: ValueRef) -> wrt_error::Result<&'a str> {
        match self.values.get(val_ref.index()).ok() {
            Some(ComponentValue::String(_s_name)) => {
                // Temporarily disabled due to lifetime issues in no_std mode
                Err(Error::parse_error(
                    "get_string temporarily disabled in no_std mode due to lifetime issues",
                ))
            },
            Some(_other_val) => Err(Error::type_error(
                // format!("Expected ComponentValue::String, found {:?}", other_val) // format!
                // Binary std/no_std choice
                "Type mismatch: Expected ComponentValue::String",
            )),
            None => Err(Error::runtime_execution_error(
                "Invalid value reference: index out of bounds",
            )),
        }
    }

    // Methods expected by to_core_value conversions
    /// Adds a string to the store and returns its handle.
    /// Binary std/no_std choice
    ///
    /// # Errors
    /// Binary std/no_std choice
    pub fn add_string(&mut self, s: &str) -> Result<u32>
    where
        P: Clone, // Needed for WasmName::from_str which takes P by value
    {
        #[cfg(feature = "std")]
        let comp_val = ComponentValue::String(s.to_string());

        #[cfg(not(any(feature = "std")))]
        let comp_val = {
            let bounded_s =
                BoundedString::<{ crate::bounded::MAX_WASM_STRING_LENGTH }, P>::from_str(
                    s,
                    self.provider.clone(),
                )
                .map_err(Error::from)?;
            ComponentValue::String(bounded_s)
        };
        let value_ref = self.add_value(comp_val)?;
        Ok(value_ref.0 as u32)
    }

    /// Adds a list of component values to the store and returns its handle.
    /// Note: Currently a stub, returns a dummy handle.
    pub fn add_list(&mut self, values: &[ComponentValue<P>]) -> Result<u32> {
        // Changed Result to Result
        // For now, just returning a dummy handle
        // In reality, this would convert ComponentValues to core Values and store them
        Ok(0)
    }

    /// Adds a record to the store and returns its handle.
    /// Note: Currently a stub, returns a dummy handle.
    pub fn add_record<S>(&mut self, fields: &[(S, ComponentValue<P>)]) -> Result<u32>
    where
        S: AsRef<str> + Debug,
        P: Clone, // Needed if WasmName is created from S for field names
    {
        // TODO: Actual implementation would iterate fields,
        // convert S to WasmName, and store as ComponentValue::Record
        // For now, maintaining stub behavior.
        Ok(1) // Dummy handle
    }

    /// Adds a variant to the store and returns its handle.
    /// Note: Currently a stub, returns a dummy handle.
    pub fn add_variant<S>(
        &mut self,
        case_name_str: S,
        value: Option<&ComponentValue<P>>,
    ) -> Result<u32>
    where
        S: AsRef<str> + Debug,
        P: Clone, // Needed for WasmName::from_str
    {
        // TODO: Convert case_name_str to WasmName
        // Implementation pending - requires ComponentValue to be Clone
        Ok(2) // Dummy handle
    }

    /// Adds a tuple of core values to the store and returns its handle.
    pub fn add_tuple<I>(&mut self, values: I) -> Result<u32>
    where
        I: IntoIterator<Item = Value>,
        P: Clone,
    {
        let mut item_value_refs =
            BoundedVec::<ValueRef, MAX_COMPONENT_TUPLE_ITEMS, P>::new(self.provider.clone())
                .map_err(|e| {
                    Error::runtime_execution_error(
                        "Failed to create BoundedVec for tuple refs (capacity issue)",
                    )
                })?;

        for core_val in values {
            // 1. Convert core::Value to ComponentValue<P>
            let component_val = self.core_value_to_component_value(core_val)?;

            // 2. Add the ComponentValue to the store to get a ValueRef
            let value_ref = self.add_value(component_val)?;

            // 3. Push the ValueRef to item_value_refs
            item_value_refs.push(value_ref).map_err(|_e: BoundedError| {
                // e is BoundedError
                Error::runtime_execution_error(
                    "Failed to push ValueRef into BoundedVec for tuple (capacity issue)",
                )
            })?;
        }

        // Create ComponentValue::Tuple with BoundedVec<ValueRef, ...>
        let tuple_comp_val = ComponentValue::Tuple(item_value_refs);
        let final_tuple_ref = self.add_value(tuple_comp_val)?;
        Ok(final_tuple_ref.0 as u32)
    }

    /// Adds flags to the store and returns its handle.
    pub fn add_flags<I, S>(&mut self, flags: I) -> Result<u32>
    where
        I: IntoIterator<Item = (S, bool)>,
        S: AsRef<str> + Debug,
        P: Clone,
    {
        let mut flag_values = BoundedVec::<
            (WasmName<MAX_WASM_NAME_LENGTH>, bool),
            MAX_COMPONENT_FLAGS,
            P,
        >::new(self.provider.clone())
        .map_err(|_e| Error::runtime_execution_error("Failed to create BoundedVec for flags"))?;

        for (name_str, val) in flags {
            let name =
                WasmName::try_from_str(name_str.as_ref()).map_err(|_e| {
                    Error::runtime_execution_error("Failed to create WasmName from string")
                })?;
            flag_values.push((name, val)).map_err(|_e| {
                Error::new(
                    wrt_error::ErrorCategory::Resource,
                    codes::RESOURCE_LIMIT_EXCEEDED,
                    "Flag values capacity exceeded",
                )
            })?;
        }
        let flags_cv = ComponentValue::<P>::Flags(flag_values);
        let new_ref = self.add_value(flags_cv)?;
        Ok(new_ref.0 as u32)
    }

    /// Adds an `enum` case to the store and returns its handle.
    /// Binary std/no_std choice
    pub fn add_enum<S: AsRef<str> + Debug>(&mut self, case: S) -> Result<u32>
    where
        P: Clone,
    {
        let name = WasmName::try_from_str(case.as_ref())
            .map_err(|_e| Error::runtime_execution_error("Failed to create enum case name"))?;
        let comp_val = ComponentValue::<P>::Enum(name);
        let new_ref = self.add_value(comp_val)?;
        Ok(new_ref.0 as u32)
    }

    /// Adds an option value to the store and returns its handle.
    /// Note: Currently a stub, returns a dummy handle.
    pub fn add_option(&mut self, opt_val: Option<Value>) -> Result<u32> {
        // Changed Result to Result
        Ok(6) // Dummy handle
    }

    /// Adds a result value to the store and returns its handle.
    /// Note: Currently a stub, returns a dummy handle.
    pub fn add_result(&mut self, ok_val: Option<Value>, err_val: Option<Value>) -> Result<u32> {
        Ok(7) // Dummy handle
    }

    /// Interns a `ValType` into the store, returning a `ValTypeRef` that
    /// identifies it. If the type already exists, returns the existing
    /// `ValTypeRef`.
    ///
    /// # Errors
    /// Binary std/no_std choice
    pub fn intern_type(&mut self, ty: ValType<P>) -> Result<ValTypeRef> {
        #[cfg(feature = "std")]
        {
            // Search through the type_to_ref_map to find existing type
            for i in 0..self.type_to_ref_map.len() {
                if let Ok((stored_type, type_ref)) = self.type_to_ref_map.get(i) {
                    if stored_type == ty {
                        return Ok(type_ref);
                    }
                }
            }
        }

        // If not found in map (or map not used), add to BoundedVec and then to map
        let type_idx = self.types.len() as u32;
        self.types.push(ty.clone()).map_err(|_e| {
            // ty needs to be Clone for this path
            Error::runtime_execution_error("Failed to add type to store: capacity exceeded")
        })?;
        let type_ref = ValTypeRef(type_idx);

        #[cfg(feature = "std")]
        {
            // Add the type-to-ref mapping to our "map" (which is actually a BoundedVec of
            // tuples)
            self.type_to_ref_map.push((ty, type_ref)).map_err(|_e: BoundedError| {
                Error::runtime_execution_error("Failed to add type mapping: capacity exceeded")
            })?;
        }
        Ok(type_ref)
    }

    /// Resolves a `ValTypeRef` to a reference to the `ValType` in the store.
    ///
    /// Returns `None` if the `ValTypeRef` is invalid (e.g., out of bounds).
    pub fn resolve_type(&self, type_ref: ValTypeRef) -> Option<ValType<P>> {
        self.types.get(type_ref.0 as usize).ok() // Assuming ValTypeRef.0 is the
                                                 // index
    }

    /// Returns a clone of the provider used by this store.
    /// Useful if external code needs to create P-dependent types like WasmName
    /// or `BoundedVec` that are compatible with this store's memory
    /// management.
    pub fn provider_clone(&self) -> P {
        self.provider.clone() // P: Clone is required by the struct bound
    }

    // Placeholder for complex conversion, real implementation needed.
    fn core_value_to_component_value(&mut self, core_value: Value) -> Result<ComponentValue<P>>
    where
        P: Clone, // Assuming provider clone might be needed
    {
        match core_value {
            Value::I32(i) => Ok(ComponentValue::S32(i)), // Changed from I32
            Value::I64(i) => Ok(ComponentValue::S64(i)), // Changed from I64
            Value::F32(f) => Ok(ComponentValue::F32(f)),
            Value::F64(f) => Ok(ComponentValue::F64(f)),
            _ => Err(Error::new(
                ErrorCategory::Type,
                codes::UNSUPPORTED_OPERATION,
                "Unsupported core value type for conversion to component value",
            )),
        }
    }
}
