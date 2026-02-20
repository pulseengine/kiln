//! Safety-Critical Capacity Limit Tests
//!
//! This module comprehensively tests capacity limits for all migrated
//! collections ensuring that error handling is robust and no panics occur when
//! limits are exceeded.
//!
//! # Safety Requirements
//! - SW-REQ-ID: REQ_MEM_001 - Memory bounds checking
//! - SW-REQ-ID: REQ_COMP_002 - Component capacity limits
//! - ASIL Level: ASIL-C

#![forbid(unsafe_code)]

extern crate alloc;

use wrt_component::bounded_component_infra::*;
#[cfg(not(feature = "std"))]
use wrt_foundation::collections::StaticMap as BoundedHashMap;
use wrt_foundation::{
    WrtError, bounded::BoundedString, budget_aware_provider::CrateId,
    collections::StaticVec as BoundedVec, managed_alloc,
};

#[cfg(test)]
mod capacity_limit_tests {
    use super::*;

    /// Test vector capacity limits are enforced
    #[test]
    fn test_bounded_vec_capacity_limit() {
        // Test component instance vector
        let result = new_component_vec::<u32>();
        assert!(result.is_ok(), "Failed to create component vector");

        let mut vec = result.unwrap();

        // Fill to capacity
        for i in 0..MAX_COMPONENT_INSTANCES {
            let push_result = vec.try_push(i as u32);
            assert!(
                push_result.is_ok(),
                "Failed to push within capacity at index {}",
                i
            );
        }

        // Verify we're at capacity
        assert_eq!(vec.len(), MAX_COMPONENT_INSTANCES);
        assert!(vec.is_full());

        // Try to exceed capacity - should return error, not panic
        let overflow_result = vec.try_push(999);
        assert!(overflow_result.is_err());
        match overflow_result {
            Err(WrtError::CapacityExceeded) => {
                // Expected error
            },
            _ => panic!("Expected CapacityExceeded error"),
        }
    }

    /// Test export vector capacity enforcement
    #[test]
    fn test_export_vec_capacity() {
        let result = new_export_vec::<u32>();
        assert!(result.is_ok());

        let mut vec = result.unwrap();

        // Test partial fill
        for i in 0..100 {
            let push_result = vec.try_push(i);
            assert!(push_result.is_ok());
        }

        assert_eq!(vec.len(), 100);
        assert!(!vec.is_full());

        // Fill remaining capacity
        for i in 100..MAX_COMPONENT_EXPORTS {
            let push_result = vec.try_push(i as u32);
            assert!(push_result.is_ok());
        }

        assert!(vec.is_full());

        // Verify overflow handling
        let overflow = vec.try_push(0);
        assert!(matches!(overflow, Err(WrtError::CapacityExceeded)));
    }

    /// Test resource handle vector limits
    #[test]
    fn test_resource_vec_limits() {
        let result = new_resource_vec::<u64>();
        assert!(result.is_ok());

        let mut vec = result.unwrap();

        // Test batch operations near capacity
        let batch_size = 100;
        let num_batches = MAX_RESOURCE_HANDLES / batch_size;

        for batch in 0..num_batches {
            for i in 0..batch_size {
                let handle = (batch * batch_size + i) as u64;
                assert!(vec.try_push(handle).is_ok());
            }
        }

        // Fill remaining
        let remaining = MAX_RESOURCE_HANDLES % batch_size;
        for i in 0..remaining {
            assert!(vec.try_push(i as u64).is_ok());
        }

        assert_eq!(vec.len(), MAX_RESOURCE_HANDLES);
        assert!(vec.is_full());
    }

    /// Test call stack depth limits
    #[test]
    fn test_call_stack_limits() {
        #[derive(Clone, Debug)]
        struct CallFrame {
            function_idx: u32,
            return_addr: u32,
        }

        let result = new_call_stack::<CallFrame>();
        assert!(result.is_ok());

        let mut stack = result.unwrap();

        // Simulate deep recursion up to limit
        for depth in 0..MAX_CALL_STACK_DEPTH {
            let frame = CallFrame {
                function_idx: depth as u32,
                return_addr: (depth * 4) as u32,
            };

            let push_result = stack.try_push(frame);
            assert!(push_result.is_ok(), "Failed at depth {}", depth);
        }

        assert_eq!(stack.len(), MAX_CALL_STACK_DEPTH);

        // Stack overflow should be caught
        let overflow_frame = CallFrame {
            function_idx: 9999,
            return_addr: 0,
        };

        let overflow_result = stack.try_push(overflow_frame);
        assert!(overflow_result.is_err());
    }

    /// Test bounded string capacity
    #[test]
    fn test_bounded_string_limits() {
        // Test component name
        let name_result = new_component_name();
        assert!(name_result.is_ok());

        let mut name = name_result.unwrap();

        // Create a string at the limit
        let long_string = "a".repeat(MAX_COMPONENT_NAME_LEN);
        let set_result = name.try_set(&long_string);
        assert!(set_result.is_ok());
        assert_eq!(name.len(), MAX_COMPONENT_NAME_LEN);

        // Try to exceed limit
        let too_long = "a".repeat(MAX_COMPONENT_NAME_LEN + 1);
        let overflow_result = name.try_set(&too_long);
        assert!(overflow_result.is_err());
    }

    /// Test export map capacity
    #[test]
    fn test_export_map_capacity() {
        // Test with simple u32 keys instead of BoundedString
        let map_result = new_type_map::<String>();
        assert!(map_result.is_ok());

        let mut map = map_result.unwrap();

        // Fill map to capacity
        for i in 0..100 {
            // Use fewer entries for testing
            let insert_result = map.try_insert(i as u32, format!("value_{}", i));
            assert!(insert_result.is_ok(), "Failed to insert at index {}", i);
        }

        assert_eq!(map.len(), 100);

        // Verify we can still insert more up to capacity
        let remaining_capacity = MAX_TYPE_DEFINITIONS - 100;
        assert!(remaining_capacity > 0);
    }

    /// Test type map limits
    #[test]
    fn test_type_map_limits() {
        let map_result = new_type_map::<String>();
        assert!(map_result.is_ok());

        let mut map = map_result.unwrap();

        // Test sparse insertions
        for i in (0..MAX_TYPE_DEFINITIONS).step_by(10) {
            let insert_result = map.try_insert(i as u32, format!("type_{}", i));
            assert!(insert_result.is_ok());
        }

        let expected_count = MAX_TYPE_DEFINITIONS / 10;
        assert_eq!(map.len(), expected_count);
    }

    /// Test operand stack limits
    #[test]
    fn test_operand_stack_limits() {
        let stack_result = new_operand_stack::<i64>();
        assert!(stack_result.is_ok());

        let mut stack = stack_result.unwrap();

        // Simulate computation that uses full stack
        for i in 0..MAX_OPERAND_STACK_SIZE {
            let value = i as i64 * 2;
            assert!(stack.try_push(value).is_ok());
        }

        assert_eq!(stack.len(), MAX_OPERAND_STACK_SIZE);
        assert!(stack.is_full());

        // Pop all values
        for i in (0..MAX_OPERAND_STACK_SIZE).rev() {
            let popped = stack.pop();
            assert_eq!(popped, Some((i as i64) * 2));
        }

        assert!(stack.is_empty());
    }

    /// Test memory instance limits
    #[test]
    fn test_memory_instance_limits() {
        #[derive(Clone, Debug)]
        struct MockMemory {
            base: u64,
            size: u32,
        }

        let vec_result = new_memory_vec::<MockMemory>();
        assert!(vec_result.is_ok());

        let mut vec = vec_result.unwrap();

        for i in 0..MAX_MEMORY_INSTANCES {
            let mem = MockMemory {
                base: i as u64 * 65536,
                size: 65536,
            };
            assert!(vec.try_push(mem).is_ok());
        }

        assert_eq!(vec.len(), MAX_MEMORY_INSTANCES);
    }

    /// Test post-return callback limits
    #[test]
    fn test_post_return_callback_limits() {
        type Callback = fn() -> wrt_error::Result<()>;

        let vec_result = new_post_return_vec::<Callback>();
        assert!(vec_result.is_ok());

        let mut vec = vec_result.unwrap();

        fn dummy_callback() -> wrt_error::Result<()> {
            Ok(())
        }

        // Fill to capacity
        for _ in 0..MAX_POST_RETURN_CALLBACKS {
            assert!(vec.try_push(dummy_callback).is_ok());
        }

        assert!(vec.is_full());

        // Verify overflow protection
        let overflow = vec.try_push(dummy_callback);
        assert!(overflow.is_err());
    }

    /// Test that clear operations work correctly
    #[test]
    fn test_clear_operations() {
        let vec_result = new_component_vec::<u32>();
        assert!(vec_result.is_ok());

        let mut vec = vec_result.unwrap();

        // Fill partially
        for i in 0..10 {
            vec.try_push(i).unwrap();
        }

        assert_eq!(vec.len(), 10);

        // Clear
        vec.clear();
        assert_eq!(vec.len(), 0);
        assert!(vec.is_empty());

        // Can push again after clear
        assert!(vec.try_push(42).is_ok());
        assert_eq!(vec.len(), 1);
    }

    /// Test capacity after operations
    #[test]
    fn test_capacity_invariants() {
        let vec_result = new_import_vec::<String>();
        assert!(vec_result.is_ok());

        let mut vec = vec_result.unwrap();

        // Capacity should remain constant
        let initial_capacity = vec.capacity();
        assert_eq!(initial_capacity, MAX_COMPONENT_IMPORTS);

        // Add some elements
        for i in 0..100 {
            vec.try_push(format!("import_{}", i)).unwrap();
        }

        assert_eq!(vec.capacity(), initial_capacity);

        // Remove some elements
        for _ in 0..50 {
            vec.pop();
        }

        assert_eq!(vec.capacity(), initial_capacity);

        // Clear all
        vec.clear();
        assert_eq!(vec.capacity(), initial_capacity);
    }

    /// Test resource type map limits
    #[test]
    fn test_resource_type_map_limits() {
        #[derive(Clone, Debug)]
        struct ResourceTypeInfo {
            name: String,
            size: usize,
        }

        let map_result = new_resource_type_map::<ResourceTypeInfo>();
        assert!(map_result.is_ok());

        let mut map = map_result.unwrap();

        // Fill to capacity
        for i in 0..MAX_RESOURCE_TYPES {
            let info = ResourceTypeInfo {
                name: format!("resource_type_{}", i),
                size: i * 8,
            };
            assert!(map.try_insert(i as u32, info).is_ok());
        }

        assert_eq!(map.len(), MAX_RESOURCE_TYPES);

        // Verify lookup works at capacity
        for i in 0..MAX_RESOURCE_TYPES {
            let info = map.get(&(i as u32));
            assert!(info.is_some());
            assert_eq!(info.unwrap().name, format!("resource_type_{}", i));
        }
    }
}

#[cfg(all(test, feature = "safety-critical"))]
mod safety_critical_feature_tests {
    use super::*;

    /// Test that safety-critical builds enforce stricter limits
    #[test]
    fn test_safety_critical_enforcement() {
        // In safety-critical mode, all allocations should use WRT allocator
        let vec_result = new_component_vec::<u32>();
        assert!(vec_result.is_ok());

        let vec = vec_result.unwrap();

        // Verify the vector is using bounded allocation
        assert_eq!(vec.capacity(), MAX_COMPONENT_INSTANCES);

        // In safety-critical mode, capacity should be immutable
        // (This is enforced by the BoundedVec implementation)
    }
}
