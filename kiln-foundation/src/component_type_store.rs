// WRT - wrt-foundation
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Provides a store for component model types that may be recursive or large,
//! such as `ComponentType`, `InstanceType`, etc. This helps in managing their
//! storage in a `no_alloc` environment.

use wrt_error::{
    codes,
    Error,
    Result,
};

#[cfg(feature = "std")]
use crate::prelude::format;
use crate::{
    bounded::BoundedVec,
    component::{
        ComponentType,
        CoreModuleType,
        InstanceType,
    }, // Add other types as needed
    traits::BoundedCapacity, // Added import
    traits::{
        FromBytes,
        ReadStream,
        ToBytes,
        WriteStream,
    }, // Added imports
    MemoryProvider,
};

/// Maximum number of component types storable in the type store.
const MAX_STORED_COMPONENT_TYPES: usize = 64; // Adjust as needed
/// Maximum number of instance types storable in the type store.
const MAX_STORED_INSTANCE_TYPES: usize = 64; // Adjust as needed
/// Maximum number of core module types storable in the type store.
const MAX_STORED_CORE_MODULE_TYPES: usize = 64; // Adjust as needed

/// Represents a reference to a type stored in the `ComponentTypeStore`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TypeRef(pub u32);

impl TypeRef {
    /// A reference that indicates no type or an invalid/uninitialized type.
    pub const fn none() -> Self {
        TypeRef(u32::MAX) // Or some other sentinel value
    }

    /// Checks if the reference is valid (not `None`).
    pub const fn is_some(&self) -> bool {
        self.0 != u32::MAX
    }
}

impl crate::traits::Checksummable for TypeRef {
    fn update_checksum(&self, checksum: &mut crate::verification::Checksum) {
        self.0.update_checksum(checksum);
    }
}

impl ToBytes for TypeRef {
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        self.0.to_bytes_with_provider(writer, _provider) // u32's to_bytes_with_provider
    }
}

impl FromBytes for TypeRef {
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let val = u32::from_bytes_with_provider(reader, _provider)?; // u32's from_bytes_with_provider
        Ok(TypeRef(val))
    }
}

/// A store for component model type definitions.
///
/// This store holds various types like `ComponentType`, `InstanceType`, etc.,
/// allowing them to be referenced by `TypeRef` to break direct recursion
/// Binary std/no_std choice
#[derive(Debug)]
pub struct ComponentTypeStore<P: MemoryProvider + Clone + Default + Eq> {
    provider:          P,
    component_types:   BoundedVec<ComponentType<P>, MAX_STORED_COMPONENT_TYPES, P>,
    instance_types:    BoundedVec<InstanceType<P>, MAX_STORED_INSTANCE_TYPES, P>,
    core_module_types: BoundedVec<CoreModuleType<P>, MAX_STORED_CORE_MODULE_TYPES, P>,
    // Add other BoundedVecs for other types like CoreType<P> if they also need to be stored
}

impl<P: MemoryProvider + Clone + Default + Eq> ComponentTypeStore<P> {
    /// Creates a new, empty `ComponentTypeStore`.
    pub fn new(provider: P) -> wrt_error::Result<Self>
    where
        P: Clone, // This is now redundant due to the struct bound, but harmless
    {
        Ok(Self {
            component_types: BoundedVec::new(provider.clone()).map_err(|e| {
                Error::runtime_execution_error("Failed to create BoundedVec for component types")
            })?,
            instance_types: BoundedVec::new(provider.clone()).map_err(|e| {
                Error::new(
                    wrt_error::ErrorCategory::Memory,
                    codes::MEMORY_ALLOCATION_ERROR,
                    "Failed to allocate memory for instance types",
                )
            })?,
            core_module_types: BoundedVec::new(provider.clone()).map_err(|e| {
                Error::runtime_execution_error("Failed to create BoundedVec for core module types")
            })?,
            provider,
        })
    }

    /// Adds a `ComponentType` to the store and returns a `TypeRef` to it.
    pub fn add_component_type(&mut self, ctype: ComponentType<P>) -> wrt_error::Result<TypeRef> {
        let index = self.component_types.len() as u32;
        self.component_types.push(ctype).map_err(|e| {
            Error::new(
                wrt_error::ErrorCategory::Resource,
                codes::RESOURCE_LIMIT_EXCEEDED,
                "Component type store capacity exceeded",
            )
        })?;
        Ok(TypeRef(index))
    }

    /// Resolves a `TypeRef` to a reference to a `ComponentType`.
    pub fn resolve_component_type(&self, type_ref: TypeRef) -> Option<ComponentType<P>> {
        if type_ref.is_some() {
            self.component_types.get(type_ref.0 as usize).ok()
        } else {
            None
        }
    }

    /// Adds an `InstanceType` to the store and returns a `TypeRef` to it.
    pub fn add_instance_type(&mut self, itype: InstanceType<P>) -> wrt_error::Result<TypeRef> {
        let index = self.instance_types.len() as u32;
        self.instance_types
            .push(itype)
            .map_err(|_e| Error::runtime_execution_error("Failed to add instance type to store"))?;
        Ok(TypeRef(index))
    }

    /// Resolves a `TypeRef` to a reference to an `InstanceType`.
    pub fn resolve_instance_type(&self, type_ref: TypeRef) -> Option<InstanceType<P>> {
        if type_ref.is_some() {
            self.instance_types.get(type_ref.0 as usize).ok()
        } else {
            None
        }
    }

    /// Adds a `CoreModuleType` to the store and returns a `TypeRef` to it.
    pub fn add_core_module_type(
        &mut self,
        cmtype: CoreModuleType<P>,
    ) -> wrt_error::Result<TypeRef> {
        let index = self.core_module_types.len() as u32;
        self.core_module_types.push(cmtype).map_err(|_e| {
            Error::new(
                wrt_error::ErrorCategory::Resource,
                codes::RESOURCE_LIMIT_EXCEEDED,
                "Core module type store capacity exceeded",
            )
        })?;
        Ok(TypeRef(index))
    }

    /// Resolves a `TypeRef` to a reference to a `CoreModuleType`.
    pub fn resolve_core_module_type(&self, type_ref: TypeRef) -> Option<CoreModuleType<P>> {
        if type_ref.is_some() {
            self.core_module_types.get(type_ref.0 as usize).ok()
        } else {
            None
        }
    }

    // Add similar add/resolve methods for other types as needed (e.g., CoreType)
}

impl<P: MemoryProvider + Clone + Default + Eq> ToBytes for ComponentTypeStore<P>
where
    ComponentType<P>: ToBytes, // Ensure contained types are serializable
    InstanceType<P>: ToBytes,
    CoreModuleType<P>: ToBytes,
{
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        stream_provider: &PStream,
    ) -> wrt_error::Result<()> {
        self.component_types.to_bytes_with_provider(writer, stream_provider)?;
        self.instance_types.to_bytes_with_provider(writer, stream_provider)?;
        self.core_module_types.to_bytes_with_provider(writer, stream_provider)?;
        Ok(())
    }
}

impl<P: MemoryProvider + Clone + Default + Eq> FromBytes for ComponentTypeStore<P>
where
    ComponentType<P>: FromBytes, // Ensure contained types are deserializable
    InstanceType<P>: FromBytes,
    CoreModuleType<P>: FromBytes,
{
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        stream_provider: &PStream,
    ) -> wrt_error::Result<Self> {
        // Create a new store. The provider for the store itself (P) must come from
        // the context where ComponentTypeStore is being deserialized.
        // For FromBytes, we often rely on P::default() if no other provider is
        // available. Here, the `new` method of ComponentTypeStore takes a P.
        // We must assume P::default() is a valid provider for constructing the store.
        let mut store = ComponentTypeStore::new(P::default())?;

        store.component_types = BoundedVec::from_bytes_with_provider(reader, stream_provider)?;
        store.instance_types = BoundedVec::from_bytes_with_provider(reader, stream_provider)?;
        store.core_module_types = BoundedVec::from_bytes_with_provider(reader, stream_provider)?;

        Ok(store)
    }
}
