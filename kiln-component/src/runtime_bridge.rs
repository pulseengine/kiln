//! Runtime Bridge for WebAssembly Core Integration
//!
//! This module provides the bridge between the Component Model execution engine
//! and the underlying WebAssembly Core runtime, enabling actual execution of
//! WebAssembly code within the Component Model framework.
//!
//! # Features
//!
//! - **Core Function Execution**: Bridge component function calls to WebAssembly Core execution
//! - **Value Conversion**: Convert between Component Model values and Core WebAssembly values
//! - **Instance Management**: Map component instances to WebAssembly module instances
//! - **Host Function Integration**: Enable calling host functions from components
//! - **Cross-Environment Support**: Works in std, no_std+alloc, and pure no_std
//!
//! # Core Concepts
//!
//! - **RuntimeBridge**: Main trait for integrating with WebAssembly runtimes
//! - **ValueConverter**: Handles conversion between value types
//! - **InstanceResolver**: Maps component instances to runtime instances
//! - **HostRegistry**: Manages host function registration and invocation


// Cross-environment imports
#[cfg(feature = "std")]
use std::{vec::Vec, string::String, collections::HashMap, boxed::Box, format};

#[cfg(all(not(feature = "std")))]
use std::{vec::Vec, string::String, collections::BTreeMap as HashMap, boxed::Box, format};

#[cfg(not(any(feature = "std", )))]
use wrt_foundation::{BoundedVec as Vec, BoundedString as String, BoundedMap as HashMap};

use wrt_error::{Error, ErrorCategory, Result, codes};
use wrt_foundation::{values::Value as CoreValue, types::ValueType};
use wrt_runtime::{ExecutionStats, Module, ModuleInstance};

use crate::prelude::*;

// Import our component types
use crate::canonical_abi::ComponentValue;
use crate::component_instantiation::{InstanceId, ComponentInstance, FunctionSignature};
use crate::execution_engine::{ExecutionContext, ExecutionState};

/// Maximum number of instances in no_std environments
const MAX_INSTANCES_NO_STD: usize = 64;

/// Maximum number of host functions in no_std environments
const MAX_HOST_FUNCTIONS_NO_STD: usize = 256;

/// Runtime bridge trait for integrating with WebAssembly Core execution
pub trait RuntimeBridge {
    /// Execute a WebAssembly Core function
    fn execute_core_function(
        &mut self,
        module_instance: &mut ModuleInstance,
        function_index: u32,
        args: &[CoreValue],
    ) -> Result<CoreValue>;

    /// Get function signature from module
    fn get_function_signature(
        &self,
        module_instance: &ModuleInstance,
        function_index: u32,
    ) -> Result<FunctionSignature>;

    /// Check if function exists in module
    fn has_function(&self, module_instance: &ModuleInstance, function_index: u32) -> bool;

    /// Get execution statistics
    fn get_execution_stats(&self) -> &ExecutionStats;

    /// Reset execution statistics
    fn reset_execution_stats(&mut self;
}

/// Value converter for translating between Component and Core value types
#[derive(Debug)]
pub struct ValueConverter {
    /// Conversion cache for performance
    #[cfg(feature = "std")]
    conversion_cache: HashMap<String, ConversionRule>,
    
    /// Configuration
    config: ValueConversionConfig,
}

/// Value conversion configuration
#[derive(Debug, Clone)]
pub struct ValueConversionConfig {
    /// Enable strict type checking
    pub strict_type_checking: bool,
    /// Enable conversion caching
    pub enable_caching: bool,
    /// Maximum string length for conversion
    pub max_string_length: usize,
    /// Maximum array/list length for conversion
    pub max_array_length: usize,
}

/// Conversion rule for value types
#[derive(Debug, Clone)]
pub struct ConversionRule {
    /// Source type name
    pub source_type: String,
    /// Target type name
    pub target_type: String,
    /// Conversion complexity
    pub complexity: ConversionComplexity,
    /// Whether conversion is lossy
    pub lossy: bool,
}

/// Conversion complexity levels
#[derive(Debug, Clone, PartialEq)]
pub enum ConversionComplexity {
    /// Direct mapping (no conversion needed)
    Direct,
    /// Simple conversion (e.g., widening)
    Simple,
    /// Complex conversion (e.g., string encoding)
    Complex,
    /// Expensive conversion (e.g., serialization)
    Expensive,
}

/// Instance resolver for mapping component instances to runtime instances
#[derive(Debug)]
pub struct InstanceResolver {
    /// Instance mappings
    #[cfg(feature = "std")]
    instances: HashMap<InstanceId, RuntimeInstanceInfo>,
    
    #[cfg(not(any(feature = "std", )))]
    instances: Vec<(InstanceId, RuntimeInstanceInfo)>,
    
    /// Next instance ID
    next_instance_id: InstanceId,
}

/// Runtime instance information
#[derive(Debug, Clone)]
pub struct RuntimeInstanceInfo {
    /// Component instance ID
    pub component_id: InstanceId,
    /// Module instance (simplified representation)
    pub module_name: String,
    /// Function count
    pub function_count: u32,
    /// Memory size in bytes
    pub memory_size: u32,
    /// Instance state
    pub state: RuntimeInstanceState,
}

/// Runtime instance state
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeInstanceState {
    /// Instance is being initialized
    Initializing,
    /// Instance is ready for execution
    Ready,
    /// Instance is currently executing
    Executing,
    /// Instance execution failed
    Failed(String),
    /// Instance has been terminated
    Terminated,
}

/// Host function registry for managing host functions
#[derive(Debug)]
pub struct HostFunctionRegistry {
    /// Registered host functions
    #[cfg(feature = "std")]
    functions: Vec<HostFunctionEntry>,
    
    #[cfg(not(any(feature = "std", )))]
    functions: Vec<HostFunctionEntry>,
    
    /// Function name lookup
    #[cfg(feature = "std")]
    name_lookup: HashMap<String, usize>,
}

/// Host function entry
#[derive(Debug)]
pub struct HostFunctionEntry {
    /// Function name
    pub name: String,
    /// Function signature
    pub signature: FunctionSignature,
    /// Function implementation
    #[cfg(feature = "std")]
    pub implementation: Box<dyn Fn(&[ComponentValue]) -> Result<ComponentValue<ComponentProvider>> + Send + Sync>,
    
    #[cfg(not(any(feature = "std", )))]
    pub implementation: fn(&[ComponentValue]) -> Result<ComponentValue<ComponentProvider>>,
    
    /// Function metadata
    pub metadata: HostFunctionMetadata,
}

/// Host function metadata
#[derive(Debug, Clone)]
pub struct HostFunctionMetadata {
    /// Function description
    pub description: String,
    /// Expected parameter count
    pub parameter_count: usize,
    /// Return value count
    pub return_count: usize,
    /// Whether function is pure (no side effects)
    pub is_pure: bool,
    /// Performance characteristics
    pub performance_hint: PerformanceHint,
}

/// Performance hint for host functions
#[derive(Debug, Clone, PartialEq)]
pub enum PerformanceHint {
    /// Fast function (< 1μs typical)
    Fast,
    /// Normal function (< 100μs typical)
    Normal,
    /// Slow function (< 10ms typical)
    Slow,
    /// Very slow function (> 10ms typical)
    VerySlow,
}

/// Main runtime bridge implementation
#[derive(Debug)]
pub struct ComponentRuntimeBridge {
    /// Value converter
    value_converter: ValueConverter,
    /// Instance resolver
    instance_resolver: InstanceResolver,
    /// Host function registry
    host_registry: HostFunctionRegistry,
    /// Execution statistics
    execution_stats: ExecutionStats,
    /// Bridge configuration
    config: RuntimeBridgeConfig,
}

/// Runtime bridge configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBridgeConfig {
    /// Enable execution tracing
    pub enable_tracing: bool,
    /// Enable performance monitoring
    pub enable_monitoring: bool,
    /// Maximum function call depth
    pub max_call_depth: u32,
    /// Function execution timeout (microseconds)
    pub execution_timeout_us: u64,
    /// Enable host function calls
    pub enable_host_functions: bool,
}

impl Default for ValueConversionConfig {
    fn default() -> Self {
        Self {
            strict_type_checking: true,
            enable_caching: true,
            max_string_length: 65536,
            max_array_length: 4096,
        }
    }
}

impl Default for RuntimeBridgeConfig {
    fn default() -> Self {
        Self {
            enable_tracing: false,
            enable_monitoring: true,
            max_call_depth: 64,
            execution_timeout_us: 5_000_000, // 5 seconds
            enable_host_functions: true,
        }
    }
}

impl ValueConverter {
    /// Create a new value converter
    pub fn new() -> Self {
        Self::with_config(ValueConversionConfig::default()
    }

    /// Create a value converter with custom configuration
    pub fn with_config(config: ValueConversionConfig) -> Self {
        Self {
            #[cfg(feature = "std")]
            conversion_cache: HashMap::new(),
            config,
        }
    }

    /// Convert a component value to a core value
    pub fn component_to_core(&self, value: &ComponentValue) -> Result<CoreValue> {
        match value {
            ComponentValue::Bool(b) => Ok(CoreValue::I32(if *b { 1 } else { 0 })),
            ComponentValue::S8(v) => Ok(CoreValue::I32(*v as i32)),
            ComponentValue::U8(v) => Ok(CoreValue::I32(*v as i32)),
            ComponentValue::S16(v) => Ok(CoreValue::I32(*v as i32)),
            ComponentValue::U16(v) => Ok(CoreValue::I32(*v as i32)),
            ComponentValue::S32(v) => Ok(CoreValue::I32(*v)),
            ComponentValue::U32(v) => Ok(CoreValue::I32(*v as i32)),
            ComponentValue::S64(v) => Ok(CoreValue::I64(*v)),
            ComponentValue::U64(v) => Ok(CoreValue::I64(*v as i64)),
            ComponentValue::F32(v) => Ok(CoreValue::F32(*v)),
            ComponentValue::F64(v) => Ok(CoreValue::F64(*v)),
            ComponentValue::Char(c) => Ok(CoreValue::I32(*c as i32)),
            ComponentValue::String(s) => {
                if s.len() > self.config.max_string_length {
                    return Err(Error::validation_error("Error occurred";
                }
                // For now, return string length as i32
                // Binary std/no_std choice
                Ok(CoreValue::I32(s.len() as i32)
            }
            ComponentValue::List(items) => {
                if items.len() > self.config.max_array_length {
                    return Err(Error::validation_error("Error occurred";
                }
                // Return list length for now
                Ok(CoreValue::I32(items.len() as i32)
            }
            _ => {
                // Complex types need special handling
                if self.config.strict_type_checking {
                    Err(Error::runtime_type_mismatch("Error occurred")
                } else {
                    // Fallback to zero value
                    Ok(CoreValue::I32(0)
                }
            }
        }
    }

    /// Convert a core value to a component value
    pub fn core_to_component(&self, value: &CoreValue, target_type: &crate::canonical_abi::ComponentType) -> Result<ComponentValue<ComponentProvider>> {
        match (value, target_type) {
            (CoreValue::I32(v), crate::canonical_abi::ComponentType::Bool) => Ok(ComponentValue::Bool(*v != 0)),
            (CoreValue::I32(v), crate::canonical_abi::ComponentType::S8) => Ok(ComponentValue::S8(*v as i8)),
            (CoreValue::I32(v), crate::canonical_abi::ComponentType::U8) => Ok(ComponentValue::U8(*v as u8)),
            (CoreValue::I32(v), crate::canonical_abi::ComponentType::S16) => Ok(ComponentValue::S16(*v as i16)),
            (CoreValue::I32(v), crate::canonical_abi::ComponentType::U16) => Ok(ComponentValue::U16(*v as u16)),
            (CoreValue::I32(v), crate::canonical_abi::ComponentType::S32) => Ok(ComponentValue::S32(*v)),
            (CoreValue::I32(v), crate::canonical_abi::ComponentType::U32) => Ok(ComponentValue::U32(*v as u32)),
            (CoreValue::I64(v), crate::canonical_abi::ComponentType::S64) => Ok(ComponentValue::S64(*v)),
            (CoreValue::I64(v), crate::canonical_abi::ComponentType::U64) => Ok(ComponentValue::U64(*v as u64)),
            (CoreValue::F32(v), crate::canonical_abi::ComponentType::F32) => Ok(ComponentValue::F32(*v)),
            (CoreValue::F64(v), crate::canonical_abi::ComponentType::F64) => Ok(ComponentValue::F64(*v)),
            (CoreValue::I32(v), crate::canonical_abi::ComponentType::Char) => {
                Ok(ComponentValue::Char(char::from_u32(*v as u32).unwrap_or('\0'))
            }
            _ => {
                if self.config.strict_type_checking {
                    Err(Error::runtime_type_mismatch("Error occurred")
                } else {
                    // Fallback conversion
                    Ok(ComponentValue::S32(0)
                }
            }
        }
    }

    /// Convert multiple values
    pub fn convert_values_component_to_core(&self, values: &[ComponentValue]) -> Result<Vec<CoreValue>> {
        let mut core_values = Vec::new();
        for value in values {
            core_values.push(self.component_to_core(value)?;
        }
        Ok(core_values)
    }

    /// Convert multiple values from core to component
    pub fn convert_values_core_to_component(
        &self, 
        values: &[CoreValue], 
        types: &[crate::canonical_abi::ComponentType]
    ) -> Result<Vec<ComponentValue>> {
        if values.len() != types.len() {
            return Err(Error::validation_error("Error occurred";
        }

        let mut component_values = Vec::new();
        for (value, target_type) in values.iter().zip(types.iter()) {
            component_values.push(self.core_to_component(value, target_type)?;
        }
        Ok(component_values)
    }

    /// Check if conversion is supported
    pub fn is_conversion_supported(&self, from: &ComponentValue, to: &ValueType) -> bool {
        // Simplified check - in practice this would be more comprehensive
        match (from, to) {
            (ComponentValue::S32(_), ValueType::I32) => true,
            (ComponentValue::S64(_), ValueType::I64) => true,
            (ComponentValue::F32(_), ValueType::F32) => true,
            (ComponentValue::F64(_), ValueType::F64) => true,
            _ => false,
        }
    }
}

impl InstanceResolver {
    /// Create a new instance resolver
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "std")]
            instances: HashMap::new(),
            
            #[cfg(not(any(feature = "std", )))]
            instances: Vec::new(),
            
            next_instance_id: 1,
        }
    }

    /// Register a new instance
    pub fn register_instance(
        &mut self,
        component_id: InstanceId,
        module_name: String,
        function_count: u32,
        memory_size: u32,
    ) -> Result<InstanceId> {
        let runtime_info = RuntimeInstanceInfo {
            component_id,
            module_name,
            function_count,
            memory_size,
            state: RuntimeInstanceState::Initializing,
        };

        #[cfg(feature = "std")]
        {
            self.instances.insert(self.next_instance_id, runtime_info;
        }

        #[cfg(not(any(feature = "std", )))]
        {
            if self.instances.len() >= MAX_INSTANCES_NO_STD {
                return Err(Error::resource_exhausted("Error occurred";
            }
            self.instances.push((self.next_instance_id, runtime_info);
        }

        let instance_id = self.next_instance_id;
        self.next_instance_id += 1;
        Ok(instance_id)
    }

    /// Get instance information
    pub fn get_instance(&self, instance_id: InstanceId) -> Option<&RuntimeInstanceInfo> {
        #[cfg(feature = "std")]
        {
            self.instances.get(&instance_id)
        }

        #[cfg(not(any(feature = "std", )))]
        {
            self.instances.iter().find(|(id, _)| *id == instance_id).map(|(_, info)| info)
        }
    }

    /// Update instance state
    pub fn update_instance_state(&mut self, instance_id: InstanceId, state: RuntimeInstanceState) -> Result<()> {
        #[cfg(feature = "std")]
        {
            if let Some(info) = self.instances.get_mut(&instance_id) {
                info.state = state;
                Ok(()
            } else {
                Err(Error::instance_not_found("Error occurred")
            }
        }

        #[cfg(not(any(feature = "std", )))]
        {
            if let Some((_, info)) = self.instances.iter_mut().find(|(id, _)| *id == instance_id) {
                info.state = state;
                Ok(()
            } else {
                Err(Error::instance_not_found("Error occurred")
            }
        }
    }

    /// Remove an instance
    pub fn remove_instance(&mut self, instance_id: InstanceId) -> Result<()> {
        #[cfg(feature = "std")]
        {
            if self.instances.remove(&instance_id).is_some() {
                Ok(()
            } else {
                Err(Error::instance_not_found("Error occurred")
            }
        }

        #[cfg(not(any(feature = "std", )))]
        {
            if let Some(pos) = self.instances.iter().position(|(id, _)| *id == instance_id) {
                self.instances.remove(pos;
                Ok(()
            } else {
                Err(Error::instance_not_found("Error occurred")
            }
        }
    }

    /// Get instance count
    pub fn instance_count(&self) -> usize {
        #[cfg(feature = "std")]
        {
            self.instances.len()
        }

        #[cfg(not(any(feature = "std", )))]
        {
            self.instances.len()
        }
    }
}

impl HostFunctionRegistry {
    /// Create a new host function registry
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
            #[cfg(feature = "std")]
            name_lookup: HashMap::new(),
        }
    }

    /// Binary std/no_std choice
    #[cfg(feature = "std")]
    pub fn register_function<F>(&mut self, name: String, signature: FunctionSignature, func: F) -> Result<usize>
    where
        F: Fn(&[ComponentValue]) -> Result<ComponentValue<ComponentProvider>> + Send + Sync + 'static,
    {
        let index = self.functions.len();
        let entry = HostFunctionEntry {
            name: name.clone(),
            signature,
            implementation: Box::new(func),
            metadata: HostFunctionMetadata {
                description: "Component not found",
                parameter_count: 0, // Would be determined from signature
                return_count: 1,
                is_pure: false,
                performance_hint: PerformanceHint::Normal,
            },
        };

        self.functions.push(entry);
        self.name_lookup.insert(name, index;
        Ok(index)
    }

    /// Register a host function (no_std version)
    #[cfg(not(any(feature = "std", )))]
    pub fn register_function(
        &mut self,
        name: String,
        signature: FunctionSignature,
        func: fn(&[ComponentValue]) -> Result<ComponentValue<ComponentProvider>>,
    ) -> Result<usize> {
        if self.functions.len() >= MAX_HOST_FUNCTIONS_NO_STD {
            return Err(Error::resource_exhausted("Error occurred";
        }

        let index = self.functions.len();
        let entry = HostFunctionEntry {
            name,
            signature,
            implementation: func,
            metadata: HostFunctionMetadata {
                description: String::new(), // Limited in no_std
                parameter_count: 0,
                return_count: 1,
                is_pure: false,
                performance_hint: PerformanceHint::Normal,
            },
        };

        self.functions.push(entry);
        Ok(index)
    }

    /// Call a host function by index
    pub fn call_function(&self, index: usize, args: &[ComponentValue]) -> Result<ComponentValue<ComponentProvider>> {
        if let Some(entry) = self.functions.get(index) {
            #[cfg(feature = "std")]
            {
                (entry.implementation)(args)
            }

            #[cfg(not(any(feature = "std", )))]
            {
                (entry.implementation)(args)
            }
        } else {
            Err(Error::runtime_function_not_found("Error occurred")
        }
    }

    /// Find function by name
    #[cfg(feature = "std")]
    pub fn find_function(&self, name: &str) -> Option<usize> {
        self.name_lookup.get(name).copied()
    }

    /// Find function by name (no_std version)
    #[cfg(not(any(feature = "std", )))]
    pub fn find_function(&self, name: &str) -> Option<usize> {
        self.functions.iter().position(|entry| entry.name == name)
    }

    /// Get function count
    pub fn function_count(&self) -> usize {
        self.functions.len()
    }
}

impl ComponentRuntimeBridge {
    /// Create a new component runtime bridge
    pub fn new() -> Self {
        Self::with_config(RuntimeBridgeConfig::default()
    }

    /// Create a bridge with custom configuration
    pub fn with_config(config: RuntimeBridgeConfig) -> Self {
        Self {
            value_converter: ValueConverter::new(),
            instance_resolver: InstanceResolver::new(),
            host_registry: HostFunctionRegistry::new(),
            execution_stats: ExecutionStats::new(),
            config,
        }
    }

    /// Execute a component function with runtime integration
    pub fn execute_component_function(
        &mut self,
        instance_id: InstanceId,
        function_name: &str,
        args: &[ComponentValue],
    ) -> Result<ComponentValue<ComponentProvider>> {
        // Get instance information
        let instance_info = self.instance_resolver.get_instance(instance_id)
            .ok_or_else(|| Error::instance_not_found("Error occurred"))?;

        // Check instance state
        if instance_info.state != RuntimeInstanceState::Ready {
            return Err(Error::runtime_invalid_state("Error occurred"),
            ;
        }

        // Check if it's a host function call
        if let Some(host_index) = self.host_registry.find_function(function_name) {
            return self.host_registry.call_function(host_index, args;
        }

        // For now, implement a simplified execution that demonstrates the bridge
        // In a full implementation, this would:
        // 1. Look up the function in the WebAssembly module
        // 2. Convert component values to core values
        // 3. Execute the WebAssembly function
        // 4. Convert results back to component values

        // Convert arguments to core values
        let core_args = self.value_converter.convert_values_component_to_core(args)?;

        // Update execution statistics
        self.execution_stats.increment_function_calls(1;
        self.execution_stats.increment_instructions(10); // Estimated

        // Simulate function execution result
        let core_result = if !core_args.is_empty() {
            core_args[0].clone()
        } else {
            CoreValue::I32(0)
        };

        // Convert result back to component value
        let component_result = self.value_converter.core_to_component(
            &core_result,
            &crate::canonical_abi::ComponentType::S32, // Assume S32 for now
        )?;

        Ok(component_result)
    }

    /// Register a host function
    #[cfg(feature = "std")]
    pub fn register_host_function<F>(
        &mut self,
        name: String,
        signature: FunctionSignature,
        func: F,
    ) -> Result<usize>
    where
        F: Fn(&[ComponentValue]) -> Result<ComponentValue<ComponentProvider>> + Send + Sync + 'static,
    {
        self.host_registry.register_function(name, signature, func)
    }

    /// Register a host function (no_std version)
    #[cfg(not(any(feature = "std", )))]
    pub fn register_host_function(
        &mut self,
        name: String,
        signature: FunctionSignature,
        func: fn(&[ComponentValue]) -> Result<ComponentValue<ComponentProvider>>,
    ) -> Result<usize> {
        self.host_registry.register_function(name, signature, func)
    }

    /// Register a component instance
    pub fn register_component_instance(
        &mut self,
        component_id: InstanceId,
        module_name: String,
        function_count: u32,
        memory_size: u32,
    ) -> Result<InstanceId> {
        self.instance_resolver.register_instance(component_id, module_name, function_count, memory_size)
    }

    /// Get value converter
    pub fn value_converter(&self) -> &ValueConverter {
        &self.value_converter
    }

    /// Get instance resolver
    pub fn instance_resolver(&self) -> &InstanceResolver {
        &self.instance_resolver
    }

    /// Get host function registry
    pub fn host_registry(&self) -> &HostFunctionRegistry {
        &self.host_registry
    }

    /// Get execution statistics
    pub fn execution_stats(&self) -> &ExecutionStats {
        &self.execution_stats
    }

    /// Reset bridge state
    pub fn reset(&mut self) {
        self.execution_stats.reset);
        // Note: We don't reset instances and host functions as they persist
    }
}

impl Default for ValueConverter {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for InstanceResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for HostFunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ComponentRuntimeBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a component runtime bridge with default configuration
pub fn create_runtime_bridge() -> ComponentRuntimeBridge {
    ComponentRuntimeBridge::new()
}

/// Create a component runtime bridge with custom configuration
pub fn create_runtime_bridge_with_config(config: RuntimeBridgeConfig) -> ComponentRuntimeBridge {
    ComponentRuntimeBridge::with_config(config)
}

}
