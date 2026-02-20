// WRT - wrt-foundation
// Module: Component Builders
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Builder pattern implementation for WebAssembly Component Model types.
//!
//! This module provides builders for complex types in the WebAssembly Component
//! Model, ensuring proper initialization, validation, and resource management.

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(feature = "std")]
use core::fmt::Debug;

// Builder capacity limits for bounded collections
const MAX_BUILDER_IMPORTS: usize = 256;
const MAX_BUILDER_EXPORTS: usize = 256;
const MAX_BUILDER_ALIASES: usize = 128;
const MAX_BUILDER_INSTANCES: usize = 64;
const MAX_BUILDER_CORE_INSTANCES: usize = 64;
const MAX_BUILDER_COMPONENT_TYPES: usize = 128;
const MAX_BUILDER_CORE_TYPES: usize = 128;

use wrt_error::Result;

use crate::{
    bounded::{
        BoundedString,
        BoundedVec,
        WasmName,
        MAX_WASM_NAME_LENGTH,
    },
    component::{
        ComponentAlias,
        ComponentInstance,
        ComponentType,
        CoreInstance,
        CoreType,
        Export,
        ExternType,
        Import,
        ImportKey,
        Namespace,
        MAX_COMPONENT_ALIASES,
        MAX_COMPONENT_EXPORTS,
        MAX_COMPONENT_IMPORTS,
        MAX_COMPONENT_INSTANCES,
        MAX_COMPONENT_TYPES,
        MAX_CORE_INSTANCES,
        MAX_CORE_TYPES,
        MAX_NAMESPACE_ELEMENTS,
        MAX_NAME_LEN,
    },
    component_type_store::TypeRef,
    resource::{
        Resource,
        ResourceRepr,
        ResourceType,
    },
    verification::VerificationLevel,
    Error,
    MemoryProvider,
};

/// Builder for `ComponentType` instances.
///
/// Provides a fluent API for constructing Component Model types with
/// proper initialization of all collections using bounded collections.
#[derive(Debug)]
pub struct ComponentTypeBuilder<P: MemoryProvider + Default + Clone + PartialEq + Eq> {
    provider:        P,
    imports:         BoundedVec<Import<P>, MAX_BUILDER_IMPORTS, P>,
    exports:         BoundedVec<Export<P>, MAX_BUILDER_EXPORTS, P>,
    aliases:         BoundedVec<ComponentAlias<P>, MAX_BUILDER_ALIASES, P>,
    instances:       BoundedVec<ComponentInstance<P>, MAX_BUILDER_INSTANCES, P>,
    core_instances:  BoundedVec<CoreInstance<P>, MAX_BUILDER_CORE_INSTANCES, P>,
    component_types: BoundedVec<TypeRef, MAX_BUILDER_COMPONENT_TYPES, P>,
    core_types:      BoundedVec<CoreType, MAX_BUILDER_CORE_TYPES, P>,
}

impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> ComponentTypeBuilder<P> {
    /// Create a new ComponentTypeBuilder with bounded collections
    pub fn new() -> wrt_error::Result<Self> {
        let provider = P::default();
        Ok(Self {
            imports: BoundedVec::new(provider.clone())?,
            exports: BoundedVec::new(provider.clone())?,
            aliases: BoundedVec::new(provider.clone())?,
            instances: BoundedVec::new(provider.clone())?,
            core_instances: BoundedVec::new(provider.clone())?,
            component_types: BoundedVec::new(provider.clone())?,
            core_types: BoundedVec::new(provider.clone())?,
            provider,
        })
    }
}

#[cfg(feature = "std")]
impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> Default for ComponentTypeBuilder<P> {
    fn default() -> Self {
        Self::new().expect("Failed to create ComponentTypeBuilder with default provider")
    }
}

#[cfg(feature = "std")]
impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> ComponentTypeBuilder<P> {
    /// Creates a new builder with default settings.
    pub fn with_defaults() -> Self {
        Self::default()
    }

    /// Sets the memory provider to use.
    pub fn with_provider(mut self, provider: P) -> Self {
        self.provider = provider;
        self
    }

    /// Adds an import to the component type.
    pub fn with_import(mut self, import: Import<P>) -> Self {
        // Note: Silently ignore capacity errors in builder pattern
        drop(self.imports.push(import));
        self
    }

    /// Adds multiple imports to the component type.
    pub fn with_imports(mut self, imports: impl IntoIterator<Item = Import<P>>) -> Self {
        self.imports.extend(imports);
        self
    }

    /// Adds an export to the component type.
    pub fn with_export(mut self, export: Export<P>) -> Self {
        // Note: Silently ignore capacity errors in builder pattern
        drop(self.exports.push(export));
        self
    }

    /// Adds multiple exports to the component type.
    pub fn with_exports(mut self, exports: impl IntoIterator<Item = Export<P>>) -> Self {
        self.exports.extend(exports);
        self
    }

    /// Adds an alias to the component type.
    pub fn with_alias(mut self, alias: ComponentAlias<P>) -> Self {
        // Note: Silently ignore capacity errors in builder pattern
        drop(self.aliases.push(alias));
        self
    }

    /// Adds multiple aliases to the component type.
    pub fn with_aliases(mut self, aliases: impl IntoIterator<Item = ComponentAlias<P>>) -> Self {
        self.aliases.extend(aliases);
        self
    }

    /// Adds a component instance to the component type.
    pub fn with_instance(mut self, instance: ComponentInstance<P>) -> Self {
        // Note: Silently ignore capacity errors in builder pattern
        drop(self.instances.push(instance));
        self
    }

    /// Adds multiple component instances to the component type.
    pub fn with_instances(
        mut self,
        instances: impl IntoIterator<Item = ComponentInstance<P>>,
    ) -> Self {
        self.instances.extend(instances);
        self
    }

    /// Adds a core instance to the component type.
    pub fn with_core_instance(mut self, core_instance: CoreInstance<P>) -> Self {
        // Note: Silently ignore capacity errors in builder pattern
        drop(self.core_instances.push(core_instance));
        self
    }

    /// Adds multiple core instances to the component type.
    pub fn with_core_instances(
        mut self,
        core_instances: impl IntoIterator<Item = CoreInstance<P>>,
    ) -> Self {
        self.core_instances.extend(core_instances);
        self
    }

    /// Adds a component type reference to the component type.
    pub fn with_component_type(mut self, component_type: TypeRef) -> Self {
        drop(self.component_types.push(component_type));
        self
    }

    /// Adds multiple component type references to the component type.
    pub fn with_component_types(
        mut self,
        component_types: impl IntoIterator<Item = TypeRef>,
    ) -> Self {
        self.component_types.extend(component_types);
        self
    }

    /// Adds a core type to the component type.
    pub fn with_core_type(mut self, core_type: CoreType) -> Self {
        drop(self.core_types.push(core_type));
        self
    }

    /// Adds multiple core types to the component type.
    pub fn with_core_types(mut self, core_types: impl IntoIterator<Item = CoreType>) -> Self {
        self.core_types.extend(core_types);
        self
    }

    /// Builds and returns a configured `ComponentType`.
    pub fn build(self) -> wrt_error::Result<ComponentType<P>> {
        // Create bounded vectors for each collection
        let mut imports = BoundedVec::new(self.provider.clone())?;
        let mut exports = BoundedVec::new(self.provider.clone())?;
        let mut aliases = BoundedVec::new(self.provider.clone())?;
        let mut instances = BoundedVec::new(self.provider.clone())?;
        let mut core_instances = BoundedVec::new(self.provider.clone())?;
        let mut component_types = BoundedVec::new(self.provider.clone())?;
        let mut core_types = BoundedVec::new(self.provider.clone())?;

        // Populate bounded vectors
        for import in self.imports {
            imports.push(import)?;
        }

        for export in self.exports {
            exports.push(export)?;
        }

        for alias in self.aliases {
            aliases.push(alias)?;
        }

        for instance in self.instances {
            instances.push(instance)?;
        }

        for core_instance in self.core_instances {
            core_instances.push(core_instance)?;
        }

        for component_type in self.component_types {
            component_types.push(component_type)?;
        }

        for core_type in self.core_types {
            core_types.push(core_type)?;
        }

        Ok(ComponentType {
            imports,
            exports,
            aliases,
            instances,
            core_instances,
            component_types,
            core_types,
        })
    }
}

#[cfg(feature = "std")]
/// Builder for `Import` instances.
#[derive(Debug)]
pub struct ImportBuilder<P: MemoryProvider + Default + Clone + PartialEq + Eq> {
    provider:  P,
    namespace: Option<Namespace<P>>,
    name:      Option<WasmName<MAX_NAME_LEN>>,
    ty:        Option<ExternType<P>>,
}

#[cfg(feature = "std")]
impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> Default for ImportBuilder<P> {
    fn default() -> Self {
        Self {
            provider:  P::default(),
            namespace: None,
            name:      None,
            ty:        None,
        }
    }
}

#[cfg(feature = "std")]
impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> ImportBuilder<P> {
    /// Creates a new builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the memory provider to use.
    pub fn with_provider(mut self, provider: P) -> Self {
        self.provider = provider;
        self
    }

    /// Sets the namespace for the import.
    pub fn with_namespace(mut self, namespace: Namespace<P>) -> Self {
        self.namespace = Some(namespace);
        self
    }

    /// Sets the namespace for the import from a string.
    pub fn with_namespace_str(mut self, namespace_str: &str) -> wrt_error::Result<Self> {
        let namespace = Namespace::from_str(namespace_str, self.provider.clone())?;
        self.namespace = Some(namespace);
        Ok(self)
    }

    /// Sets the name for the import.
    pub fn with_name(mut self, name: WasmName<MAX_NAME_LEN>) -> Self {
        self.name = Some(name);
        self
    }

    /// Sets the name for the import from a string.
    pub fn with_name_str(mut self, name_str: &str) -> wrt_error::Result<Self> {
        let name = WasmName::try_from_str(name_str)?;
        self.name = Some(name);
        Ok(self)
    }

    /// Sets the type for the import.
    pub fn with_type(mut self, ty: ExternType<P>) -> Self {
        self.ty = Some(ty);
        self
    }

    /// Builds and returns a configured `Import`.
    pub fn build(self) -> wrt_error::Result<Import<P>> {
        let namespace = self
            .namespace
            .ok_or_else(|| Error::validation_error("Import namespace is required"))?;

        let name = self.name.ok_or_else(|| Error::validation_error("Import name is required"))?;

        let ty = self.ty.ok_or_else(|| Error::validation_error("Import type is required"))?;

        Ok(Import {
            key: ImportKey { namespace, name },
            ty,
        })
    }
}

#[cfg(feature = "std")]
/// Builder for `Export` instances.
#[derive(Debug)]
pub struct ExportBuilder<P: MemoryProvider + Default + Clone + PartialEq + Eq> {
    provider: P,
    name:     Option<WasmName<MAX_NAME_LEN>>,
    ty:       Option<ExternType<P>>,
    desc:     Option<WasmName<MAX_NAME_LEN>>,
}

#[cfg(feature = "std")]
impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> Default for ExportBuilder<P> {
    fn default() -> Self {
        Self {
            provider: P::default(),
            name:     None,
            ty:       None,
            desc:     None,
        }
    }
}

#[cfg(feature = "std")]
impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> ExportBuilder<P> {
    /// Creates a new builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the memory provider to use.
    pub fn with_provider(mut self, provider: P) -> Self {
        self.provider = provider;
        self
    }

    /// Sets the name for the export.
    pub fn with_name(mut self, name: WasmName<MAX_NAME_LEN>) -> Self {
        self.name = Some(name);
        self
    }

    /// Sets the name for the export from a string.
    pub fn with_name_str(mut self, name_str: &str) -> wrt_error::Result<Self> {
        let name = WasmName::try_from_str(name_str)?;
        self.name = Some(name);
        Ok(self)
    }

    /// Sets the type for the export.
    pub fn with_type(mut self, ty: ExternType<P>) -> Self {
        self.ty = Some(ty);
        self
    }

    /// Sets the description for the export.
    pub fn with_description(mut self, desc: WasmName<MAX_NAME_LEN>) -> Self {
        self.desc = Some(desc);
        self
    }

    /// Sets the description for the export from a string.
    pub fn with_description_str(mut self, desc_str: &str) -> wrt_error::Result<Self> {
        let desc = WasmName::try_from_str(desc_str)?;
        self.desc = Some(desc);
        Ok(self)
    }

    /// Builds and returns a configured `Export`.
    pub fn build(self) -> wrt_error::Result<Export<P>> {
        let name = self.name.ok_or_else(|| Error::validation_error("Export name is required"))?;

        let ty = self.ty.ok_or_else(|| Error::validation_error("Export type is required"))?;

        Ok(Export {
            name,
            ty,
            desc: self.desc,
        })
    }
}

#[cfg(feature = "std")]
/// Builder for `Namespace` instances.
#[derive(Debug)]
pub struct NamespaceBuilder<P: MemoryProvider + Default + Clone + PartialEq + Eq> {
    provider: P,
    elements: BoundedVec<WasmName<MAX_NAME_LEN>, MAX_NAMESPACE_ELEMENTS, P>,
}

#[cfg(feature = "std")]
impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> Default for NamespaceBuilder<P> {
    fn default() -> Self {
        let provider = P::default();
        let elements = BoundedVec::new(provider.clone())
            .expect("Failed to create BoundedVec with default provider");
        Self { provider, elements }
    }
}

#[cfg(feature = "std")]
impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> NamespaceBuilder<P> {
    /// Creates a new builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the memory provider to use.
    pub fn with_provider(mut self, provider: P) -> Self {
        self.provider = provider;
        self
    }

    /// Adds an element to the namespace.
    pub fn with_element(mut self, element: WasmName<MAX_NAME_LEN>) -> wrt_error::Result<Self> {
        self.elements.push(element)?;
        Ok(self)
    }

    /// Adds an element to the namespace from a string.
    pub fn with_element_str(mut self, element_str: &str) -> wrt_error::Result<Self> {
        let element = WasmName::try_from_str(element_str)?;
        self.elements.push(element)?;
        Ok(self)
    }

    /// Adds multiple elements to the namespace.
    pub fn with_elements(
        mut self,
        elements: impl IntoIterator<Item = WasmName<MAX_NAME_LEN>>,
    ) -> wrt_error::Result<Self> {
        for element in elements {
            self.elements.push(element)?;
        }
        Ok(self)
    }

    /// Creates a namespace from a colon-separated string.
    pub fn from_str(namespace_str: &str, provider: P) -> wrt_error::Result<Self> {
        let mut builder = Self::new().with_provider(provider.clone());

        for part in namespace_str.split(':') {
            if !part.is_empty() {
                builder = builder.with_element_str(part)?;
            }
        }

        Ok(builder)
    }

    /// Builds and returns a configured `Namespace`.
    pub fn build(self) -> wrt_error::Result<Namespace<P>> {
        Ok(Namespace {
            elements: self.elements,
        })
    }
}

#[cfg(feature = "std")]
/// Builder for `ResourceType` instances.
#[derive(Debug)]
pub struct ResourceTypeBuilder<P: MemoryProvider + Default + Clone + PartialEq + Eq> {
    provider: P,
    variant:  Option<ResourceTypeVariant<P>>,
}

#[cfg(feature = "std")]
/// Enum to represent the variants of `ResourceType`.
#[derive(Debug)]
enum ResourceTypeVariant<P: MemoryProvider + Default + Clone + PartialEq + Eq> {
    Record(BoundedVec<BoundedString<MAX_WASM_NAME_LENGTH>, 32, P>),
    Aggregate(BoundedVec<u32, { crate::resource::MAX_RESOURCE_AGGREGATE_IDS }, P>),
}

#[cfg(feature = "std")]
impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> Default for ResourceTypeBuilder<P> {
    fn default() -> Self {
        Self {
            provider: P::default(),
            variant:  None,
        }
    }
}

#[cfg(feature = "std")]
impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> ResourceTypeBuilder<P> {
    /// Creates a new builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the memory provider to use.
    pub fn with_provider(mut self, provider: P) -> Self {
        self.provider = provider;
        self
    }

    /// Configures this as a record resource type with the given field names.
    pub fn as_record(
        mut self,
        field_names: BoundedVec<BoundedString<MAX_WASM_NAME_LENGTH>, 32, P>,
    ) -> Self {
        self.variant = Some(ResourceTypeVariant::Record(field_names));
        self
    }

    /// Configures this as an aggregate resource type with the given resource
    /// IDs.
    pub fn as_aggregate(
        mut self,
        resource_ids: BoundedVec<u32, { crate::resource::MAX_RESOURCE_AGGREGATE_IDS }, P>,
    ) -> Self {
        self.variant = Some(ResourceTypeVariant::Aggregate(resource_ids));
        self
    }

    /// Builds and returns a configured `ResourceType`.
    pub fn build(self) -> wrt_error::Result<ResourceType<P>> {
        let variant = self
            .variant
            .ok_or_else(|| Error::validation_error("Resource type variant must be specified"))?;

        match variant {
            ResourceTypeVariant::Record(field_names) => {
                let mut fields = BoundedVec::new(self.provider.clone())?;
                for field in field_names {
                    // Convert BoundedString<MAX_WASM_NAME_LENGTH> to
                    // BoundedString<MAX_RESOURCE_FIELD_NAME_LEN>
                    let field_str = field.as_str().map_err(Error::from)?;
                    let converted_field = BoundedString::<
                        { crate::resource::MAX_RESOURCE_FIELD_NAME_LEN },
                    >::try_from_str(field_str)
                    .map_err(Error::from)?;
                    fields.push(converted_field)?;
                }
                Ok(ResourceType::Record(fields))
            },
            ResourceTypeVariant::Aggregate(resource_ids) => {
                Ok(ResourceType::Aggregate(resource_ids))
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        budget_aware_provider::CrateId,
        safe_managed_alloc,
        traits::BoundedCapacity,
    };


    #[test]
    fn test_namespace_builder() -> wrt_error::Result<()> {
        let provider = safe_managed_alloc!(1024, CrateId::Foundation)?;

        // Test building from parts
        let namespace_builder = NamespaceBuilder::new()
            .with_provider(provider.clone())
            .with_element_str("wasm")?
            .with_element_str("component")?;

        let namespace = namespace_builder.build()?;
        assert_eq!(namespace.elements.len(), 2);

        // Test building from string
        let namespace =
            NamespaceBuilder::from_str("wasm:component:model", provider.clone())?.build()?;

        assert_eq!(namespace.elements.len(), 3);
        Ok(())
    }

    #[test]
    fn test_resource_type_builder() -> wrt_error::Result<()> {
        let provider = safe_managed_alloc!(1024, CrateId::Foundation)?;

        // Test record resource type
        let field_name = BoundedString::try_from_str("field")?;
        let mut field_names = BoundedVec::new(provider.clone())?;
        field_names.push(field_name)?;

        let resource_type = ResourceTypeBuilder::new()
            .with_provider(provider.clone())
            .as_record(field_names)
            .build()?;

        match resource_type {
            ResourceType::Record(fields) => assert_eq!(fields.len(), 1),
            _ => panic!("Expected ResourceType::Record"),
        }

        // Test aggregate resource type
        let mut resource_ids = BoundedVec::new(provider.clone())?;
        resource_ids.push(1)?;
        resource_ids.push(2)?;
        resource_ids.push(3)?;

        let resource_type = ResourceTypeBuilder::new()
            .with_provider(provider.clone())
            .as_aggregate(resource_ids)
            .build()?;

        match resource_type {
            ResourceType::Aggregate(ids) => assert_eq!(ids.len(), 3),
            _ => panic!("Expected ResourceType::Aggregate"),
        }
        Ok(())
    }
}
