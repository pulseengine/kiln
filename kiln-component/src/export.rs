//! Export implementation for the WebAssembly Component Model.
//!
//! This module provides the Export type for component exports.

use crate::bounded_component_infra::ComponentProvider;
use wrt_format::component::ExternType;
use wrt_foundation::{
    ExternType as RuntimeExternType, ToString,
    traits::{Checksummable, FromBytes, ToBytes},
};

#[cfg(feature = "std")]
use crate::components::component::ExternValue;
#[cfg(not(feature = "std"))]
use crate::components::component_no_std::ExternValue;

use crate::{prelude::*, type_conversion::bidirectional};

/// Export from a component
#[derive(Debug, Clone)]
pub struct Export {
    /// Export name
    pub name: String,
    /// Export type
    pub ty: ExternType,
    /// Export value
    pub value: ExternValue,
    /// Export kind (function, value, instance, type)
    pub kind: ExportKind,
    /// Export attributes
    pub attributes: HashMap<String, String>,
    /// Integrity hash for the export (if available)
    pub integrity_hash: Option<String>,
}

// Manual PartialEq implementation since component_no_std::ExternValue doesn't derive PartialEq
impl PartialEq for Export {
    fn eq(&self, other: &Self) -> bool {
        // Compare all fields except value since ExternValue may not implement PartialEq in no_std
        self.name == other.name
            && self.ty == other.ty
            && self.kind == other.kind
            && self.attributes == other.attributes
            && self.integrity_hash == other.integrity_hash
        // Note: value field is intentionally not compared
    }
}

impl Eq for Export {}

impl Default for Export {
    fn default() -> Self {
        #[cfg(feature = "std")]
        use crate::components::component::FunctionValue;
        #[cfg(not(feature = "std"))]
        use crate::components::component_no_std::FunctionValue;

        #[cfg(feature = "std")]
        let func_value = FunctionValue {
            ty: wrt_foundation::types::FuncType::new([], []).unwrap_or_default(),
            export_name: String::new(),
        };

        #[cfg(not(feature = "std"))]
        let func_value = {
            use wrt_foundation::bounded::MAX_WASM_NAME_LENGTH;
            use wrt_foundation::safe_memory::NoStdProvider;

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
            name: String::new(),
            ty: ExternType::Function {
                params: vec![],
                results: vec![],
            },
            value: ExternValue::Function(func_value),
            kind: ExportKind::Function { function_index: 0 },
            attributes: HashMap::new(),
            integrity_hash: None,
        }
    }
}

impl Checksummable for Export {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.name.as_bytes().update_checksum(checksum);
    }
}

impl ToBytes for Export {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        self.name.len().to_bytes_with_provider(writer, _provider)?;
        Ok(())
    }
}

impl FromBytes for Export {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        _reader: &mut wrt_foundation::traits::ReadStream<'a>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self::default())
    }
}

impl Export {
    /// Create a new export
    pub fn new(name: String, ty: ExternType, value: ExternValue) -> Self {
        // Derive kind from value
        let kind = match &value {
            ExternValue::Function(_) | ExternValue::Func(_) => {
                ExportKind::Function { function_index: 0 }
            },
            ExternValue::Memory(_) | ExternValue::Table(_) | ExternValue::Global(_) => {
                ExportKind::Value { value_index: 0 }
            },
            ExternValue::Trap(_) => ExportKind::Function { function_index: 0 }, // Trap is closest to function
        };
        Self {
            name,
            ty,
            value,
            kind,
            attributes: HashMap::new(),
            integrity_hash: None,
        }
    }

    /// Create a new export with attributes
    pub fn new_with_attributes(
        name: String,
        ty: ExternType,
        value: ExternValue,
        attributes: HashMap<String, String>,
    ) -> Self {
        // Derive kind from value
        let kind = match &value {
            ExternValue::Function(_) | ExternValue::Func(_) => {
                ExportKind::Function { function_index: 0 }
            },
            ExternValue::Memory(_) | ExternValue::Table(_) | ExternValue::Global(_) => {
                ExportKind::Value { value_index: 0 }
            },
            ExternValue::Trap(_) => ExportKind::Function { function_index: 0 }, // Trap is closest to function
        };
        Self {
            name,
            ty,
            value,
            kind,
            attributes,
            integrity_hash: None,
        }
    }

    /// Create a new export with integrity hash
    pub fn new_with_integrity(
        name: String,
        ty: ExternType,
        value: ExternValue,
        integrity_hash: String,
    ) -> Self {
        // Derive kind from value
        let kind = match &value {
            ExternValue::Function(_) | ExternValue::Func(_) => {
                ExportKind::Function { function_index: 0 }
            },
            ExternValue::Memory(_) | ExternValue::Table(_) | ExternValue::Global(_) => {
                ExportKind::Value { value_index: 0 }
            },
            ExternValue::Trap(_) => ExportKind::Function { function_index: 0 }, // Trap is closest to function
        };
        Self {
            name,
            ty,
            value,
            kind,
            attributes: HashMap::new(),
            integrity_hash: Some(integrity_hash),
        }
    }

    /// Check if the export has a specific attribute
    pub fn has_attribute(&self, name: &str) -> bool {
        self.attributes.contains_key(name)
    }

    /// Get the value of an attribute
    pub fn get_attribute(&self, name: &str) -> Option<&String> {
        self.attributes.get(name)
    }

    /// Set an attribute value
    pub fn set_attribute(&mut self, name: String, value: String) {
        self.attributes.insert(name, value);
    }

    /// Convert the export type to a runtime extern type
    pub fn to_runtime_type(&self) -> Result<RuntimeExternType<ComponentProvider>> {
        bidirectional::format_to_runtime_extern_type(&self.ty)
    }

    /// Create a new export from a RuntimeExternType
    pub fn from_runtime_type(
        name: String,
        ty: RuntimeExternType<ComponentProvider>,
        value: ExternValue,
    ) -> Result<Self> {
        let format_type = bidirectional::runtime_to_format_extern_type(&ty)?;
        Ok(Self::new(name, format_type, value))
    }
}
