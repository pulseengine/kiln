//! Component Model implementation for the WebAssembly Runtime.
//!
//! This module provides types and implementations for the WebAssembly Component
//! Model.

#[cfg(not(feature = "std"))]
use alloc::format;
// HashMap imports
#[cfg(feature = "std")]
use std::collections::HashMap;

#[cfg(feature = "std")]
use log::{debug, error, info, trace, warn};
// Import wrt_decoder types for decode and parse
// use wrt_decoder::component::decode::Component as DecodedComponent;
// Additional imports that aren't in the prelude
use wrt_format::component::{ExternType as FormatExternType, FormatValType};
use wrt_foundation::{
    budget_aware_provider::CrateId, collections::StaticVec, collections::StaticVec as BoundedVec,
    resource::ResourceOperation as FormatResourceOperation, safe_managed_alloc,
    safe_memory::NoStdProvider,
};
use wrt_intercept::LinkInterceptor;

use crate::{
    bounded_component_infra::ComponentProvider, export::Export, import::Import, prelude::*,
    resources::ResourceTable,
};
use wrt_foundation::builtin::BuiltinType;

// Simple HashMap substitute for no_std using BoundedVec
#[cfg(not(feature = "std"))]
pub struct SimpleMap<K, V> {
    entries: wrt_foundation::collections::StaticVec<(K, V), 64>,
}

#[cfg(not(feature = "std"))]
impl<K: PartialEq + Clone, V: Clone> SimpleMap<K, V> {
    pub fn new() -> Result<Self> {
        Ok(Self {
            entries: wrt_foundation::collections::StaticVec::new(),
        })
    }

    pub fn insert(&mut self, key: K, value: V) {
        // Remove existing entry if present
        self.entries.retain(|(k, _)| k != &key);
        // Add new entry
        let _ = self.entries.push((key, value));
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(not(feature = "std"))]
type ComponentMap<K, V> = SimpleMap<K, V>;

// Runtime types with explicit namespacing
use wrt_runtime::{
    // func::FuncType as RuntimeFuncType, // Not available at this path
    global::Global,
    memory::Memory,
    table::Table,
};

// Import types from wrt-foundation instead of wrt-runtime
use wrt_foundation::types::{GlobalType, MemoryType, TableType};

// Placeholder types for missing imports
pub type RuntimeFuncType = ();

// Import RwLock from prelude (it will be std::sync::RwLock or a no_std
// equivalent from the prelude)
// use wrt_runtime::execution::{run_with_time_bounds, TimeBoundedConfig,
// TimeBoundedOutcome}; Binary std/no_std choice

// core::str is already imported via prelude

// Import type conversion utilities for clear transformations
use crate::type_conversion::bidirectional::{
    complete_format_to_types_extern_type, complete_types_to_format_extern_type,
    convert_format_to_types_valtype, convert_format_valtype_to_valuetype,
    convert_types_to_format_valtype, extern_type_to_func_type, format_val_type_to_value_type,
};

// Define type aliases for missing types
type ComponentDecoder = fn(&[u8]) -> wrt_error::Result<wrt_format::component::Component>;
pub type ExportType = wrt_format::component::Export;
pub type ImportType = wrt_format::component::Import;
type TypeDef = wrt_format::component::ComponentType;
// Producers section might be moved or renamed
// ProducersSection,

/// Represents a component type
#[derive(Debug, Clone)]
pub struct WrtComponentType {
    /// Component imports
    pub imports: Vec<(String, String, ExternType<ComponentProvider>)>,
    /// Component exports
    pub exports: Vec<(String, ExternType<ComponentProvider>)>,
    /// Component instances
    pub instances: Vec<wrt_format::component::ComponentTypeDefinition>,
    /// Verification level for this component type
    pub verification_level: wrt_foundation::verification::VerificationLevel,
}

impl WrtComponentType {
    /// Creates a new empty component type
    pub fn new() -> Result<Self> {
        Ok(Self {
            imports: Vec::new(),
            exports: Vec::new(),
            instances: Vec::new(),
            verification_level: wrt_foundation::verification::VerificationLevel::Standard,
        })
    }

    /// Create a new empty component type
    pub fn empty() -> Result<Self> {
        Ok(Self {
            imports: Vec::new(),
            exports: Vec::new(),
            instances: Vec::new(),
            verification_level: wrt_foundation::verification::VerificationLevel::Standard,
        })
    }

    /// Set the verification level for memory operations
    pub fn set_verification_level(
        &mut self,
        level: wrt_foundation::verification::VerificationLevel,
    ) {
        self.verification_level = level;
    }

    /// Get the current verification level
    pub fn verification_level(&self) -> wrt_foundation::verification::VerificationLevel {
        self.verification_level
    }
}

impl Default for WrtComponentType {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            imports: Vec::new(),
            exports: Vec::new(),
            instances: Vec::new(),
            verification_level: wrt_foundation::verification::VerificationLevel::Standard,
        })
    }
}

/// Represents a component instance
// Type aliases for compatibility
pub type ComponentInstance = RuntimeInstance;
pub type ComponentType = WrtComponentType;

pub struct Component {
    /// Component unique identifier (optional)
    pub(crate) id: Option<String>,
    /// Component type
    pub(crate) component_type: WrtComponentType,
    /// Component types (type definitions)
    pub(crate) types: Vec<wrt_format::component::ComponentTypeDefinition>,
    /// Embedded core modules
    pub(crate) modules: Vec<Vec<u8>>,
    /// Component exports
    pub(crate) exports: Vec<Export>,
    /// Component imports
    pub(crate) imports: Vec<Import>,
    /// Component instances
    pub(crate) instances: Vec<InstanceValue>,
    /// Linked components with their namespaces
    pub(crate) linked_components: HashMap<String, Arc<Component>>,
    /// Host callback registry
    pub(crate) callback_registry: Option<Arc<CallbackRegistry>>,
    /// Runtime instance
    pub(crate) runtime: Option<Box<RuntimeInstance>>,
    /// Interceptor for function calls
    pub(crate) interceptor: Option<Arc<LinkInterceptor>>,
    /// Resource table for managing component resources
    pub resource_table: ResourceTable,
    /// Built-in requirements
    pub(crate) built_in_requirements: Option<BuiltinRequirements>,
    /// Original binary
    pub(crate) original_binary: Option<Vec<u8>>,
    /// Verification level for all operations
    pub(crate) verification_level: wrt_foundation::verification::VerificationLevel,
}

impl Clone for Component {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            component_type: self.component_type.clone(),
            types: self.types.clone(),
            modules: self.modules.clone(),
            exports: self.exports.clone(),
            imports: self.imports.clone(),
            instances: self.instances.clone(),
            linked_components: self.linked_components.clone(),
            callback_registry: self.callback_registry.clone(),
            // Runtime instances are stateful and should not be cloned
            runtime: None,
            interceptor: self.interceptor.clone(),
            resource_table: self.resource_table.clone(),
            built_in_requirements: self.built_in_requirements.clone(),
            original_binary: self.original_binary.clone(),
            verification_level: self.verification_level,
        }
    }
}

impl Debug for Component {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Component")
            .field("id", &self.id)
            .field("component_type", &self.component_type)
            .field("types", &self.types.len())
            .field("modules", &self.modules.len())
            .field("exports", &self.exports.len())
            .field("imports", &self.imports.len())
            .field("instances", &self.instances.len())
            .field("linked_components", &self.linked_components.keys())
            .field(
                "callback_registry",
                &self.callback_registry.as_ref().map(|_| "Some(CallbackRegistry)"),
            )
            .field(
                "runtime",
                &self.runtime.as_ref().map(|r| format!("RuntimeInstance(id={})", r.id)),
            )
            .field(
                "interceptor",
                &self.interceptor.as_ref().map(|_| "Some(LinkInterceptor)"),
            )
            .field("resource_table", &"ResourceTable")
            .field("built_in_requirements", &self.built_in_requirements)
            .field(
                "original_binary",
                &self.original_binary.as_ref().map(|b| format!("{} bytes", b.len())),
            )
            .field("verification_level", &self.verification_level)
            .finish()
    }
}

/// Represents an instance value
#[derive(Debug, Clone)]
pub struct InstanceValue {
    /// Instance type
    pub ty: wrt_format::component::ComponentTypeDefinition,
    /// Instance exports
    pub exports: Vec<Export>,
}

/// Represents a runtime instance for executing WebAssembly code
#[derive(Debug)]
pub struct RuntimeInstance {
    /// Unique instance ID
    pub id: u32,
    /// Reference to the component definition
    pub component: Component,
    /// Current instance state
    pub state: crate::types::ComponentInstanceState,
    /// Resource manager for this instance
    pub resource_manager: Option<crate::resource_management::ResourceManager>,
    /// Instance memory (if allocated)
    pub memory: Option<crate::components::component_instantiation::ComponentMemory>,
    /// Resolved imports for this instance
    #[cfg(all(feature = "std", feature = "safety-critical"))]
    pub imports: wrt_foundation::allocator::WrtVec<
        crate::instantiation::ResolvedImport,
        { wrt_foundation::allocator::CrateId::Component as u8 },
        256,
    >,
    #[cfg(all(feature = "std", not(feature = "safety-critical")))]
    pub imports: Vec<crate::instantiation::ResolvedImport>,
    #[cfg(not(feature = "std"))]
    pub imports: StaticVec<crate::instantiation::ResolvedImport, 256>,
    /// Resolved exports from this instance
    #[cfg(all(feature = "std", feature = "safety-critical"))]
    pub exports: wrt_foundation::allocator::WrtVec<
        crate::instantiation::ResolvedExport,
        { wrt_foundation::allocator::CrateId::Component as u8 },
        256,
    >,
    #[cfg(all(feature = "std", not(feature = "safety-critical")))]
    pub exports: Vec<crate::instantiation::ResolvedExport>,
    #[cfg(not(feature = "std"))]
    pub exports: StaticVec<crate::instantiation::ResolvedExport, 256>,
    /// Resource tables for this instance
    #[cfg(all(feature = "std", feature = "safety-critical"))]
    pub resource_tables: wrt_foundation::allocator::WrtVec<
        crate::instantiation::ResourceTable,
        { wrt_foundation::allocator::CrateId::Component as u8 },
        16,
    >,
    #[cfg(all(feature = "std", not(feature = "safety-critical")))]
    pub resource_tables: Vec<crate::instantiation::ResourceTable>,
    #[cfg(not(feature = "std"))]
    pub resource_tables: StaticVec<crate::instantiation::ResourceTable, 16>,
    /// Module instances embedded in this component
    #[cfg(all(feature = "std", feature = "safety-critical"))]
    pub module_instances: wrt_foundation::allocator::WrtVec<
        crate::instantiation::ModuleInstance,
        { wrt_foundation::allocator::CrateId::Component as u8 },
        64,
    >,
    #[cfg(all(feature = "std", not(feature = "safety-critical")))]
    pub module_instances: Vec<crate::instantiation::ModuleInstance>,
    #[cfg(not(feature = "std"))]
    pub module_instances: StaticVec<crate::instantiation::ModuleInstance, 64>,
    /// Functions exported by this runtime
    functions: HashMap<String, ExternValue>,
    /// Memory exported by this runtime
    memories: HashMap<String, MemoryValue>,
    /// Tables exported by this runtime
    tables: HashMap<String, TableValue>,
    /// Globals exported by this runtime
    globals: HashMap<String, GlobalValue>,
    /// Runtime module instance (will be implemented as needed)
    module_instance: Option<Arc<RwLock<ModuleInstance>>>,
    /// Verification level for memory operations
    verification_level: VerificationLevel,
}

impl Default for RuntimeInstance {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeInstance {
    /// Creates a new runtime instance
    pub fn new() -> Self {
        Self {
            id: 0,
            component: Component::new(WrtComponentType::default()),
            state: crate::types::ComponentInstanceState::Initialized,
            resource_manager: None,
            memory: None,
            #[cfg(all(feature = "std", feature = "safety-critical"))]
            imports: wrt_foundation::allocator::WrtVec::new(),
            #[cfg(all(feature = "std", not(feature = "safety-critical")))]
            imports: Vec::new(),
            #[cfg(not(feature = "std"))]
            imports: StaticVec::new(),
            #[cfg(all(feature = "std", feature = "safety-critical"))]
            exports: wrt_foundation::allocator::WrtVec::new(),
            #[cfg(all(feature = "std", not(feature = "safety-critical")))]
            exports: Vec::new(),
            #[cfg(not(feature = "std"))]
            exports: StaticVec::new(),
            #[cfg(all(feature = "std", feature = "safety-critical"))]
            resource_tables: wrt_foundation::allocator::WrtVec::new(),
            #[cfg(all(feature = "std", not(feature = "safety-critical")))]
            resource_tables: Vec::new(),
            #[cfg(not(feature = "std"))]
            resource_tables: StaticVec::new(),
            #[cfg(all(feature = "std", feature = "safety-critical"))]
            module_instances: wrt_foundation::allocator::WrtVec::new(),
            #[cfg(all(feature = "std", not(feature = "safety-critical")))]
            module_instances: Vec::new(),
            #[cfg(not(feature = "std"))]
            module_instances: StaticVec::new(),
            functions: HashMap::new(),
            memories: HashMap::new(),
            tables: HashMap::new(),
            globals: HashMap::new(),
            module_instance: None,
            verification_level: VerificationLevel::Standard,
        }
    }

    /// Register an exported function
    pub fn register_function(&mut self, name: String, function: ExternValue) -> Result<()> {
        if let ExternValue::Function(_) = &function {
            self.functions.insert(name, function);
            Ok(())
        } else {
            Err(Error::validation_error("Invalid function type"))
        }
    }

    /// Register an exported memory
    pub fn register_memory(&mut self, name: String, memory: MemoryValue) -> Result<()> {
        self.memories.insert(name, memory);
        Ok(())
    }

    /// Executes a function in the runtime
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the function to execute
    /// * `args` - Arguments to pass to the function
    ///
    /// # Returns
    ///
    /// The result values from the function execution
    ///
    /// # Errors
    ///
    /// Returns an error if the function cannot be executed
    pub fn execute_function(&self, name: &str, args: Vec<Value>) -> Result<Vec<Value>> {
        // Look up the function in our registered functions
        let function = self
            .functions
            .get(name)
            .ok_or_else(|| Error::runtime_function_not_found("Component not found"))?;

        // Get the function value
        if let ExternValue::Function(func_value) = function {
            // Validate arguments based on function signature
            if args.len() != func_value.ty.params.len() {
                return Err(Error::validation_error("Argument count mismatch"));
            }

            // Type check arguments
            for (arg, param_type) in args.iter().zip(func_value.ty.params.iter()) {
                let arg_type = arg.value_type();
                let expected_type = &param_type;

                if !self.is_type_compatible(&arg_type, expected_type) {
                    return Err(Error::validation_error("Type mismatch for argument"));
                }
            }

            // Execute the function via the module instance if available
            if let Some(module_instance) = &self.module_instance {
                // This would delegate to the actual module instance in a full implementation
                // This interface will be extended as needed

                #[cfg(feature = "std")]
                debug!(
                    "Module instance execution not yet implemented for function: {}",
                    name
                );

                // For MVP, we need to return that execution isn't implemented
                // without hard-coding specific function behavior
                return Err(Error::runtime_execution_error("Function execution failed"));
            }

            // If we reach here, there's no module instance to handle the execution,
            // so we need a structured way to handle user-provided function execution.

            // Return a not implemented error for now
            // This will be extended to support function resolution from registered
            // callbacks
            Err(Error::component_not_found("component_not_found"))
        } else {
            Err(Error::validation_error("Invalid function type"))
        }
    }

    /// Check if a value type is compatible with a format value type
    fn is_type_compatible(&self, value_type: &ValueType, expected_type: &ValueType) -> bool {
        // For MVP, we'll do simple equality check
        // This can be extended later to handle more complex type compatibility rules
        value_type == expected_type
    }

    /// Reads memory
    pub fn read_memory(&self, _offset: u32, _size: u32, _buffer: &mut [u8]) -> Result<()> {
        // Implementation would depend on the specific runtime
        Err(Error::runtime_execution_error(
            "Function execution not implemented",
        ))
    }

    /// Writes memory
    pub fn write_memory(&self, _offset: u32, _data: &[u8]) -> Result<()> {
        // Implementation would depend on the specific runtime
        Err(Error::new(
            ErrorCategory::System,
            codes::NOT_IMPLEMENTED,
            "error_message_needed",
        ))
    }

    /// Get an export by name (functions only for now)
    pub fn get_export(&self, name: &str) -> Option<&ExternValue> {
        // Check function exports first (this is what we mainly need for async
        // execution)
        self.functions.get(name)
    }

    /// Get export by name with full type support
    pub fn get_export_value(&self, name: &str) -> Option<ExternValue> {
        // Check function exports first
        if let Some(function) = self.functions.get(name) {
            return Some(function.clone());
        }

        // Check memory exports
        if let Some(memory) = self.memories.get(name) {
            return Some(ExternValue::Memory(memory.clone()));
        }

        // Check table exports
        if let Some(table) = self.tables.get(name) {
            return Some(ExternValue::Table(table.clone()));
        }

        // Check global exports
        if let Some(global) = self.globals.get(name) {
            return Some(ExternValue::Global(global.clone()));
        }

        None
    }

    /// Get function index for an exported function
    pub fn get_function_index(&self, name: &str) -> Option<u32> {
        if let Some(ExternValue::Function(func_value)) = self.functions.get(name) {
            // Extract function index from export name or derive from it
            // In a real implementation, this would come from the module's export table
            // For now, we'll use a simple hash-based approach with better distribution
            Some(self.hash_function_name_to_index(name))
        } else {
            None
        }
    }

    /// Hash a function name to a function index
    fn hash_function_name_to_index(&self, name: &str) -> u32 {
        // Simple hash-based function index generation
        // In a real implementation, this would come from the module's export table
        let mut hash: u32 = 0;
        for byte in name.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
        }
        // Keep within reasonable function index range
        hash % 1024
    }

    /// Get all function exports
    pub fn get_function_exports(&self) -> impl Iterator<Item = (&String, &ExternValue)> {
        self.functions.iter()
    }

    /// Check if a function export exists
    pub fn has_function_export(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }
}

// Private helper struct for implementing the module instance
// This is just a placeholder for now
#[derive(Debug)]
struct ModuleInstance {
    // Implementation details would go here
}

/// Helper function to convert FormatValType to ValueType
fn convert_to_valuetype(val_type_pair: &(String, FormatValType)) -> ValueType {
    format_val_type_to_value_type(&val_type_pair.1).expect("Failed to convert format value type")
}

/// Represents an external value
#[derive(Debug, Clone, PartialEq)]
pub enum ExternValue {
    /// Function value (alias for Function)
    Func(FunctionValue),
    /// Function value
    Function(FunctionValue),
    /// Table value
    Table(TableValue),
    /// Memory value
    Memory(MemoryValue),
    /// Global value
    Global(GlobalValue),
    /// Trap value
    Trap(String),
}

/// Represents a function value
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionValue {
    /// Function type
    pub ty: wrt_foundation::types::FuncType,
    /// Export name that this function refers to
    pub export_name: String,
}

/// Represents a table value
#[derive(Debug, Clone, PartialEq)]
pub struct TableValue {
    /// Table type
    pub ty: TableType,
    /// Table instance
    pub table: Table,
}

/// Represents a memory value
#[derive(Debug, Clone)]
pub struct MemoryValue {
    /// Memory type
    pub ty: MemoryType,
    /// Memory instance
    pub memory: Arc<RwLock<Memory>>,
}

impl PartialEq for MemoryValue {
    fn eq(&self, other: &Self) -> bool {
        // Compare only the type, not the memory instance
        // Memory instances are mutable and cannot be meaningfully compared
        self.ty == other.ty && Arc::ptr_eq(&self.memory, &other.memory)
    }
}

impl MemoryValue {
    /// Creates a new memory value
    ///
    /// # Arguments
    ///
    /// * `ty` - The memory type
    ///
    /// # Returns
    ///
    /// A new memory value
    ///
    /// # Errors
    ///
    /// Returns an error if the memory cannot be created
    pub fn new(ty: MemoryType) -> Result<Self> {
        // Convert MemoryType to CoreMemoryType for Memory::new
        let core_ty = wrt_runtime::CoreMemoryType {
            limits: ty.limits,
            shared: ty.shared,
        };
        let memory = Memory::new(core_ty)?;
        Ok(Self {
            ty,
            memory: Arc::new(RwLock::new(*memory)), // Dereference Box<Memory>
        })
    }

    /// Creates a new memory value with a debug name
    ///
    /// # Arguments
    ///
    /// * `ty` - The memory type
    /// * `name` - The debug name for the memory
    ///
    /// # Returns
    ///
    /// A new memory value
    ///
    /// # Errors
    ///
    /// Returns an error if the memory cannot be created
    pub fn new_with_name(ty: MemoryType, name: &str) -> Result<Self> {
        // Convert MemoryType to CoreMemoryType for Memory::new_with_name
        let core_ty = wrt_runtime::CoreMemoryType {
            limits: ty.limits,
            shared: ty.shared,
        };
        let memory = Memory::new_with_name(core_ty, name)?;
        Ok(Self {
            ty,
            memory: Arc::new(RwLock::new(memory)), // Memory::new_with_name returns Memory, not Box
        })
    }

    /// Reads from memory
    ///
    /// # Arguments
    ///
    /// * `offset` - The offset to read from
    /// * `size` - The number of bytes to read
    ///
    /// # Returns
    ///
    /// The bytes read from memory
    ///
    /// # Errors
    ///
    /// Returns an error if the read fails
    pub fn read(&self, offset: u32, size: u32) -> Result<Vec<u8>> {
        let memory = self.memory.read();

        let mut buffer = vec![0; size as usize];
        memory
            .read(offset, &mut buffer)
            .map_err(|e| Error::memory_error("Component not found"))?;

        Ok(buffer)
    }

    /// Writes to memory
    ///
    /// # Arguments
    ///
    /// * `offset` - The offset to write to
    /// * `bytes` - The bytes to write
    ///
    /// # Returns
    ///
    /// Ok(()) if the write succeeds
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails
    pub fn write(&self, offset: u32, bytes: &[u8]) -> Result<()> {
        let mut memory = self.memory.write();

        memory
            .write(offset, bytes)
            .map_err(|e| Error::memory_error("Component not found"))
    }

    /// Grows the memory by the given number of pages
    ///
    /// # Arguments
    ///
    /// * `pages` - The number of pages to grow by
    ///
    /// # Returns
    ///
    /// The previous size in pages
    ///
    /// # Errors
    ///
    /// Returns an error if the memory cannot be grown
    pub fn grow(&self, pages: u32) -> Result<u32> {
        let mut memory = self.memory.write();

        memory.grow(pages).map_err(|e| Error::memory_error("Component not found"))
    }

    /// Gets the current size of the memory in pages
    ///
    /// # Returns
    ///
    /// The current size in pages
    pub fn size(&self) -> Result<u32> {
        let memory = self.memory.read();

        Ok(memory.size())
    }

    /// Gets the current size of the memory in bytes
    ///
    /// # Returns
    ///
    /// The current size in bytes
    pub fn size_in_bytes(&self) -> Result<usize> {
        let memory = self.memory.read();

        Ok(memory.size_in_bytes())
    }

    /// Gets the peak memory usage in bytes
    ///
    /// # Returns
    ///
    /// The peak memory usage in bytes
    pub fn peak_usage(&self) -> Result<usize> {
        let memory = self.memory.read();

        // Memory doesn't have peak_usage method directly on RwLockReadGuard
        // Access the actual Memory instance inside the guard
        Ok(memory.size_in_bytes())
    }

    /// Gets the number of memory accesses performed
    ///
    /// # Returns
    ///
    /// The number of memory accesses
    pub fn access_count(&self) -> Result<u64> {
        let memory = self.memory.read();

        Ok(memory.access_count())
    }

    /// Gets a debug name for this memory, if any
    ///
    /// # Returns
    ///
    /// The debug name, if any
    pub fn debug_name(&self) -> Result<Option<String>> {
        let memory = self.memory.read();

        Ok(memory.debug_name().map(String::from))
    }

    /// Sets a debug name for this memory
    ///
    /// # Arguments
    ///
    /// * `name` - The debug name to set
    pub fn set_debug_name(&self, name: &str) -> Result<()> {
        let mut memory = self.memory.write();

        memory.set_debug_name(name);
        Ok(())
    }

    /// Verifies the integrity of the memory
    ///
    /// # Returns
    ///
    /// Ok(()) if the memory is valid
    ///
    /// # Errors
    ///
    /// Returns an error if the memory is invalid
    pub fn verify_integrity(&self) -> Result<()> {
        let memory = self.memory.read();

        memory.verify_integrity()
    }
}

/// Represents a global value
#[derive(Debug, Clone, PartialEq)]
pub struct GlobalValue {
    /// Global type
    pub ty: GlobalType,
    /// Global instance
    pub global: Global,
}

/// Represents a host with callable functions
pub struct Host {
    /// Host functions indexed by name
    functions: HashMap<String, FunctionValue>,
}

impl Default for Host {
    fn default() -> Self {
        Self::new()
    }
}

impl Host {
    /// Creates a new empty host
    #[must_use]
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }

    /// Adds a host function
    pub fn add_function(&mut self, name: String, func: FunctionValue) {
        self.functions.insert(name, func);
    }

    /// Gets a host function by name
    #[must_use]
    pub fn get_function(&self, name: &str) -> Option<&FunctionValue> {
        self.functions.get(name)
    }

    /// Calls a host function
    pub fn call_function(&self, name: &str, args: &[Value]) -> Result<Vec<Value>> {
        // Find the function
        let function = self
            .get_function(name)
            .ok_or_else(|| Error::component_not_found("Component not found"))?;

        // Validate arguments
        if args.len() != function.ty.params.len() {
            return Err(Error::validation_error("Argument count mismatch"));
        }

        // Actual function calling would happen here
        // This requires integration with the actual host function mechanism
        Err(Error::runtime_execution_error(
            "Function execution not implemented",
        ))
    }
}

impl Component {
    /// Creates a new component with the given type
    pub fn new(component_type: WrtComponentType) -> Self {
        Self {
            id: None,
            component_type,
            types: Vec::new(),
            modules: Vec::new(),
            exports: Vec::new(),
            imports: Vec::new(),
            instances: Vec::new(),
            linked_components: HashMap::new(),
            callback_registry: None,
            runtime: None,
            interceptor: None,
            resource_table: ResourceTable::new().expect("Failed to create resource table"),
            built_in_requirements: None,
            original_binary: None,
            verification_level: wrt_foundation::verification::VerificationLevel::Standard,
        }
    }

    /// Add a function export to the component
    pub fn add_function(&mut self, func: Export) -> Result<()> {
        self.exports.push(func);
        Ok(())
    }

    /// Add a memory export to the component
    pub fn add_memory(&mut self, memory: Export) -> Result<()> {
        self.exports.push(memory);
        Ok(())
    }

    /// Add a table export to the component
    pub fn add_table(&mut self, table: Export) -> Result<()> {
        self.exports.push(table);
        Ok(())
    }

    /// Add a global export to the component
    pub fn add_global(&mut self, global: Export) -> Result<()> {
        self.exports.push(global);
        Ok(())
    }

    /// Set the verification level for memory operations
    pub fn set_verification_level(
        &mut self,
        level: wrt_foundation::verification::VerificationLevel,
    ) {
        self.verification_level = level;
        // Propagate to resource table
        // TODO: Re-enable when set_default_verification_level is available
        // self.resource_table.set_default_verification_level(convert_verification_level(level));
    }

    /// Get the current verification level
    pub fn verification_level(&self) -> wrt_foundation::verification::VerificationLevel {
        self.verification_level
    }

    /// Add a type definition to the component
    ///
    /// # Arguments
    ///
    /// * `component_type` - The type definition to add
    ///
    /// # Returns
    ///
    /// Ok(()) on success
    ///
    /// # Errors
    ///
    /// Returns an error if the type cannot be added
    pub fn add_type(
        &mut self,
        component_type: wrt_format::component::ComponentTypeDefinition,
    ) -> Result<()> {
        self.types.push(component_type);
        Ok(())
    }

    /// Add a function export to the component
    ///
    /// # Arguments
    ///
    /// * `name` - The export name
    /// * `function_index` - The function index
    ///
    /// # Returns
    ///
    /// Ok(()) on success
    ///
    /// # Errors
    ///
    /// Returns an error if the export cannot be added
    pub fn add_function_export(&mut self, name: &str, function_index: u32) -> Result<()> {
        // TODO: Implement proper function export tracking
        // For now, create a placeholder export
        #[cfg(feature = "std")]
        debug!(
            "Adding function export: {} (index: {})",
            name, function_index
        );
        Ok(())
    }

    /// Add a function import to the component
    ///
    /// # Arguments
    ///
    /// * `name` - The import name
    /// * `type_index` - The type index
    ///
    /// # Returns
    ///
    /// Ok(()) on success
    ///
    /// # Errors
    ///
    /// Returns an error if the import cannot be added
    pub fn add_function_import(&mut self, name: &str, type_index: u32) -> Result<()> {
        // TODO: Implement proper function import tracking
        #[cfg(feature = "std")]
        debug!(
            "Adding function import: {} (type index: {})",
            name, type_index
        );
        Ok(())
    }

    /// Add an instance export to the component
    ///
    /// # Arguments
    ///
    /// * `name` - The export name
    /// * `instance_index` - The instance index
    ///
    /// # Returns
    ///
    /// Ok(()) on success
    ///
    /// # Errors
    ///
    /// Returns an error if the export cannot be added
    pub fn add_instance_export(&mut self, name: &str, instance_index: u32) -> Result<()> {
        // TODO: Implement proper instance export tracking
        #[cfg(feature = "std")]
        debug!(
            "Adding instance export: {} (index: {})",
            name, instance_index
        );
        Ok(())
    }

    /// Add an instance import to the component
    ///
    /// # Arguments
    ///
    /// * `name` - The import name
    /// * `type_index` - The type index
    ///
    /// # Returns
    ///
    /// Ok(()) on success
    ///
    /// # Errors
    ///
    /// Returns an error if the import cannot be added
    pub fn add_instance_import(&mut self, name: &str, type_index: u32) -> Result<()> {
        // TODO: Implement proper instance import tracking
        #[cfg(feature = "std")]
        debug!(
            "Adding instance import: {} (type index: {})",
            name, type_index
        );
        Ok(())
    }

    /// Add a type export to the component
    ///
    /// # Arguments
    ///
    /// * `name` - The export name
    /// * `type_index` - The type index
    ///
    /// # Returns
    ///
    /// Ok(()) on success
    ///
    /// # Errors
    ///
    /// Returns an error if the export cannot be added
    pub fn add_type_export(&mut self, name: &str, type_index: u32) -> Result<()> {
        // TODO: Implement proper type export tracking
        #[cfg(feature = "std")]
        debug!("Adding type export: {} (type index: {})", name, type_index);
        Ok(())
    }

    /// Add a type import to the component
    ///
    /// # Arguments
    ///
    /// * `name` - The import name
    ///
    /// # Returns
    ///
    /// Ok(()) on success
    ///
    /// # Errors
    ///
    /// Returns an error if the import cannot be added
    pub fn add_type_import(&mut self, name: &str) -> Result<()> {
        // TODO: Implement proper type import tracking
        #[cfg(feature = "std")]
        debug!("Adding type import: {}", name);
        Ok(())
    }

    /// Add a value export to the component
    ///
    /// # Arguments
    ///
    /// * `name` - The export name
    /// * `value_index` - The value index
    ///
    /// # Returns
    ///
    /// Ok(()) on success
    ///
    /// # Errors
    ///
    /// Returns an error if the export cannot be added
    pub fn add_value_export(&mut self, name: &str, value_index: u32) -> Result<()> {
        // TODO: Implement proper value export tracking
        #[cfg(feature = "std")]
        debug!("Adding value export: {} (index: {})", name, value_index);
        Ok(())
    }

    /// Add a value import to the component
    ///
    /// # Arguments
    ///
    /// * `name` - The import name
    /// * `type_index` - The type index
    ///
    /// # Returns
    ///
    /// Ok(()) on success
    ///
    /// # Errors
    ///
    /// Returns an error if the import cannot be added
    pub fn add_value_import(&mut self, name: &str, type_index: u32) -> Result<()> {
        // TODO: Implement proper value import tracking
        #[cfg(feature = "std")]
        debug!("Adding value import: {} (type index: {})", name, type_index);
        Ok(())
    }

    /// Add a module adapter to the component
    ///
    /// # Arguments
    ///
    /// * `adapter` - The module adapter to add
    ///
    /// # Returns
    ///
    /// Ok(()) on success
    ///
    /// # Errors
    ///
    /// Returns an error if the adapter cannot be added
    pub fn add_module_adapter(&mut self, adapter: crate::adapter::CoreModuleAdapter) -> Result<()> {
        // TODO: Implement proper module adapter tracking
        // For now, we don't have a field to store adapters, so we just log
        #[cfg(feature = "std")]
        debug!("Adding module adapter: {}", adapter.name);
        #[cfg(not(feature = "std"))]
        let _ = adapter; // Suppress unused variable warning in no_std
        Ok(())
    }
}

/// Container for built-in type requirements
#[derive(Debug, Clone, Default)]
pub struct BuiltinRequirements {
    /// The set of required built-in types
    requirements: HashSet<BuiltinType>,
}

impl BuiltinRequirements {
    /// Create a new, empty set of built-in requirements
    pub fn new() -> Self {
        Self {
            requirements: HashSet::new(),
        }
    }

    /// Add a requirement for a specific built-in type
    pub fn add_requirement(&mut self, builtin_type: BuiltinType) {
        self.requirements.insert(builtin_type);
    }

    /// Get the set of required built-ins
    pub fn get_requirements(&self) -> &HashSet<BuiltinType> {
        &self.requirements
    }

    /// Check if all the requirements can be satisfied with the current runtime
    pub fn is_supported(&self) -> bool {
        // For now, all built-ins are supported
        // In the future, this could check if experimental features are enabled
        true
    }

    /// Get a list of unsupported built-ins
    pub fn get_unsupported(&self) -> Vec<BuiltinType> {
        // For now, all built-ins are supported
        vec![]
    }

    /// Check if the requirements can be satisfied with the given available
    /// built-ins
    pub fn can_be_satisfied(&self, available: &HashSet<BuiltinType>) -> bool {
        self.requirements.iter().all(|req| available.contains(req))
    }
}

/// Scans a binary for required builtins
pub fn scan_builtins(bytes: &[u8]) -> Result<BuiltinRequirements> {
    // Helper to avoid boilerplate
    let mut requirements = BuiltinRequirements::new();

    // Try to decode as component or module
    #[cfg(feature = "std")]
    {
        match wrt_decoder::component::decode_component(bytes) {
            Ok(component) => {
                scan_functions_for_builtins(&(), &mut requirements)?;
                Ok(requirements)
            },
            Err(err) => Err(Error::component_not_found("Component not found")),
        }
    }
    #[cfg(not(feature = "std"))]
    {
        // Without decoder, return empty requirements
        Ok(requirements)
    }
}

/// Scans a module binary for builtins
fn scan_module_for_builtins(module: &[u8], requirements: &mut BuiltinRequirements) -> Result<()> {
    // This would need to be implemented for core modules
    // For now, we'll just return success (decoder module not available in current
    // build) TODO: Implement proper module validation when decoder API is
    // available
    if module.is_empty() {
        Err(Error::runtime_execution_error(
            "Function execution not implemented",
        ))
    } else {
        Ok(())
    }
}

/// Map an import name to a built-in type
fn map_import_to_builtin(import_name: &str) -> Option<BuiltinType> {
    // This function is now defined in the parser module, but we keep it here
    // for backward compatibility
    // Simply return the builtin type directly - no conversion needed
    crate::parser::map_import_to_builtin(import_name)
}

/// Scan component functions for built-in usage
///
/// # Arguments
///
/// * `component` - The component to scan
/// * `requirements` - The requirements to be updated
///
/// # Returns
///
/// A result indicating success or an error
fn scan_functions_for_builtins(
    _component: &(), // TODO: Re-enable with &wrt_decoder::component::Component when available
    requirements: &mut BuiltinRequirements,
) -> Result<()> {
    // Check for resource types which indicate built-in usage
    // Component types method doesn't exist - skip type checking for now
    // TODO: Implement proper type scanning when DecodedComponent API is available
    if false && false {
        // Resources typically require the ResourceCreate built-in
        requirements.add_requirement(BuiltinType::ResourceCreate);
        requirements.add_requirement(BuiltinType::ResourceDrop);
        requirements.add_requirement(BuiltinType::ResourceRep);
        requirements.add_requirement(BuiltinType::ResourceGet);
    }

    #[cfg(all(feature = "std", feature = "component-model-async"))]
    // Check for async functions which require the AsyncWait built-in
    for func in component.functions() {
        if func.is_async() {
            requirements.add_requirement(BuiltinType::AsyncWait);
            requirements.add_requirement(BuiltinType::AsyncNew);
            requirements.add_requirement(BuiltinType::AsyncGet);
            requirements.add_requirement(BuiltinType::AsyncPoll);
            break; // Only need to identify one async function
        }
    }

    Ok(())
}

/// Extracts embedded modules from a binary
fn extract_embedded_modules(bytes: &[u8]) -> Result<Vec<Vec<u8>>> {
    // Try to decode as component
    #[cfg(feature = "decoder")]
    {
        match wrt_decoder::component::decode_component(bytes) {
            Ok(component) => {
                // Extract modules from component
                // Let's create a simple mock implementation since component doesn't have
                // modules()
                let modules: Vec<Vec<u8>> = {
                    #[cfg(feature = "std")]
                    {
                        std::vec::Vec::new()
                    }
                    #[cfg(not(feature = "std"))]
                    {
                        Vec::new()
                    }
                }; // Create an empty vector as a placeholder
                return Ok(modules);
            },
            Err(err) => {
                return Err(Error::component_not_found("Component not found"));
            },
        }
    }
    #[cfg(not(feature = "decoder"))]
    {
        // Without decoder, return empty modules list
        Ok({
            #[cfg(feature = "std")]
            {
                std::vec::Vec::new()
            }
            #[cfg(not(feature = "std"))]
            {
                Vec::new()
            }
        })
    }
}

/// Convert a component value to a runtime value
pub fn component_value_to_value(
    component_value: &crate::prelude::WrtComponentValue<ComponentProvider>,
) -> wrt_intercept::Value {
    use wrt_intercept::Value;

    use crate::type_conversion::types_componentvalue_to_core_value;

    // Use the new conversion function
    types_componentvalue_to_core_value(component_value).unwrap_or(Value::I32(0))
    // Provide a sensible default on error
}

/// Convert a runtime value to a component value
pub fn value_to_component_value(
    value: &wrt_intercept::Value,
) -> crate::prelude::WrtComponentValue<ComponentProvider> {
    // WrtComponentValue is already imported from prelude

    use crate::type_conversion::core_value_to_types_componentvalue;

    // Use the new conversion function
    core_value_to_types_componentvalue(value).unwrap_or(WrtComponentValue::Void)
    // Provide a sensible default on error
}

/// Convert parameter to value type
pub fn convert_param_to_value_type(param: &FormatValType) -> wrt_foundation::types::ValueType {
    crate::type_conversion::format_val_type_to_value_type(param)
        .unwrap_or(wrt_foundation::types::ValueType::I32)
}

/// Convert verification level
pub fn convert_verification_level(
    level: wrt_foundation::VerificationLevel,
) -> crate::resources::VerificationLevel {
    match level {
        wrt_foundation::VerificationLevel::Off => crate::resources::VerificationLevel::None,
        wrt_foundation::VerificationLevel::Basic => crate::resources::VerificationLevel::None,
        wrt_foundation::VerificationLevel::Sampling => {
            crate::resources::VerificationLevel::Critical
        },
        wrt_foundation::VerificationLevel::Standard => {
            crate::resources::VerificationLevel::Critical
        },
        wrt_foundation::VerificationLevel::Full => crate::resources::VerificationLevel::Full,
        wrt_foundation::VerificationLevel::Redundant => crate::resources::VerificationLevel::Full,
    }
}
