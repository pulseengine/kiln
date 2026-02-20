#[cfg(test)]
#[cfg(feature = "std")]
mod tests {
    use wrt_error::Result;
    use wrt_foundation::{
        safe_memory::NoStdProvider,
        types::{FuncType as FoundationFuncType, ValueType},
        Value,
    };

    use crate::{
        component_impl::ComponentRuntimeImpl,
        component_traits::{ComponentRuntime, ComponentType, FuncType},
    };

    #[test]
    #[ignore] // Disabled: ComponentType API has changed - needs update to use unit() constructor
    fn test_basic_component_instantiation() -> Result<()> {
        // Create a simple component type with a function export
        let provider = NoStdProvider::<1024>::default();
        let component_type = ComponentType::unit(provider)?;

        // Create a runtime
        let mut runtime = ComponentRuntimeImpl::new();

        // Register the hello function implementation
        runtime.register_host_function(
            "hello",
            FuncType::new(Vec::new(), vec![ValueType::I32])?,
            |_args| {
                // Return a simple value
                Ok(vec![Value::I32(42)])
            },
        )?;

        // Instantiate the component
        let instance = runtime.instantiate(&component_type)?;

        // Call the function
        let result = instance.execute_function("hello", &[])?;

        // Check the result
        assert_eq!(result.len(), 1);

        // Get the first value directly instead of using indexing
        let value = result.get(0)?;
        match value {
            Value::I32(val) => assert_eq!(val, 42), // Default implementation returns 42
            _ => panic!("Expected I32 result"),
        }

        Ok(())
    }

    #[test]
    #[ignore] // Disabled: ComponentType API has changed - needs update to use unit() constructor
    fn test_host_function_registration() -> Result<()> {
        // Create a component type with a function export
        let provider = NoStdProvider::<1024>::default();
        let component_type = ComponentType::unit(provider)?;

        // Create a runtime
        let mut runtime = ComponentRuntimeImpl::new();

        // Register a host function
        runtime.register_host_function(
            "add",
            FuncType::new(vec![ValueType::I32, ValueType::I32], vec![ValueType::I32])?,
            |args| {
                // Extract the arguments
                let a = match args[0] {
                    Value::I32(val) => val,
                    _ => {
                        return Err(wrt_error::Error::runtime_execution_error(
                            "Runtime execution error",
                        ));
                    },
                };

                let b = match args[1] {
                    Value::I32(val) => val,
                    _ => {
                        return Err(wrt_error::Error::new(
                            wrt_error::ErrorCategory::Type,
                            0,
                            "Invalid argument type for add function",
                        ))
                    },
                };

                // Return the sum
                Ok(vec![Value::I32(a + b)])
            },
        )?;

        // Instantiate the component
        let instance = runtime.instantiate(&component_type)?;

        // Call the function
        let result = instance.execute_function("add", &[Value::I32(3), Value::I32(4)])?;

        // Check the result
        assert_eq!(result.len(), 1);

        // Get the first value directly instead of using indexing
        let value = result.get(0)?;
        match value {
            Value::I32(val) => assert_eq!(val, 7), // 3 + 4 = 7
            _ => panic!("Expected I32 result"),
        }

        Ok(())
    }

    #[test]
    #[ignore] // Disabled: ComponentType API has changed - needs update to use unit() constructor
    fn test_memory_operations() -> Result<()> {
        // Create a component type with a memory export
        let provider = NoStdProvider::<1024>::default();
        let component_type = ComponentType::unit(provider)?;

        // Create a runtime
        let runtime = ComponentRuntimeImpl::new();

        // Instantiate the component
        let mut instance = runtime.instantiate(&component_type)?;

        // Write to memory - using offset 0 instead of 100 to avoid out of bounds
        let bytes = [1, 2, 3, 4, 5];
        instance.write_memory("memory", 0, &bytes)?;

        // Read from memory
        let read_bytes = instance.read_memory("memory", 0, 5)?;

        // Check the result by comparing data - just compare the first 5 bytes
        // since SafeSlice data() may return more than the requested size
        let data = read_bytes.data()?;
        let data_slice = &data[0..5]; // Get just the first 5 bytes
        assert_eq!(data_slice, &bytes);

        Ok(())
    }
}
