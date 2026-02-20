// WebAssembly Component Model Built-ins Implementation
//
// This module provides the infrastructure and implementations for WebAssembly
// Component Model built-ins, including resource handling, async operations,
// error contexts, and threading.

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, sync::Arc, vec::Vec};
#[cfg(feature = "std")]
use std::{
    boxed::Box,
    sync::{Arc, Mutex},
    vec::Vec,
};

use wrt_error::{Error, Result};
#[cfg(feature = "std")]
use wrt_foundation::builtin::BuiltinType;
#[cfg(not(feature = "std"))]
use wrt_foundation::{
    bounded::BoundedString, budget_aware_provider::CrateId, collections::StaticVec as BoundedVec,
    safe_managed_alloc, safe_memory::NoStdProvider,
};
#[cfg(not(feature = "std"))]
use wrt_sync::Mutex;

use crate::bounded_component_infra::ComponentProvider;
use crate::prelude::{WrtComponentValue, *};
#[cfg(not(feature = "std"))]
use crate::types::Value;

// Define a unified BuiltinType for no_std
#[cfg(not(feature = "std"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BuiltinType {
    // Resource built-ins (always available)
    ResourceCreate,
    ResourceDrop,
    ResourceRep,
    ResourceGet,
    // Resource built-ins (async feature)
    #[cfg(feature = "component-model-async")]
    ResourceNew,
    // Task built-ins
    #[cfg(feature = "component-model-async")]
    TaskYield,
    #[cfg(feature = "component-model-async")]
    TaskWait,
    #[cfg(feature = "component-model-async")]
    TaskReturn,
    #[cfg(feature = "component-model-async")]
    TaskPoll,
    #[cfg(feature = "component-model-async")]
    TaskBackpressure,
    // Subtask built-ins
    #[cfg(feature = "component-model-async")]
    SubtaskDrop,
    // Stream built-ins
    #[cfg(feature = "component-model-async")]
    StreamNew,
    #[cfg(feature = "component-model-async")]
    StreamRead,
    #[cfg(feature = "component-model-async")]
    StreamWrite,
    #[cfg(feature = "component-model-async")]
    StreamCancelRead,
    #[cfg(feature = "component-model-async")]
    StreamCancelWrite,
    #[cfg(feature = "component-model-async")]
    StreamCloseReadable,
    #[cfg(feature = "component-model-async")]
    StreamCloseWritable,
    // Future built-ins
    #[cfg(feature = "component-model-async")]
    FutureNew,
    #[cfg(feature = "component-model-async")]
    FutureCancelRead,
    #[cfg(feature = "component-model-async")]
    FutureCancelWrite,
    #[cfg(feature = "component-model-async")]
    FutureCloseReadable,
    #[cfg(feature = "component-model-async")]
    FutureCloseWritable,
    // Async built-ins
    #[cfg(feature = "component-model-async")]
    AsyncNew,
    #[cfg(feature = "component-model-async")]
    AsyncGet,
    #[cfg(feature = "component-model-async")]
    AsyncPoll,
    #[cfg(feature = "component-model-async")]
    AsyncWait,
    // Error Context built-ins
    #[cfg(feature = "component-model-error-context")]
    ErrorNew,
    #[cfg(feature = "component-model-error-context")]
    ErrorTrace,
    #[cfg(feature = "component-model-error-context")]
    ErrorContextNew,
    #[cfg(feature = "component-model-error-context")]
    ErrorContextDrop,
    #[cfg(feature = "component-model-error-context")]
    ErrorContextDebugMessage,
    // Threading built-ins
    #[cfg(feature = "component-model-threading")]
    ThreadingSpawn,
    #[cfg(feature = "component-model-threading")]
    ThreadingJoin,
    #[cfg(feature = "component-model-threading")]
    ThreadingSync,
}
// Commented out until wrt_intercept is properly available
// use wrt_intercept::{BeforeBuiltinResult, BuiltinInterceptor,
// InterceptContext};

// No_std stubs for interception (simplified)
#[cfg(not(feature = "std"))]
#[derive(Debug)]
pub struct InterceptContext {
    component_name: BoundedString<128>,
    builtin_type: BuiltinType,
    host_id: BoundedString<128>,
}

#[cfg(not(feature = "std"))]
impl InterceptContext {
    pub fn new(component_name: &str, builtin_type: BuiltinType, host_id: &str) -> Result<Self> {
        let provider1 = safe_managed_alloc!(512, CrateId::Component)?;
        let provider2 = safe_managed_alloc!(512, CrateId::Component)?;
        Ok(Self {
            component_name: BoundedString::try_from_str(component_name).unwrap_or_default(),
            builtin_type,
            host_id: BoundedString::try_from_str(host_id).unwrap_or_default(),
        })
    }
}

#[cfg(not(feature = "std"))]
#[derive(Debug)]
pub enum BeforeBuiltinResult {
    Continue(BoundedVec<WrtComponentValue<ComponentProvider>, 16>),
    Override(BoundedVec<WrtComponentValue<ComponentProvider>, 16>),
    Deny,
}

#[cfg(not(feature = "std"))]
pub trait BuiltinInterceptor {
    fn before_builtin(
        &self,
        context: &InterceptContext,
        args: &[WrtComponentValue<ComponentProvider>],
    ) -> Result<BeforeBuiltinResult>;
}

// Import the real types for std
#[cfg(feature = "std")]
use wrt_intercept::{BeforeBuiltinResult, BuiltinInterceptor, InterceptContext};

use crate::resources::ResourceManager;

/// Resource built-ins implementation
pub mod resource;

/// Async built-ins implementation
#[cfg(feature = "component-model-async")]
pub mod async_ops;

/// Error context built-ins implementation
#[cfg(feature = "component-model-error-context")]
pub mod error;

/// Threading built-ins implementation
#[cfg(feature = "component-model-threading")]
pub mod threading;

/// Safe threading built-ins implementation using platform-aware architecture
#[cfg(feature = "component-model-threading")]
pub mod safe_threading;

/// Trait for built-in function handlers
///
/// This trait defines the interface for handlers that implement built-in
/// functions for the WebAssembly Component Model.
#[cfg(feature = "std")]
pub trait BuiltinHandler: Send + Sync {
    /// Get the type of built-in this handler manages
    fn builtin_type(&self) -> BuiltinType;

    /// Execute the built-in function with the given arguments
    ///
    /// # Arguments
    ///
    /// * `args` - The arguments to the built-in function
    ///
    /// # Returns
    ///
    /// A `Result` containing the function results or an error
    fn execute(
        &self,
        args: &[WrtComponentValue<ComponentProvider>],
    ) -> Result<Vec<WrtComponentValue<ComponentProvider>>>;

    /// Clone this handler
    ///
    /// # Returns
    ///
    /// A boxed clone of this handler
    fn clone_handler(&self) -> Box<dyn BuiltinHandler>;
}

#[cfg(not(feature = "std"))]
pub trait BuiltinHandler {
    /// Get the type of built-in this handler manages
    fn builtin_type(&self) -> BuiltinType;

    /// Execute the built-in function with the given arguments (no_std version)
    fn execute(
        &self,
        args: &[WrtComponentValue<ComponentProvider>],
    ) -> core::result::Result<BoundedVec<WrtComponentValue<ComponentProvider>, 16>, Error>;

    /// Clone this handler
    fn clone_handler(&self) -> Box<dyn BuiltinHandler>;
}

/// Function executor type for threading built-ins
#[cfg(feature = "component-model-threading")]
pub type FunctionExecutor = Arc<
    dyn Fn(
            u32,
            Vec<WrtComponentValue<ComponentProvider>>,
        ) -> Result<Vec<WrtComponentValue<ComponentProvider>>>
        + Send
        + Sync,
>;

/// Registry of built-in handlers
///
/// This struct manages the available built-in handlers and routes calls
/// to the appropriate implementation.
pub struct BuiltinRegistry {
    /// Registered handlers for built-in functions
    handlers: Vec<Box<dyn BuiltinHandler>>,
    /// Optional interceptor for built-in calls
    // interceptor: Option<Arc<dyn BuiltinInterceptor>>,
    /// Component name for context
    component_name: String,
    /// Host ID for context
    host_id: String,
    /// Store for async values
    #[cfg(feature = "component-model-async")]
    async_store: Arc<Mutex<async_ops::AsyncValueStore>>,
    /// Store for error contexts
    #[cfg(feature = "component-model-error-context")]
    error_store: Arc<Mutex<error::ErrorContextStore>>,
    /// Function executor for threading handlers
    #[cfg(feature = "component-model-threading")]
    function_executor: FunctionExecutor,
}

impl BuiltinRegistry {
    /// Create a new built-in registry
    ///
    /// # Arguments
    ///
    /// * `component_name` - The name of the component
    /// * `host_id` - The host identifier
    /// * `resource_manager` - The resource manager to use
    ///
    /// # Returns
    ///
    /// A new `BuiltinRegistry` instance with default handlers
    pub fn new(
        component_name: &str,
        host_id: &str,
        resource_manager: Arc<Mutex<ResourceManager>>,
    ) -> Self {
        #[cfg(feature = "component-model-async")]
        let async_store = Arc::new(Mutex::new(async_ops::AsyncValueStore::new()));

        #[cfg(feature = "component-model-error-context")]
        let error_store = Arc::new(Mutex::new(error::ErrorContextStore::new()));

        // Define a default function executor for threading that just returns an error
        #[cfg(feature = "component-model-threading")]
        let function_executor: FunctionExecutor = Arc::new(|function_id, _args| {
            Err(Error::runtime_execution_error(
                "Function executor not configured",
            ))
        });

        let mut registry = Self {
            handlers: Vec::new(),
            // interceptor: None,
            component_name: component_name.to_string(),
            host_id: host_id.to_string(),
            #[cfg(feature = "component-model-async")]
            async_store,
            #[cfg(feature = "component-model-error-context")]
            error_store,
            #[cfg(feature = "component-model-threading")]
            function_executor,
        };

        // Register default resource handlers
        let resource_handlers = resource::create_resource_handlers(resource_manager);
        for handler in resource_handlers {
            registry.register_handler(handler);
        }

        // Register async handlers if the feature is enabled
        #[cfg(feature = "component-model-async")]
        {
            let async_handlers = async_ops::create_async_handlers(registry.async_store.clone());
            for handler in async_handlers {
                registry.register_handler(handler);
            }
        }

        // Register error context handlers if the feature is enabled
        #[cfg(feature = "component-model-error-context")]
        {
            let error_handlers = error::create_error_handlers();
            for handler in error_handlers {
                registry.register_handler(handler);
            }
        }

        // Register threading handlers if the feature is enabled
        #[cfg(feature = "component-model-threading")]
        {
            let threading_handlers =
                threading::create_threading_handlers(registry.function_executor.clone());
            for handler in threading_handlers {
                registry.register_handler(handler);
            }
        }

        registry
    }

    /// Register a built-in handler
    ///
    /// # Arguments
    ///
    /// * `handler` - The handler to register
    pub fn register_handler(&mut self, handler: Box<dyn BuiltinHandler>) {
        // Check if we already have a handler for this built-in type
        let builtin_type = handler.builtin_type();
        if self.handlers.iter().any(|h| h.builtin_type() == builtin_type) {
            // Replace the existing handler
            let idx = self.handlers.iter().position(|h| h.builtin_type() == builtin_type).unwrap();
            self.handlers[idx] = handler;
        } else {
            // Add a new handler
            self.handlers.push(handler);
        }
    }

    /// Set the interceptor for built-in calls
    ///
    /// # Arguments
    ///
    /// * `interceptor` - The interceptor to use
    // pub fn set_interceptor(&mut self, interceptor: Arc<dyn BuiltinInterceptor>) {
    //     self.interceptor = Some(interceptor;
    // }

    /// Check if a built-in type is supported
    ///
    /// # Arguments
    ///
    /// * `builtin_type` - The built-in type to check
    ///
    /// # Returns
    ///
    /// `true` if the built-in is supported, `false` otherwise
    pub fn supports_builtin(&self, builtin_type: BuiltinType) -> bool {
        self.handlers.iter().any(|h| h.builtin_type() == builtin_type)
    }

    /// Call a built-in function (std version)
    ///
    /// # Arguments
    ///
    /// * `builtin_type` - The type of built-in to call
    /// * `args` - The arguments to the function
    ///
    /// # Returns
    ///
    /// A `Result` containing the function results or an error
    #[cfg(feature = "std")]
    pub fn call(
        &self,
        builtin_type: BuiltinType,
        args: &[WrtComponentValue<ComponentProvider>],
    ) -> Result<Vec<WrtComponentValue<ComponentProvider>>> {
        // Find the handler for this built-in
        let handler = self
            .handlers
            .iter()
            .find(|h| h.builtin_type() == builtin_type)
            .ok_or_else(|| Error::component_not_found("Component not found"))?;

        // No interceptor currently, just execute
        handler.execute(args)
    }

    /// Call a built-in function (no_std version)
    ///
    /// # Arguments
    ///
    /// * `builtin_type` - The type of built-in to call
    /// * `args` - The arguments to the function
    ///
    /// # Returns
    ///
    /// A `Result` containing the function results or an error
    #[cfg(not(feature = "std"))]
    pub fn call(
        &self,
        builtin_type: BuiltinType,
        args: &[WrtComponentValue<ComponentProvider>],
    ) -> Result<BoundedVec<WrtComponentValue<ComponentProvider>, 16>> {
        // Find the handler for this built-in
        let handler = self
            .handlers
            .iter()
            .find(|h| h.builtin_type() == builtin_type)
            .ok_or_else(|| Error::component_not_found("Component not found"))?;

        // Create interception context
        let context = InterceptContext::new(&self.component_name, builtin_type, &self.host_id)?;

        // No interceptor currently, just execute
        handler.execute(args)
    }

    /// Get the async store
    #[cfg(feature = "component-model-async")]
    pub fn async_store(&self) -> Arc<Mutex<async_ops::AsyncValueStore>> {
        self.async_store.clone()
    }

    /// Set the function executor for threading built-ins
    ///
    /// # Arguments
    ///
    /// * `executor` - The function executor to use
    #[cfg(feature = "component-model-threading")]
    pub fn set_function_executor(&mut self, executor: FunctionExecutor) {
        self.function_executor = executor;

        // Re-register threading handlers with the new executor
        let threading_handlers =
            threading::create_threading_handlers(self.function_executor.clone());
        for handler in threading_handlers {
            self.register_handler(handler);
        }
    }
}

impl Clone for BuiltinRegistry {
    fn clone(&self) -> Self {
        Self {
            handlers: self.handlers.iter().map(|h| h.clone_handler()).collect(),
            // interceptor: self.interceptor.clone(),
            component_name: self.component_name.clone(),
            host_id: self.host_id.clone(),
            #[cfg(feature = "component-model-async")]
            async_store: self.async_store.clone(),
            #[cfg(feature = "component-model-error-context")]
            error_store: self.error_store.clone(),
            #[cfg(feature = "component-model-threading")]
            function_executor: self.function_executor.clone(),
        }
    }
}
