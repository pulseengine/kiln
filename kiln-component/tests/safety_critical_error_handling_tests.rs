//! Safety-Critical Error Handling Tests
//!
//! This module tests that all error conditions are handled gracefully
//! with proper error returns instead of panics, ensuring robust operation
//! in safety-critical environments.
//!
//! # Safety Requirements
//! - SW-REQ-ID: REQ_ERR_001 - Graceful error handling
//! - SW-REQ-ID: REQ_ERR_002 - No panic on resource exhaustion
//! - ASIL Level: ASIL-C

#![forbid(unsafe_code)]

use kiln_component::{
    bounded_component_infra::*,
    resource_management::{ResourceError, ResourceTable},
    resources::resource_lifecycle::ResourceLifecycleManager,
};
use kiln_error::codes;

#[cfg(test)]
mod error_handling_tests {
    use super::*;

    /// Test that capacity exceeded returns error, not panic
    #[test]
    fn test_no_panic_on_capacity_exceeded() {
        let mut vec = new_component_vec::<u32>();

        // Fill to capacity
        for i in 0..MAX_COMPONENT_INSTANCES {
            let result = vec.push(i as u32);
            assert!(result.is_ok());
        }

        // All subsequent pushes should return error, not panic
        for i in 0..100 {
            match vec.push(i) {
                Err(e) => assert_eq!(e.code, codes::FOUNDATION_BOUNDED_CAPACITY_EXCEEDED),
                Ok(()) => panic!("Push should have failed"),
            }
        }
    }

    /// Test error handling in string operations
    #[test]
    fn test_string_error_handling() {
        // Test empty string
        let empty = bounded_component_name_from_str("").unwrap();
        assert_eq!(empty.len(), 0);

        // Test exact limit
        let exact_limit = "a".repeat(MAX_COMPONENT_NAME_LEN);
        let name = bounded_component_name_from_str(&exact_limit).unwrap();
        assert_eq!(name.len(), MAX_COMPONENT_NAME_LEN);

        // Test over limit - should return error
        let over_limit = "a".repeat(MAX_COMPONENT_NAME_LEN + 1);
        match bounded_component_name_from_str(&over_limit) {
            Err(e) => assert_eq!(e.code, codes::FOUNDATION_BOUNDED_CAPACITY_EXCEEDED),
            Ok(_) => panic!("Expected capacity-exceeded error"),
        }

        // The previously built name is unchanged by the failed construction
        assert_eq!(name.len(), MAX_COMPONENT_NAME_LEN);
    }

    /// Test map error handling
    #[test]
    fn test_map_error_handling() {
        let mut map = new_type_map::<u32>();

        let key1 = 42u32;
        assert_eq!(map.insert(key1, 100).unwrap(), None);

        // Insert with same key replaces the value and returns the old one
        assert_eq!(map.insert(key1, 200).unwrap(), Some(100));
        assert_eq!(map.get(&key1), Some(&200));

        // Test non-existent key lookup
        let missing_key = 999u32;
        assert_eq!(map.get(&missing_key), None); // Should return None, not panic
    }

    /// Test resource table error handling
    #[test]
    fn test_resource_table_error_handling() {
        let mut table = ResourceTable::new();

        // u32::MAX is the reserved invalid handle
        let invalid_handle = 0xFFFF_FFFF;

        let result = table.deallocate(invalid_handle);
        assert_eq!(result, Err(ResourceError::InvalidHandle));

        let result = table.get(invalid_handle);
        assert!(matches!(result, Err(ResourceError::InvalidHandle)));

        // A well-formed handle that was never allocated is not found
        assert_eq!(table.deallocate(12345), Err(ResourceError::NotFound));

        // Allocate and deallocate correctly
        let handle = table.allocate().expect("Failed to allocate");
        assert!(table.deallocate(handle).is_ok());

        // Double deallocate should error
        assert_eq!(table.deallocate(handle), Err(ResourceError::NotFound));
    }

    /// Test resource lifecycle error handling
    #[test]
    fn test_resource_lifecycle_error_handling() {
        let mut manager = ResourceLifecycleManager::new();

        manager
            .register_type(1, "TestResource", Some(100))
            .expect("Failed to register resource type");

        // Creating a resource of an unregistered type is an error
        assert!(manager.create_resource(2, 0, None).is_err());

        let handle = manager.create_resource(1, 0, None).expect("Failed to create resource");

        // Test invalid operations
        let invalid_handle = handle + 1000;

        // Borrow non-existent resource
        assert!(manager.borrow_resource(invalid_handle, 1, false).is_err());

        // Release non-existent borrow
        assert!(manager.release_borrow(invalid_handle, 1).is_err());

        // Transfer non-existent resource
        assert!(manager.transfer_ownership(invalid_handle, 0, 999).is_err());

        // Drop non-existent resource
        assert!(manager.drop_resource(invalid_handle).is_err());

        // Test double operations
        assert!(manager.drop_resource(handle).is_ok());
        assert!(manager.drop_resource(handle).is_err()); // Already dropped
    }

    /// Test stack overflow protection
    #[test]
    fn test_stack_overflow_protection() {
        let mut stack = new_call_stack::<u32>();

        // Push until full
        while !stack.is_full() {
            assert!(stack.push(1).is_ok());
        }

        // Further pushes should error, not overflow
        for _ in 0..100 {
            assert!(stack.push(2).is_err());
        }

        // Pop should still work
        assert_eq!(stack.pop(), Some(1));

        // Can push again after pop
        assert!(stack.push(3).is_ok());
    }

    /// Test empty collection operations
    #[test]
    fn test_empty_collection_operations() {
        let mut vec = new_component_vec::<u32>();

        // Pop from empty should return None, not panic
        assert_eq!(vec.pop(), None);

        // Multiple pops should continue returning None
        for _ in 0..10 {
            assert_eq!(vec.pop(), None);
        }

        let map = new_type_map::<String>();

        // Get from empty map
        let key = 42u32;
        assert_eq!(map.get(&key), None);

        // Iteration over empty map should work
        assert_eq!(map.iter().count(), 0);
    }

    /// Test boundary value error handling
    #[test]
    fn test_boundary_value_errors() {
        let mut map = new_type_map::<u32>();

        // Insert at boundary values
        assert!(map.insert(0, 100).is_ok());
        assert!(map.insert(u32::MAX, 200).is_ok());
        assert!(map.insert(u32::MAX / 2, 300).is_ok());

        // Lookup at boundaries
        assert_eq!(map.get(&0), Some(&100));
        assert_eq!(map.get(&u32::MAX), Some(&200));
        assert_eq!(map.get(&(u32::MAX - 1)), None);
    }

    /// Test error propagation through layers
    #[test]
    fn test_error_propagation() {
        fn allocate_nested(
            count: usize,
        ) -> kiln_error::Result<BoundedMemoryVec<BoundedComponentName>> {
            let mut outer = BoundedMemoryVec::new();

            for i in 0..count {
                let inner = bounded_component_name_from_str(&format!("name_{}", i))?;
                outer.push(inner)?;
            }

            Ok(outer)
        }

        // Within capacity: success
        let nested = allocate_nested(MAX_MEMORY_INSTANCES).unwrap();
        assert_eq!(nested.len(), MAX_MEMORY_INSTANCES);

        // Beyond capacity: the push error propagates through `?`
        match allocate_nested(MAX_MEMORY_INSTANCES + 1) {
            Err(e) => assert_eq!(e.code, codes::FOUNDATION_BOUNDED_CAPACITY_EXCEEDED),
            Ok(_) => panic!("Expected capacity-exceeded error"),
        }
    }

    /// Test error recovery
    #[test]
    fn test_error_recovery() {
        let mut vec = new_component_vec::<u32>();

        // Fill vector
        while !vec.is_full() {
            vec.push(1).unwrap();
        }

        // Cause error
        assert!(vec.push(2).is_err());

        // Vector should still be usable
        assert_eq!(vec.len(), MAX_COMPONENT_INSTANCES);
        assert_eq!(vec.pop(), Some(1));

        // Can continue operations after error
        assert!(vec.push(3).is_ok());
        assert_eq!(vec.len(), MAX_COMPONENT_INSTANCES);
    }
}
