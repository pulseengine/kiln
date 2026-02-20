// WRT - wrt-foundation
// Binary std/no_std choice
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Provides builder patterns for constructing memory-safe types
//! optimized for no_std/no_alloc environments.
//!
//! These builders ensure proper initialization, validation, and
//! resource management for bounded types and resource-related components.

#![cfg_attr(not(feature = "std"), allow(unused_imports))]

// Standard imports
#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(not(feature = "std"))]
use core::fmt;
use core::marker::PhantomData;
#[cfg(feature = "std")]
use std::fmt;
#[cfg(feature = "std")]
use std::vec::Vec;

// Result type alias
type Result<T> = wrt_error::Result<T>;

// Import error codes
use crate::codes;
#[cfg(feature = "std")]
use crate::prelude::{
    String,
    ToString,
};
// Crate-level imports
use crate::{
    bounded::{
        BoundedStack,
        BoundedString,
        BoundedVec,
        WasmName,
    },
    resource::{
        Resource,
        ResourceItem,
        ResourceRepr,
        ResourceType,
        MAX_RESOURCE_FIELD_NAME_LEN,
    },
    safe_memory::{
        DefaultNoStdProvider,
        NoStdProvider,
        SafeMemoryHandler,
        DEFAULT_MEMORY_PROVIDER_CAPACITY,
    },
    traits::{
        Checksummable,
        FromBytes,
        ToBytes,
    },
    verification::VerificationLevel,
    Error,
    ErrorCategory,
    MemoryProvider,
};

/// Generic builder for bounded collections.
///
/// This builder simplifies the creation and configuration of bounded
/// collections like `BoundedVec`, `BoundedStack`, etc. with proper resource
/// Binary std/no_std choice
pub struct BoundedBuilder<T, const N: usize, P: MemoryProvider + Default + Clone> {
    provider:           P,
    verification_level: VerificationLevel,
    initial_capacity:   Option<usize>,
    _phantom:           PhantomData<T>,
}

impl<T, const N: usize, P: MemoryProvider + Default + Clone> Default for BoundedBuilder<T, N, P>
where
    T: Sized + Checksummable + ToBytes + FromBytes + Default,
{
    fn default() -> Self {
        Self {
            provider:           P::default(),
            verification_level: VerificationLevel::default(),
            initial_capacity:   None,
            _phantom:           PhantomData,
        }
    }
}

impl<T, const N: usize, P: MemoryProvider + Default + Clone> BoundedBuilder<T, N, P>
where
    T: Sized + Checksummable + ToBytes + FromBytes + Default,
{
    /// Creates a new builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the memory provider to use.
    pub fn with_provider(mut self, provider: P) -> Self {
        self.provider = provider;
        self
    }

    /// Sets the verification level for the bounded collection.
    pub fn with_verification_level(mut self, level: VerificationLevel) -> Self {
        self.verification_level = level;
        self
    }

    /// Sets the initial reserved capacity (not exceeding N).
    pub fn with_initial_capacity(mut self, capacity: usize) -> Self {
        self.initial_capacity = Some(capacity.min(N));
        self
    }

    /// Builds a `BoundedVec` with the configured settings.
    pub fn build_vec(self) -> wrt_error::Result<BoundedVec<T, N, P>>
    where
        T: Clone + PartialEq + Eq,
        P: PartialEq + Eq,
    {
        let vec = BoundedVec::with_verification_level(self.provider, self.verification_level)?;
        // Initial capacity is handled internally by the BoundedVec
        Ok(vec)
    }

    /// Builds a BoundedStack with the configured settings.
    pub fn build_stack(self) -> wrt_error::Result<BoundedStack<T, N, P>> {
        BoundedStack::with_verification_level(self.provider, self.verification_level)
    }
}

/// Builder for `BoundedString` and `WasmName` types.
pub struct StringBuilder<const N: usize, P: MemoryProvider + Default + Clone> {
    provider:           P,
    initial_content:    Option<&'static str>,
    truncate_if_needed: bool,
}

impl<const N: usize, P: MemoryProvider + Default + Clone> Default for StringBuilder<N, P> {
    fn default() -> Self {
        Self {
            provider:           P::default(),
            initial_content:    None,
            truncate_if_needed: false,
        }
    }
}

impl<const N: usize, P: MemoryProvider + Default + Clone + PartialEq + Eq> StringBuilder<N, P> {
    /// Creates a new string builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the memory provider to use.
    pub fn with_provider(mut self, provider: P) -> Self {
        self.provider = provider;
        self
    }

    /// Sets the initial content for the string.
    pub fn with_content(mut self, content: &'static str) -> Self {
        self.initial_content = Some(content);
        self
    }

    /// Sets whether to truncate the string if it exceeds the maximum capacity.
    pub fn with_truncation(mut self, truncate: bool) -> Self {
        self.truncate_if_needed = truncate;
        self
    }

    /// Builds a `BoundedString` with the configured settings.
    pub fn build_string(self) -> wrt_error::Result<BoundedString<N>> {
        match (self.initial_content, self.truncate_if_needed) {
            (Some(content), true) => {
                BoundedString::from_str_truncate(content).map_err(Error::from)
            },
            (Some(content), false) => {
                BoundedString::try_from_str(content).map_err(Error::from)
            },
            (None, _) => BoundedString::from_str_truncate("").map_err(Error::from),
        }
    }

    /// Builds a WasmName with the configured settings.
    pub fn build_wasm_name(self) -> wrt_error::Result<WasmName<N>> {
        match (self.initial_content, self.truncate_if_needed) {
            (Some(content), true) => {
                WasmName::from_str_truncate(content).map_err(Error::from)
            },
            (Some(content), false) => {
                WasmName::try_from_str(content).map_err(Error::from)
            },
            (None, _) => WasmName::new().map_err(Error::from),
        }
    }
}

/// Builder for Resource instances.
///
/// This builder provides a fluent API for constructing Resource objects,
/// which represent WebAssembly component model resources.
pub struct ResourceBuilder<P: MemoryProvider + Default + Clone + Eq> {
    id:                 Option<u32>,
    repr:               Option<ResourceRepr<P>>,
    name:               Option<&'static str>,
    verification_level: VerificationLevel,
}

impl<P: MemoryProvider + Default + Clone + Eq + fmt::Debug> Default for ResourceBuilder<P> {
    fn default() -> Self {
        Self {
            id:                 None,
            repr:               None,
            name:               None,
            verification_level: VerificationLevel::default(),
        }
    }
}

impl<P: MemoryProvider + Default + Clone + Eq + fmt::Debug> ResourceBuilder<P> {
    /// Creates a new resource builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the resource ID.
    pub fn with_id(mut self, id: u32) -> Self {
        self.id = Some(id);
        self
    }

    /// Sets the resource representation.
    pub fn with_repr(mut self, repr: ResourceRepr<P>) -> Self {
        self.repr = Some(repr);
        self
    }

    /// Sets the resource name.
    pub fn with_name(mut self, name: &'static str) -> Self {
        self.name = Some(name);
        self
    }

    /// Sets the verification level for the resource.
    pub fn with_verification_level(mut self, level: VerificationLevel) -> Self {
        self.verification_level = level;
        self
    }

    /// Builds a Resource with the configured settings.
    pub fn build(self) -> wrt_error::Result<Resource<P>> {
        let id = self.id.ok_or_else(|| {
            Error::new_static(
                ErrorCategory::Validation,
                codes::VALIDATION_ERROR,
                "Resource ID is required",
            )
        })?;

        let repr = self.repr.ok_or_else(|| {
            Error::new_static(
                ErrorCategory::Validation,
                codes::VALIDATION_ERROR,
                "Resource representation is required",
            )
        })?;

        let name = match self.name {
            Some(name_str) => {
                let wasm_name =
                    WasmName::from_str_truncate(name_str).map_err(Error::from)?;
                Some(wasm_name)
            },
            None => None,
        };

        Ok(Resource::new(id, repr, name, self.verification_level))
    }
}

/// Builder for ResourceType instances.
///
/// This builder provides a fluent API for constructing ResourceType objects
/// used in the WebAssembly component model.
pub struct ResourceTypeBuilder<P: MemoryProvider + Default + Clone + Eq + fmt::Debug> {
    variant:  Option<ResourceTypeVariant>,
    provider: P,
}

#[cfg(feature = "std")]
/// Enum to represent the possible variants of ResourceType
enum ResourceTypeVariant {
    /// Record type with field names
    Record(Vec<String>),
    /// Aggregate type with resource IDs
    Aggregate(Vec<u32>),
}

#[cfg(not(feature = "std"))]
/// Enum to represent the possible variants of ResourceType
enum ResourceTypeVariant {
    /// Record type with field names
    Record(BoundedString<MAX_RESOURCE_FIELD_NAME_LEN>),
    /// Aggregate type with resource IDs
    Aggregate(u32),
}

impl<P: MemoryProvider + Default + Clone + Eq + fmt::Debug> Default for ResourceTypeBuilder<P> {
    fn default() -> Self {
        Self {
            variant:  None,
            provider: P::default(),
        }
    }
}

impl<P: MemoryProvider + Default + Clone + Eq + fmt::Debug> ResourceTypeBuilder<P> {
    /// Creates a new resource type builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the memory provider to use.
    pub fn with_provider(mut self, provider: P) -> Self {
        self.provider = provider;
        self
    }

    #[cfg(feature = "std")]
    /// Configures this as a Record resource type with the given field names.
    pub fn as_record<S: AsRef<str>>(mut self, field_names: Vec<S>) -> Result<Self> {
        let fields = field_names
            .into_iter()
            .map(|s| s.as_ref().to_string())
            .collect();

        self.variant = Some(ResourceTypeVariant::Record(fields));
        Ok(self)
    }

    #[cfg(not(feature = "std"))]
    /// Configures this as a Record resource type with the given field name.
    pub fn as_record<S: AsRef<str>>(mut self, field_name: S) -> Result<Self> {
        let field = BoundedString::try_from_str(field_name.as_ref())?;
        self.variant = Some(ResourceTypeVariant::Record(field));
        Ok(self)
    }

    #[cfg(feature = "std")]
    /// Configures this as an Aggregate resource type with the given resource
    /// IDs.
    pub fn as_aggregate(mut self, resource_ids: Vec<u32>) -> Self {
        self.variant = Some(ResourceTypeVariant::Aggregate(resource_ids));
        self
    }

    #[cfg(not(feature = "std"))]
    /// Configures this as an Aggregate resource type with the given resource
    /// ID.
    pub fn as_aggregate(mut self, resource_id: u32) -> Self {
        self.variant = Some(ResourceTypeVariant::Aggregate(resource_id));
        self
    }

    #[cfg(feature = "std")]
    /// Builds a ResourceType with the configured settings.
    pub fn build(self) -> Result<ResourceType<P>> {
        let variant = self.variant.ok_or_else(|| {
            Error::new_static(
                ErrorCategory::Validation,
                codes::VALIDATION_ERROR,
                "Resource type variant is required",
            )
        })?;

        match variant {
            ResourceTypeVariant::Record(fields) => {
                let mut bounded_fields = BoundedVec::new(self.provider.clone())?;
                for name in fields {
                    let bounded_name = BoundedString::try_from_str(&name)?;
                    bounded_fields.push(bounded_name)?;
                }
                Ok(ResourceType::Record(bounded_fields))
            },
            ResourceTypeVariant::Aggregate(ids) => {
                let mut bounded_ids = BoundedVec::new(self.provider)?;
                for id in ids {
                    bounded_ids.push(id)?;
                }
                Ok(ResourceType::Aggregate(bounded_ids))
            },
        }
    }

    #[cfg(not(feature = "std"))]
    /// Builds a ResourceType with the configured settings.
    pub fn build(self) -> Result<ResourceType<P>> {
        let variant = self.variant.ok_or_else(|| {
            Error::new_static(
                ErrorCategory::Validation,
                codes::VALIDATION_ERROR,
                "Resource type variant is required",
            )
        })?;

        match variant {
            ResourceTypeVariant::Record(field) => {
                let mut bounded_fields = BoundedVec::new(self.provider.clone())?;
                bounded_fields.push(field)?;
                Ok(ResourceType::Record(bounded_fields))
            },
            ResourceTypeVariant::Aggregate(id) => {
                let mut bounded_ids = BoundedVec::new(self.provider)?;
                bounded_ids.push(id)?;
                Ok(ResourceType::Aggregate(bounded_ids))
            },
        }
    }
}

/// Builder for ResourceItem instances.
///
/// This builder provides a fluent API for constructing ResourceItem objects
/// used in the WebAssembly component model.
pub struct ResourceItemBuilder<P: MemoryProvider + Default + Clone + Eq + fmt::Debug> {
    id:       Option<u32>,
    type_:    Option<ResourceType<P>>,
    name:     Option<&'static str>,
    provider: P,
}

impl<P: MemoryProvider + Default + Clone + Eq + fmt::Debug> Default for ResourceItemBuilder<P> {
    fn default() -> Self {
        Self {
            id:       None,
            type_:    None,
            name:     None,
            provider: P::default(),
        }
    }
}

impl<P: MemoryProvider + Default + Clone + Eq + fmt::Debug> ResourceItemBuilder<P> {
    /// Creates a new resource item builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the memory provider to use.
    pub fn with_provider(mut self, provider: P) -> Self {
        self.provider = provider;
        self
    }

    /// Sets the resource ID.
    pub fn with_id(mut self, id: u32) -> Self {
        self.id = Some(id);
        self
    }

    /// Sets the resource type.
    pub fn with_type(mut self, type_: ResourceType<P>) -> Self {
        self.type_ = Some(type_);
        self
    }

    /// Sets the resource name.
    pub fn with_name(mut self, name: &'static str) -> Self {
        self.name = Some(name);
        self
    }

    /// Builds a ResourceItem with the configured settings.
    pub fn build(self) -> Result<ResourceItem<P>> {
        let id = self.id.ok_or_else(|| {
            Error::new_static(
                ErrorCategory::Validation,
                codes::VALIDATION_ERROR,
                "Resource ID is required",
            )
        })?;

        let type_ = self.type_.ok_or_else(|| {
            Error::new_static(
                ErrorCategory::Validation,
                codes::VALIDATION_ERROR,
                "Resource type is required",
            )
        })?;

        let name = match self.name {
            Some(name_str) => {
                let wasm_name = WasmName::from_str_truncate(name_str)?;
                Some(wasm_name)
            },
            None => None,
        };

        Ok(ResourceItem { id, type_, name })
    }
}

/// Builder for memory providers and adapters.
pub struct MemoryBuilder<P: MemoryProvider + Default + Clone> {
    provider:           P,
    required_size:      Option<usize>,
    alignment:          Option<usize>,
    verification_level: VerificationLevel,
}

impl<P: MemoryProvider + Default + Clone> Default for MemoryBuilder<P> {
    fn default() -> Self {
        Self {
            provider:           P::default(),
            required_size:      None,
            alignment:          None,
            verification_level: VerificationLevel::default(),
        }
    }
}

impl<P: MemoryProvider + Default + Clone> MemoryBuilder<P> {
    /// Creates a new memory builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the memory provider to use.
    pub fn with_provider(mut self, provider: P) -> Self {
        self.provider = provider;
        self
    }

    /// Sets the required memory size.
    pub fn with_size(mut self, size: usize) -> Self {
        self.required_size = Some(size);
        self
    }

    /// Sets the memory alignment.
    pub fn with_alignment(mut self, alignment: usize) -> Self {
        self.alignment = Some(alignment);
        self
    }

    /// Sets the verification level for memory operations.
    pub fn with_verification_level(mut self, level: VerificationLevel) -> Self {
        self.verification_level = level;
        self
    }

    /// Builds a SafeMemoryHandler with the configured settings.
    pub fn build_safe_memory_handler(self) -> wrt_error::Result<SafeMemoryHandler<P>> {
        // First, configure the provider with the required verification level
        let mut provider = self.provider;
        provider.set_verification_level(self.verification_level);

        // Create the safe memory handler with the configured provider
        let mut handler = SafeMemoryHandler::new(provider);

        // If a specific memory size requirement was provided, ensure it's properly
        // handled
        if let Some(size) = self.required_size {
            if size > 0 {
                handler.ensure_used_up_to(size)?;
            }
        }

        // Configure the handler if needed
        if let Some(_alignment) = self.alignment {
            // For now, we don't have a way to set alignment after creation
            // This would need to be added to SafeMemoryHandler
            // We'll just note this for future enhancement
        }

        Ok(handler)
    }
}

// Note: NoStdProviderBuilder and NoStdProviderBuilder1 have been removed.
// Use safe_managed_alloc!(size, crate_id) instead.

#[cfg(test)]
mod tests {
    #[cfg(all(not(feature = "std"), feature = "alloc"))]
    use alloc::format;
    #[cfg(feature = "std")]
    use std::format;

    use super::*;
    use crate::traits::BoundedCapacity;

    #[test]
    fn test_bounded_builder() {
        let builder = BoundedBuilder::<u32, 10, NoStdProvider<1024>>::new()
            .with_verification_level(VerificationLevel::Full);

        let stack = builder.build_stack().unwrap();
        assert_eq!(stack.verification_level(), VerificationLevel::Full);
        assert_eq!(stack.capacity(), 10);
    }

    #[test]
    fn test_string_builder() {
        let builder = StringBuilder::<256, NoStdProvider<1024>>::new()
            .with_content("test")
            .with_truncation(true);

        let string = builder.build_string().unwrap();
        assert_eq!(string.as_str().unwrap(), "test");
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_resource_type_builder() {
        // Test Record type
        let builder = ResourceTypeBuilder::<NoStdProvider<1024>>::new();
        let resource_type = builder.as_record(vec!["field1", "field2"]).unwrap().build().unwrap();

        match resource_type {
            ResourceType::Record(fields) => {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields.get(0).unwrap().as_str().unwrap(), "field1");
                assert_eq!(fields.get(1).unwrap().as_str().unwrap(), "field2");
            },
            _ => panic!("Expected ResourceType::Record"),
        }

        // Test Aggregate type
        let builder = ResourceTypeBuilder::<NoStdProvider<1024>>::new();
        let resource_type = builder.as_aggregate(vec![1, 2, 3]).build().unwrap();

        match resource_type {
            ResourceType::Aggregate(ids) => {
                assert_eq!(ids.len(), 3);
                assert_eq!(ids.get(0).unwrap(), 1);
                assert_eq!(ids.get(1).unwrap(), 2);
                assert_eq!(ids.get(2).unwrap(), 3);
            },
            _ => panic!("Expected ResourceType::Aggregate"),
        }
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_resource_item_builder() {
        // First create a resource type
        let resource_type = ResourceTypeBuilder::<NoStdProvider<1024>>::new()
            .as_record(vec!["field1", "field2"])
            .unwrap()
            .build()
            .unwrap();

        // Now create a resource item
        let builder = ResourceItemBuilder::<NoStdProvider<1024>>::new()
            .with_id(42)
            .with_type(resource_type)
            .with_name("test_resource");

        let resource_item = builder.build().unwrap();

        assert_eq!(resource_item.id, 42);
        assert_eq!(
            resource_item.name.unwrap().as_str().unwrap(),
            "test_resource"
        );
        match &resource_item.type_ {
            ResourceType::Record(fields) => {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields.get(0).unwrap().as_str().unwrap(), "field1");
                assert_eq!(fields.get(1).unwrap().as_str().unwrap(), "field2");
            },
            _ => panic!("Expected ResourceType::Record"),
        }
    }

    // NOTE: NoStdProviderBuilder tests removed - use safe_managed_alloc!()
    // macro instead

    // NOTE: NoStdProviderBuilder1 tests removed - use safe_managed_alloc!()
    // macro instead
}
