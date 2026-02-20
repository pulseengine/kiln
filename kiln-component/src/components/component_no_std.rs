// WRT - wrt-component
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! No-std compatible Component Model implementation for WebAssembly Runtime
//!
//! This module provides types and implementations for the WebAssembly Component
//! Model in a no_std environment.

// use wrt_decoder::component::decode::Component as DecodedComponent;
use wrt_error::{Error, ErrorCategory, Result, codes};
use wrt_format::component::ExternType;
use wrt_foundation::{
    MemoryProvider,
    bounded::{BoundedString, MAX_COMPONENT_TYPES, MAX_WASM_NAME_LENGTH},
    budget_aware_provider::CrateId,
    builtin::BuiltinType,
    collections::StaticVec as BoundedVec,
    component_value::ComponentValue,
    resource::ResourceOperation as FormatResourceOperation,
    safe_managed_alloc,
    safe_memory::NoStdProvider,
    types::ValueType,
    values::Value,
    verification::VerificationLevel,
};
// Import types from wrt-foundation instead of wrt-runtime
use wrt_foundation::types::{MemoryType, TableType};

// InstanceValue handles std/no_std internally now
use crate::instance::InstanceValue;
use crate::{
    bounded_component_infra::ComponentProvider,
    export::Export,
    import::Import,
    prelude::*,
    resources::{ResourceStrategyNoStd, ResourceTable},
};

// Type alias for component provider
// ComponentProvider removed - using capability-based allocation via safe_managed_alloc!

// Implement required traits for BoundedVec compatibility
use wrt_foundation::traits::{Checksummable, FromBytes, ReadStream, ToBytes, WriteStream};

// Macro to implement basic traits for complex types
macro_rules! impl_basic_traits {
    ($type:ty, $default_val:expr) => {
        impl Checksummable for $type {
            fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
                0u32.update_checksum(checksum);
            }
        }

        impl ToBytes for $type {
            fn to_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
                &self,
                _writer: &mut WriteStream<'a>,
                _provider: &PStream,
            ) -> wrt_error::Result<()> {
                Ok(())
            }
        }

        impl FromBytes for $type {
            fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
                _reader: &mut ReadStream<'a>,
                _provider: &PStream,
            ) -> wrt_error::Result<Self> {
                Ok($default_val)
            }
        }
    };
}

// Define types for resources, memories, tables, and function types
/// Type alias for function type
pub type FuncType = wrt_foundation::types::FuncType;
/// Type alias for global type
pub type GlobalType = wrt_foundation::types::GlobalType;
/// Type alias for memory limit (using Limits from foundation)
pub type MemoryLimit = wrt_foundation::types::Limits;

// Maximum sizes for bounded collections
/// Maximum table size
pub const MAX_TABLE_SIZE: usize = 1024;
/// Maximum memory size (16 MB)
pub const MAX_MEMORY_SIZE: usize = 16 * 1024 * 1024;
/// Maximum function reference size
pub const MAX_FUNCTION_REF_SIZE: usize = 64;

/// Represents an external value
#[derive(Debug, Clone)]
pub enum ExternValue {
    /// Function value
    Function(FunctionValue),
    /// Function value (alternative naming for compatibility)
    Func(FunctionValue),
    /// Table value
    Table(TableValue),
    /// Memory value
    Memory(MemoryValue),
    /// Global value
    Global(GlobalValue),
    /// Trap value
    Trap(BoundedString<MAX_WASM_NAME_LENGTH>),
}

/// Represents a function value
#[derive(Debug, Clone)]
pub struct FunctionValue {
    /// Function type
    pub ty: FuncType,
    /// Export name that this function refers to
    pub export_name: BoundedString<MAX_WASM_NAME_LENGTH>,
}

/// Represents a table value
#[derive(Debug, Clone)]
pub struct TableValue {
    /// Table type
    pub ty: TableType,
    /// Table instance - in no_std this is a bounded buffer
    pub table: BoundedVec<u32, MAX_TABLE_SIZE>,
}

/// Represents a memory value
#[derive(Debug, Clone)]
pub struct MemoryValue {
    /// Memory type
    pub ty: MemoryType,
    /// Memory instance
    pub memory: BoundedVec<u8, MAX_MEMORY_SIZE>,
    /// Memory access count
    pub access_count: u64,
    /// Debug name
    pub debug_name: Option<BoundedString<MAX_WASM_NAME_LENGTH>>,
}

impl MemoryValue {
    /// Creates a new memory value
    pub fn new(ty: MemoryType) -> Result<Self> {
        let memory = BoundedVec::new();
        Ok(Self {
            ty,
            memory,
            access_count: 0,
            debug_name: None,
        })
    }

    /// Creates a new memory value with a debug name
    pub fn new_with_name(ty: MemoryType, name: &str) -> Result<Self> {
        let memory = BoundedVec::new();
        let provider = safe_managed_alloc!(512, CrateId::Component)?;
        let debug_name = Some(BoundedString::try_from_str(name).map_err(|_| {
            Error::new(
                ErrorCategory::Parameter,
                codes::VALIDATION_ERROR,
                "Memory name too long",
            )
        })?);

        Ok(Self {
            ty,
            memory,
            access_count: 0,
            debug_name,
        })
    }

    /// Reads from memory
    pub fn read(&self, offset: u32, size: u32) -> Result<Vec<u8>> {
        let size_usize = size as usize;
        let offset_usize = offset as usize;

        // Bounds check
        if offset_usize + size_usize > self.memory.len() {
            return Err(Error::new(
                ErrorCategory::Memory,
                codes::MEMORY_ACCESS_ERROR,
                "Memory read out of bounds",
            ));
        }

        // Create a vec to hold the result
        let mut result = Vec::with_capacity(size_usize);
        for i in 0..size_usize {
            if let Some(&byte) = self.memory.get(offset_usize + i) {
                result.push(byte);
            } else {
                return Err(Error::new(
                    ErrorCategory::Memory,
                    codes::MEMORY_ACCESS_ERROR,
                    "Memory read failed",
                ));
            }
        }

        Ok(result)
    }

    /// Writes to memory
    pub fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<()> {
        let offset_usize = offset as usize;

        // Bounds check
        if offset_usize + bytes.len() > MAX_MEMORY_SIZE {
            return Err(Error::new(
                ErrorCategory::Memory,
                codes::MEMORY_ACCESS_ERROR,
                "Memory write out of bounds",
            ));
        }

        // Resize memory if needed
        if offset_usize + bytes.len() > self.memory.len() {
            let new_size = offset_usize + bytes.len();
            for _ in self.memory.len()..new_size {
                // Grow memory with zeros
                self.memory.push(0).map_err(|_| {
                    Error::new(
                        ErrorCategory::Memory,
                        codes::MEMORY_ACCESS_ERROR,
                        "Memory cannot be grown past maximum size",
                    )
                })?;
            }
        }

        // Write bytes to memory
        for (i, &byte) in bytes.iter().enumerate() {
            if let Some(mem_byte) = self.memory.get_mut(offset_usize + i) {
                *mem_byte = byte;
            } else {
                return Err(Error::new(
                    ErrorCategory::Memory,
                    codes::MEMORY_ACCESS_ERROR,
                    "Memory write failed",
                ));
            }
        }

        // Increment access count
        self.access_count += 1;

        Ok(())
    }

    /// Grows the memory by the given number of pages
    pub fn grow(&mut self, pages: u32) -> Result<u32> {
        let page_size = 64 * 1024; // 64KB pages per WebAssembly spec
        let prev_pages = self.size();
        let additional_bytes = (pages as usize) * page_size;

        // Check if growing would exceed max memory size
        if self.memory.len() + additional_bytes > MAX_MEMORY_SIZE {
            return Err(Error::new(
                ErrorCategory::Memory,
                codes::MEMORY_ACCESS_ERROR,
                "Memory cannot be grown past maximum size",
            ));
        }

        // Add zero bytes to grow memory
        for _ in 0..additional_bytes {
            self.memory.push(0).map_err(|_| {
                Error::new(
                    ErrorCategory::Memory,
                    codes::MEMORY_ACCESS_ERROR,
                    "Memory cannot be grown past maximum size",
                )
            })?;
        }

        Ok(prev_pages)
    }

    /// Gets the current size of the memory in pages
    pub fn size(&self) -> u32 {
        let page_size = 64 * 1024; // 64KB pages per WebAssembly spec
        (self.memory.len() / page_size) as u32
    }

    /// Gets the current size of the memory in bytes
    pub fn size_in_bytes(&self) -> usize {
        self.memory.len()
    }

    /// Gets the peak memory usage in bytes
    pub fn peak_usage(&self) -> usize {
        self.memory.len() // In this simple implementation, current size is peak
        // usage
    }

    /// Gets the number of memory accesses performed
    pub fn access_count(&self) -> u64 {
        self.access_count
    }

    /// Gets a debug name for this memory, if any
    pub fn debug_name(&self) -> Option<&str> {
        self.debug_name.as_ref().and_then(|s| s.as_str().ok())
    }

    /// Sets a debug name for this memory
    pub fn set_debug_name(&mut self, name: &str) {
        if let Ok(provider) = safe_managed_alloc!(512, CrateId::Component) {
            if let Ok(bounded_name) = BoundedString::try_from_str(name) {
                self.debug_name = Some(bounded_name);
            }
        }
    }

    /// Verifies the integrity of the memory
    pub fn verify_integrity(&self) -> Result<()> {
        // In this simple implementation, we just check that memory size doesn't exceed
        // max
        if self.memory.len() > MAX_MEMORY_SIZE {
            return Err(Error::new(
                ErrorCategory::Memory,
                codes::MEMORY_ACCESS_ERROR,
                "Memory size exceeds maximum",
            ));
        }
        Ok(())
    }
}

/// Represents a global value
#[derive(Debug, Clone)]
pub struct GlobalValue {
    /// Global type
    pub ty: GlobalType,
    /// Global value
    pub value: Value,
}

// Define constants for bounded collections
/// Maximum number of exports per component
pub const MAX_COMPONENT_EXPORTS: usize = 64;
/// Maximum number of imports per component
pub const MAX_COMPONENT_IMPORTS: usize = 64;
/// Maximum number of instances per component
pub const MAX_COMPONENT_INSTANCES: usize = 32;
/// Maximum number of linked components per component
pub const MAX_LINKED_COMPONENTS: usize = 16;
/// Maximum size for a binary WebAssembly module
pub const MAX_BINARY_SIZE: usize = 1024 * 1024; // 1 MB

/// No-std compatible component type
#[derive(Debug, Clone)]
pub struct WrtComponentType {
    /// Component imports
    pub imports: BoundedVec<
        (
            BoundedString<MAX_WASM_NAME_LENGTH>,
            BoundedString<MAX_WASM_NAME_LENGTH>,
            ExternType,
        ),
        MAX_COMPONENT_IMPORTS,
    >,
    /// Component exports
    pub exports:
        BoundedVec<(BoundedString<MAX_WASM_NAME_LENGTH>, ExternType), MAX_COMPONENT_EXPORTS>,
    /// Component instances
    pub instances:
        BoundedVec<wrt_format::component::ComponentTypeDefinition, MAX_COMPONENT_INSTANCES>,
    /// Verification level for this component type
    pub verification_level: VerificationLevel,
}

impl WrtComponentType {
    /// Creates a new empty component type
    pub fn new() -> Result<Self> {
        Ok(Self {
            imports: BoundedVec::new(),
            exports: BoundedVec::new(),
            instances: BoundedVec::new(),
            verification_level: VerificationLevel::Standard,
        })
    }

    /// Create a new empty component type
    pub fn empty() -> Result<Self> {
        Self::new()
    }

    /// Set the verification level for memory operations
    pub fn set_verification_level(&mut self, level: VerificationLevel) {
        self.verification_level = level;
    }

    /// Get the current verification level
    pub fn verification_level(&self) -> VerificationLevel {
        self.verification_level
    }

    /// Add an import to the component type
    pub fn add_import(&mut self, namespace: &str, name: &str, ty: ExternType) -> Result<()> {
        // Create bounded strings
        let provider1 = safe_managed_alloc!(512, CrateId::Component)?;
        let bounded_namespace = BoundedString::try_from_str(namespace).map_err(|_| {
            Error::new(
                ErrorCategory::Parameter,
                codes::VALIDATION_ERROR,
                "Import namespace too long",
            )
        })?;

        let provider2 = safe_managed_alloc!(512, CrateId::Component)?;
        let bounded_name = BoundedString::try_from_str(name).map_err(|_| {
            Error::new(
                ErrorCategory::Parameter,
                codes::VALIDATION_ERROR,
                "Import name too long",
            )
        })?;

        // Add to imports list
        self.imports.push((bounded_namespace, bounded_name, ty)).map_err(|_| {
            Error::new(
                ErrorCategory::Capacity,
                codes::CAPACITY_EXCEEDED,
                "Maximum number of imports exceeded",
            )
        })?;

        Ok(())
    }

    /// Add an export to the component type
    pub fn add_export(&mut self, name: &str, ty: ExternType) -> Result<()> {
        // Create bounded string
        let provider = safe_managed_alloc!(512, CrateId::Component)?;
        let bounded_name = BoundedString::try_from_str(name).map_err(|_| {
            Error::new(
                ErrorCategory::Parameter,
                codes::VALIDATION_ERROR,
                "Export name too long",
            )
        })?;

        // Add to exports list
        self.exports.push((bounded_name, ty)).map_err(|_| {
            Error::new(
                ErrorCategory::Capacity,
                codes::CAPACITY_EXCEEDED,
                "Maximum number of exports exceeded",
            )
        })?;

        Ok(())
    }

    /// Add an instance to the component type
    pub fn add_instance(
        &mut self,
        instance: wrt_format::component::ComponentTypeDefinition,
    ) -> Result<()> {
        self.instances.push(instance).map_err(|_| {
            Error::new(
                ErrorCategory::Capacity,
                codes::CAPACITY_EXCEEDED,
                "Maximum number of instances exceeded",
            )
        })?;

        Ok(())
    }
}

impl Default for WrtComponentType {
    fn default() -> Self {
        Self::new()
            .unwrap_or_else(|_| panic!("Failed to allocate memory for WrtComponentType::default"))
    }
}

/// Builder for WrtComponentType
pub struct WrtComponentTypeBuilder {
    /// Component imports
    imports: Vec<(String, String, ExternType)>,
    /// Component exports
    exports: Vec<(String, ExternType)>,
    /// Component instances
    instances: Vec<wrt_format::component::ComponentTypeDefinition>,
    /// Verification level for this component type
    verification_level: VerificationLevel,
}

impl WrtComponentTypeBuilder {
    /// Creates a new component type builder
    pub fn new() -> Self {
        Self {
            imports: Vec::new(),
            exports: Vec::new(),
            instances: Vec::new(),
            verification_level: VerificationLevel::Standard,
        }
    }

    /// Add an import to the component type
    pub fn with_import(mut self, namespace: &str, name: &str, ty: ExternType) -> Self {
        self.imports.push((namespace.to_string(), name.to_string(), ty));
        self
    }

    /// Add multiple imports to the component type
    pub fn with_imports(mut self, imports: Vec<(String, String, ExternType)>) -> Self {
        self.imports.extend(imports);
        self
    }

    /// Add an export to the component type
    pub fn with_export(mut self, name: &str, ty: ExternType) -> Self {
        self.exports.push((name.to_string(), ty));
        self
    }

    /// Add multiple exports to the component type
    pub fn with_exports(mut self, exports: Vec<(String, ExternType)>) -> Self {
        self.exports.extend(exports);
        self
    }

    /// Add an instance to the component type
    pub fn with_instance(
        mut self,
        instance: wrt_format::component::ComponentTypeDefinition,
    ) -> Self {
        self.instances.push(instance);
        self
    }

    /// Add multiple instances to the component type
    pub fn with_instances(
        mut self,
        instances: Vec<wrt_format::component::ComponentTypeDefinition>,
    ) -> Self {
        self.instances.extend(instances);
        self
    }

    /// Set the verification level for memory operations
    pub fn with_verification_level(mut self, level: VerificationLevel) -> Self {
        self.verification_level = level;
        self
    }

    /// Build the component type
    pub fn build(self) -> Result<WrtComponentType> {
        let mut component_type = WrtComponentType::new()?;
        component_type.verification_level = self.verification_level;

        // Add imports
        for (namespace, name, ty) in self.imports {
            component_type.add_import(&namespace, &name, ty)?;
        }

        // Add exports
        for (name, ty) in self.exports {
            component_type.add_export(&name, ty)?;
        }

        // Add instances
        for instance in self.instances {
            component_type.add_instance(instance)?;
        }

        Ok(component_type)
    }
}

impl Default for WrtComponentTypeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Struct representing built-in dependencies
#[derive(Debug, Clone)]
pub struct BuiltinRequirements {
    /// List of required builtins
    pub required: BoundedVec<BuiltinType, MAX_COMPONENT_TYPES>,
    /// Map of required builtin instances
    pub instances:
        BoundedVec<(BoundedString<MAX_WASM_NAME_LENGTH>, BuiltinType), MAX_COMPONENT_INSTANCES>,
}

impl Default for BuiltinRequirements {
    fn default() -> Self {
        Self {
            required: BoundedVec::new(),
            instances: BoundedVec::new(),
        }
    }
}

/// Runtime instance type for no_std
#[derive(Debug, Clone)]
pub struct RuntimeInstance {
    /// Functions exported by this runtime
    functions:
        BoundedVec<(BoundedString<MAX_WASM_NAME_LENGTH>, ExternValue), MAX_COMPONENT_EXPORTS>,
    /// Memory exported by this runtime
    memories: BoundedVec<(BoundedString<MAX_WASM_NAME_LENGTH>, MemoryValue), MAX_COMPONENT_EXPORTS>,
    /// Tables exported by this runtime
    tables: BoundedVec<(BoundedString<MAX_WASM_NAME_LENGTH>, TableValue), MAX_COMPONENT_EXPORTS>,
    /// Globals exported by this runtime
    globals: BoundedVec<(BoundedString<MAX_WASM_NAME_LENGTH>, GlobalValue), MAX_COMPONENT_EXPORTS>,
    /// Verification level for memory operations
    verification_level: VerificationLevel,
}

/// Type alias for component instance compatibility
pub type ComponentInstance = RuntimeInstance;

impl RuntimeInstance {
    /// Creates a new runtime instance
    pub fn new() -> Result<Self> {
        let functions_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let memories_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let tables_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let globals_provider = safe_managed_alloc!(65536, CrateId::Component)?;

        Ok(Self {
            functions: BoundedVec::new(),
            memories: BoundedVec::new(),
            tables: BoundedVec::new(),
            globals: BoundedVec::new(),
            verification_level: VerificationLevel::Standard,
        })
    }

    /// Register an exported function
    pub fn register_function(&mut self, name: &str, function: ExternValue) -> Result<()> {
        if let ExternValue::Function(_) = &function {
            let provider = safe_managed_alloc!(512, CrateId::Component)?;
            let bounded_name = BoundedString::try_from_str(name).map_err(|_| {
                Error::new(
                    ErrorCategory::Parameter,
                    codes::VALIDATION_ERROR,
                    "Function name too long",
                )
            })?;

            self.functions.push((bounded_name, function)).map_err(|_| {
                Error::new(
                    ErrorCategory::Capacity,
                    codes::CAPACITY_EXCEEDED,
                    "Maximum number of functions exceeded",
                )
            })?;

            Ok(())
        } else {
            Err(Error::new(
                ErrorCategory::Validation,
                codes::VALIDATION_ERROR,
                "Expected function value",
            ))
        }
    }

    /// Register an exported memory
    pub fn register_memory(&mut self, name: &str, memory: MemoryValue) -> Result<()> {
        let provider = safe_managed_alloc!(512, CrateId::Component)?;
        let bounded_name = BoundedString::try_from_str(name).map_err(|_| {
            Error::new(
                ErrorCategory::Parameter,
                codes::VALIDATION_ERROR,
                "Memory name too long",
            )
        })?;

        self.memories.push((bounded_name, memory)).map_err(|_| {
            Error::new(
                ErrorCategory::Capacity,
                codes::CAPACITY_EXCEEDED,
                "Maximum number of memories exceeded",
            )
        })?;

        Ok(())
    }

    /// Get a function by name
    pub fn get_function(&self, name: &str) -> Option<&ExternValue> {
        self.functions
            .iter()
            .find(|(n, _)| n.as_str().ok() == Some(name))
            .map(|(_, f)| f)
    }

    /// Get a memory by name
    pub fn get_memory(&self, name: &str) -> Option<&MemoryValue> {
        self.memories
            .iter()
            .find(|(n, _)| n.as_str().ok() == Some(name))
            .map(|(_, m)| m)
    }

    /// Set the verification level
    pub fn set_verification_level(&mut self, level: VerificationLevel) {
        self.verification_level = level;
    }
}

impl Default for RuntimeInstance {
    fn default() -> Self {
        Self::new()
            .unwrap_or_else(|_| panic!("Failed to allocate memory for RuntimeInstance::default"))
    }
}

/// Represents a WebAssembly component (no_std version)
#[derive(Debug, Clone)]
pub struct Component {
    /// Component type
    pub component_type: WrtComponentType,
    /// Component exports
    pub exports: BoundedVec<Export, MAX_COMPONENT_EXPORTS>,
    /// Component imports
    pub imports: BoundedVec<Import, MAX_COMPONENT_IMPORTS>,
    /// Component instances
    pub instances: BoundedVec<InstanceValue, MAX_COMPONENT_INSTANCES>,
    /// Linked components with their namespaces (names and component IDs)
    pub linked_components:
        BoundedVec<(BoundedString<MAX_WASM_NAME_LENGTH>, usize), MAX_LINKED_COMPONENTS>,
    /// Runtime instance
    pub runtime: Option<RuntimeInstance>,
    /// Resource table for managing component resources
    pub resource_table: ResourceTable,
    /// Built-in requirements
    pub built_in_requirements: Option<BuiltinRequirements>,
    /// Original binary
    pub original_binary: Option<BoundedVec<u8, MAX_BINARY_SIZE>>,
    /// Verification level for all operations
    pub verification_level: VerificationLevel,
}

impl Component {
    /// Creates a new, empty component instance
    pub fn new() -> Result<Self> {
        let exports_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let imports_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let instances_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let linked_components_provider = safe_managed_alloc!(65536, CrateId::Component)?;

        Ok(Self {
            component_type: WrtComponentType::new()?,
            exports: BoundedVec::new(),
            imports: BoundedVec::new(),
            instances: BoundedVec::new(),
            linked_components: BoundedVec::new(),
            runtime: None,
            resource_table: ResourceTable::new()?,
            built_in_requirements: None,
            original_binary: None,
            verification_level: VerificationLevel::Standard,
        })
    }

    /// Creates a new, empty component instance with an explicit resource table
    pub fn new_with_resource_table(resource_table: ResourceTable) -> Result<Self> {
        let exports_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let imports_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let instances_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let linked_components_provider = safe_managed_alloc!(65536, CrateId::Component)?;

        Ok(Self {
            component_type: WrtComponentType::new()?,
            exports: BoundedVec::new(),
            imports: BoundedVec::new(),
            instances: BoundedVec::new(),
            linked_components: BoundedVec::new(),
            runtime: None,
            resource_table,
            built_in_requirements: None,
            original_binary: None,
            verification_level: VerificationLevel::Standard,
        })
    }

    /// Creates a new component from a binary WebAssembly module
    pub fn from_binary(binary: &[u8]) -> Result<Self> {
        // This is a placeholder implementation
        // In a real implementation, we would parse the binary and create a component
        Err(Error::runtime_not_implemented(
            "Component::from_binary is not implemented for no_std environment",
        ))
    }

    /// Creates a new component from a type definition
    pub fn from_type(component_type: WrtComponentType) -> Result<Self> {
        let exports_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let imports_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let instances_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let linked_components_provider = safe_managed_alloc!(65536, CrateId::Component)?;

        Ok(Self {
            component_type,
            exports: BoundedVec::new(),
            imports: BoundedVec::new(),
            instances: BoundedVec::new(),
            linked_components: BoundedVec::new(),
            runtime: None,
            resource_table: ResourceTable::new()?,
            built_in_requirements: None,
            original_binary: None,
            verification_level: VerificationLevel::Standard,
        })
    }

    /// Add an export to the component
    pub fn add_export(&mut self, export: Export) -> Result<()> {
        self.exports.push(export).map_err(|_| {
            Error::new(
                ErrorCategory::Capacity,
                codes::CAPACITY_EXCEEDED,
                "Maximum number of exports exceeded",
            )
        })?;

        Ok(())
    }

    /// Add a function export to the component
    pub fn add_function(&mut self, func: Export) -> Result<()> {
        self.add_export(func)
    }

    /// Add a memory export to the component
    pub fn add_memory(&mut self, memory: Export) -> Result<()> {
        self.add_export(memory)
    }

    /// Add a table export to the component
    pub fn add_table(&mut self, table: Export) -> Result<()> {
        self.add_export(table)
    }

    /// Add a global export to the component
    pub fn add_global(&mut self, global: Export) -> Result<()> {
        self.add_export(global)
    }

    /// Add an import to the component
    pub fn add_import(&mut self, import: Import) -> Result<()> {
        self.imports.push(import).map_err(|_| {
            Error::new(
                ErrorCategory::Capacity,
                codes::CAPACITY_EXCEEDED,
                "Maximum number of imports exceeded",
            )
        })?;

        Ok(())
    }

    /// Add an instance to the component
    pub fn add_instance(&mut self, instance: InstanceValue) -> Result<()> {
        self.instances.push(instance).map_err(|_| {
            Error::new(
                ErrorCategory::Capacity,
                codes::CAPACITY_EXCEEDED,
                "Maximum number of instances exceeded",
            )
        })?;

        Ok(())
    }

    /// Link a component with a namespace
    pub fn link_component(&mut self, name: &str, component_id: usize) -> Result<()> {
        let provider = safe_managed_alloc!(512, CrateId::Component)?;
        let bounded_name = BoundedString::try_from_str(name).map_err(|_| {
            Error::new(
                ErrorCategory::Parameter,
                codes::VALIDATION_ERROR,
                "Component name too long",
            )
        })?;

        self.linked_components.push((bounded_name, component_id)).map_err(|_| {
            Error::new(
                ErrorCategory::Capacity,
                codes::CAPACITY_EXCEEDED,
                "Maximum number of linked components exceeded",
            )
        })?;

        Ok(())
    }

    /// Set the runtime instance
    pub fn set_runtime(&mut self, runtime: RuntimeInstance) {
        self.runtime = Some(runtime);
    }

    /// Set the verification level
    pub fn set_verification_level(&mut self, level: VerificationLevel) {
        self.verification_level = level;
    }

    /// Get an export by name
    pub fn get_export(&self, name: &str) -> Option<&Export> {
        self.exports.iter().find(|export| export.name == name)
    }

    /// Get an import by namespace and name
    pub fn get_import(&self, namespace: &str, name: &str) -> Option<&Import> {
        self.imports.iter().find(|import| {
            // Compare namespace by converting to string representation
            // Create owned strings to avoid borrowing issues
            let namespace_parts: Vec<String> = import
                .namespace
                .elements
                .iter()
                .filter_map(|elem| elem.as_str().ok().map(|s| s.to_string()))
                .collect();
            let ns_str = namespace_parts.join(":");
            ns_str == namespace && import.name == name
        })
    }

    /// Get an instance by name
    pub fn get_instance(&self, name: &str) -> Option<&InstanceValue> {
        self.instances.iter().find(|instance| {
            #[cfg(feature = "std")]
            {
                instance.name.as_str() == name
            }
            #[cfg(not(feature = "std"))]
            {
                instance.name.as_str().ok().as_deref() == Some(name)
            }
        })
    }

    /// Creates an empty component
    pub fn empty() -> Result<Self> {
        Self::new()
    }
}

impl Default for Component {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| panic!("Failed to allocate memory for Component::default"))
    }
}

/// Builder for Component in no_std environment
pub struct ComponentBuilder {
    /// Component type
    component_type: Option<WrtComponentType>,
    /// Component exports
    exports: Vec<Export>,
    /// Component imports
    imports: Vec<Import>,
    /// Component instances
    instances: Vec<InstanceValue>,
    /// Linked components with their namespaces (identifier by index)
    linked_components: Vec<(String, usize)>,
    /// Runtime instance
    runtime: Option<RuntimeInstance>,
    /// Resource table for managing component resources
    resource_table: Option<ResourceTable>,
    /// Built-in requirements
    built_in_requirements: Option<BuiltinRequirements>,
    /// Original binary
    original_binary: Option<Vec<u8>>,
    /// Verification level for all operations
    verification_level: VerificationLevel,
}

impl ComponentBuilder {
    /// Creates a new component builder
    pub fn new() -> Self {
        Self {
            component_type: None,
            exports: Vec::new(),
            imports: Vec::new(),
            instances: Vec::new(),
            linked_components: Vec::new(),
            runtime: None,
            resource_table: None,
            built_in_requirements: None,
            original_binary: None,
            verification_level: VerificationLevel::Standard,
        }
    }

    /// Set the component type
    pub fn with_component_type(mut self, component_type: WrtComponentType) -> Self {
        self.component_type = Some(component_type);
        self
    }

    /// Add an export to the component
    pub fn with_export(mut self, export: Export) -> Self {
        self.exports.push(export);
        self
    }

    /// Add multiple exports to the component
    pub fn with_exports(mut self, exports: Vec<Export>) -> Self {
        self.exports.extend(exports);
        self
    }

    /// Add an import to the component
    pub fn with_import(mut self, import: Import) -> Self {
        self.imports.push(import);
        self
    }

    /// Add multiple imports to the component
    pub fn with_imports(mut self, imports: Vec<Import>) -> Self {
        self.imports.extend(imports);
        self
    }

    /// Add an instance to the component
    pub fn with_instance(mut self, instance: InstanceValue) -> Self {
        self.instances.push(instance);
        self
    }

    /// Add multiple instances to the component
    pub fn with_instances(mut self, instances: Vec<InstanceValue>) -> Self {
        self.instances.extend(instances);
        self
    }

    /// Link a component with a namespace
    pub fn with_linked_component(mut self, name: &str, component_id: usize) -> Self {
        self.linked_components.push((name.to_string(), component_id));
        self
    }

    /// Set the runtime instance
    pub fn with_runtime(mut self, runtime: RuntimeInstance) -> Self {
        self.runtime = Some(runtime);
        self
    }

    /// Set the resource table
    pub fn with_resource_table(mut self, resource_table: ResourceTable) -> Self {
        self.resource_table = Some(resource_table);
        self
    }

    /// Set the built-in requirements
    pub fn with_built_in_requirements(mut self, requirements: BuiltinRequirements) -> Self {
        self.built_in_requirements = Some(requirements);
        self
    }

    /// Set the original binary
    pub fn with_original_binary(mut self, binary: Vec<u8>) -> Self {
        self.original_binary = Some(binary);
        self
    }

    /// Set the verification level
    pub fn with_verification_level(mut self, level: VerificationLevel) -> Self {
        self.verification_level = level;
        self
    }

    /// Build the component
    pub fn build(self) -> Result<Component> {
        let component_type = self.component_type.unwrap_or_default();
        let resource_table = match self.resource_table {
            Some(table) => table,
            None => ResourceTable::new()?,
        };

        let exports_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let imports_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let instances_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let linked_components_provider = safe_managed_alloc!(65536, CrateId::Component)?;

        let mut component = Component {
            component_type,
            exports: BoundedVec::new(),
            imports: BoundedVec::new(),
            instances: BoundedVec::new(),
            linked_components: BoundedVec::new(),
            runtime: self.runtime,
            resource_table,
            built_in_requirements: self.built_in_requirements,
            original_binary: None,
            verification_level: self.verification_level,
        };

        // Add exports
        for export in self.exports {
            component.add_export(export)?;
        }

        // Add imports
        for import in self.imports {
            component.add_import(import)?;
        }

        // Add instances
        for instance in self.instances {
            component.add_instance(instance)?;
        }

        // Link components
        for (name, component_id) in self.linked_components {
            component.link_component(&name, component_id)?;
        }

        // Set original binary if provided
        if let Some(binary) = self.original_binary {
            let binary_provider = safe_managed_alloc!(65536, CrateId::Component)?;
            let mut bounded_binary = BoundedVec::new();
            for byte in binary {
                bounded_binary.push(byte).map_err(|_| {
                    Error::new(
                        ErrorCategory::Capacity,
                        codes::CAPACITY_EXCEEDED,
                        "Binary too large for BoundedVec",
                    )
                })?;
            }
            component.original_binary = Some(bounded_binary);
        }

        Ok(component)
    }
}

impl Default for ComponentBuilder {
    fn default() -> Self {
        Self::new()
    }
}
