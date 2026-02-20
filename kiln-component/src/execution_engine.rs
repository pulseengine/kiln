//! Component execution engine for WebAssembly Component Model
//!
//! This module provides the execution environment for WebAssembly components,
//! handling function calls, resource management, and interface interactions.

#[cfg(not(feature = "std"))]
use core::{fmt, mem};
#[cfg(feature = "std")]
use std::{boxed::Box, format, string::String, vec, vec::Vec};
#[cfg(feature = "std")]
use std::{fmt, mem};
#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, format, string::String, vec, vec::Vec};

#[cfg(not(feature = "std"))]
use wrt_foundation::{
    budget_aware_provider::CrateId, collections::StaticVec as BoundedVec,
    component_value::ComponentValue, safe_managed_alloc,
};
#[cfg(feature = "std")]
use wrt_foundation::{
    collections::StaticVec as BoundedVec, component_value::ComponentValue, prelude::*,
};

use crate::bounded_component_infra::ComponentProvider;

// Placeholder types for time-bounded execution
#[derive(Debug, Clone)]
pub struct TimeBoundedConfig {
    pub timeout_ms: u64,
}

#[derive(Debug, Clone)]
pub struct TimeBoundedContext {
    pub config: TimeBoundedConfig,
}

#[derive(Debug, Clone)]
pub enum TimeBoundedOutcome {
    Success,
    Completed,
    Timeout,
    TimedOut,
    Terminated,
    Error(String),
}

pub fn run_with_time_bounds<F, R>(_config: TimeBoundedConfig, _func: F) -> TimeBoundedOutcome
where
    F: FnOnce() -> core::result::Result<R, String>,
{
    TimeBoundedOutcome::Success
}

use crate::{
    components::component::{Component, ComponentInstance, ComponentType, ExportType, ImportType},
    memory_layout::MemoryLayout,
    prelude::WrtComponentType,
    string_encoding::StringEncoding,
    types::{ValType, Value},
    unified_execution_agent_stubs::{
        CanonicalAbi, CanonicalOptions, ComponentRuntimeBridge, ResourceHandle,
        ResourceLifecycleManager, RuntimeBridgeConfig,
    },
};

// Temporary module alias for canonical_abi types
mod canonical_abi {
    #[cfg(feature = "std")]
    pub use wrt_foundation::component_value::ComponentValue;

    pub use crate::types::ValType as ComponentType;
    #[cfg(not(feature = "std"))]
    pub use crate::types::Value as ComponentValue;
}

/// Maximum number of call frames in no_std environments
const MAX_CALL_FRAMES: usize = 256;

/// Maximum number of imported functions in no_std environments
const MAX_IMPORTS: usize = 512;

/// Represents a call frame in the component execution stack
#[derive(Debug, Clone)]
pub struct CallFrame {
    /// The component instance being executed
    pub instance_id: u32,
    /// The function being called
    pub function_index: u32,
    /// Local variables for this frame
    #[cfg(feature = "std")]
    pub locals: Vec<Value>,
    #[cfg(not(any(feature = "std",)))]
    pub locals: BoundedVec<Value, 64>,
    /// Return address information
    pub return_address: Option<u32>,
}

impl CallFrame {
    /// Create a new call frame
    pub fn new(instance_id: u32, function_index: u32) -> wrt_error::Result<Self> {
        Ok(Self {
            instance_id,
            function_index,
            #[cfg(feature = "std")]
            locals: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            locals: BoundedVec::new(),
            return_address: None,
        })
    }

    /// Push a local variable
    pub fn push_local(&mut self, value: Value) -> wrt_error::Result<()> {
        #[cfg(feature = "std")]
        {
            self.locals.push(value);
            Ok(())
        }
        #[cfg(not(any(feature = "std",)))]
        {
            self.locals
                .push(value)
                .map_err(|_| wrt_error::Error::resource_exhausted("Too many local variables"))?;
            Ok(())
        }
    }

    /// Get a local variable by index
    pub fn get_local(&self, index: usize) -> wrt_error::Result<&Value> {
        self.locals
            .get(index)
            .ok_or_else(|| wrt_error::Error::validation_invalid_input("Invalid input"))
    }

    /// Set a local variable by index
    pub fn set_local(&mut self, index: usize, value: Value) -> wrt_error::Result<()> {
        if index < self.locals.len() {
            self.locals[index] = value;
            Ok(())
        } else {
            Err(wrt_error::Error::validation_invalid_input("Invalid input"))
        }
    }
}

/// Host function callback trait
pub trait HostFunction: Send + Sync {
    /// Call the host function with the given arguments
    fn call(&mut self, args: &[Value]) -> wrt_error::Result<Value>;

    /// Get the function signature
    fn signature(&self) -> &WrtComponentType<ComponentProvider>;
}

/// Component execution engine
pub struct ComponentExecutionEngine {
    /// Call stack
    #[cfg(feature = "std")]
    call_stack: Vec<CallFrame>,
    #[cfg(not(any(feature = "std",)))]
    call_stack: BoundedVec<CallFrame, 256>,

    /// Canonical ABI processor
    canonical_abi: CanonicalAbi,

    /// Resource lifecycle manager
    resource_manager: ResourceLifecycleManager,

    /// Runtime bridge for WebAssembly Core integration
    runtime_bridge: ComponentRuntimeBridge,

    /// Host function registry (legacy - now handled by runtime bridge)
    #[cfg(feature = "std")]
    host_functions: Vec<Box<dyn HostFunction>>,
    #[cfg(not(any(feature = "std",)))]
    host_functions: BoundedVec<fn(&[Value]) -> wrt_error::Result<Value>, 64>,

    /// Current component instance
    current_instance: Option<u32>,

    /// Execution state
    state: ExecutionState,
}

impl fmt::Debug for ComponentExecutionEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ComponentExecutionEngine")
            .field("call_stack", &self.call_stack)
            .field("canonical_abi", &"<CanonicalAbi>")
            .field("resource_manager", &"<ResourceLifecycleManager>")
            .field("runtime_bridge", &"<ComponentRuntimeBridge>")
            .field("host_functions_count", &self.host_functions.len())
            .field("current_instance", &self.current_instance)
            .field("state", &self.state)
            .finish()
    }
}

/// Execution state of the engine
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionState {
    /// Ready to execute
    Ready,
    /// Currently executing
    Running,
    /// Execution completed successfully
    Completed,
    /// Execution failed with error
    Failed,
    /// Execution suspended (for async operations)
    Suspended,
}

impl fmt::Display for ExecutionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionState::Ready => write!(f, "Ready"),
            ExecutionState::Running => write!(f, "Running"),
            ExecutionState::Completed => write!(f, "Completed"),
            ExecutionState::Failed => write!(f, "Failed"),
            ExecutionState::Suspended => write!(f, "Suspended"),
        }
    }
}

impl ComponentExecutionEngine {
    /// Create a new component execution engine
    pub fn new() -> wrt_error::Result<Self> {
        Ok(Self {
            #[cfg(feature = "std")]
            call_stack: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            call_stack: BoundedVec::new(),
            canonical_abi: CanonicalAbi::new(),
            resource_manager: ResourceLifecycleManager::new(),
            runtime_bridge: ComponentRuntimeBridge::new(),
            #[cfg(feature = "std")]
            host_functions: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            host_functions: BoundedVec::new(),
            current_instance: None,
            state: ExecutionState::Ready,
        })
    }

    /// Create a new component execution engine with custom runtime bridge
    /// configuration
    pub fn with_runtime_config(bridge_config: RuntimeBridgeConfig) -> wrt_error::Result<Self> {
        Ok(Self {
            #[cfg(feature = "std")]
            call_stack: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            call_stack: BoundedVec::new(),
            canonical_abi: CanonicalAbi::new(),
            resource_manager: ResourceLifecycleManager::new(),
            runtime_bridge: ComponentRuntimeBridge::with_config(bridge_config),
            #[cfg(feature = "std")]
            host_functions: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            host_functions: BoundedVec::new(),
            current_instance: None,
            state: ExecutionState::Ready,
        })
    }

    /// Register a host function
    #[cfg(feature = "std")]
    pub fn register_host_function(
        &mut self,
        func: Box<dyn HostFunction>,
    ) -> wrt_error::Result<u32> {
        let index = self.host_functions.len() as u32;
        self.host_functions.push(func);
        Ok(index)
    }

    /// Register a host function (no_std version)
    #[cfg(not(any(feature = "std",)))]
    pub fn register_host_function(
        &mut self,
        func: fn(&[Value]) -> wrt_error::Result<Value>,
    ) -> wrt_error::Result<u32> {
        let index = self.host_functions.len() as u32;
        self.host_functions
            .push(func)
            .map_err(|_| wrt_error::Error::resource_exhausted("Too many host functions"))?;
        Ok(index)
    }

    /// Call a component function
    pub fn call_function(
        &mut self,
        instance_id: u32,
        function_index: u32,
        args: &[Value],
    ) -> wrt_error::Result<Value> {
        self.state = ExecutionState::Running;
        self.current_instance = Some(instance_id);

        // Create new call frame
        let mut frame = CallFrame::new(instance_id, function_index)?;

        // Push arguments as locals
        for arg in args {
            frame.push_local(arg.clone())?;
        }

        // Push frame to call stack
        #[cfg(feature = "std")]
        {
            self.call_stack.push(frame);
        }
        #[cfg(not(any(feature = "std",)))]
        {
            self.call_stack
                .push(frame)
                .map_err(|_| wrt_error::Error::resource_exhausted("Call stack overflow"))?;
        }

        // Execute the function
        let result = self.execute_function_internal(function_index, args);

        // Pop the frame
        #[cfg(feature = "std")]
        {
            self.call_stack.pop();
        }
        #[cfg(not(any(feature = "std",)))]
        {
            let _ = self.call_stack.pop();
        }

        // Update state based on result
        match &result {
            Ok(_) => self.state = ExecutionState::Completed,
            Err(_) => self.state = ExecutionState::Failed,
        }

        self.current_instance = None;
        result
    }

    /// Execute function internal implementation
    fn execute_function_internal(
        &mut self,
        function_index: u32,
        args: &[Value],
    ) -> wrt_error::Result<Value> {
        // Get current instance ID
        let instance_id = self
            .current_instance
            .ok_or_else(|| wrt_error::Error::runtime_error("No current instance set"))?;

        // Convert component values to canonical ABI format
        let component_values = self.convert_values_to_component(args)?;

        // Delegate to runtime bridge for execution
        let function_name = {
            #[cfg(feature = "std")]
            {
                format!("func_{}", function_index)
            }
            #[cfg(not(any(feature = "std",)))]
            {
                // Use static string for function name placeholder
                "func_unknown"
            }
        };
        let result = self
            .runtime_bridge
            .execute_component_function(instance_id, &function_name, &component_values)
            .map_err(|_| wrt_error::Error::runtime_error("Failed to execute component function"))?;

        // Convert result back to engine value format
        self.convert_component_value_to_value(&result)
    }

    /// Call a host function
    pub fn call_host_function(&mut self, index: u32, args: &[Value]) -> wrt_error::Result<Value> {
        #[cfg(feature = "std")]
        {
            if let Some(func) = self.host_functions.get_mut(index as usize) {
                func.call(args)
            } else {
                Err(wrt_error::Error::validation_invalid_input("Invalid input"))
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            if let Some(func) = self.host_functions.get(index as usize) {
                func(args)
            } else {
                Err(wrt_error::Error::validation_invalid_input("Invalid input"))
            }
        }
    }

    /// Get the current execution state
    pub fn state(&self) -> &ExecutionState {
        &self.state
    }

    /// Get the current call stack depth
    pub fn call_stack_depth(&self) -> usize {
        self.call_stack.len()
    }

    /// Get the current instance ID
    pub fn current_instance(&self) -> Option<u32> {
        self.current_instance
    }

    /// Create a new resource
    pub fn create_resource(
        &mut self,
        type_id: u32,
        data: ComponentValue<ComponentProvider>,
    ) -> wrt_error::Result<ResourceHandle> {
        self.resource_manager.create_resource(type_id, data)
    }

    /// Drop a resource
    pub fn drop_resource(&mut self, handle: ResourceHandle) -> wrt_error::Result<()> {
        self.resource_manager.drop_resource(handle)
    }

    /// Borrow a resource
    pub fn borrow_resource(
        &mut self,
        handle: ResourceHandle,
    ) -> wrt_error::Result<&ComponentValue<ComponentProvider>> {
        self.resource_manager.borrow_resource(handle)
    }

    /// Transfer resource ownership
    pub fn transfer_resource(
        &mut self,
        handle: ResourceHandle,
        new_owner: u32,
    ) -> wrt_error::Result<()> {
        self.resource_manager.transfer_ownership(handle, new_owner)
    }

    /// Reset the execution engine
    pub fn reset(&mut self) {
        #[cfg(feature = "std")]
        {
            self.call_stack.clear();
        }
        #[cfg(not(any(feature = "std",)))]
        {
            self.call_stack.clear();
        }

        self.current_instance = None;
        self.state = ExecutionState::Ready;
        self.resource_manager = ResourceLifecycleManager::new();
    }

    /// Get canonical ABI processor
    pub fn canonical_abi(&self) -> &CanonicalAbi {
        &self.canonical_abi
    }

    /// Get mutable canonical ABI processor
    pub fn canonical_abi_mut(&mut self) -> &mut CanonicalAbi {
        &mut self.canonical_abi
    }

    /// Get resource manager
    pub fn resource_manager(&self) -> &ResourceLifecycleManager {
        &self.resource_manager
    }

    /// Get mutable resource manager
    pub fn resource_manager_mut(&mut self) -> &mut ResourceLifecycleManager {
        &mut self.resource_manager
    }

    /// Get runtime bridge
    pub fn runtime_bridge(&self) -> &ComponentRuntimeBridge {
        &self.runtime_bridge
    }

    /// Get mutable runtime bridge
    pub fn runtime_bridge_mut(&mut self) -> &mut ComponentRuntimeBridge {
        &mut self.runtime_bridge
    }

    /// Convert engine values to component values
    #[cfg(feature = "std")]
    fn convert_values_to_component(
        &self,
        values: &[Value],
    ) -> wrt_error::Result<Vec<ComponentValue<ComponentProvider>>> {
        let mut component_values = Vec::new();
        for value in values {
            let component_value = self.convert_value_to_component(value)?;
            component_values.push(component_value);
        }
        Ok(component_values)
    }

    /// Convert engine values to component values (no_std version)
    #[cfg(not(any(feature = "std",)))]
    fn convert_values_to_component(
        &self,
        values: &[Value],
    ) -> wrt_error::Result<Vec<ComponentValue<ComponentProvider>>> {
        let mut component_values = Vec::new();
        for value in values {
            let component_value = self.convert_value_to_component(value)?;
            component_values.push(component_value);
        }
        Ok(component_values)
    }

    /// Convert a single engine value to component value
    fn convert_value_to_component(
        &self,
        value: &Value,
    ) -> wrt_error::Result<ComponentValue<ComponentProvider>> {
        match value {
            Value::Bool(b) => Ok(ComponentValue::Bool(*b)),
            Value::U8(v) => Ok(ComponentValue::U8(*v)),
            Value::U16(v) => Ok(ComponentValue::U16(*v)),
            Value::U32(v) => Ok(ComponentValue::U32(*v)),
            Value::U64(v) => Ok(ComponentValue::U64(*v)),
            Value::S8(v) => Ok(ComponentValue::S8(*v)),
            Value::S16(v) => Ok(ComponentValue::S16(*v)),
            Value::S32(v) => Ok(ComponentValue::S32(*v)),
            Value::S64(v) => Ok(ComponentValue::S64(*v)),
            Value::F32(v) => Ok(ComponentValue::F32(
                wrt_foundation::float_repr::FloatBits32::from_f32(*v),
            )),
            Value::F64(v) => Ok(ComponentValue::F64(
                wrt_foundation::float_repr::FloatBits64::from_f64(*v),
            )),
            Value::Char(c) => Ok(ComponentValue::Char(*c)),
            #[cfg(feature = "std")]
            Value::String(s) => {
                // Convert BoundedString to std String for ComponentValue
                let str_slice = s.as_str().map_err(|_| {
                    wrt_error::Error::validation_invalid_input(
                        "Failed to convert BoundedString to str",
                    )
                })?;
                Ok(ComponentValue::String(str_slice.to_string()))
            },
            #[cfg(not(any(feature = "std",)))]
            Value::String(s) => {
                // Convert BoundedString to String for ComponentValue
                let str_slice = s.as_str().map_err(|_| {
                    wrt_error::Error::validation_invalid_input(
                        "Failed to convert BoundedString to str",
                    )
                })?;
                Ok(ComponentValue::String(String::from(str_slice)))
            },
            _ => Err(wrt_error::Error::validation_invalid_input("Invalid input")),
        }
    }

    /// Convert component value back to engine value
    fn convert_component_value_to_value(
        &self,
        component_value: &ComponentValue<ComponentProvider>,
    ) -> wrt_error::Result<Value> {
        match component_value {
            ComponentValue::Bool(b) => Ok(Value::Bool(*b)),
            ComponentValue::U8(v) => Ok(Value::U8(*v)),
            ComponentValue::U16(v) => Ok(Value::U16(*v)),
            ComponentValue::U32(v) => Ok(Value::U32(*v)),
            ComponentValue::U64(v) => Ok(Value::U64(*v)),
            ComponentValue::S8(v) => Ok(Value::S8(*v)),
            ComponentValue::S16(v) => Ok(Value::S16(*v)),
            ComponentValue::S32(v) => Ok(Value::S32(*v)),
            ComponentValue::S64(v) => Ok(Value::S64(*v)),
            ComponentValue::F32(v) => Ok(Value::F32(v.to_f32())),
            ComponentValue::F64(v) => Ok(Value::F64(v.to_f64())),
            ComponentValue::Char(c) => Ok(Value::Char(*c)),
            #[cfg(feature = "std")]
            ComponentValue::String(s) => {
                // Convert std String to BoundedString for Value
                let _provider = safe_managed_alloc!(2048, CrateId::Component).map_err(|_| {
                    wrt_error::Error::validation_invalid_input(
                        "Failed to allocate memory provider for string conversion",
                    )
                })?;
                let bounded_str = wrt_foundation::bounded::BoundedString::try_from_str(s.as_str())
                    .map_err(|_| {
                        wrt_error::Error::validation_invalid_input(
                            "Failed to convert String to BoundedString",
                        )
                    })?;
                Ok(Value::String(bounded_str))
            },
            #[cfg(not(any(feature = "std",)))]
            ComponentValue::String(s) => {
                // Convert String to BoundedString for Value
                let _provider = safe_managed_alloc!(2048, CrateId::Component).map_err(|_| {
                    wrt_error::Error::validation_invalid_input(
                        "Failed to allocate memory provider for string conversion",
                    )
                })?;
                let bounded_str = wrt_foundation::bounded::BoundedString::try_from_str(s.as_str())
                    .map_err(|_| {
                        wrt_error::Error::validation_invalid_input(
                            "Failed to convert String to BoundedString",
                        )
                    })?;
                Ok(Value::String(bounded_str))
            },
            _ => Err(wrt_error::Error::validation_invalid_input("Invalid input")),
        }
    }

    /// Register a component instance with the runtime bridge
    pub fn register_component_instance(
        &mut self,
        component_id: u32,
        module_name: &str,
        function_count: u32,
        memory_size: u32,
    ) -> wrt_error::Result<u32> {
        let module_name_string = {
            #[cfg(feature = "std")]
            {
                String::from(module_name)
            }
            #[cfg(not(any(feature = "std",)))]
            {
                // In no_std mode, convert BoundedString to String for runtime_bridge
                let provider = safe_managed_alloc!(512, CrateId::Component)
                    .map_err(|_| wrt_error::Error::validation_invalid_input("Invalid input"))?;
                let bounded: wrt_foundation::bounded::BoundedString<256> =
                    wrt_foundation::bounded::BoundedString::try_from_str(module_name)
                        .map_err(|_| wrt_error::Error::validation_invalid_input("Invalid input"))?;
                let str_slice = bounded.as_str().map_err(|_| {
                    wrt_error::Error::validation_invalid_input(
                        "Failed to convert BoundedString to str",
                    )
                })?;
                String::from(str_slice)
            }
        };
        self.runtime_bridge
            .register_component_instance(
                component_id,
                module_name_string,
                function_count,
                memory_size,
            )
            .map_err(|_| wrt_error::Error::runtime_error("Conversion error"))
    }

    /// Register a host function with the runtime bridge
    #[cfg(feature = "std")]
    pub fn register_runtime_host_function<F>(
        &mut self,
        name: &str,
        func: F,
    ) -> wrt_error::Result<usize>
    where
        F: Fn(
                &[ComponentValue<ComponentProvider>],
            )
                -> core::result::Result<ComponentValue<ComponentProvider>, wrt_error::Error>
            + Send
            + Sync
            + 'static,
    {
        use crate::canonical_abi::ComponentType;
        use wrt_foundation::{budget_aware_provider::CrateId, safe_managed_alloc};

        let name_string = String::from(name);

        // Create params and returns using component_instantiation types
        use crate::component_instantiation::FunctionSignature;
        #[cfg(feature = "std")]
        let signature = FunctionSignature {
            name: name_string.clone(),
            params: vec![ComponentType::S32],
            returns: vec![ComponentType::S32],
        };
        #[cfg(not(feature = "std"))]
        let signature = {
            let provider_params =
                safe_managed_alloc!(4096, CrateId::Component).expect("Memory allocation failed");
            let mut params =
                wrt_foundation::BoundedVec::<ComponentType, 16, ComponentProvider>::new(
                    provider_params,
                )
                .expect("BoundedVec creation failed");
            params.push(ComponentType::S32).expect("Push failed");

            let provider_returns =
                safe_managed_alloc!(4096, CrateId::Component).expect("Memory allocation failed");
            let mut returns =
                wrt_foundation::BoundedVec::<ComponentType, 16, ComponentProvider>::new(
                    provider_returns,
                )
                .expect("BoundedVec creation failed");
            returns.push(ComponentType::S32).expect("Push failed");

            FunctionSignature {
                name: name_string.clone(),
                params,
                returns,
            }
        };

        self.runtime_bridge
            .register_host_function(name_string, signature, func)
            .map_err(|_| wrt_error::Error::runtime_error("Conversion error"))
    }

    /// Register a host function with the runtime bridge (no_std version)
    #[cfg(not(any(feature = "std",)))]
    pub fn register_runtime_host_function(
        &mut self,
        name: &str,
        func: fn(
            &[ComponentValue<ComponentProvider>],
        )
            -> core::result::Result<ComponentValue<ComponentProvider>, wrt_error::Error>,
    ) -> wrt_error::Result<usize> {
        use crate::canonical_abi::ComponentType;

        let provider = safe_managed_alloc!(512, CrateId::Component)
            .map_err(|_| wrt_error::Error::validation_invalid_input("Invalid input"))?;
        let name_bounded: wrt_foundation::bounded::BoundedString<64> =
            wrt_foundation::bounded::BoundedString::try_from_str(name)
                .map_err(|_| wrt_error::Error::validation_invalid_input("Invalid input"))?;

        // Convert BoundedString to String for FunctionSignature.name
        let name_str = name_bounded.as_str().map_err(|_| {
            wrt_error::Error::validation_invalid_input("Failed to convert BoundedString to str")
        })?;
        let name_string = String::from(name_str);

        let signature = crate::component_instantiation::FunctionSignature {
            name: name_string,
            params: wrt_foundation::collections::StaticVec::from_slice(&[ComponentType::S32])
                .map_err(|_| wrt_error::Error::resource_exhausted("Too many parameters"))?,
            returns: wrt_foundation::collections::StaticVec::from_slice(&[ComponentType::S32])
                .map_err(|_| wrt_error::Error::resource_exhausted("Too many return values"))?,
        };

        self.runtime_bridge
            .register_host_function(name_bounded, signature, func)
            .map_err(|_| wrt_error::Error::runtime_error("Conversion error"))
    }
}

impl Default for ComponentExecutionEngine {
    fn default() -> Self {
        Self::new().expect("Failed to create default ComponentExecutionEngine")
    }
}

/// Execution context for component calls
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Memory layout information
    pub memory_layout: MemoryLayout,
    /// String encoding options
    pub string_encoding: StringEncoding,
    /// Canonical options
    pub canonical_options: CanonicalOptions,
    /// Maximum call depth
    pub max_call_depth: u32,
    /// Maximum memory usage
    pub max_memory: u32,
}

impl ExecutionContext {
    /// Create a new execution context
    pub fn new() -> Self {
        Self {
            memory_layout: MemoryLayout::new(1, 1),
            string_encoding: StringEncoding::Utf8,
            canonical_options: CanonicalOptions,
            max_call_depth: 1024,
            max_memory: 1024 * 1024, // 1MB default
        }
    }

    /// Set memory layout
    pub fn with_memory_layout(mut self, layout: MemoryLayout) -> Self {
        self.memory_layout = layout;
        self
    }

    /// Set string encoding
    pub fn with_string_encoding(mut self, encoding: StringEncoding) -> Self {
        self.string_encoding = encoding;
        self
    }

    /// Set canonical options
    pub fn with_canonical_options(mut self, options: CanonicalOptions) -> Self {
        self.canonical_options = options;
        self
    }

    /// Set maximum call depth
    pub fn with_max_call_depth(mut self, depth: u32) -> Self {
        self.max_call_depth = depth;
        self
    }

    /// Set maximum memory usage
    pub fn with_max_memory(mut self, memory: u32) -> Self {
        self.max_memory = memory;
        self
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self::new()
    }
}
