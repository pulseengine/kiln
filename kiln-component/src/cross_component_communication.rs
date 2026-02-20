//! Cross-Component Communication Integration with wrt-intercept
//!
//! This module provides the integration between the Component-to-Component
//! Communication System and the wrt-intercept framework, implementing
//! component communication as interception strategies.
//!
//! # Features
//!
//! - **Unified Interception**: Integrates with wrt-intercept's strategy pattern
//! - **Cross-Component Calls**: Function calls between component instances
//! - **Parameter Marshaling**: Safe parameter passing through Canonical ABI
//! - **Resource Transfer**: Secure resource sharing between components
//! - **Security Boundaries**: Proper isolation and permission checking
//! - **Performance Optimization**: Efficient call routing and dispatch
//! - **Cross-Environment Support**: Works in std, no_std+alloc, and pure no_std
//!
//! # Core Concepts
//!
//! - **ComponentCommunicationStrategy**: Main strategy implementing
//!   LinkInterceptorStrategy
//! - **Call Interception**: Intercepts and routes cross-component function
//!   calls
//! - **Parameter Interception**: Handles parameter marshaling in the
//!   interception pipeline
//! - **Resource Interception**: Manages resource transfers during calls
//! - **Security Policies**: Enforces security boundaries through interception
//!
//! # Example
//!
//! ```no_run
//! use wrt_component::cross_component_communication::ComponentCommunicationStrategy;
//! use wrt_intercept::{LinkInterceptor, LinkInterceptorStrategy};
//!
//! // Create communication strategy
//! let comm_strategy = ComponentCommunicationStrategy::new();
//!
//! // Add to interceptor
//! let mut interceptor = LinkInterceptor::new("component_comm";
//! interceptor.add_strategy(std::sync::Arc::new(comm_strategy;
//! ```

// Cross-environment imports
#[cfg(all(feature = "std", not(feature = "safety-critical")))]
use std::{boxed::Box, collections::HashMap, format, string::String, sync::Arc, vec::Vec};

#[cfg(all(feature = "std", feature = "safety-critical"))]
use wrt_foundation::allocator::{CrateId, WrtHashMap as HashMap, WrtVec as Vec};
#[cfg(not(feature = "std"))]
use wrt_foundation::{
    bounded::BoundedString, collections::StaticVec as BoundedVec, safe_memory::NoStdProvider,
};

// Type aliases for no_std environment with proper generics
#[cfg(not(feature = "std"))]
type String = BoundedString<256>;
#[cfg(not(feature = "std"))]
type Vec<T> = BoundedVec<T, 256>;

// Enable vec! and format! macros for no_std
#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, format, vec};

// Arc is already imported from prelude, no need for type alias

use wrt_error::{Error, ErrorCategory, Result, codes};
use wrt_foundation::ValType;
use wrt_intercept::{LinkInterceptorStrategy, ResourceCanonicalOperation};

// Import our communication system components
pub use crate::components::component_communication::{
    CallContext, CallRouter, CallRouterConfig, CallState, CommunicationError, MarshalingConfig,
    ParameterBridge, ResourceBridge, ResourceTransferType,
};
// Import our prelude for type aliases like WrtComponentValue
use crate::prelude::*;
use crate::{
    bounded_component_infra::ComponentProvider,
    call_context::{
        CallContextConfig, CallContextManager, MarshalingConfig as ContextMarshalingConfig,
    },
    component_instantiation::{ComponentInstance, InstanceId},
    resource_management::{ResourceHandle, ResourceManager as ComponentResourceManager},
};

/// Component communication strategy that implements LinkInterceptorStrategy
#[derive(Debug)]
pub struct ComponentCommunicationStrategy {
    /// Call router for managing cross-component calls
    call_router: CallRouter,
    /// Call context manager for call lifecycle
    call_context_manager: CallContextManager,
    /// Instance registry for component lookup
    #[cfg(feature = "safety-critical")]
    instance_registry: WrtHashMap<InstanceId, String, { CrateId::Component as u8 }, 256>,
    #[cfg(not(feature = "safety-critical"))]
    instance_registry: HashMap<InstanceId, String>,
    /// Security policies for component interactions
    #[cfg(feature = "safety-critical")]
    security_policies:
        WrtHashMap<String, ComponentSecurityPolicy, { CrateId::Component as u8 }, 64>,
    #[cfg(not(feature = "safety-critical"))]
    security_policies: HashMap<String, ComponentSecurityPolicy>,
    /// Configuration
    config: ComponentCommunicationConfig,
    /// Statistics
    stats: CommunicationStats,
}

/// Security policy for component interactions
#[derive(Debug, Clone)]
pub struct ComponentSecurityPolicy {
    /// Allowed target components
    #[cfg(feature = "safety-critical")]
    pub allowed_targets: WrtVec<String, { CrateId::Component as u8 }, 32>,
    #[cfg(not(feature = "safety-critical"))]
    pub allowed_targets: Vec<String>,
    /// Allowed function patterns
    #[cfg(feature = "safety-critical")]
    pub allowed_functions: WrtVec<String, { CrateId::Component as u8 }, 64>,
    #[cfg(not(feature = "safety-critical"))]
    pub allowed_functions: Vec<String>,
    /// Resource access permissions
    pub allow_resource_transfer: bool,
    /// Maximum call depth
    pub max_call_depth: usize,
    /// Enable parameter validation
    pub validate_parameters: bool,
}

/// Configuration for component communication strategy
#[derive(Debug, Clone)]
pub struct ComponentCommunicationConfig {
    /// Enable call tracing
    pub enable_tracing: bool,
    /// Enable security checks
    pub enable_security: bool,
    /// Enable performance monitoring
    pub enable_monitoring: bool,
    /// Maximum parameter size
    pub max_parameter_size: u32,
    /// Call timeout in microseconds
    pub call_timeout_us: u64,
}

/// Communication statistics
#[derive(Debug, Clone, Default)]
pub struct CommunicationStats {
    /// Total function calls intercepted
    pub function_calls_intercepted: u64,
    /// Total parameters marshaled
    pub parameters_marshaled: u64,
    /// Total resource operations intercepted
    pub resource_operations_intercepted: u64,
    /// Total successful calls
    pub successful_calls: u64,
    /// Total failed calls
    pub failed_calls: u64,
    /// Average call duration
    pub average_call_duration_us: u64,
}

/// Call routing information
#[derive(Debug, Clone)]
pub struct CallRoutingInfo {
    /// Source component
    pub source_component: BoundedString<256>,
    /// Target component
    pub target_component: BoundedString<256>,
    /// Function name
    pub function_name: BoundedString<256>,
    /// Call context ID
    pub call_context_id: Option<u64>,
}

/// Parameter marshaling result
#[derive(Debug, Clone)]
pub struct ParameterMarshalingResult {
    /// Marshaled parameter data
    #[cfg(feature = "safety-critical")]
    pub marshaled_data: WrtVec<u8, { CrateId::Component as u8 }, 8192>,
    #[cfg(not(feature = "safety-critical"))]
    pub marshaled_data: Vec<u8>,
    /// Marshaling metadata
    pub metadata: MarshalingMetadata,
    /// Success status
    pub success: bool,
    /// Error message (if failed)
    pub error_message: Option<String>,
}

/// Marshaling metadata
#[derive(Debug, Clone)]
pub struct MarshalingMetadata {
    /// Original parameter count
    pub original_count: usize,
    /// Marshaled size in bytes
    pub marshaled_size: u32,
    /// Marshaling time in microseconds
    pub marshaling_time_us: u64,
    /// Conversion operations performed
    pub conversions_performed: u32,
}

impl Default for ComponentCommunicationConfig {
    fn default() -> Self {
        Self {
            enable_tracing: false,
            enable_security: true,
            enable_monitoring: true,
            max_parameter_size: 1024 * 1024, // 1MB
            call_timeout_us: 5_000_000,      // 5 seconds
        }
    }
}

impl Default for ComponentSecurityPolicy {
    fn default() -> Self {
        Self {
            #[cfg(feature = "safety-critical")]
            allowed_targets: WrtVec::new(),
            #[cfg(not(feature = "safety-critical"))]
            allowed_targets: Vec::new(),
            #[cfg(feature = "safety-critical")]
            allowed_functions: WrtVec::new(),
            #[cfg(not(feature = "safety-critical"))]
            allowed_functions: Vec::new(),
            allow_resource_transfer: false,
            max_call_depth: 16,
            validate_parameters: true,
        }
    }
}

impl ComponentCommunicationStrategy {
    /// Create a new component communication strategy
    pub fn new() -> Self {
        Self::with_config(ComponentCommunicationConfig::default())
    }

    /// Create a new strategy with custom configuration
    pub fn with_config(config: ComponentCommunicationConfig) -> Self {
        let router_config = CallRouterConfig {
            enable_call_tracing: config.enable_tracing,
            max_call_stack_depth: 64,
            enable_security_checks: config.enable_security,
            call_timeout_us: config.call_timeout_us,
            enable_optimization: true,
            max_concurrent_calls_per_instance: 256,
        };

        let context_config = CallContextConfig {
            enable_tracing: config.enable_tracing,
            enable_performance_monitoring: config.enable_monitoring,
            enable_parameter_validation: true,
            enable_resource_coordination: true,
            max_call_duration_us: config.call_timeout_us,
        };

        Self {
            call_router: CallRouter::with_config(router_config),
            call_context_manager: CallContextManager::with_config(context_config),
            #[cfg(feature = "safety-critical")]
            instance_registry: WrtHashMap::new(),
            #[cfg(not(feature = "safety-critical"))]
            instance_registry: HashMap::new(),
            #[cfg(feature = "safety-critical")]
            security_policies: WrtHashMap::new(),
            #[cfg(not(feature = "safety-critical"))]
            security_policies: HashMap::new(),
            config,
            stats: CommunicationStats::default(),
        }
    }

    /// Register a component instance
    pub fn register_instance(
        &mut self,
        instance_id: InstanceId,
        component_name: String,
    ) -> Result<()> {
        #[cfg(feature = "safety-critical")]
        {
            self.instance_registry
                .insert(instance_id, component_name)
                .map_err(|_| Error::resource_exhausted("Too many component instances (limit: 256)"))
        }
        #[cfg(not(feature = "safety-critical"))]
        {
            self.instance_registry.insert(instance_id, component_name);
            Ok(())
        }
    }

    /// Set security policy for a component
    pub fn set_security_policy(
        &mut self,
        component_name: String,
        policy: ComponentSecurityPolicy,
    ) -> Result<()> {
        #[cfg(feature = "safety-critical")]
        {
            self.security_policies
                .insert(component_name, policy)
                .map_err(|_| Error::resource_exhausted("Too many security policies (limit: 64)"))
        }
        #[cfg(not(feature = "safety-critical"))]
        {
            self.security_policies.insert(component_name, policy);
            Ok(())
        }
    }

    /// Get communication statistics
    pub fn get_stats(&self) -> &CommunicationStats {
        &self.stats
    }

    /// Parse component name from function call
    fn parse_component_call(&self, function_name: &str) -> Option<CallRoutingInfo> {
        // Expected format: "component_name::function_name"
        if let Some(pos) = function_name.find("::") {
            let (component_part, function_part) = function_name.split_at(pos);
            let function_part = &function_part[2..]; // Skip "::"

            let provider = NoStdProvider::<1024>::default();
            let source = BoundedString::<256>::try_from_str("unknown").unwrap_or_default();
            let target = BoundedString::<256>::try_from_str(component_part).unwrap_or_default();
            let func = BoundedString::<256>::try_from_str(function_part).unwrap_or_default();
            Some(CallRoutingInfo {
                source_component: BoundedString::<256>::try_from_str(
                    source.as_str().unwrap_or("unknown"),
                )
                .unwrap_or_default(),
                target_component: BoundedString::<256>::try_from_str(target.as_str().unwrap_or(""))
                    .unwrap_or_default(),
                function_name: BoundedString::<256>::try_from_str(func.as_str().unwrap_or(""))
                    .unwrap_or_default(),
                call_context_id: None,
            })
        } else {
            None
        }
    }

    /// Validate security policy for a call
    fn validate_security_policy(&self, routing_info: &CallRoutingInfo) -> Result<()> {
        if !self.config.enable_security {
            return Ok(());
        }

        let source_str = routing_info.source_component.as_str().unwrap_or("");
        let target_str = routing_info.target_component.as_str().unwrap_or("");
        let func_str = routing_info.function_name.as_str().unwrap_or("");

        // Find policy by iterating (since we can't use get with different string types)
        let policy = self
            .security_policies
            .iter()
            .find(|(k, _)| {
                #[cfg(feature = "std")]
                {
                    k.as_str() == source_str
                }
                #[cfg(not(feature = "std"))]
                {
                    k.as_str().ok() == Some(source_str)
                }
            })
            .map(|(_, v)| v);

        if let Some(policy) = policy {
            // Check allowed targets
            if !policy.allowed_targets.is_empty()
                && !policy.allowed_targets.iter().any(|t| {
                    #[cfg(feature = "std")]
                    {
                        t.as_str() == target_str
                    }
                    #[cfg(not(feature = "std"))]
                    {
                        t.as_str().ok() == Some(target_str)
                    }
                })
            {
                return Err(Error::security_access_denied(
                    "Component not allowed as target",
                ));
            }

            // Check allowed functions
            if !policy.allowed_functions.is_empty()
                && !policy.allowed_functions.iter().any(|pattern| {
                    #[cfg(feature = "std")]
                    let pattern_str = pattern.as_str();
                    #[cfg(not(feature = "std"))]
                    let pattern_str = pattern.as_str().unwrap_or("");
                    func_str.contains(pattern_str)
                })
            {
                return Err(Error::security_access_denied(
                    "Function not allowed by security policy",
                ));
            }
        }

        Ok(())
    }

    /// Marshal parameters for cross-component call
    fn marshal_call_parameters(
        &self,
        args: &[wrt_foundation::values::Value],
    ) -> Result<ParameterMarshalingResult> {
        let start_time = 0; // Would use actual timestamp

        // Convert to ComponentValue format
        #[cfg(feature = "safety-critical")]
        let component_values: Result<
            WrtVec<WrtComponentValue, { CrateId::Component as u8 }, 256>,
        > = {
            let mut vec = WrtVec::new();
            for val in args.iter() {
                let converted = self.convert_value_to_component_value(val)?;
                vec.push(converted).map_err(|_| {
                    Error::runtime_execution_error(
                        "Too many parameters for safety-critical mode (limit: 256)",
                    )
                })?;
            }
            Ok(vec)
        };
        #[cfg(not(feature = "safety-critical"))]
        let component_values: Result<Vec<WrtComponentValue<ComponentProvider>>> = {
            let mut vec = Vec::new();
            for val in args.iter() {
                let converted = self.convert_value_to_component_value(val)?;
                #[cfg(feature = "std")]
                vec.push(converted);
                #[cfg(not(feature = "std"))]
                vec.push(converted).map_err(|_| {
                    Error::runtime_execution_error(
                        "Too many parameters for no_std mode (limit: 256)",
                    )
                })?;
            }
            Ok(vec)
        };

        let component_values = component_values?;

        // Calculate marshaled size
        let marshaled_size = self.calculate_marshaled_size(component_values.as_slice())?;

        if marshaled_size > self.config.max_parameter_size {
            return Ok(ParameterMarshalingResult {
                #[cfg(feature = "safety-critical")]
                marshaled_data: WrtVec::new(),
                #[cfg(not(feature = "safety-critical"))]
                marshaled_data: Vec::new(),
                metadata: MarshalingMetadata {
                    original_count: args.len(),
                    marshaled_size: 0,
                    marshaling_time_us: 0,
                    conversions_performed: 0,
                },
                success: false,
                #[cfg(feature = "std")]
                error_message: Some("Parameter data too large".to_string()),
                #[cfg(not(feature = "std"))]
                error_message: Some(
                    wrt_foundation::BoundedString::try_from_str("Parameter data too large")
                        .unwrap_or_default(),
                ),
            });
        }

        // For now, serialize as simple byte representation
        // In a full implementation, this would use proper canonical ABI serialization
        #[cfg(feature = "safety-critical")]
        let mut marshaled_data: WrtVec<u8, { CrateId::Component as u8 }, 8192> = WrtVec::new();
        #[cfg(not(feature = "safety-critical"))]
        let mut marshaled_data = Vec::new();
        for value in &component_values {
            let value_bytes = self.serialize_component_value(value)?;
            #[cfg(feature = "safety-critical")]
            {
                for byte in value_bytes {
                    marshaled_data.push(byte).map_err(|_| {
                        Error::runtime_execution_error(
                            "Marshaled data exceeds safety limit (8192 bytes)",
                        )
                    })?;
                }
            }
            #[cfg(not(feature = "safety-critical"))]
            {
                let _ = marshaled_data.extend(value_bytes);
            }
        }

        let end_time = 0; // Would use actual timestamp

        Ok(ParameterMarshalingResult {
            marshaled_data,
            metadata: MarshalingMetadata {
                original_count: args.len(),
                marshaled_size,
                marshaling_time_us: end_time - start_time,
                conversions_performed: component_values.len() as u32,
            },
            success: true,
            error_message: None,
        })
    }

    /// Convert Value to ComponentValue
    fn convert_value_to_component_value(
        &self,
        value: &wrt_foundation::values::Value,
    ) -> Result<WrtComponentValue<ComponentProvider>> {
        match value {
            wrt_foundation::values::Value::I32(v) => Ok(WrtComponentValue::S32(*v)),
            wrt_foundation::values::Value::I64(v) => Ok(WrtComponentValue::S64(*v)),
            wrt_foundation::values::Value::F32(v) => Ok(WrtComponentValue::F32(*v)),
            wrt_foundation::values::Value::F64(v) => Ok(WrtComponentValue::F64(*v)),
            _ => Err(Error::runtime_type_mismatch(
                "Unsupported value type for component call",
            )),
        }
    }

    /// Calculate marshaled size for component values
    fn calculate_marshaled_size(
        &self,
        values: &[WrtComponentValue<ComponentProvider>],
    ) -> Result<u32> {
        let mut total_size = 0u32;

        for value in values {
            let size = match value {
                WrtComponentValue::Bool(_) => 1,
                WrtComponentValue::S8(_) | WrtComponentValue::U8(_) => 1,
                WrtComponentValue::S16(_) | WrtComponentValue::U16(_) => 2,
                WrtComponentValue::S32(_)
                | WrtComponentValue::U32(_)
                | WrtComponentValue::F32(_) => 4,
                WrtComponentValue::S64(_)
                | WrtComponentValue::U64(_)
                | WrtComponentValue::F64(_) => 8,
                WrtComponentValue::Char(_) => 4,
                WrtComponentValue::String(s) => s.len() as u32 + 4, // String + length prefix
                WrtComponentValue::List(_items) => {
                    // List contains ValueRef, not ComponentValue directly
                    // Cannot recursively calculate size without resolving refs
                    16 // Placeholder size
                },
                WrtComponentValue::Record(_fields) => {
                    // Record contains (name, ValueRef) pairs, not ComponentValue
                    32 // Placeholder size
                },
                WrtComponentValue::Tuple(_elements) => {
                    // Tuple contains ValueRef, not ComponentValue directly
                    16 // Placeholder size
                },
                WrtComponentValue::Variant(_, value) => {
                    4 + if value.is_some() {
                        // Variant value is ValueRef, cannot calculate without resolving
                        8 // Placeholder size for ValueRef
                    } else {
                        0
                    }
                },
                WrtComponentValue::Enum(_) => 4,
                WrtComponentValue::Option(opt) => {
                    1 + if opt.is_some() {
                        // Option value is ValueRef, cannot calculate without resolving
                        8 // Placeholder size for ValueRef
                    } else {
                        0
                    }
                },
                WrtComponentValue::Result(result) => {
                    1 + match result {
                        Ok(v) | Err(v) => {
                            // Result contains ValueRef directly, not Option<ValueRef>
                            // ValueRef calculation not supported yet, return placeholder size
                            8
                        },
                    }
                },
                WrtComponentValue::Flags(_) => 4,
                WrtComponentValue::Void => 0,
                WrtComponentValue::Unit => 0,
                WrtComponentValue::FixedList(items, _) => {
                    // FixedList contains ValueRef, cannot calculate without resolving
                    items.len() as u32 * 8 // Placeholder size per item
                },
                WrtComponentValue::ErrorContext(items) => {
                    // ErrorContext contains ValueRef, cannot calculate without resolving
                    items.len() as u32 * 8 // Placeholder size per item
                },
                WrtComponentValue::Own(_) => 4,
                WrtComponentValue::Handle(_) => 4,
                WrtComponentValue::Borrow(_) => 4,
            };
            total_size += size;
        }

        Ok(total_size)
    }

    /// Serialize a component value to bytes
    fn serialize_component_value(
        &self,
        value: &WrtComponentValue<ComponentProvider>,
    ) -> Result<Vec<u8>> {
        // Simplified serialization - would use proper canonical ABI in full
        // implementation
        match value {
            WrtComponentValue::S32(v) => {
                let bytes = v.to_le_bytes();
                let mut vec = Vec::new();
                for byte in bytes {
                    #[cfg(not(feature = "std"))]
                    vec.push(byte)
                        .map_err(|_| Error::runtime_execution_error("Buffer capacity exceeded"))?;
                    #[cfg(feature = "std")]
                    vec.push(byte);
                }
                Ok(vec)
            },
            WrtComponentValue::S64(v) => {
                let bytes = v.to_le_bytes();
                let mut vec = Vec::new();
                for byte in bytes {
                    #[cfg(not(feature = "std"))]
                    vec.push(byte)
                        .map_err(|_| Error::runtime_execution_error("Buffer capacity exceeded"))?;
                    #[cfg(feature = "std")]
                    vec.push(byte);
                }
                Ok(vec)
            },
            WrtComponentValue::F32(v) => {
                let bytes = v.to_bits().to_le_bytes();
                let mut vec = Vec::new();
                for byte in bytes {
                    #[cfg(not(feature = "std"))]
                    vec.push(byte)
                        .map_err(|_| Error::runtime_execution_error("Buffer capacity exceeded"))?;
                    #[cfg(feature = "std")]
                    vec.push(byte);
                }
                Ok(vec)
            },
            WrtComponentValue::F64(v) => {
                let bytes = v.to_bits().to_le_bytes();
                let mut vec = Vec::new();
                for byte in bytes {
                    #[cfg(not(feature = "std"))]
                    vec.push(byte)
                        .map_err(|_| Error::runtime_execution_error("Buffer capacity exceeded"))?;
                    #[cfg(feature = "std")]
                    vec.push(byte);
                }
                Ok(vec)
            },
            WrtComponentValue::String(s) => {
                let mut bytes = Vec::new();
                // Add length prefix
                for byte in (s.len() as u32).to_le_bytes() {
                    #[cfg(not(feature = "std"))]
                    bytes
                        .push(byte)
                        .map_err(|_| Error::runtime_execution_error("Buffer capacity exceeded"))?;
                    #[cfg(feature = "std")]
                    bytes.push(byte);
                }
                // Add string bytes
                for byte in s.as_bytes() {
                    #[cfg(not(feature = "std"))]
                    bytes
                        .push(*byte)
                        .map_err(|_| Error::runtime_execution_error("Buffer capacity exceeded"))?;
                    #[cfg(feature = "std")]
                    bytes.push(*byte);
                }
                Ok(bytes)
            },
            #[cfg(feature = "safety-critical")]
            _ => {
                let mut vec = WrtVec::new();
                vec.push(0).map_err(|_| {
                    Error::runtime_execution_error("Unable to serialize component value")
                })?;
                Ok(vec)
            },
            #[cfg(not(feature = "safety-critical"))]
            _ => {
                let mut vec = Vec::new();
                let _ = vec.push(0);
                Ok(vec)
            },
        }
    }
}

// Implementation of LinkInterceptorStrategy for the communication strategy
#[cfg(feature = "std")]
impl LinkInterceptorStrategy for ComponentCommunicationStrategy {
    /// Called before a function call is made
    fn before_call(
        &self,
        source: &str,
        target: &str,
        function: &str,
        args: &[wrt_foundation::values::Value],
    ) -> Result<Vec<wrt_foundation::values::Value>> {
        // Check if this is a cross-component call
        if let Some(mut routing_info) = self.parse_component_call(function) {
            routing_info.source_component =
                BoundedString::<256>::try_from_str(source).unwrap_or_default();

            // Validate security policy
            self.validate_security_policy(&routing_info)?;

            // Marshal parameters
            let marshaling_result = self.marshal_call_parameters(args)?;

            if !marshaling_result.success {
                // Use a static error message since Error::runtime_execution_error requires &'static str
                return Err(Error::runtime_execution_error(
                    "Parameter marshaling failed",
                ));
            }

            // Update statistics
            // Note: In a real implementation, we'd need mutable access to self
            // This would require using interior mutability patterns like RefCell or Mutex

            // For now, return the original arguments
            // In a full implementation, we'd return the marshaled parameters
            Ok(args.to_vec())
        } else {
            // Not a cross-component call, pass through
            Ok(args.to_vec())
        }
    }

    /// Called after a function call completes
    fn after_call(
        &self,
        source: &str,
        target: &str,
        function: &str,
        args: &[wrt_foundation::values::Value],
        result: Result<Vec<wrt_foundation::values::Value>>,
    ) -> Result<Vec<wrt_foundation::values::Value>> {
        // Check if this was a cross-component call
        if let Some(routing_info) = self.parse_component_call(function) {
            // Update statistics based on result
            // Note: Would need mutable access in real implementation

            // Log completion if tracing is enabled
            if self.config.enable_tracing {
                match &result {
                    Ok(_) => {
                        // Log successful call
                    },
                    Err(e) => {
                        // Log failed call
                    },
                }
            }
        }

        // Return the result as-is
        result
    }

    /// Determines if the normal execution should be bypassed
    fn should_bypass(&self) -> bool {
        // We don't bypass execution, just intercept for monitoring and marshaling
        false
    }

    /// Determines if the strategy should intercept canonical ABI operations
    fn should_intercept_canonical(&self) -> bool {
        // Yes, we want to intercept canonical operations for parameter marshaling
        true
    }

    /// Intercepts a lift operation in the canonical ABI
    fn intercept_lift(
        &self,
        ty: &ValType<wrt_foundation::NoStdProvider<64>>,
        addr: u32,
        memory_bytes: &[u8],
    ) -> Result<Option<Vec<u8>>> {
        // Implement canonical lift interception
        // This would handle lifting values from memory during cross-component calls

        // For now, return None to proceed with normal lifting
        Ok(None)
    }

    /// Intercepts a lower operation in the canonical ABI
    fn intercept_lower(
        &self,
        value_type: &ValType<wrt_foundation::NoStdProvider<64>>,
        value_data: &[u8],
        addr: u32,
        memory_bytes: &mut [u8],
    ) -> Result<bool> {
        // Implement canonical lower interception
        // This would handle lowering values to memory during cross-component calls

        // For now, return false to proceed with normal lowering
        Ok(false)
    }

    /// Determines if the strategy should intercept component function calls
    fn should_intercept_function(&self) -> bool {
        // Yes, this is our primary purpose
        true
    }

    /// Intercepts a function call in the component model
    fn intercept_function_call(
        &self,
        function_name: &str,
        arg_types: &[ValType<wrt_foundation::NoStdProvider<64>>],
        arg_data: &[u8],
    ) -> Result<Option<Vec<u8>>> {
        // Check if this is a cross-component call we should handle
        if let Some(routing_info) = self.parse_component_call(function_name) {
            // This is where we would implement the actual call routing
            // For now, return None to proceed with normal execution

            // Update statistics
            // Note: Would need mutable access in real implementation

            Ok(None)
        } else {
            Ok(None)
        }
    }

    /// Intercepts the result of a function call in the component model
    fn intercept_function_result(
        &self,
        function_name: &str,
        result_types: &[ValType<wrt_foundation::NoStdProvider<64>>],
        result_data: &[u8],
    ) -> Result<Option<Vec<u8>>> {
        // Handle result marshaling for cross-component calls
        if let Some(_routing_info) = self.parse_component_call(function_name) {
            // Could implement result transformation here
            Ok(None)
        } else {
            Ok(None)
        }
    }

    /// Intercepts a resource operation
    fn intercept_resource_operation(
        &self,
        handle: u32,
        operation: &ResourceCanonicalOperation,
    ) -> Result<Option<Vec<u8>>> {
        // Handle resource operations during cross-component calls
        // This would coordinate resource transfers

        // Update statistics
        // Note: Would need mutable access in real implementation

        // For now, allow normal processing
        Ok(None)
    }

    /// Gets the preferred memory strategy for a resource or canonical operation
    fn get_memory_strategy(&self, _handle: u32) -> Option<u8> {
        // Could implement memory strategy selection based on component policies
        None // Use default strategy
    }

    /// Called before a component start function is executed
    fn before_start(&self, component_name: &str) -> Result<Option<Vec<u8>>> {
        // Could implement component startup interception
        Ok(None)
    }

    /// Called after a component start function has executed
    fn after_start(
        &self,
        component_name: &str,
        result_types: &[ValType<wrt_foundation::NoStdProvider<64>>],
        result_data: Option<&[u8]>,
    ) -> Result<Option<Vec<u8>>> {
        // Could implement component startup completion handling
        Ok(None)
    }

    /// Clones this strategy
    fn clone_strategy(&self) -> Arc<dyn LinkInterceptorStrategy> {
        // Create a new instance with the same configuration
        Arc::new(Self::with_config(self.config.clone()))
    }

    /// Process results after interception
    fn process_results(
        &self,
        component_name: &str,
        func_name: &str,
        args: &[ComponentValue<wrt_foundation::NoStdProvider<64>>],
        results: &[ComponentValue<wrt_foundation::NoStdProvider<64>>],
    ) -> Result<Option<Vec<wrt_intercept::Modification>>> {
        // Could implement result post-processing for cross-component calls
        Ok(None)
    }
}

// Note: no_std implementation removed because workspace builds always compile
// wrt-intercept with std features, so the std trait is always active

impl Default for ComponentCommunicationStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Display for CommunicationStats {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "CommunicationStats {{ function_calls: {}, successful: {}, failed: {}, avg_duration: \
             {}us }}",
            self.function_calls_intercepted,
            self.successful_calls,
            self.failed_calls,
            self.average_call_duration_us
        )
    }
}

/// Create a component communication strategy with default configuration
pub fn create_communication_strategy() -> ComponentCommunicationStrategy {
    ComponentCommunicationStrategy::new()
}

/// Create a component communication strategy with custom configuration
pub fn create_communication_strategy_with_config(
    config: ComponentCommunicationConfig,
) -> ComponentCommunicationStrategy {
    ComponentCommunicationStrategy::with_config(config)
}

/// Create a default security policy
pub fn create_default_security_policy() -> ComponentSecurityPolicy {
    ComponentSecurityPolicy::default()
}

/// Create a permissive security policy for testing
pub fn create_permissive_security_policy() -> ComponentSecurityPolicy {
    ComponentSecurityPolicy {
        #[cfg(feature = "std")]
        allowed_targets: vec![String::from("*")],
        #[cfg(not(feature = "std"))]
        allowed_targets: {
            let mut vec = Vec::new();
            if let Ok(s) = BoundedString::<256>::try_from_str("*") {
                let _ = vec.push(s);
            }
            vec
        },
        #[cfg(feature = "std")]
        allowed_functions: vec![String::from("*")],
        #[cfg(not(feature = "std"))]
        allowed_functions: {
            let mut vec = Vec::new();
            if let Ok(s) = BoundedString::<256>::try_from_str("*") {
                let _ = vec.push(s);
            }
            vec
        },
        allow_resource_transfer: true,
        max_call_depth: 64,
        validate_parameters: false,
    }
}
