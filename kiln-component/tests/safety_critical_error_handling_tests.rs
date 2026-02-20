//! Safety-Critical Error Handling Tests
//!
//! This module ensures that all errors are handled through Result<T,E>
//! and that no panics occur in any error condition. All error paths
//! are tested to verify graceful degradation.
//!
//! # Safety Requirements
//! - SW-REQ-ID: REQ_ERR_001 - No panic paths allowed
//! - SW-REQ-ID: REQ_ERR_002 - Explicit error propagation
//! - ASIL Level: ASIL-C

#![forbid(unsafe_code)]

extern crate alloc;

use wrt_component::{
    bounded_component_infra::*,
    canonical_abi::{CanonicalABI, CanonicalOptions},
    resource_management::ResourceTable,
    resources::resource_lifecycle::{
        Resource, ResourceLifecycleManager, ResourceMetadata, ResourceType,
    },
};
use wrt_foundation::{
    WrtError,
    bounded::{BoundedString, BoundedVec},
};

#[cfg(test)]
mod error_handling_tests {
    use super::*;

    /// Test that capacity exceeded returns error, not panic
    #[test]
    fn test_no_panic_on_capacity_exceeded() {
        let vec_result = new_component_vec::<u32>();
        assert!(vec_result.is_ok());

        let mut vec = vec_result.unwrap();

        // Fill to capacity
        for i in 0..MAX_COMPONENT_INSTANCES {
            let result = vec.try_push(i as u32);
            assert!(result.is_ok());
        }

        // All subsequent pushes should return error, not panic
        for i in 0..100 {
            let result = vec.try_push(i);
            match result {
                Err(WrtError::CapacityExceeded) => {
                    // Expected - no panic
                },
                Ok(_) => panic!("Push should have failed"),
                Err(e) => panic!("Unexpected error: {:?}", e),
            }
        }
    }

    /// Test error handling in string operations
    #[test]
    fn test_string_error_handling() {
        let name_result = new_component_name();
        assert!(name_result.is_ok());

        let mut name = name_result.unwrap();

        // Test empty string
        let result = name.try_set("Error");
        assert!(result.is_ok());
        assert_eq!(name.len(), 0);

        // Test exact limit
        let exact_limit = "a".repeat(MAX_COMPONENT_NAME_LEN);
        let result = name.try_set(&exact_limit);
        assert!(result.is_ok());

        // Test over limit - should return error
        let over_limit = "a".repeat(MAX_COMPONENT_NAME_LEN + 1);
        let result = name.try_set(&over_limit);
        match result {
            Err(WrtError::CapacityExceeded) => {
                // Expected
            },
            _ => panic!("Expected CapacityExceeded error"),
        }

        // Original value should be unchanged after error
        assert_eq!(name.len(), MAX_COMPONENT_NAME_LEN);
    }

    /// Test map error handling
    #[test]
    fn test_map_error_handling() {
        let map_result = new_type_map::<u32>();
        assert!(map_result.is_ok());

        let mut map = map_result.unwrap();

        // Test duplicate key handling
        let key1 = 42u32;
        let result1 = map.try_insert(key1, 100);
        assert!(result1.is_ok());

        // Insert with same key should handle gracefully
        let result2 = map.try_insert(key1, 200);
        match result2 {
            Ok(_) => {
                // Key was updated
                assert_eq!(map.get(&key1), Some(&200));
            },
            Err(_) => {
                // Or error was returned - both are valid
            },
        }

        // Test non-existent key lookup
        let missing_key = 999u32;
        let value = map.get(&missing_key);
        assert_eq!(value, None); // Should return None, not panic
    }

    /// Test resource table error handling
    #[test]
    fn test_resource_table_error_handling() {
        let mut table = ResourceTable::new();

        // Test invalid handle operations
        let invalid_handle = 0xFFFFFFFF;

        // Deallocate non-existent handle
        let result = table.deallocate(invalid_handle);
        assert!(result.is_err());
        match result {
            Err(WrtError::InvalidHandle) => {
                // Expected
            },
            _ => panic!("Expected InvalidHandle error"),
        }

        // Get non-existent handle
        let result = table.get(invalid_handle);
        assert!(result.is_err());

        // Allocate and deallocate correctly
        let handle = table.allocate().expect("Failed to allocate");
        assert!(table.deallocate(handle).is_ok());

        // Double deallocate should error
        let result = table.deallocate(handle);
        assert!(result.is_err());
    }

    /// Test canonical ABI error handling
    #[test]
    fn test_canonical_abi_error_handling() {
        let abi = CanonicalABI::new();

        // Test with null memory
        let null_memory = core::ptr::null();
        let options = CanonicalOptions::default();

        // These operations should handle null memory gracefully
        // (Note: actual implementation details may vary)

        // Test invalid offset
        let result = abi.lift_flat_value(null_memory, 0xFFFFFFFF, &options);
        assert!(result.is_err());

        // Test invalid size
        let result = abi.lower_flat_value(null_memory, 0, usize::MAX, &options);
        assert!(result.is_err());
    }

    /// Test resource lifecycle error handling
    #[test]
    fn test_resource_lifecycle_error_handling() {
        let mut manager = ResourceLifecycleManager::new();

        let resource_type = ResourceType {
            type_idx: 1,
            name: bounded_component_name_from_str("TestResource").unwrap(),
            destructor: Some(100),
        };

        let metadata = ResourceMetadata {
            created_at: Some(0),
            last_accessed: None,
            creator: 0,
            owner: 0,
            user_data: None,
        };

        // Create resource
        let handle = manager
            .create_resource(resource_type, metadata)
            .expect("Failed to create resource");

        // Test invalid operations
        let invalid_handle = handle + 1000;

        // Borrow non-existent resource
        let result = manager.borrow_resource(invalid_handle);
        assert!(result.is_err());

        // Release non-existent borrow
        let result = manager.release_borrow(invalid_handle);
        assert!(result.is_err());

        // Transfer non-existent resource
        let result = manager.transfer_ownership(invalid_handle, 999);
        assert!(result.is_err());

        // Drop non-existent resource
        let result = manager.drop_resource(invalid_handle);
        assert!(result.is_err());

        // Test double operations
        assert!(manager.drop_resource(handle).is_ok());
        let result = manager.drop_resource(handle);
        assert!(result.is_err()); // Already dropped
    }

    /// Test stack overflow protection
    #[test]
    fn test_stack_overflow_protection() {
        let stack_result = new_call_stack::<u32>();
        assert!(stack_result.is_ok());

        let mut stack = stack_result.unwrap();

        // Push until full
        while !stack.is_full() {
            assert!(stack.try_push(1).is_ok());
        }

        // Further pushes should error, not overflow
        for _ in 0..100 {
            let result = stack.try_push(2);
            assert!(result.is_err());
        }

        // Pop should still work
        assert_eq!(stack.pop(), Some(1));

        // Can push again after pop
        assert!(stack.try_push(3).is_ok());
    }

    /// Test empty collection operations
    #[test]
    fn test_empty_collection_operations() {
        // Test empty vector operations
        let vec_result = new_component_vec::<u32>();
        assert!(vec_result.is_ok());

        let mut vec = vec_result.unwrap();

        // Pop from empty should return None, not panic
        assert_eq!(vec.pop(), None);

        // Multiple pops should continue returning None
        for _ in 0..10 {
            assert_eq!(vec.pop(), None);
        }

        // Test empty map operations
        let map_result = new_type_map::<String>();
        assert!(map_result.is_ok());

        let map = map_result.unwrap();

        // Get from empty map
        let key = 42u32;
        assert_eq!(map.get(&key), None);

        // Iteration over empty map should work
        let count = map.iter().count();
        assert_eq!(count, 0);
    }

    /// Test boundary value error handling
    #[test]
    fn test_boundary_value_errors() {
        // Test with maximum indices
        let vec_result = new_type_map::<u32>();
        assert!(vec_result.is_ok());

        let mut map = vec_result.unwrap();

        // Insert at boundary values
        assert!(map.try_insert(0, 100).is_ok());
        assert!(map.try_insert(u32::MAX, 200).is_ok());
        assert!(map.try_insert(u32::MAX / 2, 300).is_ok());

        // Lookup at boundaries
        assert_eq!(map.get(&0), Some(&100));
        assert_eq!(map.get(&u32::MAX), Some(&200));
        assert_eq!(map.get(&(u32::MAX - 1)), None);
    }

    /// Test error propagation through layers
    #[test]
    fn test_error_propagation() {
        fn allocate_nested() -> wrt_error::Result<BoundedComponentVec<BoundedExportVec<u32>>> {
            let mut outer = new_component_vec()?;

            // Try to allocate nested vectors
            for _ in 0..10 {
                let inner = new_export_vec()?;
                outer.try_push(inner)?;
            }

            Ok(outer)
        }

        // Should propagate any allocation errors
        match allocate_nested() {
            Ok(_) => {
                // Success case
            },
            Err(e) => {
                // Error propagated correctly
                match e {
                    WrtError::OutOfMemory => {},
                    WrtError::CapacityExceeded => {},
                    _ => panic!("Unexpected error type: {:?}", e),
                }
            },
        }
    }

    /// Test that all operations return Result
    #[test]
    fn test_all_operations_return_result() {
        // This test verifies the API design
        // All operations that can fail should return Result<T, E>

        let vec_result: wrt_error::Result<_> = new_component_vec::<u32>();
        assert!(vec_result.is_ok());

        let map_result: wrt_error::Result<_> = new_export_map::<u32>();
        assert!(map_result.is_ok());

        let string_result: wrt_error::Result<_> = new_component_name();
        assert!(string_result.is_ok());

        let bounded_string_result: wrt_error::Result<_> = bounded_component_name_from_str("test");
        assert!(bounded_string_result.is_ok());

        // All constructors return Result, enabling proper error handling
    }

    /// Test error recovery
    #[test]
    fn test_error_recovery() {
        let vec_result = new_component_vec::<u32>();
        assert!(vec_result.is_ok());

        let mut vec = vec_result.unwrap();

        // Fill vector
        while !vec.is_full() {
            vec.try_push(1).unwrap();
        }

        // Cause error
        assert!(vec.try_push(2).is_err());

        // Vector should still be usable
        assert_eq!(vec.len(), MAX_COMPONENT_INSTANCES);
        assert_eq!(vec.pop(), Some(1));

        // Can continue operations after error
        assert!(vec.try_push(3).is_ok());
        assert_eq!(vec.len(), MAX_COMPONENT_INSTANCES);
    }
}

#[cfg(all(test, not(feature = "std")))]
mod no_std_error_tests {
    use super::*;

    /// Test error handling in no_std environment
    #[test]
    fn test_no_std_error_handling() {
        // Verify error types work without std
        let error = WrtError::CapacityExceeded;

        // Error should have a representation
        let _ = format!("{:?}", error);

        // Result type should work
        let result: wrt_error::Result<()> = Err(WrtError::OutOfMemory);
        assert!(result.is_err());
    }
}
