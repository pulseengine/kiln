//! Component instantiation runtime logic.
//!
//! This module implements the runtime behavior for instantiating WebAssembly
//! components and core modules within components.

use wrt_error::{
    Error,
    ErrorCategory,
    Result,
};
use wrt_format::{
    component::{
        Component,
        CoreInstance,
        CoreInstanceExpr,
        CoreSort,
        Instance,
        InstanceExpr,
        Sort,
    },
    module::Module,
};
// Always use BoundedMap for HashMap to ensure trait compatibility
// alloc is imported in lib.rs with proper feature gates
use wrt_foundation::{
    safe_memory::NoStdProvider,
    BoundedMap,
};

use crate::bounded_runtime_infra::{
    create_runtime_provider,
    RuntimeProvider,
};

// Always use BoundedMap regardless of std/no_std to ensure serialization traits
type HashMap<K, V> = BoundedMap<K, V, 256, RuntimeProvider>;

// Use BoundedString for component names to ensure trait compatibility
use wrt_foundation::{
    budget_aware_provider::CrateId,
    safe_managed_alloc,
    BoundedString,
};
type ComponentString = BoundedString<256>;

/// Result of component instantiation
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Default)]
pub struct InstantiationResult {
    /// Instance handle
    pub handle:  u32,
    /// Exported items from the instance
    pub exports: HashMap<ComponentString, ExportedItem>,
}


impl wrt_foundation::traits::Checksummable for InstantiationResult {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.handle.update_checksum(checksum);
        self.exports.update_checksum(checksum);
    }
}

impl wrt_foundation::traits::ToBytes for InstantiationResult {
    fn serialized_size(&self) -> usize {
        4 + self.exports.serialized_size()
    }

    fn to_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &PStream,
    ) -> Result<()> {
        writer.write_u32_le(self.handle)?;
        self.exports.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for InstantiationResult {
    fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &PStream,
    ) -> Result<Self> {
        let handle = reader.read_u32_le()?;
        let exports = HashMap::from_bytes_with_provider(reader, provider)?;
        Ok(Self { handle, exports })
    }
}

/// An exported item from an instance
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportedItem {
    /// Core function
    CoreFunction(u32),
    /// Core table
    CoreTable(u32),
    /// Core memory
    CoreMemory(u32),
    /// Core global
    CoreGlobal(u32),
    /// Component function
    Function(u32),
    /// Component value
    Value(u32),
    /// Nested instance
    Instance(u32),
}

impl Default for ExportedItem {
    fn default() -> Self {
        ExportedItem::CoreFunction(0)
    }
}

impl wrt_foundation::traits::ToBytes for ExportedItem {
    fn serialized_size(&self) -> usize {
        5 // 1 byte discriminant + 4 bytes u32 value
    }

    fn to_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        _provider: &PStream,
    ) -> Result<()> {
        // Write discriminant
        let discriminant = match self {
            ExportedItem::CoreFunction(_) => 0u8,
            ExportedItem::CoreTable(_) => 1u8,
            ExportedItem::CoreMemory(_) => 2u8,
            ExportedItem::CoreGlobal(_) => 3u8,
            ExportedItem::Function(_) => 4u8,
            ExportedItem::Value(_) => 5u8,
            ExportedItem::Instance(_) => 6u8,
        };
        writer.write_u8(discriminant)?;

        // Write value
        let value = match self {
            ExportedItem::CoreFunction(v)
            | ExportedItem::CoreTable(v)
            | ExportedItem::CoreMemory(v)
            | ExportedItem::CoreGlobal(v)
            | ExportedItem::Function(v)
            | ExportedItem::Value(v)
            | ExportedItem::Instance(v) => *v,
        };
        writer.write_u32_le(value)?;

        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for ExportedItem {
    fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        _provider: &PStream,
    ) -> Result<Self> {
        let discriminant = reader.read_u8()?;
        let value = reader.read_u32_le()?;

        match discriminant {
            0 => Ok(ExportedItem::CoreFunction(value)),
            1 => Ok(ExportedItem::CoreTable(value)),
            2 => Ok(ExportedItem::CoreMemory(value)),
            3 => Ok(ExportedItem::CoreGlobal(value)),
            4 => Ok(ExportedItem::Function(value)),
            5 => Ok(ExportedItem::Value(value)),
            6 => Ok(ExportedItem::Instance(value)),
            _ => Err(wrt_error::Error::runtime_execution_error(
                "Runtime execution error",
            )),
        }
    }
}

impl wrt_foundation::traits::Checksummable for ExportedItem {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        let discriminant = match self {
            ExportedItem::CoreFunction(_) => 0u32,
            ExportedItem::CoreTable(_) => 1u32,
            ExportedItem::CoreMemory(_) => 2u32,
            ExportedItem::CoreGlobal(_) => 3u32,
            ExportedItem::Function(_) => 4u32,
            ExportedItem::Value(_) => 5u32,
            ExportedItem::Instance(_) => 6u32,
        };
        let value = match self {
            ExportedItem::CoreFunction(v)
            | ExportedItem::CoreTable(v)
            | ExportedItem::CoreMemory(v)
            | ExportedItem::CoreGlobal(v)
            | ExportedItem::Function(v)
            | ExportedItem::Value(v)
            | ExportedItem::Instance(v) => *v,
        };
        for byte in discriminant.to_le_bytes() {
            checksum.update(byte);
        }
        for byte in value.to_le_bytes() {
            checksum.update(byte);
        }
    }
}

/// Context for component instantiation
pub struct InstantiationContext {
    /// Next available instance handle
    next_instance_handle: u32,
    /// Registry of instantiated components
    instances:            HashMap<u32, InstantiationResult>,
    /// Core module instances
    core_instances:       HashMap<u32, CoreModuleInstance>,
}

/// Represents an instantiated core module
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Default)]
pub struct CoreModuleInstance {
    /// Module reference
    pub module_idx: u32,
    /// Imported items resolved during instantiation
    pub imports:    HashMap<ComponentString, u32>,
    /// Exported items from the module
    pub exports:    HashMap<ComponentString, ExportedItem>,
}


impl wrt_foundation::traits::Checksummable for CoreModuleInstance {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.module_idx.update_checksum(checksum);
        self.imports.update_checksum(checksum);
        self.exports.update_checksum(checksum);
    }
}

impl wrt_foundation::traits::ToBytes for CoreModuleInstance {
    fn serialized_size(&self) -> usize {
        4 + self.imports.serialized_size() + self.exports.serialized_size()
    }

    fn to_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &PStream,
    ) -> Result<()> {
        writer.write_u32_le(self.module_idx)?;
        self.imports.to_bytes_with_provider(writer, provider)?;
        self.exports.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for CoreModuleInstance {
    fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &PStream,
    ) -> Result<Self> {
        let module_idx = reader.read_u32_le()?;
        let imports = HashMap::from_bytes_with_provider(reader, provider)?;
        let exports = HashMap::from_bytes_with_provider(reader, provider)?;
        Ok(Self {
            module_idx,
            imports,
            exports,
        })
    }
}

/// Error during linking/instantiation
#[derive(Debug)]
pub enum LinkingError {
    /// Import not found
    ImportNotFound {
        /// Module name.
        module: ComponentString,
        /// Import name.
        name:   ComponentString,
    },
    /// Type mismatch during linking
    TypeMismatch {
        /// Expected type.
        expected: ComponentString,
        /// Actual type.
        actual:   ComponentString,
    },
    /// Circular dependency detected
    CircularDependency,
    /// Instance not found
    InstanceNotFound(u32),
}

impl From<LinkingError> for Error {
    fn from(err: LinkingError) -> Self {
        match err {
            LinkingError::ImportNotFound { module, name } => Error::new(
                ErrorCategory::Component,
                wrt_error::codes::COMPONENT_LINKING_ERROR,
                "Import not found",
            ),
            LinkingError::TypeMismatch { expected, actual } => {
                Error::type_error("Type mismatch during linking")
            },
            LinkingError::CircularDependency => {
                Error::runtime_execution_error("Circular dependency detected")
            },
            LinkingError::InstanceNotFound(idx) => Error::new(
                ErrorCategory::Component,
                wrt_error::codes::COMPONENT_LINKING_ERROR,
                "Instance not found",
            ),
        }
    }
}

/// Component instantiator - handles runtime instantiation of components
pub struct ComponentInstantiator {
    context: InstantiationContext,
}

impl ComponentInstantiator {
    /// Create a new component instantiator
    pub fn new() -> Self {
        let instances_provider =
            create_runtime_provider().unwrap_or_else(|_| RuntimeProvider::default());
        let core_provider =
            create_runtime_provider().unwrap_or_else(|_| RuntimeProvider::default());

        Self {
            context: InstantiationContext {
                next_instance_handle: 1,
                instances:            HashMap::new(instances_provider).unwrap_or_default(),
                core_instances:       HashMap::new(core_provider).unwrap_or_default(),
            },
        }
    }

    /// Instantiate a component
    pub fn instantiate_component(
        &mut self,
        component: &Component,
        imports: HashMap<ComponentString, ExportedItem>,
    ) -> Result<InstantiationResult> {
        // This is a placeholder implementation
        // Real implementation would:
        // 1. Validate imports match component requirements
        // 2. Create instances for all nested components
        // 3. Link all dependencies
        // 4. Initialize the component

        let handle = self.context.next_instance_handle;
        self.context.next_instance_handle += 1;

        let exports_provider =
            create_runtime_provider().unwrap_or_else(|_| RuntimeProvider::default());
        let result = InstantiationResult {
            handle,
            exports: HashMap::new(exports_provider).unwrap_or_default(),
        };

        self.context.instances.insert(handle, result.clone())?;

        let exports_provider =
            create_runtime_provider().unwrap_or_else(|_| RuntimeProvider::default());
        Ok(InstantiationResult {
            handle,
            exports: HashMap::new(exports_provider).unwrap_or_default(),
        })
    }

    /// Instantiate a core module within a component
    pub fn instantiate_core_module(
        &mut self,
        module: &Module,
        imports: HashMap<ComponentString, ExportedItem>,
    ) -> Result<CoreModuleInstance> {
        // Placeholder for core module instantiation
        // Real implementation would resolve imports and create runtime instance

        // Direct assignment since types already match
        let core_exports = imports;

        let imports_provider =
            create_runtime_provider().unwrap_or_else(|_| RuntimeProvider::default());
        Ok(CoreModuleInstance {
            module_idx: 0,
            imports:    HashMap::new(imports_provider).unwrap_or_default(), /* Core module
                                                                             * imports are empty
                                                                             * for now */
            exports:    core_exports,
        })
    }
}

/// Core module instantiator - handles runtime instantiation of core WebAssembly
/// modules
pub struct CoreModuleInstantiator {
    /// Module instances registry
    instances:        HashMap<u32, CoreModuleInstance>,
    /// Next instance ID
    next_instance_id: u32,
}

impl CoreModuleInstantiator {
    /// Create a new core module instantiator
    pub fn new() -> Self {
        let provider = create_runtime_provider().unwrap_or_else(|_| {
            // Fallback for initialization errors
            RuntimeProvider::default()
        });
        Self {
            instances:        HashMap::new(provider).unwrap_or_default(),
            next_instance_id: 1,
        }
    }

    /// Process a core instance definition
    pub fn process_core_instance(
        &mut self,
        instance: &CoreInstance,
        available_modules: &[Module],
    ) -> Result<u32> {
        match &instance.instance_expr {
            CoreInstanceExpr::ModuleReference {
                module_idx,
                arg_refs,
            } => {
                // Validate module index
                if *module_idx as usize >= available_modules.len() {
                    return Err(Error::runtime_execution_error("Module index out of bounds"));
                }

                // Create instance
                let instance_id = self.next_instance_id;
                self.next_instance_id += 1;

                let imports_provider =
                    create_runtime_provider().unwrap_or_else(|_| RuntimeProvider::default());
                let exports_provider =
                    create_runtime_provider().unwrap_or_else(|_| RuntimeProvider::default());

                let core_instance = CoreModuleInstance {
                    module_idx: *module_idx,
                    imports:    HashMap::new(imports_provider).unwrap_or_default(),
                    exports:    HashMap::new(exports_provider).unwrap_or_default(),
                };

                self.instances.insert(instance_id, core_instance)?;
                Ok(instance_id)
            },
            CoreInstanceExpr::InlineExports(exports) => {
                // Handle inline exports
                let instance_id = self.next_instance_id;
                self.next_instance_id += 1;

                let provider = create_runtime_provider()?;
                let mut export_map = HashMap::new(provider)?;
                for export in exports {
                    let item = match export.sort {
                        CoreSort::Function => ExportedItem::CoreFunction(export.idx),
                        CoreSort::Table => ExportedItem::CoreTable(export.idx),
                        CoreSort::Memory => ExportedItem::CoreMemory(export.idx),
                        CoreSort::Global => ExportedItem::CoreGlobal(export.idx),
                        _ => {
                            return Err(Error::new(
                                ErrorCategory::Component,
                                wrt_error::codes::COMPONENT_LINKING_ERROR,
                                "Unsupported export sort",
                            ));
                        },
                    };
                    // Convert String to ComponentString
                    let component_name =
                        ComponentString::from_str_truncate(&export.name)?;
                    export_map.insert(component_name, item)?;
                }

                let import_provider = create_runtime_provider()?;
                let imports = HashMap::new(import_provider)?;

                let core_instance = CoreModuleInstance {
                    module_idx: 0, // Inline exports don't have a module
                    imports,
                    exports: export_map,
                };

                self.instances.insert(instance_id, core_instance)?;
                Ok(instance_id)
            },
        }
    }
}

impl Default for ComponentInstantiator {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for CoreModuleInstantiator {
    fn default() -> Self {
        Self::new()
    }
}
