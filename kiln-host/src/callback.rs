// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Callback registry for host functions.
//!
//! This module provides a registry for callbacks that can be invoked from
//! WebAssembly components, including host functions and interceptors.

// Use the prelude for consistent imports
#[cfg(feature = "std")]
use crate::prelude::LinkInterceptor;
use crate::prelude::{
    codes,
    str,
    Any,
    BuiltinType,
    Debug,
    Eq,
    Error,
    ErrorCategory,
    HashMap,
    HostFunctionHandler,
    Ord,
    PartialEq,
    PartialOrd,
    Result,
    Value,
};
#[cfg(feature = "std")]
use crate::prelude::{
    fmt,
    Arc,
    BuiltinHost,
};

// Type aliases for no_std compatibility
// In no_std mode, we can't use Box<dyn Any>, so we'll use a wrapper type
#[cfg(not(feature = "std"))]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
/// Callback data for `no_std` environments
pub struct CallbackData {
    _phantom: core::marker::PhantomData<()>,
}

#[cfg(not(feature = "std"))]
use crate::bounded_host_infra::{
    HostProvider,
    HOST_MEMORY_SIZE,
};

#[cfg(not(feature = "std"))]
type CallbackMap = HashMap<CallbackType, CallbackData, 32, HostProvider>;

// Value vectors for function parameters/returns
#[cfg(feature = "std")]
type ValueVec = Vec<Value>;

#[cfg(not(feature = "std"))]
type ValueVec = wrt_foundation::BoundedVec<Value, 16, HostProvider>;

// String vectors for registry queries
#[cfg(feature = "std")]
#[allow(dead_code)]
type StringVec = Vec<String>;

#[cfg(not(feature = "std"))]
type StringVec = wrt_foundation::BoundedVec<
    wrt_foundation::bounded::BoundedString<64>,
    32,
    HostProvider,
>;

// For returning references, we'll use a simplified approach in no_std
#[cfg(feature = "std")]
#[allow(dead_code)]
type StringRefVec<'a> = Vec<&'a String>;

#[cfg(not(feature = "std"))]
#[allow(dead_code)]
type StringRefVec<'a> = StringVec; // In no_std, we return owned strings instead of references

// For no_std mode, we'll use a simpler approach without nested maps
#[cfg(not(feature = "std"))]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
/// Host functions registry for `no_std` environments
pub struct HostFunctionsNoStd {
    // In no_std mode, we'll just store a flag indicating functions are registered
    // This is a placeholder - a real implementation would need a different approach
    _has_functions: bool,
}

#[cfg(not(feature = "std"))]
impl wrt_foundation::traits::Checksummable for HostFunctionsNoStd {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        checksum.update_slice(&[u8::from(self._has_functions)]);
    }
}

#[cfg(not(feature = "std"))]
impl wrt_foundation::traits::ToBytes for HostFunctionsNoStd {
    fn serialized_size(&self) -> usize {
        1
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        writer.write_u8(u8::from(self._has_functions))
    }
}

#[cfg(not(feature = "std"))]
impl wrt_foundation::traits::FromBytes for HostFunctionsNoStd {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        let has_functions = reader.read_u8()? != 0;
        Ok(HostFunctionsNoStd {
            _has_functions: has_functions,
        })
    }
}

/// Types of callbacks that can be registered
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum CallbackType {
    /// Callback for setup before execution
    #[default]
    Setup,
    /// Callback for cleanup after execution
    Cleanup,
    /// Binary `std/no_std` choice
    Allocate,
    /// Binary `std/no_std` choice
    Deallocate,
    /// Callback for custom interceptors
    Intercept,
    /// Callback for logging
    Logging,
}

// Implement required traits for BoundedMap compatibility
impl wrt_foundation::traits::Checksummable for CallbackType {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        checksum.update_slice(&[*self as u8]);
    }
}

impl wrt_foundation::traits::ToBytes for CallbackType {
    fn serialized_size(&self) -> usize {
        1
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        writer.write_u8(*self as u8)
    }
}

impl wrt_foundation::traits::FromBytes for CallbackType {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        match reader.read_u8()? {
            0 => Ok(CallbackType::Setup),
            1 => Ok(CallbackType::Cleanup),
            2 => Ok(CallbackType::Allocate),
            3 => Ok(CallbackType::Deallocate),
            4 => Ok(CallbackType::Intercept),
            5 => Ok(CallbackType::Logging),
            _ => Err(wrt_error::Error::parse_error(
                "Invalid CallbackType discriminant",
            )),
        }
    }
}

// Implement required traits for CallbackData to work with BoundedMap in no_std
// mode
#[cfg(not(feature = "std"))]
impl wrt_foundation::traits::Checksummable for CallbackData {
    fn update_checksum(&self, _checksum: &mut wrt_foundation::verification::Checksum) {
        // CallbackData has no content to checksum
    }
}

#[cfg(not(feature = "std"))]
impl wrt_foundation::traits::ToBytes for CallbackData {
    fn serialized_size(&self) -> usize {
        0
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        _writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        Ok(())
    }
}

#[cfg(not(feature = "std"))]
impl wrt_foundation::traits::FromBytes for CallbackData {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        _reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(CallbackData::default())
    }
}

/// A callback registry for handling WebAssembly component operations
pub struct CallbackRegistry {
    /// Generic callback storage for different types of callbacks
    #[cfg(feature = "std")]
    callbacks: HashMap<CallbackType, Box<dyn Any + Send + Sync>>,

    /// Generic callback storage for different types of callbacks (`no_std`
    /// version)
    #[cfg(not(feature = "std"))]
    callbacks: CallbackMap,

    /// Host functions registry (module name -> function name -> handler)
    #[cfg(feature = "std")]
    host_functions: HashMap<String, HashMap<String, HostFunctionHandler>>,

    /// Host functions registry (`no_std` version)
    #[cfg(not(feature = "std"))]
    host_functions: HostFunctionsNoStd,

    /// Optional interceptor for monitoring and modifying function calls
    #[cfg(feature = "std")]
    interceptor: Option<Arc<LinkInterceptor>>,
}

#[cfg(feature = "std")]
impl fmt::Debug for CallbackRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CallbackRegistry")
            .field("registered_callbacks", &self.callbacks.keys())
            .field("registered_modules", &self.host_functions.keys())
            .finish()
    }
}

#[cfg(not(feature = "std"))]
impl core::fmt::Debug for CallbackRegistry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CallbackRegistry")
            .field("callback_count", &"<no_std>")
            .field("host_functions_count", &"<no_std>")
            .finish()
    }
}

impl Default for CallbackRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CallbackRegistry {
    /// Create a new callback registry
    #[must_use]
    #[cfg(feature = "std")]
    pub fn new() -> Self {
        Self {
            callbacks:      HashMap::with_capacity(0),
            interceptor:    None,
            host_functions: HashMap::with_capacity(0),
        }
    }

    /// Create a new callback registry (`no_std` version)
    ///
    /// # Panics
    /// Panics if provider creation fails
    #[must_use]
    #[cfg(not(feature = "std"))]
    pub fn new() -> Self {
        // In no_std mode, we need to provide memory providers for the bounded
        // collections
        use crate::bounded_host_infra::create_host_provider;
        let provider = create_host_provider().expect("Failed to create host provider");
        Self {
            callbacks:      HashMap::new(provider).unwrap_or_default(),
            host_functions: HostFunctionsNoStd::default(),
        }
    }

    /// Sets an interceptor for this registry
    #[cfg(feature = "std")]
    pub fn with_interceptor(mut self, interceptor: Arc<LinkInterceptor>) -> Self {
        self.interceptor = Some(interceptor);
        self
    }

    /// Get the interceptor if one is set
    #[cfg(feature = "std")]
    pub fn get_interceptor(&self) -> Option<&LinkInterceptor> {
        self.interceptor.as_ref().map(|arc| arc.as_ref())
    }

    /// Register a callback
    #[cfg(feature = "std")]
    pub fn register_callback<T: 'static + Send + Sync>(
        &mut self,
        callback_type: CallbackType,
        callback: T,
    ) {
        self.callbacks.insert(callback_type, Box::new(callback));
    }

    /// Register a callback (`no_std` version - placeholder)
    #[cfg(not(feature = "std"))]
    pub fn register_callback<T>(&mut self, callback_type: CallbackType, _callback: T) {
        // Binary std/no_std choice
        // This is a placeholder implementation
        let _ = self.callbacks.insert(callback_type, CallbackData::default());
    }

    /// Get a callback
    #[cfg(feature = "std")]
    pub fn get_callback<T: 'static + Send + Sync>(
        &self,
        callback_type: &CallbackType,
    ) -> Option<&T> {
        self.callbacks.get(callback_type).and_then(|cb| cb.downcast_ref())
    }

    /// Get a callback (`no_std` version - placeholder)
    #[cfg(not(feature = "std"))]
    pub fn get_callback<T>(&self, _callback_type: &CallbackType) -> Option<&T> {
        // Binary std/no_std choice
        None
    }

    /// Get a mutable callback
    #[cfg(feature = "std")]
    pub fn get_callback_mut<T: 'static + Send + Sync>(
        &mut self,
        callback_type: &CallbackType,
    ) -> Option<&mut T> {
        self.callbacks.get_mut(callback_type).and_then(|cb| cb.downcast_mut())
    }

    /// Get a mutable callback (`no_std` version - placeholder)
    #[cfg(not(feature = "std"))]
    pub fn get_callback_mut<T>(&mut self, _callback_type: &CallbackType) -> Option<&mut T> {
        // Binary std/no_std choice
        None
    }

    /// Register a host function
    #[cfg(feature = "std")]
    pub fn register_host_function(
        &mut self,
        module_name: &str,
        function_name: &str,
        handler: HostFunctionHandler,
    ) {
        let module_name = module_name.to_string();
        let function_name = function_name.to_string();

        let module_functions = self.host_functions.entry(module_name).or_default();
        module_functions.insert(function_name, handler);
    }

    /// Register a host function (`no_std` version)
    #[cfg(not(feature = "std"))]
    pub fn register_host_function(
        &mut self,
        _module_name: &str,
        _function_name: &str,
        _handler: HostFunctionHandler,
    ) {
        // Binary std/no_std choice
        // This is a placeholder implementation
        self.host_functions._has_functions = true;
    }

    /// Check if a host function is registered
    #[must_use]
    #[cfg(feature = "std")]
    pub fn has_host_function(&self, module_name: &str, function_name: &str) -> bool {
        self.host_functions
            .get(module_name)
            .and_then(|funcs| funcs.get(function_name))
            .is_some()
    }

    /// Check if a host function is registered (`no_std` version)
    #[must_use]
    #[cfg(not(feature = "std"))]
    pub fn has_host_function(&self, _module_name: &str, _function_name: &str) -> bool {
        // In no_std mode, we can't check specific functions
        self.host_functions._has_functions
    }

    /// Call a host function
    ///
    /// # Errors
    ///
    /// Returns an error if the host function is not found or fails during
    /// execution
    pub fn call_host_function(
        &self,
        engine: &mut dyn Any,
        module_name: &str,
        function_name: &str,
        args: ValueVec,
    ) -> Result<ValueVec> {
        // If we have an interceptor, use it to intercept the call
        #[cfg(feature = "std")]
        {
            if let Some(interceptor) = self.get_interceptor() {
                return interceptor.intercept_call(
                    "host",
                    &function_key(module_name, function_name),
                    &args,
                    |modified_args| {
                        self.call_host_function_internal(
                            engine,
                            module_name,
                            function_name,
                            modified_args,
                        )
                    },
                );
            }
        }

        self.call_host_function_internal(engine, module_name, function_name, args)
    }

    /// Internal implementation of call_host_function without interception
    #[cfg(feature = "std")]
    fn call_host_function_internal(
        &self,
        engine: &mut dyn Any,
        module_name: &str,
        function_name: &str,
        args: ValueVec,
    ) -> Result<ValueVec> {
        if let Some(module_functions) = self.host_functions.get(module_name) {
            if let Some(handler) = module_functions.get(function_name) {
                return handler.call(engine, args);
            }
        }

        // Return error if the function is not found
        Err(Error::runtime_error("Host function not found"))
    }

    /// Internal implementation of `call_host_function` without interception
    /// (`no_std` version)
    #[cfg(not(feature = "std"))]
    fn call_host_function_internal(
        &self,
        _engine: &mut dyn Any,
        _module_name: &str,
        _function_name: &str,
        _args: ValueVec,
    ) -> Result<ValueVec> {
        // In no_std mode, we can't dynamically call host functions
        Err(Error::runtime_error(
            "Host functions not supported in no_std mode",
        ))
    }

    /// Get all registered module names
    #[must_use]
    #[cfg(feature = "std")]
    pub fn get_registered_modules(&self) -> Vec<&String> {
        self.host_functions.keys().collect()
    }

    /// Get all registered module names (`no_std` version)
    #[must_use]
    #[cfg(not(feature = "std"))]
    pub fn get_registered_modules(&self) -> StringVec {
        // In no_std mode, we can't return dynamic module names
        use crate::bounded_host_infra::create_host_provider;
        let provider = create_host_provider().expect("Failed to create host provider");
        StringVec::new(provider).unwrap_or_default()
    }

    /// Get all registered function names for a module
    #[must_use]
    #[cfg(feature = "std")]
    pub fn get_registered_functions(&self, module_name: &str) -> Vec<&String> {
        if let Some(module_functions) = self.host_functions.get(module_name) {
            module_functions.keys().collect()
        } else {
            Vec::with_capacity(0)
        }
    }

    /// Get all registered function names for a module (`no_std` version)
    #[must_use]
    #[cfg(not(feature = "std"))]
    pub fn get_registered_functions(&self, _module_name: &str) -> StringVec {
        // In no_std mode, we can't return dynamic function names
        use crate::bounded_host_infra::create_host_provider;
        let provider = create_host_provider().expect("Failed to create host provider");
        StringVec::new(provider).unwrap_or_default()
    }

    /// Get all available built-in types provided by this registry
    ///
    /// This method returns a set of all built-in types that are available
    /// through this registry's host functions.
    #[must_use]
    #[cfg(feature = "std")]
    pub fn get_available_builtins(&self) -> crate::prelude::HashSet<BuiltinType> {
        use crate::prelude::HashSet;

        let mut builtins = HashSet::new();

        // Check for built-ins in the wasi_builtin module
        if let Some(builtin_funcs) = self.host_functions.get("wasi_builtin") {
            for func_name in builtin_funcs.keys() {
                if let Ok(builtin_type) = func_name.parse::<BuiltinType>() {
                    builtins.insert(builtin_type);
                }
            }
        }

        builtins
    }

    /// Get all available built-in types provided by this registry (`no_std`
    /// version)
    ///
    /// This method returns a set of all built-in types that are available
    /// through this registry's host functions.
    #[must_use]
    #[cfg(not(feature = "std"))]
    pub fn get_available_builtins(
        &self,
    ) -> wrt_foundation::BoundedSet<BuiltinType, 32, HostProvider> {
        // In no_std mode, we can't dynamically track built-ins
        use crate::bounded_host_infra::create_host_provider;
        let provider = create_host_provider().expect("Failed to create host provider");
        wrt_foundation::BoundedSet::new(provider).unwrap_or_else(|_| {
            let fallback_provider =
                create_host_provider().expect("Failed to create fallback host provider");
            wrt_foundation::BoundedSet::new(fallback_provider)
                .expect("Failed to create bounded set")
        })
    }

    /// Call a built-in function
    ///
    /// # Arguments
    ///
    /// * `engine` - The engine context
    /// * `builtin_host` - The built-in host to use
    /// * `builtin_type` - The built-in type to call
    /// * `args` - The arguments to the function
    ///
    /// # Returns
    ///
    /// A `Result` containing the function results or an error
    ///
    /// # Errors
    ///
    /// Returns an error if the built-in is not implemented or fails during
    /// execution
    #[cfg(feature = "std")]
    pub fn call_builtin_function(
        &self,
        engine: &mut dyn Any,
        builtin_host: &BuiltinHost,
        builtin_type: BuiltinType,
        args: ValueVec,
    ) -> Result<ValueVec> {
        // First check if we have a direct host function registered
        let builtin_name = builtin_type.name();
        if self.has_host_function("wasi_builtin", builtin_name) {
            return self.call_host_function(engine, "wasi_builtin", builtin_name, args);
        }

        // If not, delegate to the built-in host
        builtin_host.call_builtin(engine, builtin_type, args)
    }
}

impl Clone for CallbackRegistry {
    fn clone(&self) -> Self {
        // Create a new registry
        let mut new_registry = Self::new();

        // Clone the interceptor if present
        #[cfg(feature = "std")]
        {
            if let Some(interceptor) = &self.interceptor {
                new_registry.interceptor = Some(interceptor.clone());
            }
        }

        // Clone host functions by creating new mappings with cloned handlers
        #[cfg(feature = "std")]
        {
            for (module_name, function_map) in &self.host_functions {
                for (function_name, handler) in function_map {
                    new_registry.register_host_function(
                        module_name,
                        function_name,
                        handler.clone(),
                    );
                }
            }
        }

        #[cfg(not(feature = "std"))]
        {
            new_registry.host_functions = self.host_functions.clone();
        }

        // Note: We can't easily clone the callbacks since they're Any type
        // In a real implementation, you would need to find a way to clone these as well

        new_registry
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "std"))]
    extern crate alloc;
    #[cfg(not(feature = "std"))]
    use alloc::vec;

    use wrt_foundation::{
        builtin::BuiltinType,
        values::Value,
    };

    use super::*;

    #[test]
    fn test_callback_registry() {
        let mut registry = CallbackRegistry::new();

        // Register a host function
        let handler = HostFunctionHandler::new(|_| Ok(vec![Value::I32(42)]));
        registry.register_host_function("test_module", "test_function", handler);

        // Verify it can be found
        assert!(registry.has_host_function("test_module", "test_function"));
        assert!(!registry.has_host_function("nonexistent", "function"));

        // Call the function
        let mut engine = ();
        let result =
            registry.call_host_function(&mut engine, "test_module", "test_function", vec![]);

        // Fix the assertion to not rely on PartialEq for Error type
        match result {
            Ok(values) => {
                assert_eq!(values.len(), 1);
                assert!(matches!(values.get(0).unwrap(), Value::I32(42)));
            },
            Err(_) => panic!("Expected successful function call"),
        }

        // Test calling a nonexistent function
        let err = registry.call_host_function(&mut engine, "nonexistent", "function", vec![]);
        assert!(err.is_err());
    }

    #[test]
    fn test_callback_registry_callback() {
        let mut registry = CallbackRegistry::new();

        // Register a callback
        registry.register_callback(CallbackType::Intercept, 42);

        // Get the callback
        let callback = registry.get_callback::<i32>(&CallbackType::Intercept);
        assert!(callback.is_some());
        assert_eq!(*callback.unwrap(), 42);

        // Modify the callback
        if let Some(callback) = registry.get_callback_mut::<i32>(&CallbackType::Intercept) {
            *callback = 24;
        }

        // Verify it was modified
        let callback = registry.get_callback::<i32>(&CallbackType::Intercept);
        assert!(callback.is_some());
        assert_eq!(*callback.unwrap(), 24);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_call_builtin_function() {
        // Create a registry with a host function for resource.create
        let mut registry = CallbackRegistry::new();
        let handler = HostFunctionHandler::new(|_| Ok(vec![Value::I32(42)]));
        registry.register_host_function("wasi_builtin", "resource.create", handler);

        // Create a built-in host with a different implementation
        let mut builtin_host = BuiltinHost::new("test-component", "test-host");
        builtin_host.register_handler(BuiltinType::ResourceCreate, |_, _| Ok(vec![Value::I32(99)]));

        // Test calling via registry - should use the registry's implementation
        let mut engine = ();
        let result = registry.call_builtin_function(
            &mut engine,
            &builtin_host,
            BuiltinType::ResourceCreate,
            vec![],
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![Value::I32(42)]);

        // Now test with a built-in that's only in the host
        let result = registry.call_builtin_function(
            &mut engine,
            &builtin_host,
            BuiltinType::ResourceDrop,
            vec![],
        );

        // Should fail because neither registry nor host implements it
        assert!(result.is_err());

        // Now add it to the host
        builtin_host.register_handler(BuiltinType::ResourceDrop, |_, _| Ok(vec![Value::I32(55)]));

        // Try again
        let result = registry.call_builtin_function(
            &mut engine,
            &builtin_host,
            BuiltinType::ResourceDrop,
            vec![],
        );

        // Should work now
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![Value::I32(55)]);
    }
}

/// Generate a unique function key from module and function names
///
/// This function creates a unique identifier for a function by combining
/// the module name and function name. It's used to lookup functions in
/// registries and for interception.
///
/// # Arguments
///
/// * `module_name` - The name of the module containing the function
/// * `function_name` - The name of the function
///
/// # Returns
///
/// A string in the format `module_name::function_name`
#[cfg(feature = "std")]
pub fn function_key(module_name: &str, function_name: &str) -> String {
    #[cfg(feature = "std")]
    return format!("{}::{}", module_name, function_name);

    #[cfg(not(feature = "std"))]
    return alloc::format!("{}::{}", module_name, function_name);
}

/// Generate a unique function key from module and function names (`no_std`
/// version)
///
/// Binary `std/no_std` choice
#[cfg(not(feature = "std"))]
#[must_use]
pub fn function_key(_module_name: &str, _function_name: &str) -> &'static str {
    // In pure no_std environments, we can't create dynamic strings
    // This is a placeholder - in practice, we'd need a different approach
    // Binary std/no_std choice
    "function_key"
}
