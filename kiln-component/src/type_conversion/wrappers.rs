//! Type wrapper implementations for Component Model types
//!
//! This module provides wrapper types for Component Model types to avoid
//! orphan rule violations when implementing conversions between different
//! representations.
//!
//! # Orphan Rules
//!
//! Rust's orphan rules prevent implementing traits for types from a different
//! crate unless the trait is defined in the current crate. This module defines
//! local wrapper types around external types to enable safe trait
//! implementations.
//!
//! # Examples
//!
//! ```
//! use wrt_component::type_conversion::wrappers::{
//!     FormatComponentType,
//!     RuntimeComponentType,
//! };
//! use wrt_format::component::ComponentTypeDefinition;
//! use wrt_foundation::component::ComponentType as TypesComponentType;
//!
//! // Create a wrapper around a runtime type
//! let rt_type = TypesComponentType {
//!     imports:   vec![],
//!     exports:   vec![],
//!     instances: vec![],
//! };
//! let wrapper = RuntimeComponentType::new(rt_type);
//!
//! // Get the underlying type
//! let inner_type = wrapper.into_inner();
//! ```

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{
    borrow::ToOwned,
    string::{String, ToString},
    vec::Vec,
};

// Additional imports
use wrt_error::{Error, Result};
use wrt_format::component::{ComponentTypeDefinition, ExternType as FormatExternType};
use wrt_foundation::{
    component::{ComponentType, ExternType as TypesExternType, InstanceType},
    safe_memory::NoStdProvider,
};

// For no_std, override prelude's bounded::BoundedVec with StaticVec
#[cfg(not(feature = "std"))]
use wrt_foundation::collections::StaticVec as BoundedVec;

use super::bidirectional::{
    IntoFormatType, IntoRuntimeType, format_to_runtime_extern_type, runtime_to_format_extern_type,
};

use crate::bounded_component_infra::ComponentProvider;

/// Helper function to convert Namespace<P> to String
fn namespace_to_string<P>(namespace: &wrt_foundation::component::Namespace<P>) -> Result<String>
where
    P: wrt_foundation::MemoryProvider + Clone + Default + Eq + core::fmt::Debug,
{
    let parts: Result<Vec<String>> = namespace
        .elements
        .iter()
        .map(|elem| {
            elem.as_str()
                .map(|s| s.to_string())
                .map_err(|_| Error::runtime_execution_error("Invalid namespace element"))
        })
        .collect();
    Ok(parts?.join(":"))
}

/// Wrapper around wrt_foundation::component::ComponentType
#[derive(Debug, Clone)]
pub struct RuntimeComponentType {
    /// The wrapped component type
    inner: ComponentType<NoStdProvider<4096>>,
}

/// Wrapper around wrt_format::component::ComponentTypeDefinition::Component
#[derive(Debug, Clone)]
pub struct FormatComponentType {
    /// The component imports
    pub imports: Vec<(String, String, FormatExternType)>,
    /// The component exports
    pub exports: Vec<(String, FormatExternType)>,
}

/// Wrapper around wrt_foundation::component::InstanceType
#[derive(Debug, Clone)]
pub struct RuntimeInstanceType {
    /// The wrapped instance type
    inner: InstanceType<NoStdProvider<4096>>,
}

/// Wrapper around wrt_format::component::ComponentTypeDefinition::Instance
#[derive(Debug, Clone)]
pub struct FormatInstanceType {
    /// The instance exports
    pub exports: Vec<(String, FormatExternType)>,
}

impl RuntimeComponentType {
    /// Create a new runtime component type wrapper
    pub fn new(component_type: ComponentType<NoStdProvider<4096>>) -> Self {
        Self {
            inner: component_type,
        }
    }

    /// Get the inner component type
    pub fn inner(&self) -> &ComponentType<NoStdProvider<4096>> {
        &self.inner
    }

    /// Consume the wrapper and return the inner component type
    pub fn into_inner(self) -> ComponentType<NoStdProvider<4096>> {
        self.inner
    }
}

impl From<ComponentType<NoStdProvider<4096>>> for RuntimeComponentType {
    fn from(component_type: ComponentType<NoStdProvider<4096>>) -> Self {
        Self::new(component_type)
    }
}

impl From<RuntimeComponentType> for ComponentType<NoStdProvider<4096>> {
    fn from(wrapper: RuntimeComponentType) -> Self {
        wrapper.into_inner()
    }
}

impl FormatComponentType {
    /// Create a new format component type wrapper
    pub fn new(
        imports: Vec<(String, String, FormatExternType)>,
        exports: Vec<(String, FormatExternType)>,
    ) -> Self {
        Self { imports, exports }
    }

    /// Convert to ComponentTypeDefinition
    pub fn to_component_type_definition(&self) -> ComponentTypeDefinition {
        ComponentTypeDefinition::Component {
            imports: self.imports.clone(),
            exports: self.exports.clone(),
        }
    }
}

impl From<ComponentTypeDefinition> for FormatComponentType {
    fn from(type_def: ComponentTypeDefinition) -> Self {
        match type_def {
            ComponentTypeDefinition::Component { imports, exports } => Self::new(imports, exports),
            _ => panic!("Expected Component type definition"),
        }
    }
}

impl RuntimeInstanceType {
    /// Create a new runtime instance type wrapper
    pub fn new(instance_type: InstanceType<NoStdProvider<4096>>) -> Self {
        Self {
            inner: instance_type,
        }
    }

    /// Get the inner instance type
    pub fn inner(&self) -> &InstanceType<NoStdProvider<4096>> {
        &self.inner
    }

    /// Consume the wrapper and return the inner instance type
    pub fn into_inner(self) -> InstanceType<NoStdProvider<4096>> {
        self.inner
    }
}

impl From<InstanceType<NoStdProvider<4096>>> for RuntimeInstanceType {
    fn from(instance_type: InstanceType<NoStdProvider<4096>>) -> Self {
        Self::new(instance_type)
    }
}

impl From<RuntimeInstanceType> for InstanceType<NoStdProvider<4096>> {
    fn from(wrapper: RuntimeInstanceType) -> Self {
        wrapper.into_inner()
    }
}

impl FormatInstanceType {
    /// Create a new format instance type wrapper
    pub fn new(exports: Vec<(String, FormatExternType)>) -> Self {
        Self { exports }
    }

    /// Convert to ComponentTypeDefinition
    pub fn to_component_type_definition(&self) -> ComponentTypeDefinition {
        ComponentTypeDefinition::Instance {
            exports: self.exports.clone(),
        }
    }
}

impl From<ComponentTypeDefinition> for FormatInstanceType {
    fn from(type_def: ComponentTypeDefinition) -> Self {
        match type_def {
            ComponentTypeDefinition::Instance { exports } => Self::new(exports),
            _ => panic!("Expected Instance type definition"),
        }
    }
}

// Bidirectional conversion implementations

impl TryFrom<RuntimeComponentType> for FormatComponentType {
    type Error = Error;

    fn try_from(runtime_type: RuntimeComponentType) -> Result<Self> {
        let runtime_type = runtime_type.into_inner();

        // Convert imports from Import<P> structs to tuples
        let imports_result: Result<Vec<(String, String, FormatExternType)>> = runtime_type
            .imports
            .into_iter()
            .map(|import| {
                let namespace = namespace_to_string(&import.key.namespace)?;
                let name = import
                    .key
                    .name
                    .as_str()
                    .map_err(|_| Error::runtime_execution_error("Invalid import name"))?
                    .to_owned();
                runtime_to_format_extern_type(&import.ty)
                    .map(|format_type| (namespace, name, format_type))
            })
            .collect();

        // Convert exports from Export<P> structs to tuples
        let exports_result: Result<Vec<(String, FormatExternType)>> = runtime_type
            .exports
            .into_iter()
            .map(|export| {
                let name = export
                    .name
                    .as_str()
                    .map_err(|_| Error::runtime_execution_error("Invalid export name"))?
                    .to_owned();
                runtime_to_format_extern_type(&export.ty).map(|format_type| (name, format_type))
            })
            .collect();

        Ok(Self {
            imports: imports_result?,
            exports: exports_result?,
        })
    }
}

impl TryFrom<FormatComponentType> for RuntimeComponentType {
    type Error = Error;

    fn try_from(format_type: FormatComponentType) -> Result<Self> {
        // Get a provider for creating the bounded structures
        #[cfg(feature = "std")]
        let provider = ComponentProvider::default();
        #[cfg(not(feature = "std"))]
        let provider = {
            use wrt_foundation::{CrateId, safe_managed_alloc};
            safe_managed_alloc!(4096, CrateId::Component)?
        };

        // Convert imports from tuples to Import<P> structs
        let mut import_vec: wrt_foundation::BoundedVec<
            wrt_foundation::Import<ComponentProvider>,
            128,
            ComponentProvider,
        > = wrt_foundation::BoundedVec::new(provider.clone())?;

        for (namespace, name, extern_type) in format_type.imports {
            let runtime_type = format_to_runtime_extern_type(&extern_type)?;
            let namespace_obj = wrt_foundation::Namespace::from_str(&namespace, provider.clone())?;
            let name_wasm = wrt_foundation::WasmName::try_from_str(&name)
                .map_err(|_| Error::runtime_execution_error("Invalid import name"))?;
            let import = wrt_foundation::Import {
                key: wrt_foundation::ImportKey {
                    namespace: namespace_obj,
                    name: name_wasm,
                },
                ty: runtime_type,
            };
            import_vec
                .push(import)
                .map_err(|_| Error::capacity_exceeded("Too many imports"))?;
        }

        // Convert exports from tuples to Export<P> structs
        let mut export_vec: wrt_foundation::BoundedVec<
            wrt_foundation::Export<ComponentProvider>,
            128,
            ComponentProvider,
        > = wrt_foundation::BoundedVec::new(provider.clone())?;

        for (name, extern_type) in format_type.exports {
            let runtime_type = format_to_runtime_extern_type(&extern_type)?;
            let name_wasm = wrt_foundation::WasmName::try_from_str(&name)
                .map_err(|_| Error::runtime_execution_error("Invalid export name"))?;
            let export = wrt_foundation::Export {
                name: name_wasm,
                ty: runtime_type,
                desc: None,
            };
            export_vec
                .push(export)
                .map_err(|_| Error::capacity_exceeded("Too many exports"))?;
        }

        // Create empty instances for now - can be enhanced in future
        let instances = wrt_foundation::BoundedVec::new(provider.clone())?;

        Ok(Self::new(ComponentType {
            imports: import_vec,
            exports: export_vec,
            aliases: wrt_foundation::BoundedVec::new(provider.clone())?,
            instances,
            core_instances: wrt_foundation::BoundedVec::new(provider.clone())?,
            component_types: wrt_foundation::BoundedVec::new(provider.clone())?,
            core_types: wrt_foundation::BoundedVec::new(provider.clone())?,
        }))
    }
}

impl TryFrom<RuntimeInstanceType> for FormatInstanceType {
    type Error = Error;

    fn try_from(runtime_type: RuntimeInstanceType) -> Result<Self> {
        let runtime_type = runtime_type.into_inner();

        // Convert exports from Export<P> structs to tuples
        let exports_result: Result<Vec<(String, FormatExternType)>> = runtime_type
            .exports
            .into_iter()
            .map(|export| {
                let name = export
                    .name
                    .as_str()
                    .map_err(|_| Error::runtime_execution_error("Invalid export name"))?
                    .to_owned();
                runtime_to_format_extern_type(&export.ty).map(|format_type| (name, format_type))
            })
            .collect();

        Ok(Self {
            exports: exports_result?,
        })
    }
}

impl TryFrom<FormatInstanceType> for RuntimeInstanceType {
    type Error = Error;

    fn try_from(format_type: FormatInstanceType) -> Result<Self> {
        // Get a provider for creating the bounded structures
        #[cfg(feature = "std")]
        let provider = ComponentProvider::default();
        #[cfg(not(feature = "std"))]
        let provider = {
            use wrt_foundation::{CrateId, safe_managed_alloc};
            safe_managed_alloc!(4096, CrateId::Component)?
        };

        // Convert exports from tuples to Export<P> structs
        let mut export_vec: wrt_foundation::BoundedVec<
            wrt_foundation::Export<ComponentProvider>,
            128,
            ComponentProvider,
        > = wrt_foundation::BoundedVec::new(provider.clone())?;

        for (name, extern_type) in format_type.exports {
            let runtime_type = format_to_runtime_extern_type(&extern_type)?;
            let name_wasm = wrt_foundation::WasmName::try_from_str(&name)
                .map_err(|_| Error::runtime_execution_error("Invalid export name"))?;
            let export = wrt_foundation::Export {
                name: name_wasm,
                ty: runtime_type,
                desc: None,
            };
            export_vec
                .push(export)
                .map_err(|_| Error::capacity_exceeded("Too many exports"))?;
        }

        Ok(Self::new(InstanceType {
            exports: export_vec,
        }))
    }
}

// Extension traits to make conversions easier to use

/// Trait for converting to RuntimeComponentType
pub trait IntoRuntimeComponentType {
    /// Convert to RuntimeComponentType
    fn into_runtime_component_type(self) -> Result<RuntimeComponentType>;
}

/// Trait for converting to FormatComponentType
pub trait IntoFormatComponentType {
    /// Convert to FormatComponentType
    fn into_format_component_type(self) -> Result<FormatComponentType>;
}

/// Trait for converting to RuntimeInstanceType
pub trait IntoRuntimeInstanceType {
    /// Convert to RuntimeInstanceType
    fn into_runtime_instance_type(self) -> Result<RuntimeInstanceType>;
}

/// Trait for converting to FormatInstanceType
pub trait IntoFormatInstanceType {
    /// Convert to FormatInstanceType
    fn into_format_instance_type(self) -> Result<FormatInstanceType>;
}

impl IntoRuntimeComponentType for FormatComponentType {
    fn into_runtime_component_type(self) -> Result<RuntimeComponentType> {
        self.try_into()
    }
}

impl IntoFormatComponentType for RuntimeComponentType {
    fn into_format_component_type(self) -> Result<FormatComponentType> {
        self.try_into()
    }
}

impl IntoRuntimeInstanceType for FormatInstanceType {
    fn into_runtime_instance_type(self) -> Result<RuntimeInstanceType> {
        self.try_into()
    }
}

impl IntoFormatInstanceType for RuntimeInstanceType {
    fn into_format_instance_type(self) -> Result<FormatInstanceType> {
        self.try_into()
    }
}

// Implementation for ComponentTypeDefinition conversion
impl TryFrom<ComponentTypeDefinition> for RuntimeComponentType {
    type Error = Error;

    fn try_from(type_def: ComponentTypeDefinition) -> Result<Self> {
        match type_def {
            ComponentTypeDefinition::Component { imports, exports } => {
                let format_type = FormatComponentType::new(imports, exports);
                format_type.try_into()
            },
            _ => Err(Error::validation_error("Error occurred")),
        }
    }
}

impl TryFrom<ComponentTypeDefinition> for RuntimeInstanceType {
    type Error = Error;

    fn try_from(type_def: ComponentTypeDefinition) -> Result<Self> {
        match type_def {
            ComponentTypeDefinition::Instance { exports } => {
                let format_type = FormatInstanceType::new(exports);
                format_type.try_into()
            },
            _ => Err(Error::validation_error("Error occurred")),
        }
    }
}
