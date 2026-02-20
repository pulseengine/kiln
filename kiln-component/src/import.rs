//! Import implementation for the WebAssembly Component Model.
//!
//! This module provides the Import type for component imports.

use wrt_format::component::ExternType;
use wrt_foundation::{
    // component::WrtComponentType, // Not available
    ExternType as RuntimeExternType,
    component::Namespace,
    traits::{Checksummable, FromBytes, ToBytes},
};

// Placeholder type for missing import
// WrtComponentType now exported from crate root
#[cfg(feature = "std")]
use crate::components::component::ExternValue;
#[cfg(not(feature = "std"))]
use crate::components::component_no_std::ExternValue;

use crate::{
    bounded_component_infra::ComponentProvider,
    // namespace::Namespace,
    prelude::*,
    type_conversion::bidirectional,
};

/// Type of import in the component model
#[derive(Debug, Clone)]
pub enum ImportType {
    /// Function import
    Function(WrtComponentType<ComponentProvider>),
    /// Value import (global, memory, table)
    Value(WrtComponentType<ComponentProvider>),
    /// Instance import
    Instance(WrtComponentType<ComponentProvider>),
    /// Type import
    Type(WrtComponentType<ComponentProvider>),
}

/// Import to a component
#[derive(Debug, Clone)]
pub struct Import {
    /// Import namespace
    pub namespace: Namespace<ComponentProvider>,
    /// Import name
    pub name: String,
    /// Import type
    pub import_type: ImportType,
    /// Legacy extern type for compatibility
    pub ty: ExternType,
    /// Import value (runtime representation)
    pub value: ExternValue,
}

impl Default for Import {
    fn default() -> Self {
        #[cfg(feature = "std")]
        use crate::components::component::FunctionValue;
        #[cfg(not(feature = "std"))]
        use crate::components::component_no_std::FunctionValue;

        use wrt_foundation::safe_memory::NoStdProvider;
        // Create a default Unit type with a provider
        let component_type = WrtComponentType::unit(NoStdProvider::<4096>::default())
            .unwrap_or_else(|_| panic!("Failed to create unit component type"));

        #[cfg(feature = "std")]
        let func_value = FunctionValue {
            ty: wrt_foundation::types::FuncType::new([], []).unwrap_or_default(),
            export_name: String::new(),
        };

        #[cfg(not(feature = "std"))]
        let func_value = {
            use wrt_foundation::bounded::MAX_WASM_NAME_LENGTH;

            // Create function type using Default
            let func_type = wrt_foundation::types::FuncType::default();

            // Create export name using BoundedString
            let export_name_provider = NoStdProvider::<512>::default();
            let export_name =
                wrt_foundation::BoundedString::<MAX_WASM_NAME_LENGTH>::from_str_truncate("")
                    .unwrap_or_else(|_| panic!("Failed to create default export name"));

            FunctionValue {
                ty: func_type,
                export_name,
            }
        };

        Self {
            namespace: Namespace::default(),
            name: String::new(),
            import_type: ImportType::Function(component_type),
            ty: ExternType::Function {
                params: vec![],
                results: vec![],
            },
            value: ExternValue::Function(func_value),
        }
    }
}

impl PartialEq for Import {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.namespace == other.namespace
    }
}

impl Eq for Import {}

impl Checksummable for Import {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.namespace.update_checksum(checksum);
        self.name.update_checksum(checksum);
    }
}

impl ToBytes for Import {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.namespace.to_bytes_with_provider(writer, provider)?;
        self.name.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for Import {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self::default())
    }
}

impl Import {
    /// Creates a new import
    pub fn new(
        namespace: Namespace<ComponentProvider>,
        name: String,
        ty: ExternType,
        value: ExternValue,
    ) -> Self {
        use wrt_foundation::safe_memory::NoStdProvider;
        // Default import type based on ExternType - this is a simplified mapping
        let import_type = ImportType::Function(
            WrtComponentType::unit(NoStdProvider::<4096>::default())
                .unwrap_or_else(|_| panic!("Failed to create unit component type")),
        );
        Self {
            namespace,
            name,
            import_type,
            ty,
            value,
        }
    }

    /// Creates a new import with explicit import type
    pub fn new_with_type(
        namespace: Namespace<ComponentProvider>,
        name: String,
        import_type: ImportType,
        ty: ExternType,
        value: ExternValue,
    ) -> Self {
        Self {
            namespace,
            name,
            import_type,
            ty,
            value,
        }
    }

    /// Creates an import identifier by combining namespace and name
    pub fn identifier(&self) -> String {
        // Namespace elements need to be joined
        #[cfg(feature = "std")]
        {
            let ns_parts: Vec<String> = self
                .namespace
                .elements
                .iter()
                .filter_map(|elem| elem.as_str().ok().map(|s| s.to_string()))
                .collect();
            let ns_str = ns_parts.join(":");
            if ns_str.is_empty() {
                self.name.clone()
            } else {
                format!("{}:{}", ns_str, self.name)
            }
        }
        #[cfg(not(feature = "std"))]
        {
            // In no_std, simplify to just the name if namespace is complex
            self.name.clone()
        }
    }

    /// Convert the import type to a runtime extern type
    pub fn to_runtime_type(&self) -> Result<RuntimeExternType<ComponentProvider>> {
        bidirectional::format_to_runtime_extern_type(&self.ty)
    }

    /// Create a new import from a RuntimeExternType
    pub fn from_runtime_type(
        namespace: Namespace<ComponentProvider>,
        name: String,
        ty: RuntimeExternType<ComponentProvider>,
        value: ExternValue,
    ) -> Result<Self> {
        let format_type = bidirectional::runtime_to_format_extern_type(&ty)?;
        Ok(Self::new(namespace, name, format_type, value))
    }
}

/// A collection of imports, organized by namespace
#[derive(Debug, Default)]
pub struct ImportCollection {
    imports: HashMap<String, Import>,
}

impl ImportCollection {
    /// Creates a new, empty import collection
    pub fn new() -> Self {
        Self {
            imports: HashMap::new(),
        }
    }

    /// Adds an import to the collection
    pub fn add(&mut self, import: Import) {
        let id = import.identifier();
        self.imports.insert(id, import);
    }

    /// Gets an import by its identifier
    pub fn get(&self, identifier: &str) -> Option<&Import> {
        self.imports.get(identifier)
    }

    /// Returns an iterator over all imports
    pub fn iter(&self) -> impl Iterator<Item = &Import> {
        self.imports.values()
    }

    /// Returns the number of imports
    pub fn len(&self) -> usize {
        self.imports.len()
    }

    /// Returns true if the collection is empty
    pub fn is_empty(&self) -> bool {
        self.imports.is_empty()
    }
}
