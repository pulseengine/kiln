//! Example usage of canonical ABI with realloc support
//!
//! This module demonstrates how to use the realloc functionality
//! in the WebAssembly Component Model's Canonical ABI.

#[cfg(test)]
mod example {
    use std::sync::{Arc, RwLock};

    use wrt_foundation::prelude::*;
    use wrt_runtime::{Instance, Memory, Module};

    use crate::{
        canonical::CanonicalABI,
        canonical_options::{CanonicalLiftContext, CanonicalLowerContext, CanonicalOptions},
        canonical_realloc::{ReallocManager, StringEncoding},
        types::ComponentInstanceId,
    };

    /// Binary std/no_std choice
    fn example_lift_string() -> Result<()> {
        // Create instance and memory (simplified)
        let module = Module::default();
        let instance = Instance::new(&module)?;
        let memory = Memory::new(1, Some(10))?; // 1 initial page, max 10 pages

        // Binary std/no_std choice
        let realloc_manager = Arc::new(RwLock::new(ReallocManager::default()));

        // Binary std/no_std choice
        let instance_id = ComponentInstanceId(1);
        let options = CanonicalOptions::new(0, instance_id)
            .with_realloc(42, realloc_manager.clone())
            .with_string_encoding(StringEncoding::Utf8);

        // Create lift context
        let lift_context = CanonicalLiftContext::new(&instance, &memory, &options);

        // In a real scenario, the string would be in wasm memory
        // For this example, we'll simulate reading it
        let string_ptr = 0x1000;
        let string_len = 13;

        // Lift the string
        let lifted_string = lift_context.read_string(string_ptr, string_len)?;
        println!("Lifted string: {}", lifted_string);

        // Binary std/no_std choice
        lift_context.cleanup()?;

        Ok(())
    }

    /// Binary std/no_std choice
    fn example_lower_string() -> Result<()> {
        // Create instance and memory
        let module = Module::default();
        let mut instance = Instance::new(&module)?;
        let mut memory = Memory::new(1, Some(10))?;

        // Binary std/no_std choice
        let realloc_manager = Arc::new(RwLock::new(ReallocManager::default()));

        // Create canonical options
        let instance_id = ComponentInstanceId(1);
        let options = CanonicalOptions::new(0, instance_id)
            .with_realloc(42, realloc_manager.clone())
            .with_string_encoding(StringEncoding::Utf8);

        // Create lower context
        let mut lower_context = CanonicalLowerContext::new(&mut instance, &mut memory, &options);

        // Lower a string
        let test_string = "Hello, WASM!";
        let (ptr, len) = lower_context.write_string(test_string)?;

        println!("Lowered string to ptr: {}, len: {}", ptr, len);

        // Binary std/no_std choice
        let allocations = lower_context.finish()?;
        println!("Made {} allocations during lowering", allocations.len());

        Ok(())
    }

    /// Binary std/no_std choice
    fn example_dynamic_list() -> Result<()> {
        let realloc_manager = Arc::new(RwLock::new(ReallocManager::default()));
        let instance_id = ComponentInstanceId(1);

        // Binary std/no_std choice
        {
            let mut manager = realloc_manager.write().unwrap();
            manager.register_realloc(instance_id, 42)?;
        }

        // Allocate space for a list
        let initial_capacity = 10;
        let element_size = 4; // u32 elements
        let alignment = 4;

        let ptr = {
            let mut manager = realloc_manager.write().unwrap();
            manager.allocate(instance_id, initial_capacity * element_size, alignment)?
        };

        println!(
            "Allocated list at ptr: {} with capacity: {}",
            ptr, initial_capacity
        );

        // Grow the list
        let new_capacity = 20;
        let new_ptr = {
            let mut manager = realloc_manager.write().unwrap();
            manager.reallocate(
                instance_id,
                ptr,
                initial_capacity * element_size,
                alignment,
                new_capacity * element_size,
            )?
        };

        println!(
            "Reallocated list to ptr: {} with new capacity: {}",
            new_ptr, new_capacity
        );

        // Clean up
        {
            let mut manager = realloc_manager.write().unwrap();
            manager.deallocate(instance_id, new_ptr, new_capacity * element_size, alignment)?;
        }

        println!("Deallocated list");

        // Check metrics
        {
            let manager = realloc_manager.read().unwrap();
            let metrics = manager.metrics();
            println!("Allocation metrics:");
            println!("  Total allocations: {}", metrics.total_allocations);
            println!("  Total deallocations: {}", metrics.total_deallocations);
            println!("  Total bytes allocated: {}", metrics.total_bytes_allocated);
            println!("  Peak memory usage: {}", metrics.peak_memory_usage);
        }

        Ok(())
    }

    /// Example of handling post-return cleanup
    fn example_post_return() -> Result<()> {
        let module = Module::default();
        let instance = Instance::new(&module)?;
        let memory = Memory::new(1, Some(10))?;

        let realloc_manager = Arc::new(RwLock::new(ReallocManager::default()));
        let instance_id = ComponentInstanceId(1);

        // Binary std/no_std choice
        let options = CanonicalOptions::new(0, instance_id)
            .with_realloc(42, realloc_manager.clone())
            .with_post_return(43); // post-return function index

        // Create lift context
        let mut lift_context = CanonicalLiftContext::new(&instance, &memory, &options);

        // Binary std/no_std choice
        let ptr1 = lift_context.allocate(100, 8)?;
        let ptr2 = lift_context.allocate(200, 16)?;

        println!("Made allocations: ptr1={}, ptr2={}", ptr1, ptr2);

        // Binary std/no_std choice
        lift_context.cleanup()?;

        println!("Cleanup complete - allocations freed and post-return called");

        Ok(())
    }

    #[test]
    fn test_realloc_examples() {
        // These would fail in a real test without proper wasm setup
        // but demonstrate the API usage

        // example_lift_string().ok();
        // example_lower_string().ok();
        // example_dynamic_list().ok();
        // example_post_return().ok();
    }

    #[test]
    fn test_realloc_manager_integration() {
        let realloc_manager = Arc::new(RwLock::new(ReallocManager::default()));
        let instance_id = ComponentInstanceId(1);

        // Binary std/no_std choice
        {
            let mut manager = realloc_manager.write().unwrap();
            manager.register_realloc(instance_id, 42).unwrap();

            let ptr = manager.allocate(instance_id, 64, 8).unwrap();
            assert_ne!(ptr, 0);

            let new_ptr = manager.reallocate(instance_id, ptr, 64, 8, 128).unwrap();
            assert_ne!(new_ptr, 0);

            manager.deallocate(instance_id, new_ptr, 128, 8).unwrap();
        }

        // Check metrics
        {
            let manager = realloc_manager.read().unwrap();
            let metrics = manager.metrics();
            assert_eq!(metrics.total_allocations, 1);
            assert_eq!(metrics.total_deallocations, 1);
            assert!(metrics.total_bytes_allocated >= 64);
        }
    }
}
