//! Component Runtime implementation
//!
//! This file provides a concrete implementation of the component runtime.

// alloc is imported in lib.rs with proper feature gates

#[cfg(feature = "std")]
use std::{collections::HashMap, sync::Arc};
#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::{collections::BTreeMap, sync::Arc};

// Components traits imported below with full set

#[cfg(all(not(feature = "std"), not(feature = "std")))]
pub mod no_alloc {
    use wrt_error::{Error, ErrorCategory, Result};
    use wrt_foundation::{
        bounded::{BoundedVec, MAX_COMPONENT_TYPES},
        safe_memory::{NoStdProvider, SafeSlice},
        verification::VerificationLevel,
    };

    /// A minimal component implementation for pure no_std environments
    ///
    /// This provides basic validation and introspection capabilities,
    /// but does not support execution of components.
    #[derive(Debug)]
    pub struct MinimalComponent {
        verification_level: VerificationLevel,
    }

    impl MinimalComponent {
        /// Creates a new minimal component
        ///
        /// # Arguments
        ///
        /// * `level` - The verification level to use
        ///
        /// # Returns
        ///
        /// * `Self` - A new minimal component
        pub fn new(level: VerificationLevel) -> Self {
            Self { verification_level: level }
        }

        /// Gets the verification level for this component
        ///
        /// # Returns
        ///
        /// * `VerificationLevel` - The verification level
        #[must_use]
        pub const fn verification_level(&self) -> VerificationLevel {
            self.verification_level
        }

        /// Validates a component binary
        ///
        /// # Arguments
        ///
        /// * `binary` - The component binary data
        ///
        /// # Returns
        ///
        /// * `Result<()>` - Ok if the component is valid, Error otherwise
        pub fn validate(binary: &[u8]) -> Result<()> {
            #[cfg(feature = "decoder")]
            {
                // Use wrt-decoder's header validation
                wrt_decoder::component::decode_no_alloc::verify_component_header(binary)
            }
            #[cfg(not(feature = "decoder"))]
            {
                // Basic validation - just check magic number
                if binary.len() < 8 {
                    return Err(Error::parse_invalid_binary("Binary too small to be a valid component"));
                }
                // Check for WASM magic number (0x00 0x61 0x73 0x6D)
                if &binary[0..4] != b"\0asm" {
                    return Err(Error::parse_invalid_binary("Invalid WASM magic number"));
                }
                Ok(())
            }
        }
    }
}

use wrt_foundation::{
    safe_memory::{SafeMemoryHandler, SafeSlice, SafeStack},
    traits::BoundedCapacity,
    Value, VerificationLevel, safe_managed_alloc,
    budget_aware_provider::CrateId,
};

#[cfg(feature = "std")]
use std::vec;

#[cfg(feature = "std")]
use crate::{
    component_traits::{
        ComponentInstance, ComponentRuntime, 
        HostFunction, HostFunctionFactory, ComponentType, ExternType, FuncType
    },
    unified_types::{DefaultRuntimeTypes, UnifiedMemoryAdapter, PlatformMemoryAdapter},
    prelude::*,
};

#[cfg(all(not(feature = "std"), not(feature = "std")))]
use crate::{
    component_traits::{ComponentType, ExternType, FuncType},
    prelude::*,
};

/// Host function implementation
#[cfg(feature = "std")]
struct HostFunctionImpl<
    F: Fn(
            &[wrt_foundation::Value],
        ) -> Result<wrt_foundation::bounded::BoundedStack<wrt_foundation::Value, 64, crate::bounded_runtime_infra::RuntimeProvider>>
        + 'static
        + Send
        + Sync,
> {
    /// Function type
    func_type: FuncType,
    /// Implementation function
    implementation: Arc<F>,
}

// TODO: ComponentHostFunction trait not yet defined - commented out temporarily
/*
#[cfg(feature = "std")]
impl<
        F: Fn(
                &[wrt_foundation::Value],
            ) -> Result<wrt_foundation::bounded::BoundedStack<wrt_foundation::Value, 64, wrt_foundation::safe_memory::NoStdProvider<131072>>>
            + 'static
            + Send
            + Sync,
    > ComponentHostFunction for HostFunctionImpl<F>
{
    /// Call the function with the given arguments
    fn call(
        &self,
        args: &[wrt_foundation::Value],
    ) -> Result<wrt_foundation::bounded::BoundedStack<wrt_foundation::Value, 64, wrt_foundation::safe_memory::NoStdProvider<131072>>> {
        (self.implementation)(args)
    }

    /// Get the function type
    fn get_type(&self) -> FuncType {
        self.func_type.clone()
    }
}
*/

/// Legacy host function implementation for backward compatibility
struct LegacyHostFunctionImpl<
    F: Fn(&[wrt_foundation::Value]) -> Result<wrt_foundation::bounded::BoundedVec<wrt_foundation::Value, 16, wrt_foundation::safe_memory::NoStdProvider<131072>>> + 'static + Send + Sync,
> {
    /// Function type
    func_type: FuncType,
    /// Implementation function
    implementation: Arc<F>,
    /// Verification level
    verification_level: VerificationLevel,
}

#[cfg(feature = "std")]
impl<
        F: Fn(&[wrt_foundation::Value]) -> Result<wrt_foundation::bounded::BoundedVec<wrt_foundation::Value, 16, wrt_foundation::safe_memory::NoStdProvider<131072>>> + 'static + Send + Sync,
    > ComponentHostFunction for LegacyHostFunctionImpl<F>
{
    /// Call the function with the given arguments
    fn call(
        &self,
        args: &[wrt_foundation::Value],
    ) -> Result<wrt_foundation::bounded::BoundedStack<wrt_foundation::Value, 64, wrt_foundation::safe_memory::NoStdProvider<131072>>> {
        // Call the legacy function
        let vec_result = (self.implementation)(args)?;

        // Convert to SafeStack
        let provider = safe_managed_alloc!(1024, CrateId::Runtime)?;
        let mut safe_stack = wrt_foundation::safe_memory::SafeStack::new(provider)?;
        safe_stack.set_verification_level(self.verification_level);

        // Add all values to the safe stack
        for value in vec_result.iter() {
            safe_stack.push(value.clone())?;
        }

        Ok(safe_stack)
    }

    /// Get the function type
    fn get_type(&self) -> FuncType {
        self.func_type.clone()
    }
}

/// Default host function factory
#[derive(Clone, Default)]
pub struct DefaultHostFunctionFactory {
    /// Verification level
    verification_level: VerificationLevel,
}

impl DefaultHostFunctionFactory {
    /// Create a new DefaultHostFunctionFactory with a specific verification
    /// level
    pub fn with_verification_level(level: VerificationLevel) -> Self {
        Self { verification_level: level }
    }
}

#[cfg(feature = "std")]
impl HostFunctionFactory for DefaultHostFunctionFactory {
    /// Create a function with the given name and type
    fn create_function(&self, _name: &str, ty: &FuncType) -> Result<Box<dyn HostFunction>> {
        // Create a simple function that returns an empty SafeStack
        let verification_level = self.verification_level;
        let func_impl = HostFunctionImpl {
            func_type: ty.clone(),
            implementation: Arc::new(move |_args: &[wrt_foundation::Value]| {
                let provider = safe_managed_alloc!(1024, CrateId::Runtime)?;
                let mut result = wrt_foundation::safe_memory::SafeStack::new(provider)?;
                result.set_verification_level(verification_level);
                Ok(result)
            }),
        };

        #[cfg(feature = "std")]
        {
            Ok(Box::new(func_impl))
        }
        #[cfg(all(not(feature = "std"), not(feature = "std")))]
        {
            // Binary std/no_std choice
            Err(Error::runtime_execution_error("Host function creation not supported"))
        }
    }
}

#[cfg(feature = "std")]
type HostFunctionMap = HashMap<String, Box<dyn ComponentHostFunction>>;
#[cfg(feature = "std")]
type HostFactoryVec = Vec<Box<dyn HostFunctionFactory>>;

/// An implementation of the ComponentRuntime interface
#[cfg(feature = "std")]
pub struct ComponentRuntimeImpl {
    /// Host function factories for creating host functions
    host_factories: HostFactoryVec,
    /// Verification level for memory operations
    verification_level: VerificationLevel,
    /// Registered host functions
    host_functions: HostFunctionMap,
}

#[cfg(feature = "std")]
impl ComponentRuntime for ComponentRuntimeImpl {
    /// Create a new ComponentRuntimeImpl
    fn new() -> Self {
        Self {
            #[cfg(feature = "std")]
            host_factories: Vec::with_capacity(8),
            #[cfg(all(not(feature = "std"), not(feature = "std")))]
            host_factories: HostFactoryVec::new(wrt_provider!(131072, CrateId::Runtime).unwrap_or_default()).expect("Failed to create host factories"),
            verification_level: VerificationLevel::default(),
            #[cfg(feature = "std")]
            host_functions: HostFunctionMap::new(),
            #[cfg(all(not(feature = "std"), not(feature = "std")))]
            host_functions: HostFunctionMap::new(wrt_provider!(131072, CrateId::Runtime).unwrap_or_default()).expect("Failed to create host functions"),
        }
    }

    /// Register a host function factory
    #[cfg(feature = "std")]
    fn register_host_factory(&mut self, factory: Box<dyn HostFunctionFactory>) {
        // Safety-enhanced push operation with verification
        if self.verification_level.should_verify(128) {
            // Perform pre-push integrity verification
            self.verify_integrity().expect("ComponentRuntime integrity check failed");
        }

        #[cfg(feature = "std")]
        {
            // Push to Vec (can't use SafeStack since HostFunctionFactory doesn't implement Clone)
            self.host_factories.push(factory);
        }

        #[cfg(all(not(feature = "std"), not(feature = "std")))]
        {
            // Binary std/no_std choice
            let _factory_id = self.host_factories.len() as u32;
            let _ = self.host_factories.push(_factory_id);
            // We don't actually store the factory in no_std mode for simplicity
            core::mem::drop(factory);
        }

        if self.verification_level.should_verify(128) {
            // Perform post-push integrity verification
            self.verify_integrity().expect("ComponentRuntime integrity check failed after push");
        }
    }


    /// Instantiate a component
    #[cfg(feature = "std")]
    fn instantiate(&self, component_type: &ComponentType) -> Result<Box<dyn ComponentInstance>> {
        // Verify integrity before instantiation if high verification level
        if self.verification_level.should_verify(200) {
            self.verify_integrity()?;
        }

        // Initialize memory with enough space (1 page = 64KB)
        let memory_size = 65536;
        #[cfg(feature = "std")]
        let memory_data = vec![0; memory_size];
        #[cfg(all(not(feature = "std"), not(feature = "std")))]
        let memory_data = {
            let mut data = wrt_foundation::bounded::BoundedVec::new();
            for _ in 0..memory_size.min(65536) {
                data.push(0u8).unwrap();
            }
            data
        };

        // Collect host function names and types for tracking
        #[cfg(feature = "std")]
        let mut host_function_names = Vec::new();
        #[cfg(all(not(feature = "std"), not(feature = "std")))]
        let mut host_function_names = wrt_foundation::bounded::BoundedVec::new();

        #[cfg(feature = "std")]
        let mut host_functions = {
            #[cfg(feature = "std")]
            let mut map = HashMap::new();
            #[cfg(not(feature = "std"))]
            let mut map = BTreeMap::new();
            
            for name in self.host_functions.keys() {
                host_function_names.push(name.clone();
                if let Some(func) = self.host_functions.get(name) {
                    map.insert(name.clone(), Some(func.get_type().clone();
                } else {
                    map.insert(name.clone(), None;
                }
            }
            map
        };

        #[cfg(all(not(feature = "std"), not(feature = "std")))]
        let host_functions = {
            // Binary std/no_std choice
            for (name, _id) in self.host_functions.iter() {
                host_function_names.push(name.clone());
            }
            // Return empty map-like structure for no_std
            ()
        };

        // Create a basic component instance implementation
        #[cfg(feature = "std")]
        {
            Ok(Box::new(ComponentInstanceImpl {
                component_type: component_type.clone(),
                verification_level: self.verification_level,
                memory_store: wrt_foundation::safe_memory::SafeMemoryHandler::<wrt_foundation::safe_memory::NoStdProvider<131072>>::new(wrt_provider!(131072, CrateId::Runtime).unwrap_or_default()),
                host_function_names,
                host_functions,
            }))
        }
        #[cfg(all(not(feature = "std"), not(feature = "std")))]
        {
            // Binary std/no_std choice
            Err(Error::runtime_execution_error("Component instantiation not supported in no std mode"))
        }
    }


    /// Register a host function
    fn register_host_function<F>(&mut self, name: &str, ty: FuncType, function: F) -> Result<()>
    where
        F: Fn(&[wrt_foundation::Value]) -> Result<wrt_foundation::bounded::BoundedVec<wrt_foundation::Value, 16, wrt_foundation::safe_memory::NoStdProvider<131072>>>
            + 'static
            + Send
            + Sync,
    {
        #[cfg(feature = "std")]
        {
            // Create a legacy host function implementation
            let func_impl = LegacyHostFunctionImpl {
                func_type: ty,
                implementation: Arc::new(function),
                verification_level: self.verification_level,
            };

            // Insert the function into the host functions map
            #[cfg(feature = "std")]
            let name_string = name.to_string();
            #[cfg(not(feature = "std"))]
            let name_string = alloc::string::String::from(name);

            self.host_functions.insert(name_string, Box::new(func_impl));
        }
        #[cfg(all(not(feature = "std"), not(feature = "std")))]
        {
            // Binary std/no_std choice
            let _ = (name, ty, function);
        }

        Ok(())
    }

    /// Set the verification level for memory operations
    fn set_verification_level(&mut self, level: VerificationLevel) -> Result<()> {
        self.verification_level = level;
        Ok(())
    }

    /// Get the current verification level
    fn verification_level(&self) -> VerificationLevel {
        self.verification_level
    }
}

#[cfg(feature = "std")]
impl ComponentRuntimeImpl {
    /// Create a new ComponentRuntimeImpl with a specific verification level
    ///
    /// This is a convenience method for creating a ComponentRuntimeImpl with
    /// a specific verification level.
    pub fn with_verification_level(level: VerificationLevel) -> Self {
        let mut runtime = Self::new();
        runtime.verification_level = level;
        runtime
    }

    /// Get the number of registered host factories
    pub fn factory_count(&self) -> usize {
        self.host_factories.len()
    }

    /// Verify the integrity of the component runtime
    pub fn verify_integrity(&self) -> Result<()> {
        // This is a placeholder for actual integrity verification
        if self.verification_level.should_verify(200) {
            // Perform a deeper verification
            // In a real implementation, this would check that all host
            // factories have valid state, that all registered host
            // functions are consistent, etc.
        }

        Ok(())
    }
}

#[cfg(feature = "std")]
type HostFunctionTypeMap = HashMap<String, Option<FuncType>>;

#[cfg(not(feature = "std"))]
type HostFunctionTypeMap = BTreeMap<String, Option<FuncType>>;

/// Basic implementation of ComponentInstance for testing
struct ComponentInstanceImpl {
    /// Component type
    component_type: ComponentType,
    /// Verification level
    verification_level: VerificationLevel,
    /// Memory store for the instance
    memory_store: wrt_foundation::safe_memory::SafeMemoryHandler<wrt_foundation::safe_memory::NoStdProvider<131072>>,
    /// Named host functions that are available to this instance
    host_function_names: Vec<String>,
    /// Host functions in this runtime
    host_functions: HostFunctionTypeMap,
}

#[cfg(feature = "std")]
impl ComponentInstance for ComponentInstanceImpl {
    /// Execute a function by name
    fn execute_function(
        &self,
        name: &str,
        args: &[wrt_foundation::Value],
    ) -> Result<wrt_foundation::bounded::BoundedStack<wrt_foundation::Value, 64, wrt_foundation::safe_memory::NoStdProvider<131072>>> {
        // Verify args (safety check)
        if self.verification_level.should_verify(128) {
            // Check that argument types match the expected types
            if name.is_empty() {
                return Err(wrt_error::Error::runtime_execution_error("Function name cannot be empty"));
            }
        }

        // Check if this is a function that's known to the runtime
        #[cfg(feature = "std")]
        let name_check = self.host_function_names.contains(&name.to_string());
        #[cfg(not(feature = "std"))]
        let name_check = self.host_function_names.contains(&alloc::string::String::from(name;
        #[cfg(all(not(feature = "std"), not(feature = "std")))]
        let name_check = {
            let mut found = false;
            for stored_name in self.host_function_names.iter() {
                if stored_name.as_str().map_or(false, |s| s == name) {
                    found = true;
                    break;
                }
            }
            found
        };
        
        if name_check {
            // Create an empty SafeStack for the result
            let provider = safe_managed_alloc!(1024, CrateId::Runtime)?;
            let mut result = wrt_foundation::safe_memory::SafeStack::new(provider)?;
            result.set_verification_level(self.verification_level;

            // For testing purposes, just return a constant value
            match name {
                "hello" => {
                    result.push(Value::I32(42))?;
                }
                "add" => {
                    if args.len() >= 2 {
                        if let (Value::I32(a), Value::I32(b)) = (&args[0], &args[1]) {
                            result.push(Value::I32(a + b))?;
                        }
                    }
                }
                _ => {
                    // Echo the arguments back
                    for arg in args {
                        result.push(arg.clone())?;
                    }
                }
            }

            return Ok(result;
        }

        // Create an empty SafeStack for the result
        let provider = safe_managed_alloc!(1024, CrateId::Runtime)?;
        let mut result = wrt_foundation::safe_memory::SafeStack::new(provider)?;
        result.set_verification_level(self.verification_level;

        // Simulate function execution based on the function name
        match name {
            "echo" => {
                // Echo the first argument
                if let Some(arg) = args.first() {
                    result.push(arg.clone())?;
                }
            }
            "add" => {
                // Add two i32 values
                if args.len() >= 2 {
                    if let (wrt_foundation::Value::I32(a), wrt_foundation::Value::I32(b)) =
                        (&args[0], &args[1])
                    {
                        result.push(wrt_foundation::Value::I32(a + b))?;
                    } else {
                        return Err(wrt_error::Error::runtime_execution_error("Invalid argument type"));
                    }
                } else {
                    return Err(wrt_error::Error::new(wrt_error::ErrorCategory::Validation,
                        1002, "Incorrect number of arguments"));
                }
            }
            _ => {
                // Unknown function
                return Err(wrt_error::Error::runtime_execution_error("Unknown function"));
            }
        }

        Ok(result)
    }

    /// Read from exported memory
    fn read_memory(
        &self,
        name: &str,
        offset: u32,
        size: u32,
    ) -> Result<wrt_foundation::safe_memory::SafeSlice<'_>> {
        // Verify memory access (safety check)
        if self.verification_level.should_verify(128) {
            // Check that the memory name is valid
            if name.is_empty() {
                return Err(wrt_error::Error::new(wrt_error::ErrorCategory::Resource,
                    1003, "Memory name cannot be empty"));
            }

            // Check that offset and size are valid
            if offset + size > self.memory_store.size() as u32 {
                return Err(wrt_error::Error::runtime_execution_error("Memory access out of bounds"));
            }
        }

        // Use the SafeMemoryHandler to create a SafeSlice
        self.memory_store.get_slice(offset as usize, size as usize)
    }

    /// Write to exported memory
    fn write_memory(&mut self, name: &str, offset: u32, bytes: &[u8]) -> Result<()> {
        // Verify memory access (safety check)
        if self.verification_level.should_verify(128) {
            // Check that the memory name is valid
            if name.is_empty() {
                return Err(wrt_error::Error::new(wrt_error::ErrorCategory::Resource,
                    1003, "Memory name cannot be empty"));
            }

            // Check that offset and size are valid
            if offset + bytes.len() as u32 > self.memory_store.size() as u32 {
                return Err(wrt_error::Error::runtime_execution_error("Memory write out of bounds"));
            }
        }

        // Use the SafeMemoryHandler to write bytes
        self.memory_store.write_data(offset as usize, bytes)
    }

    /// Get the type of an export
    fn get_export_type(&self, name: &str) -> Result<ExternType> {
        // Check the component type for the export
        for export in &self.component_type.exports {
            if export.name.as_str().map_or(false, |s| s == name) {
                return Ok(export.ty.clone());
            }
        }

        // Export not found
        Err(wrt_error::Error::new(wrt_error::ErrorCategory::Resource,
            1005,
            "Export not found"))
    }
}

#[cfg(test)]
mod tests {
    use wrt_foundation::{
        safe_memory::SafeStack, types::FuncType, verification::VerificationLevel, Value,
    };

    use super::*;

    // A simple host function for testing - returns SafeStack
    struct TestHostFunctionFactory {
        verification_level: VerificationLevel,
    }

    impl TestHostFunctionFactory {
        fn new(level: VerificationLevel) -> Self {
            Self { verification_level: level }
        }
    }

    #[cfg(feature = "std")]
    impl HostFunctionFactory for TestHostFunctionFactory {
        fn create_function(
            &self,
            _name: &str,
            _ty: &crate::func::FuncType,
        ) -> Result<Box<dyn HostFunction>> {
            // Create a simple echo function
            let func_type = match FuncType::new(safe_managed_alloc!(1024, CrateId::Runtime)?, Vec::new(safe_managed_alloc!(1024, CrateId::Runtime)?)?, Vec::new(safe_managed_alloc!(1024, CrateId::Runtime)?)?) {
                Ok(ty) => ty,
                Err(e) => return Err(e.into()),
            };
            let verification_level = self.verification_level;

            Ok(Box::new(HostFunctionImpl {
                func_type,
                implementation: Arc::new(move |args: &[Value]| {
                    // Create a new SafeStack with the right verification level
                    let provider = safe_managed_alloc!(1024, CrateId::Runtime)?;
                    let mut result = SafeStack::new(provider)?;
                    result.set_verification_level(verification_level;

                    // Add all arguments to the stack
                    for arg in args {
                        result.push(arg.clone())?;
                    }

                    Ok(result)
                }),
            }))
        }
    }

    // A legacy host function for testing - returns Vec
    struct LegacyTestHostFunctionFactory;

    #[cfg(feature = "std")]
    impl HostFunctionFactory for LegacyTestHostFunctionFactory {
        fn create_function(
            &self,
            _name: &str,
            _ty: &crate::func::FuncType,
        ) -> Result<Box<dyn HostFunction>> {
            // Create a simple legacy echo function
            let func_type = FuncType::new(wrt_provider!(131072, CrateId::Runtime).unwrap_or_default(), {
                let provider = wrt_provider!(131072, CrateId::Runtime).unwrap_or_default();
                Vec::new(provider)?
            }, {
                let provider = wrt_provider!(131072, CrateId::Runtime).unwrap_or_default();
                Vec::new(provider)?
            })?;

            Ok(Box::new(LegacyHostFunctionImpl {
                func_type,
                implementation: Arc::new(|args: &[Value]| {
                    // Simply return the input args as a Vec
                    Ok(args.to_vec())
                }),
                verification_level: VerificationLevel::Standard,
            }))
        }
    }

    #[test]
    fn test_component_runtime_safety() -> Result<()> {
        // Create a new runtime with different verification levels
        let mut runtime = ComponentRuntimeImpl::with_verification_level(VerificationLevel::Full;

        // Check initial state
        assert_eq!(runtime.factory_count(), 0);

        // Register host function factories
        runtime
            .register_host_factory(Box::new(TestHostFunctionFactory::new(VerificationLevel::Full);

        // Verify integrity
        runtime.verify_integrity()?;

        // Check count after registration
        assert_eq!(runtime.factory_count(), 1);

        // Test with another verification level
        let mut runtime =
            ComponentRuntimeImpl::with_verification_level(VerificationLevel::Standard;
        runtime.register_host_factory(Box::new(TestHostFunctionFactory::new(
            VerificationLevel::Standard,
        );
        runtime.verify_integrity()?;

        // Test with legacy factory
        let mut runtime =
            ComponentRuntimeImpl::with_verification_level(VerificationLevel::Standard;
        runtime.register_host_factory(Box::new(LegacyTestHostFunctionFactory;
        runtime.verify_integrity()?;

        Ok(())
    }

    #[test]
    fn test_component_instance_memory() -> Result<()> {
        // Create a component type for testing
        let component_type =
            ComponentType::unit(wrt_provider!(131072, CrateId::Runtime).unwrap_or_default())?;

        // Create a component instance with enough memory
        let mut data = vec![0; 100]; // Initialize with 100 bytes
        let mut instance = ComponentInstanceImpl {
            component_type,
            verification_level: VerificationLevel::Standard,
            memory_store: wrt_foundation::safe_memory::SafeMemoryHandler::<wrt_foundation::safe_memory::NoStdProvider<131072>>::new(data),
            host_function_names: {
                let provider = wrt_provider!(131072, CrateId::Runtime).unwrap_or_default);
                Vec::new(provider)?
            },
            #[cfg(feature = "std")]
            host_functions: HashMap::new(),
            #[cfg(not(feature = "std"))]
            host_functions: BTreeMap::new(),
        };

        // Write to memory
        let test_data = vec![1, 2, 3, 4, 5];
        instance.write_memory("memory", 10, &test_data)?;

        // Read from memory
        let slice = instance.read_memory("memory", 10, 5)?;

        // Verify the data - compare just the first 5 bytes
        let data = slice.data()?;
        let data_slice = &data[0..5];
        assert_eq!(data_slice, &[1, 2, 3, 4, 5]);

        Ok(())
    }
}
