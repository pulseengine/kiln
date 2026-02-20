// WRT - wrt-foundation
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

// ToString comes from prelude
use core::fmt::Debug;

use wrt_error::{
    ErrorCategory,
    Result,
};

#[cfg(feature = "std")]
use crate::component_type_store::TypeRef;
// --- Traits needed for BoundedVec items ---
use crate::traits::{
    FromBytes,
    ReadStream,
    SerializationError,
    ToBytes,
    WriteStream,
};
use crate::{
    bounded::{
        BoundedVec,
        WasmName,
        MAX_WASM_NAME_LENGTH,
    },
    codes,
    prelude::*,
    traits::Checksummable,
    types::{
        FuncType,
        GlobalType,
        MemoryType,
        TableType,
    },
    Error,
    MemoryProvider,
};

// Binary std/no_std choice
#[cfg(not(feature = "std"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub struct TypeRef(pub u32);

#[cfg(not(feature = "std"))]
impl TypeRef {
    pub const fn new(index: u32) -> Self {
        Self(index)
    }
}

#[cfg(not(feature = "std"))]
impl Checksummable for TypeRef {
    fn update_checksum(&self, checksum: &mut crate::verification::Checksum) {
        self.0.update_checksum(checksum);
    }
}

#[cfg(not(feature = "std"))]
impl ToBytes for TypeRef {
    fn to_bytes_with_provider<P: MemoryProvider>(
        &self,
        writer: &mut WriteStream,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.0.to_bytes_with_provider(writer, provider)
    }
}

#[cfg(not(feature = "std"))]
impl FromBytes for TypeRef {
    fn from_bytes_with_provider<P: MemoryProvider>(
        reader: &mut ReadStream,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let index = u32::from_bytes_with_provider(reader, provider)?;
        Ok(Self(index))
    }
}

// --- Capacity Constants ---
/// Maximum number of component imports
pub const MAX_COMPONENT_IMPORTS: usize = 128;
/// Maximum number of component exports
pub const MAX_COMPONENT_EXPORTS: usize = 128;
/// Maximum number of component aliases
pub const MAX_COMPONENT_ALIASES: usize = 128;
/// Maximum number of component instances
pub const MAX_COMPONENT_INSTANCES: usize = 64;
/// Maximum number of core instances
pub const MAX_CORE_INSTANCES: usize = 64;
/// Maximum number of component types
pub const MAX_COMPONENT_TYPES: usize = 64;
/// Maximum number of core types
pub const MAX_CORE_TYPES: usize = 64;
/// Maximum number of namespace elements
pub const MAX_NAMESPACE_ELEMENTS: usize = 64;
/// Maximum length of names, matching MAX_WASM_NAME_LENGTH from bounded.rs
pub const MAX_NAME_LEN: usize = MAX_WASM_NAME_LENGTH;

use core::marker::PhantomData;

/// Represents the type of a WebAssembly component.
#[derive(Clone, Debug, PartialEq, Eq, Default, Hash)]
pub struct ComponentType<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    pub imports:         BoundedVec<Import<P>, MAX_COMPONENT_IMPORTS, P>,
    pub exports:         BoundedVec<Export<P>, MAX_COMPONENT_EXPORTS, P>,
    pub aliases:         BoundedVec<ComponentAlias<P>, MAX_COMPONENT_ALIASES, P>,
    pub instances:       BoundedVec<ComponentInstance<P>, MAX_COMPONENT_INSTANCES, P>,
    pub core_instances:  BoundedVec<CoreInstance<P>, MAX_CORE_INSTANCES, P>,
    pub component_types: BoundedVec<TypeRef, MAX_COMPONENT_TYPES, P>,
    pub core_types:      BoundedVec<CoreType, MAX_CORE_TYPES, P>,
}

impl<P> ComponentType<P>
where
    P: MemoryProvider + Clone + Default + Eq + Debug,
{
    /// Creates a unit component type (empty component with no imports/exports)
    pub fn unit(provider: P) -> wrt_error::Result<Self> {
        Ok(Self {
            imports:         BoundedVec::new(provider.clone())?,
            exports:         BoundedVec::new(provider.clone())?,
            aliases:         BoundedVec::new(provider.clone())?,
            instances:       BoundedVec::new(provider.clone())?,
            core_instances:  BoundedVec::new(provider.clone())?,
            component_types: BoundedVec::new(provider.clone())?,
            core_types:      BoundedVec::new(provider)?,
        })
    }

    /// Constant-like accessor for Unit type (requires provider)
    /// This is used in pattern matching contexts like `ComponentType::Unit`
    #[allow(non_upper_case_globals)]
    pub const Unit: fn(P) -> wrt_error::Result<Self> = Self::unit;
}

/// Represents an import for a component or core module.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Import<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug, /* For WasmName default and
                                                                  * BoundedVec usage if this
                                                                  * struct becomes Default */
{
    pub key: ImportKey<P>,
    pub ty:  ExternType<P>,
}

/// Represents an export from a component or core module.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Export<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug, /* For WasmName default and
                                                                  * BoundedVec usage */
{
    pub name: WasmName<MAX_NAME_LEN>,
    pub ty:   ExternType<P>,
    pub desc: Option<WasmName<MAX_NAME_LEN>>,
}

/// Key for an import, consisting of a namespace and a name.
#[derive(Clone, Debug, PartialEq, Eq, Hash)] // Hash might be problematic if P is not fixed
pub struct ImportKey<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug, /* For WasmName default and
                                                                  * Namespace default */
{
    pub namespace: Namespace<P>,
    pub name:      WasmName<MAX_NAME_LEN>,
}

/// Namespace for imports.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Namespace<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug, // For BoundedVec default
{
    pub elements: BoundedVec<WasmName<MAX_NAME_LEN>, MAX_NAMESPACE_ELEMENTS, P>,
}

impl<P> Namespace<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    /// Creates a namespace from a string like "namespace:name".
    /// The provider P must be supplied to construct WasmName instances.
    pub fn from_str(s: &str, provider: P) -> wrt_error::Result<Self>
    where
        P: Clone,
    {
        let mut elements = BoundedVec::new(provider.clone())?;
        for part in s.split(':') {
            if !part.is_empty() {
                let name_element = WasmName::try_from_str(part)?;
                elements.push(name_element)?;
            }
        }
        Ok(Self { elements })
    }
}

/// External types that can be imported or exported.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ExternType<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    Func(FuncType),
    Table(TableType),
    Memory(MemoryType),
    Global(GlobalType),
    Tag(FuncType),
    Component(ComponentType<P>),
    Instance(InstanceType<P>),
    CoreModule(TypeRef),
    TypeDef(TypeRef),
    Resource(ResourceType<P>),
}

/// Type definition for a core module.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct CoreModuleType<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug, /* For BoundedVec default on
                                                                  * its fields */
{
    pub imports: BoundedVec<Import<P>, MAX_COMPONENT_IMPORTS, P>,
    pub exports: BoundedVec<Export<P>, MAX_COMPONENT_EXPORTS, P>,
}

/// Represents an instance type for a component.
#[derive(Clone, Debug, PartialEq, Eq, Default, Hash)]
pub struct InstanceType<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug, /* For BoundedVec default on
                                                                  * its fields */
{
    pub exports: BoundedVec<Export<P>, MAX_COMPONENT_EXPORTS, P>,
}

/// Represents an alias in a component.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ComponentAlias<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    InstanceExport(ComponentAliasInstanceExport),
    CoreInstanceExport(ComponentAliasCoreInstanceExport),
    Outer(ComponentAliasOuter),
    #[doc(hidden)]
    _Phantom(core::marker::PhantomData<P>),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ComponentAliasInstanceExport {
    pub instance_idx: u32,
    pub name:         WasmName<MAX_NAME_LEN>,
    pub kind:         ComponentAliasExportKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ComponentAliasCoreInstanceExport {
    pub core_instance_idx: u32,
    pub name:              WasmName<MAX_NAME_LEN>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Hash)]
pub struct ComponentAliasOuter {
    pub count: u32, // Number of levels to go up
    pub index: u32, // Index in the outer component's items (types, instances etc)
    pub kind:  ComponentAliasOuterKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Copy, Hash, Default)]
#[repr(u8)]
pub enum ComponentAliasExportKind {
    #[default]
    Func     = 0,
    Table    = 1,
    Memory   = 2,
    Global   = 3,
    TypeDef  = 4,
    Resource = 5, // Added for resource aliasing
}

#[derive(Clone, Debug, PartialEq, Eq, Copy, Hash, Default)]
#[repr(u8)]
pub enum ComponentAliasOuterKind {
    #[default]
    Type       = 0,
    Component  = 1,
    CoreType   = 2,
    CoreModule = 3,
}

impl Checksummable for ComponentAliasOuterKind {
    fn update_checksum(&self, checksum: &mut crate::verification::Checksum) {
        checksum.update(*self as u8);
    }
}

impl ToBytes for ComponentAliasOuterKind {
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        writer.write_u8(*self as u8)
    }
}

impl FromBytes for ComponentAliasOuterKind {
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let byte = reader.read_u8()?;
        match byte {
            0 => Ok(Self::Type),
            1 => Ok(Self::Component),
            2 => Ok(Self::CoreType),
            3 => Ok(Self::CoreModule),
            _ => Err(Error::runtime_execution_error(
                "Invalid component kind discriminant",
            )),
        }
    }
}

/// Represents a component instance declaration within a component.
#[derive(Clone, Debug, PartialEq, Eq, Default, Hash)]
pub struct ComponentInstance<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    pub kind: ComponentInstanceKind<P>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Hash)]
pub enum ComponentInstanceKind<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    #[default]
    Unknown,
    Instantiate {
        component_idx: u32,
        args:          BoundedVec<ComponentInstantiationArg<P>, MAX_COMPONENT_IMPORTS, P>,
    },
    FromExports {
        exports: BoundedVec<Export<P>, MAX_COMPONENT_EXPORTS, P>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Hash)]
pub struct ComponentInstantiationArg<P> {
    pub name:  WasmName<MAX_NAME_LEN>,
    pub index: u32, // Index of the item being passed as argument (e.g. func_idx, table_idx)
    pub kind:  ExternKind, // The kind of the item being passed
    #[doc(hidden)]
    _phantom: core::marker::PhantomData<P>,
}

/// Represents a core WebAssembly module instance declaration.
#[derive(Clone, Debug, PartialEq, Eq, Default, Hash)]
pub struct CoreInstance<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    pub kind: CoreInstanceKind<P>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Hash)]
pub enum CoreInstanceKind<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    #[default]
    Unknown,
    Instantiate {
        module_idx: u32, // Index of the core module type
        args:       BoundedVec<CoreInstantiationArg<P>, MAX_COMPONENT_IMPORTS, P>,
    },
    FromExports {
        exports: BoundedVec<Export<P>, MAX_COMPONENT_EXPORTS, P>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Hash)]
pub struct CoreInstantiationArg<P> {
    pub name:  WasmName<MAX_NAME_LEN>,
    pub index: u32,
    pub kind:  ExternKind,
    #[doc(hidden)]
    _phantom: core::marker::PhantomData<P>,
}

/// Represents a core type definition (func, table, memory, global, tag).
///
/// **Migration Note:** Removed MemoryProvider generic parameter P (Issue #118)
#[derive(Clone, Debug, PartialEq, Eq, Default, Hash)]
pub enum CoreType {
    #[default]
    Unknown,
    Func(FuncType),
    Table(TableType),
    Memory(MemoryType),
    Global(GlobalType),
    Tag(FuncType),
}

/// General kind of an external item for instantiation arguments.
#[derive(Clone, Debug, PartialEq, Eq, Copy, Hash, Default)]
#[repr(u8)]
pub enum ExternKind {
    #[default]
    Func       = 0,
    Table      = 1,
    Memory     = 2,
    Global     = 3,
    Tag        = 4,
    Component  = 5,
    Instance   = 6,
    CoreModule = 7,
    TypeDef    = 8,
    Resource   = 9,
}

impl Checksummable for ExternKind {
    fn update_checksum(&self, checksum: &mut crate::verification::Checksum) {
        checksum.update(*self as u8);
    }
}

impl ToBytes for ExternKind {
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        writer.write_u8(*self as u8)
    }
}

impl FromBytes for ExternKind {
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let byte = reader.read_u8()?;
        match byte {
            0 => Ok(Self::Func),
            1 => Ok(Self::Table),
            2 => Ok(Self::Memory),
            3 => Ok(Self::Global),
            4 => Ok(Self::Tag),
            5 => Ok(Self::Component),
            6 => Ok(Self::Instance),
            7 => Ok(Self::CoreModule),
            8 => Ok(Self::TypeDef),
            9 => Ok(Self::Resource),
            _ => Err(Error::new(
                ErrorCategory::Parse,
                codes::INVALID_VALUE,
                "Invalid enum value",
            )),
        }
    }
}

/// Placeholder for a Resource Type definition.
/// In the component model, resources are opaque handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceType<P: MemoryProvider>(pub u32, pub PhantomData<P>);

// Manual Default for ResourceType<P>
impl<P: MemoryProvider> Default for ResourceType<P> {
    fn default() -> Self {
        ResourceType(0, PhantomData) // Default ID 0
    }
}

// TODO: Implement Checksummable, ToBytes, FromBytes for many of these types.
// For Checksummable, ToBytes, FromBytes on structs:
// Iterate fields, update checksum / write bytes / read bytes.
// For enums: write/read a discriminant then handle the variant.

// Example for Export<P>: Checksummable
// impl<P: MemoryProvider + Clone + Default> Checksummable for Export<P>
// where
//     WasmName<MAX_NAME_LEN>: Checksummable,
//     ExternType<P>: Checksummable, // This will be complex for an enum
// {
//     fn update_checksum(&self, checksum: &mut crate::verification::Checksum) {
//         self.name.update_checksum(checksum;
//         self.ty.update_checksum(checksum;
//         if let Some(desc) = &self.desc {
//             desc.update_checksum(checksum;
//         } else {
//             // Handle None case for checksum, perhaps a specific byte pattern
//             checksum.consume(&[0u8]); // Example
//         }
//     }
// }

// This is a starting point. The ToBytes/FromBytes/Checksummable implementations
// will be non-trivial, especially for enums like ExternType and for ensuring
// all nested types also implement them correctly with the right P bounds.

// --- Default Implementations ---

// Default for Import<P>
impl<P> Default for Import<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    fn default() -> Self {
        // Assumes ImportKey<P> and ExternType<P> have correct Default impls
        // that satisfy their own P bounds (including Eq if needed by their fields like
        // WasmName).
        Self {
            key: ImportKey::default(),
            ty:  ExternType::default(),
        }
    }
}

// TODO: Implement Checksummable, ToBytes, FromBytes for Import<P>

// Default for Export<P>
impl<P> Default for Export<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    fn default() -> Self {
        // Assumes WasmName<P> and ExternType<P> have correct Default impls.
        Self {
            name: WasmName::default(),
            ty:   ExternType::default(),
            desc: None, // Option<WasmName> can be None by default
        }
    }
}

// TODO: Implement Checksummable, ToBytes, FromBytes for Export<P>

// Default for ImportKey<P>
impl<P> Default for ImportKey<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    fn default() -> Self {
        Self {
            namespace: Namespace::default(),
            name:      WasmName::default(),
        }
    }
}

// Default for Namespace<P>
impl<P> Default for Namespace<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    fn default() -> Self {
        Self {
            elements: BoundedVec::default(), // BoundedVec is now Default
        }
    }
}

// Default for ExternType<P>
impl<P> Default for ExternType<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    fn default() -> Self {
        // Defaulting to a simple variant, e.g., TypeDef with a default TypeRef.
        // FuncType<P> would require P: Debug for its Default, which is specific.
        // TypeRef::none() or TypeRef::default() should be available.
        ExternType::TypeDef(TypeRef::default()) // Assuming TypeRef is Default
    }
}

// Default for ComponentAlias<P>
impl<P> Default for ComponentAlias<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    fn default() -> Self {
        // Preferring the more specific default from Outer variant if that's intended
        // Or, provide a more sensible default based on typical usage.
        // For now, let's assume Outer is a common default.
        Self::Outer(ComponentAliasOuter::default())
    }
}

// Default for ComponentAliasCoreInstanceExport
impl Default for ComponentAliasCoreInstanceExport {
    fn default() -> Self {
        Self {
            core_instance_idx: 0,
            name:              WasmName::default(), // Assuming WasmName has a suitable default
        }
    }
}

// Default for ComponentAliasInstanceExport
impl Default for ComponentAliasInstanceExport {
    fn default() -> Self {
        Self {
            instance_idx: 0,
            name:         WasmName::default(), // Assuming WasmName also has a sensible Default
            kind:         ComponentAliasExportKind::default(),
        }
    }
}

// Default for CoreModuleType<P>
// impl<P> Default for
// CoreModuleType<P> { fn default() -> Self {
// Self {
// imports: BoundedVec::default(),
// exports: BoundedVec::default(),
// }
// }
// }

// TODO: Implement Checksummable, ToBytes, FromBytes for CoreModuleType<P>

// impl<P> Default for
// InstanceType<P> { fn default() -> Self {
// Self {
// exports: BoundedVec::default(),
// }
// }
// }

#[cfg(feature = "std")]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::safe_memory::StdProvider; // For testing with a concrete provider

    #[test]
    fn namespace_from_str_works() {
        let provider = StdProvider::new(Vec::new()); // Example provider
        let ns = Namespace::from_str("wasi:filesystem/types", provider).unwrap();
        assert_eq!(ns.elements.len(), 3);
        assert_eq!(ns.elements.get(0).unwrap().as_str().unwrap(), "wasi");
        assert_eq!(ns.elements.get(1).unwrap().as_str().unwrap(), "filesystem");
        assert_eq!(ns.elements.get(2).unwrap().as_str().unwrap(), "types");
    }

    #[test]
    fn namespace_from_str_empty_parts() {
        let provider = StdProvider::new(Vec::new());
        let ns = Namespace::from_str("foo::bar", provider).unwrap(); // Handles empty part
        assert_eq!(ns.elements.len(), 2);
        assert_eq!(ns.elements.get(0).unwrap().as_str().unwrap(), "foo");
        assert_eq!(ns.elements.get(1).unwrap().as_str().unwrap(), "bar");
    }
}

// --- Implementations for Checksummable, ToBytes, FromBytes ---

// Helper macro for simple struct Checksummable
macro_rules! impl_checksummable_struct {
    ($type:ident < $( $lt:tt $( : $clt:tt $(+ $dlt:tt )* )? ),* > , P: $pbound:ident, $($field:ident),+) => {
        impl<P: $pbound + Default + Clone $(, $lt $( : $clt $(+ $dlt )* )? )* > Checksummable for $type<P $(, $lt)* > {
            fn update_checksum(&self, checksum: &mut crate::verification::Checksum) {
                $( self.$field.update_checksum(checksum); )+
            }
        }
    };
    ($type:ident < $( $lt:tt $( : $clt:tt $(+ $dlt:tt )* )? ),* > , $($field:ident),+) => {
        impl< $( $lt $( : $clt $(+ $dlt )* )? ),* > Checksummable for $type< $( $lt),* > {
            fn update_checksum(&self, checksum: &mut crate::verification::Checksum) {
                $( self.$field.update_checksum(checksum); )+
            }
        }
    };
     ($type:ident, $($field:ident),+) => {
        impl Checksummable for $type {
            fn update_checksum(&self, checksum: &mut crate::verification::Checksum) {
                $( self.$field.update_checksum(checksum); )+
            }
        }
    };
}

// Helper macro for simple struct ToBytes
macro_rules! impl_tobytes_struct {
    ($type:ident < $( $lt:tt $( : $clt:tt $(+ $dlt:tt )* )? ),* >, P: $pbound:ident, $($field:ident),+) => {
        impl<P: $pbound + Default + Clone $(, $lt $( : $clt $(+ $dlt )* )? )* > ToBytes for $type<P $(, $lt)* > {
            fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
                &self,
                writer: &mut WriteStream<'a>,
                provider: &PStream,
            ) -> wrt_error::Result<()> {
                $( self.$field.to_bytes_with_provider(writer, provider)?; )+
                Ok(())
            }
            // to_bytes is provided by the trait
        }
    };
     ($type:ident < $( $lt:tt $( : $clt:tt $(+ $dlt:tt )* )? ),* >, $($field:ident),+) => {
        impl< $( $lt $( : $clt $(+ $dlt )* )? ),* > ToBytes for $type< $( $lt),* > {
            fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
                &self,
                writer: &mut WriteStream<'a>,
                provider: &PStream,
            ) -> wrt_error::Result<()> {
                $( self.$field.to_bytes_with_provider(writer, provider)?; )+
                Ok(())
            }
            // to_bytes is provided by the trait
        }
    };
    ($type:ident, $($field:ident),+) => {
        impl ToBytes for $type {
            fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
                &self,
                writer: &mut WriteStream<'a>,
                provider: &PStream,
            ) -> wrt_error::Result<()> {
                $( self.$field.to_bytes_with_provider(writer, provider)?; )+
                Ok(())
            }
            // to_bytes is provided by the trait
        }
    };
}

// Helper macro for simple struct FromBytes
macro_rules! impl_frombytes_struct {
    ($type:ident < $( $lt:tt $( : $clt:tt $(+ $dlt:tt )* )? ),* >, P: $pbound:ident, $($field:ident: $fieldtype:ty),+) => {
        impl<P: $pbound + Default + Clone $(, $lt $( : $clt $(+ $dlt )* )? )* > FromBytes for $type<P $(, $lt)* > {
            fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
                reader: &mut ReadStream<'a>,
                provider: &PStream,
            ) -> wrt_error::Result<Self> {
                $(
                    let $field = <$fieldtype>::from_bytes_with_provider(reader, provider)?;
                )+
                Ok(Self { $($field,)+ })
            }
            // from_bytes is provided by the trait
        }
    };
    ($type:ident < $( $lt:tt $( : $clt:tt $(+ $dlt:tt )* )? ),* >, $($field:ident: $fieldtype:ty),+) => {
        impl< $( $lt $( : $clt $(+ $dlt )* )? ),* > FromBytes for $type< $( $lt),* > {
            fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
                reader: &mut ReadStream<'a>,
                provider: &PStream,
            ) -> wrt_error::Result<Self> {
                $(
                    let $field = <$fieldtype>::from_bytes_with_provider(reader, provider)?;
                )+
                Ok(Self { $($field,)+ })
            }
            // from_bytes is provided by the trait
        }
    };
     ($type:ident, $($field:ident: $fieldtype:ty),+) => {
        impl FromBytes for $type {
            fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
                reader: &mut ReadStream<'a>,
                provider: &PStream,
            ) -> wrt_error::Result<Self> {
                $(
                    let $field = <$fieldtype>::from_bytes_with_provider(reader, provider)?;
                )+
                Ok(Self { $($field,)+ })
            }
            // from_bytes is provided by the trait
        }
    };
}

// Import<P>
impl_checksummable_struct!(Import<P: MemoryProvider + Clone + Default + Eq + Debug>, key, ty);
impl_tobytes_struct!(Import<P: MemoryProvider + Clone + Default + Eq + Debug>, key, ty);
impl_frombytes_struct!(Import<P: MemoryProvider + Clone + Default + Eq + Debug>, key: ImportKey<P>, ty: ExternType<P>);

// Export<P>
impl<P> Checksummable for Export<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    fn update_checksum(&self, checksum: &mut crate::verification::Checksum) {
        self.name.update_checksum(checksum);
        self.ty.update_checksum(checksum);
        self.desc.update_checksum(checksum);
    }
}

impl<P> ToBytes for Export<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<()> {
        self.name.to_bytes_with_provider(writer, provider)?;
        self.ty.to_bytes_with_provider(writer, provider)?;
        self.desc.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
    // to_bytes is provided by the trait
}

impl<P> FromBytes for Export<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let name = WasmName::<MAX_NAME_LEN>::from_bytes_with_provider(reader, provider)?;
        let ty = ExternType::<P>::from_bytes_with_provider(reader, provider)?;
        let desc = Option::<WasmName<MAX_NAME_LEN>>::from_bytes_with_provider(reader, provider)?;
        Ok(Self { name, ty, desc })
    }
    // from_bytes is provided by the trait
}

// ImportKey<P>
impl_checksummable_struct!(ImportKey<P: MemoryProvider + Clone + Default + Eq + Debug>, namespace, name);
impl_tobytes_struct!(ImportKey<P: MemoryProvider + Clone + Default + Eq + Debug>, namespace, name);
impl_frombytes_struct!(ImportKey<P: MemoryProvider + Clone + Default + Eq + Debug>, namespace: Namespace<P>, name: WasmName<MAX_NAME_LEN>);

// Namespace<P>
impl_checksummable_struct!(Namespace<P: MemoryProvider + Clone + Default + Eq + Debug>, elements);
impl_tobytes_struct!(Namespace<P: MemoryProvider + Clone + Default + Eq + Debug>, elements);
impl_frombytes_struct!(Namespace<P: MemoryProvider + Clone + Default + Eq + Debug>,
    elements: BoundedVec<WasmName<MAX_NAME_LEN>, MAX_NAMESPACE_ELEMENTS, P>
);

// ExternType<P>
impl<P> Checksummable for ExternType<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    fn update_checksum(&self, checksum: &mut crate::verification::Checksum) {
        let discriminant_byte = match self {
            ExternType::Func(_) => 0u8,
            ExternType::Table(_) => 1u8,
            ExternType::Memory(_) => 2u8,
            ExternType::Global(_) => 3u8,
            ExternType::Tag(_) => 4u8,
            ExternType::Component(_) => 5u8,
            ExternType::Instance(_) => 6u8,
            ExternType::CoreModule(_) => 7u8,
            ExternType::TypeDef(_) => 8u8,
            ExternType::Resource(_) => 9u8,
        };
        discriminant_byte.update_checksum(checksum); // u8 implements Checksummable

        match self {
            ExternType::Func(t) => t.update_checksum(checksum),
            ExternType::Table(t) => t.update_checksum(checksum),
            ExternType::Memory(t) => t.update_checksum(checksum),
            ExternType::Global(t) => t.update_checksum(checksum),
            ExternType::Tag(t) => t.update_checksum(checksum),
            ExternType::Component(t) => t.update_checksum(checksum),
            ExternType::Instance(t) => t.update_checksum(checksum),
            ExternType::CoreModule(t) | ExternType::TypeDef(t) => t.update_checksum(checksum),
            ExternType::Resource(t) => t.update_checksum(checksum),
        }
    }
}

impl<P> ToBytes for ExternType<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<()> {
        match self {
            ExternType::Func(ft) => {
                writer.write_u8(0)?;
                ft.to_bytes_with_provider(writer, provider)?;
            },
            ExternType::Table(tt) => {
                writer.write_u8(1)?;
                tt.to_bytes_with_provider(writer, provider)?;
            },
            ExternType::Memory(mt) => {
                writer.write_u8(2)?;
                mt.to_bytes_with_provider(writer, provider)?;
            },
            ExternType::Global(gt) => {
                writer.write_u8(3)?;
                gt.to_bytes_with_provider(writer, provider)?;
            },
            ExternType::Tag(ty) => {
                // Was FuncType<P>
                writer.write_u8(4)?;
                ty.to_bytes_with_provider(writer, provider)?;
            },
            ExternType::Component(ct) => {
                writer.write_u8(5)?;
                ct.to_bytes_with_provider(writer, provider)?;
            },
            ExternType::Instance(it) => {
                writer.write_u8(6)?;
                it.to_bytes_with_provider(writer, provider)?;
            },
            ExternType::CoreModule(cmt) => {
                writer.write_u8(7)?;
                cmt.to_bytes_with_provider(writer, provider)?;
            },
            ExternType::TypeDef(tdt) => {
                // This is TypeRef
                writer.write_u8(8)?;
                tdt.to_bytes_with_provider(writer, provider)?;
            },
            ExternType::Resource(rt) => {
                // This is ResourceType<P>
                writer.write_u8(9)?;
                rt.to_bytes_with_provider(writer, provider)?;
            },
        }
        Ok(())
    }
}

impl<P> FromBytes for ExternType<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let variant_tag = reader.read_u8()?;
        match variant_tag {
            0 => Ok(Self::Func(FuncType::from_bytes_with_provider(
                reader, provider,
            )?)),
            1 => Ok(Self::Table(TableType::from_bytes_with_provider(
                reader, provider,
            )?)),
            2 => Ok(Self::Memory(MemoryType::from_bytes_with_provider(
                reader, provider,
            )?)),
            3 => Ok(Self::Global(GlobalType::from_bytes_with_provider(
                reader, provider,
            )?)),
            4 => Ok(Self::Tag(FuncType::from_bytes_with_provider(
                reader, provider,
            )?)),
            5 => Ok(Self::Component(ComponentType::<P>::from_bytes_with_provider(
                reader, provider,
            )?)),
            6 => Ok(Self::Instance(InstanceType::<P>::from_bytes_with_provider(
                reader, provider,
            )?)),
            7 => Ok(Self::CoreModule(TypeRef::from_bytes_with_provider(
                reader, provider,
            )?)),
            8 => Ok(Self::TypeDef(TypeRef::from_bytes_with_provider(
                reader, provider,
            )?)), // This is TypeRef
            9 => Ok(Self::Resource(ResourceType::<P>::from_bytes_with_provider(
                reader, provider,
            )?)), // This is ResourceType<P>
            _ => Err(Error::runtime_execution_error(
                "Invalid component type kind discriminant",
            )),
        }
    }
}

// CoreModuleType<P>
impl_checksummable_struct!(CoreModuleType<P: MemoryProvider + Clone + Default + Eq + Debug>, imports, exports);
impl_tobytes_struct!(CoreModuleType<P: MemoryProvider + Clone + Default + Eq + Debug>, imports, exports);
impl_frombytes_struct!(CoreModuleType<P: MemoryProvider + Clone + Default + Eq + Debug>,
    imports: BoundedVec<Import<P>, MAX_COMPONENT_IMPORTS, P>,
    exports: BoundedVec<Export<P>, MAX_COMPONENT_EXPORTS, P>
);

// InstanceType<P>
impl_checksummable_struct!(InstanceType<P: MemoryProvider + Clone + Default + Eq + Debug>, exports);
impl_tobytes_struct!(InstanceType<P: MemoryProvider + Clone + Default + Eq + Debug>, exports);
impl_frombytes_struct!(InstanceType<P: MemoryProvider + Clone + Default + Eq + Debug>, exports: BoundedVec<Export<P>, MAX_COMPONENT_EXPORTS, P>);

// ResourceType<P>
impl<P: MemoryProvider> Checksummable for ResourceType<P> {
    fn update_checksum(&self, checksum: &mut crate::verification::Checksum) {
        self.0.update_checksum(checksum); // u32 is Checksummable
    }
}

impl<P: MemoryProvider> ToBytes for ResourceType<P> {
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PStream, // Pass provider along to u32's method
    ) -> wrt_error::Result<()> {
        self.0.to_bytes_with_provider(writer, provider)
    }
    // to_bytes is provided by the trait
}

impl<P: MemoryProvider> FromBytes for ResourceType<P> {
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream, // Pass provider along to u32's method
    ) -> wrt_error::Result<Self> {
        let val = u32::from_bytes_with_provider(reader, provider)?;
        Ok(ResourceType(val, PhantomData))
    }
    // from_bytes is provided by the trait
}

// ComponentAliasExportKind
impl Checksummable for ComponentAliasExportKind {
    fn update_checksum(&self, checksum: &mut crate::verification::Checksum) {
        checksum.update(*self as u8);
    }
}

impl ToBytes for ComponentAliasExportKind {
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        writer.write_u8(*self as u8)
    }
}

impl FromBytes for ComponentAliasExportKind {
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let byte = reader.read_u8()?;
        match byte {
            0 => Ok(Self::Func),
            1 => Ok(Self::Table),
            2 => Ok(Self::Memory),
            3 => Ok(Self::Global),
            4 => Ok(Self::TypeDef),
            5 => Ok(Self::Resource),
            _ => Err(Error::new(
                ErrorCategory::Parse,
                codes::INVALID_VALUE,
                "Invalid enum value",
            )),
        }
    }
}

// ComponentAliasCoreInstanceExport
impl_checksummable_struct!(ComponentAliasCoreInstanceExport, core_instance_idx, name);
impl_tobytes_struct!(ComponentAliasCoreInstanceExport, core_instance_idx, name);
impl_frombytes_struct!(ComponentAliasCoreInstanceExport, core_instance_idx: u32, name: WasmName<MAX_NAME_LEN>);

// ComponentAliasInstanceExport
impl_checksummable_struct!(ComponentAliasInstanceExport, instance_idx, name, kind);
impl_tobytes_struct!(ComponentAliasInstanceExport, instance_idx, name, kind);
impl_frombytes_struct!(ComponentAliasInstanceExport, instance_idx: u32, name: WasmName<MAX_NAME_LEN>, kind: ComponentAliasExportKind);

// ComponentAlias<P>
impl<P> Checksummable for ComponentAlias<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    fn update_checksum(&self, checksum: &mut crate::verification::Checksum) {
        let discriminant_byte = match self {
            ComponentAlias::InstanceExport(_) => 0u8,
            ComponentAlias::CoreInstanceExport(_) => 1u8,
            ComponentAlias::Outer(_) => 2u8,
            ComponentAlias::_Phantom(_) => {
                // Safety: _Phantom variant should never be constructed
                panic!("_Phantom variant should never be instantiated")
            },
        };
        discriminant_byte.update_checksum(checksum);
        match self {
            ComponentAlias::InstanceExport(e) => e.update_checksum(checksum),
            ComponentAlias::CoreInstanceExport(e) => e.update_checksum(checksum),
            ComponentAlias::Outer(e) => e.update_checksum(checksum),
            ComponentAlias::_Phantom(_) => {
                // Safety: _Phantom variant should never be constructed
                panic!("_Phantom variant should never be instantiated")
            },
        }
    }
}

impl<P> ToBytes for ComponentAlias<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<()> {
        match self {
            ComponentAlias::InstanceExport(e) => {
                writer.write_u8(0)?;
                e.to_bytes_with_provider(writer, provider)?;
            },
            ComponentAlias::CoreInstanceExport(e) => {
                writer.write_u8(1)?;
                e.to_bytes_with_provider(writer, provider)?;
            },
            ComponentAlias::Outer(e) => {
                writer.write_u8(2)?;
                e.to_bytes_with_provider(writer, provider)?;
            },
            ComponentAlias::_Phantom(_) => {
                // PhantomData variant should never be instantiated
panic!("PhantomData variant should never be serialized")
            },
        }
        Ok(())
    }
    // to_bytes is provided by the trait
}

impl<P> FromBytes for ComponentAlias<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let variant_idx = reader.read_u8()?;
        match variant_idx {
            0 => {
                let inner =
                    ComponentAliasInstanceExport::from_bytes_with_provider(reader, provider)?;
                Ok(ComponentAlias::InstanceExport(inner))
            },
            1 => {
                let inner = ComponentAliasCoreInstanceExport::from_bytes_with_provider(
                    reader, provider,
                )?;
                Ok(ComponentAlias::CoreInstanceExport(inner))
            },
            2 => {
                let inner = ComponentAliasOuter::from_bytes_with_provider(reader, provider)?;
                Ok(ComponentAlias::Outer(inner))
            },
            _ => Err(Error::runtime_execution_error(
                "Invalid variant index for ComponentAlias<P>",
            )),
        }
    }
    // from_bytes is provided by the trait
}

// ComponentInstanceKind<P>
impl<P> Checksummable for ComponentInstanceKind<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    fn update_checksum(&self, checksum: &mut crate::verification::Checksum) {
        let discriminant_byte = match self {
            ComponentInstanceKind::Unknown => 0u8,
            ComponentInstanceKind::Instantiate { .. } => 1u8,
            ComponentInstanceKind::FromExports { .. } => 2u8,
        };
        discriminant_byte.update_checksum(checksum);

        match self {
            ComponentInstanceKind::Unknown => {}, // No data to checksum
            ComponentInstanceKind::Instantiate {
                component_idx,
                args,
            } => {
                component_idx.update_checksum(checksum);
                args.update_checksum(checksum);
            },
            ComponentInstanceKind::FromExports { exports } => {
                exports.update_checksum(checksum);
            },
        }
    }
}

impl<P> ToBytes for ComponentInstanceKind<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<()> {
        match self {
            ComponentInstanceKind::Unknown => {
                writer.write_u8(0)?;
            },
            ComponentInstanceKind::Instantiate {
                component_idx,
                args,
            } => {
                writer.write_u8(1)?;
                writer.write_u32_le(*component_idx)?;
                args.to_bytes_with_provider(writer, provider)?;
            },
            ComponentInstanceKind::FromExports { exports } => {
                writer.write_u8(2)?;
                exports.to_bytes_with_provider(writer, provider)?;
            },
        }
        Ok(())
    }
}

impl<P> FromBytes for ComponentInstanceKind<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let discriminant = reader.read_u8()?;
        match discriminant {
            0 => Ok(ComponentInstanceKind::Unknown),
            1 => {
                let component_idx = reader.read_u32_le()?;
                let args = BoundedVec::<ComponentInstantiationArg<P>, MAX_COMPONENT_IMPORTS, P>::from_bytes_with_provider(reader, provider)?;
                Ok(ComponentInstanceKind::Instantiate {
                    component_idx,
                    args,
                })
            },
            2 => {
                let exports =
                    BoundedVec::<Export<P>, MAX_COMPONENT_EXPORTS, P>::from_bytes_with_provider(
                        reader, provider,
                    )?;
                Ok(ComponentInstanceKind::FromExports { exports })
            },
            _ => Err(SerializationError::InvalidFormat.into()),
        }
    }
}

// ComponentInstance<P>
impl_checksummable_struct!(ComponentInstance<P: MemoryProvider + Clone + Default + Eq + Debug>, kind);
impl_tobytes_struct!(ComponentInstance<P: MemoryProvider + Clone + Default + Eq + Debug>, kind);
impl_frombytes_struct!(ComponentInstance<P: MemoryProvider + Clone + Default + Eq + Debug>, kind: ComponentInstanceKind<P>);

// ComponentType<P>
impl_checksummable_struct!(ComponentType<P: MemoryProvider + Clone + Default + Eq + Debug>, imports, exports, aliases, instances, core_instances, component_types, core_types);
impl_tobytes_struct!(ComponentType<P: MemoryProvider + Clone + Default + Eq + Debug>, imports, exports, aliases, instances, core_instances, component_types, core_types);
impl_frombytes_struct!(ComponentType<P: MemoryProvider + Clone + Default + Eq + Debug>,
    imports: BoundedVec<Import<P>, MAX_COMPONENT_IMPORTS, P>,
    exports: BoundedVec<Export<P>, MAX_COMPONENT_EXPORTS, P>,
    aliases: BoundedVec<ComponentAlias<P>, MAX_COMPONENT_ALIASES, P>,
    instances: BoundedVec<ComponentInstance<P>, MAX_COMPONENT_INSTANCES, P>,
    core_instances: BoundedVec<CoreInstance<P>, MAX_CORE_INSTANCES, P>,
    component_types: BoundedVec<TypeRef, MAX_COMPONENT_TYPES, P>,
    core_types: BoundedVec<CoreType, MAX_CORE_TYPES, P>
);

// CoreInstanceKind<P>
impl<P> Checksummable for CoreInstanceKind<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    fn update_checksum(&self, checksum: &mut crate::verification::Checksum) {
        let discriminant_byte = match self {
            CoreInstanceKind::Unknown => 0u8,
            CoreInstanceKind::Instantiate { .. } => 1u8,
            CoreInstanceKind::FromExports { .. } => 2u8,
        };
        discriminant_byte.update_checksum(checksum);

        match self {
            CoreInstanceKind::Unknown => {}, // No data to checksum
            CoreInstanceKind::Instantiate { module_idx, args } => {
                module_idx.update_checksum(checksum);
                args.update_checksum(checksum);
            },
            CoreInstanceKind::FromExports { exports } => {
                exports.update_checksum(checksum);
            },
        }
    }
}

impl<P> ToBytes for CoreInstanceKind<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<()> {
        match self {
            CoreInstanceKind::Unknown => {
                writer.write_u8(0)?;
            },
            CoreInstanceKind::Instantiate { module_idx, args } => {
                writer.write_u8(1)?;
                writer.write_u32_le(*module_idx)?;
                args.to_bytes_with_provider(writer, provider)?;
            },
            CoreInstanceKind::FromExports { exports } => {
                writer.write_u8(2)?;
                exports.to_bytes_with_provider(writer, provider)?;
            },
        }
        Ok(())
    }
}

impl<P> FromBytes for CoreInstanceKind<P>
where
    P: MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let discriminant = reader.read_u8()?;
        match discriminant {
            0 => Ok(CoreInstanceKind::Unknown),
            1 => {
                let module_idx = reader.read_u32_le()?;
                let args = BoundedVec::<CoreInstantiationArg<P>, MAX_COMPONENT_IMPORTS, P>::from_bytes_with_provider(reader, provider)?;
                Ok(CoreInstanceKind::Instantiate { module_idx, args })
            },
            2 => {
                let exports =
                    BoundedVec::<Export<P>, MAX_COMPONENT_EXPORTS, P>::from_bytes_with_provider(
                        reader, provider,
                    )?;
                Ok(CoreInstanceKind::FromExports { exports })
            },
            _ => Err(SerializationError::InvalidFormat.into()),
        }
    }
}

// CoreInstance<P>
impl_checksummable_struct!(CoreInstance<P: MemoryProvider + Clone + Default + Eq + Debug>, kind);
impl_tobytes_struct!(CoreInstance<P: MemoryProvider + Clone + Default + Eq + Debug>, kind);
impl_frombytes_struct!(CoreInstance<P: MemoryProvider + Clone + Default + Eq + Debug>, kind: CoreInstanceKind<P>);

// CoreType (no generic parameter after migration)
impl Checksummable for CoreType {
    fn update_checksum(&self, checksum: &mut crate::verification::Checksum) {
        let discriminant_byte = match self {
            CoreType::Unknown => 0u8,
            CoreType::Func(_) => 1u8,
            CoreType::Table(_) => 2u8,
            CoreType::Memory(_) => 3u8,
            CoreType::Global(_) => 4u8,
            CoreType::Tag(_) => 5u8, /* Assuming Tag is the last variant based on typical enum
                                      * layouts */
        };
        discriminant_byte.update_checksum(checksum);

        match self {
            CoreType::Unknown => {}, // No data
            CoreType::Func(ft) => ft.update_checksum(checksum),
            CoreType::Table(tt) => tt.update_checksum(checksum),
            CoreType::Memory(mt) => mt.update_checksum(checksum),
            CoreType::Global(gt) => gt.update_checksum(checksum),
            CoreType::Tag(t) => t.update_checksum(checksum),
        }
    }
}

impl ToBytes for CoreType {
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<()> {
        match self {
            CoreType::Unknown => {
                writer.write_u8(0)?;
            },
            CoreType::Func(ft) => {
                writer.write_u8(1)?;
                ft.to_bytes_with_provider(writer, provider)?;
            },
            CoreType::Table(tt) => {
                writer.write_u8(2)?;
                tt.to_bytes_with_provider(writer, provider)?;
            },
            CoreType::Memory(mt) => {
                writer.write_u8(3)?;
                mt.to_bytes_with_provider(writer, provider)?;
            },
            CoreType::Global(gt) => {
                writer.write_u8(4)?;
                gt.to_bytes_with_provider(writer, provider)?;
            },
            CoreType::Tag(tag_ft) => {
                // Assuming Tag is a FuncType variant for now
                writer.write_u8(5)?;
                tag_ft.to_bytes_with_provider(writer, provider)?;
            },
        }
        Ok(())
    }
}

impl FromBytes for CoreType {
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let discriminant = reader.read_u8()?;
        match discriminant {
            0 => Ok(CoreType::Unknown),
            1 => {
                let ft = FuncType::from_bytes_with_provider(reader, provider)?;
                Ok(CoreType::Func(ft))
            },
            2 => {
                let tt = TableType::from_bytes_with_provider(reader, provider)?;
                Ok(CoreType::Table(tt))
            },
            3 => {
                let mt = MemoryType::from_bytes_with_provider(reader, provider)?;
                Ok(CoreType::Memory(mt))
            },
            4 => {
                let gt = GlobalType::from_bytes_with_provider(reader, provider)?;
                Ok(CoreType::Global(gt))
            },
            5 => {
                let tag_ft = FuncType::from_bytes_with_provider(reader, provider)?;
                Ok(CoreType::Tag(tag_ft))
            },
            _ => Err(SerializationError::InvalidFormat.into()),
        }
    }
}

// ComponentAliasOuter (Manual Impls, replacing old macro calls for
// ComponentAliasOuter<P>)
impl Checksummable for ComponentAliasOuter {
    fn update_checksum(&self, checksum: &mut crate::verification::Checksum) {
        self.count.update_checksum(checksum);
        self.index.update_checksum(checksum);
        self.kind.update_checksum(checksum); // ComponentAliasOuterKind impls
                                             // Checksummable
    }
}
impl ToBytes for ComponentAliasOuter {
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<()> {
        self.count.to_bytes_with_provider(writer, provider)?;
        self.index.to_bytes_with_provider(writer, provider)?;
        self.kind.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
    // to_bytes is provided by the trait
}
impl FromBytes for ComponentAliasOuter {
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let count = u32::from_bytes_with_provider(reader, provider)?;
        let index = u32::from_bytes_with_provider(reader, provider)?;
        let kind = ComponentAliasOuterKind::from_bytes_with_provider(reader, provider)?;
        Ok(Self { count, index, kind })
    }
    // from_bytes is provided by the trait
}

// ComponentInstantiationArg<P>
// Note: P is only in PhantomData, so these impls don't actually use P
impl<P> Checksummable for ComponentInstantiationArg<P> {
    fn update_checksum(&self, checksum: &mut crate::verification::Checksum) {
        self.name.update_checksum(checksum);
        self.index.update_checksum(checksum);
        self.kind.update_checksum(checksum);
    }
}

impl<P> ToBytes for ComponentInstantiationArg<P> {
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<()> {
        self.name.to_bytes_with_provider(writer, provider)?;
        self.index.to_bytes_with_provider(writer, provider)?;
        self.kind.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl<P> FromBytes for ComponentInstantiationArg<P> {
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let name = WasmName::<MAX_NAME_LEN>::from_bytes_with_provider(reader, provider)?;
        let index = u32::from_bytes_with_provider(reader, provider)?;
        let kind = ExternKind::from_bytes_with_provider(reader, provider)?;
        Ok(Self {
            name,
            index,
            kind,
            _phantom: core::marker::PhantomData,
        })
    }
}

// CoreInstantiationArg<P>
// Note: P is only in PhantomData, so these impls don't actually use P
impl<P> Checksummable for CoreInstantiationArg<P> {
    fn update_checksum(&self, checksum: &mut crate::verification::Checksum) {
        self.name.update_checksum(checksum);
        self.index.update_checksum(checksum);
        self.kind.update_checksum(checksum);
    }
}

impl<P> ToBytes for CoreInstantiationArg<P> {
    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<()> {
        self.name.to_bytes_with_provider(writer, provider)?;
        self.index.to_bytes_with_provider(writer, provider)?;
        self.kind.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl<P> FromBytes for CoreInstantiationArg<P> {
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let name = WasmName::<MAX_NAME_LEN>::from_bytes_with_provider(reader, provider)?;
        let index = u32::from_bytes_with_provider(reader, provider)?;
        let kind = ExternKind::from_bytes_with_provider(reader, provider)?;
        Ok(Self {
            name,
            index,
            kind,
            _phantom: core::marker::PhantomData,
        })
    }
}
