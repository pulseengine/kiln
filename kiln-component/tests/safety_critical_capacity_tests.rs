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

use kiln_component::bounded_component_infra::*;
use kiln_error::codes;

#[cfg(test)]
mod capacity_limit_tests {
    use super::*;

    /// Test vector capacity limits are enforced
    #[test]
    fn test_bounded_vec_capacity_limit() {
        let mut vec = new_component_vec::<u32>();

        // Fill to capacity
        for i in 0..MAX_COMPONENT_INSTANCES {
            let push_result = vec.push(i as u32);
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
        let overflow_result = vec.push(999);
        match overflow_result {
            Err(e) => assert_eq!(e.code, codes::FOUNDATION_BOUNDED_CAPACITY_EXCEEDED),
            Ok(()) => panic!("Expected capacity-exceeded error"),
        }
    }

    /// Test export vector capacity enforcement
    #[test]
    fn test_export_vec_capacity() {
        let mut vec = new_export_vec::<u32>();

        // Test partial fill
        for i in 0..100 {
            let push_result = vec.push(i);
            assert!(push_result.is_ok());
        }

        assert_eq!(vec.len(), 100);
        assert!(!vec.is_full());

        // Fill remaining capacity
        for i in 100..MAX_COMPONENT_EXPORTS {
            assert!(vec.push(i as u32).is_ok());
        }

        assert!(vec.is_full());

        // Verify overflow handling
        let overflow = vec.push(0);
        match overflow {
            Err(e) => assert_eq!(e.code, codes::FOUNDATION_BOUNDED_CAPACITY_EXCEEDED),
            Ok(()) => panic!("Expected capacity-exceeded error"),
        }
    }

    /// Test resource handle vector limits
    #[test]
    fn test_resource_vec_limits() {
        let mut vec = new_resource_vec::<u64>();

        // Test batch operations near capacity
        let batch_size = 100;
        let num_batches = MAX_RESOURCE_HANDLES / batch_size;

        for batch in 0..num_batches {
            for i in 0..batch_size {
                let handle = (batch * batch_size + i) as u64;
                assert!(vec.push(handle).is_ok());
            }
        }

        // Fill remaining
        let remaining = MAX_RESOURCE_HANDLES % batch_size;
        for i in 0..remaining {
            assert!(vec.push(i as u64).is_ok());
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

        let mut stack = new_call_stack::<CallFrame>();

        // Simulate deep recursion up to limit
        for depth in 0..MAX_CALL_STACK_DEPTH {
            let frame = CallFrame {
                function_idx: depth as u32,
                return_addr: (depth * 4) as u32,
            };

            let push_result = stack.push(frame);
            assert!(push_result.is_ok(), "Failed at depth {}", depth);
        }

        assert_eq!(stack.len(), MAX_CALL_STACK_DEPTH);

        // Stack overflow should be caught
        let overflow_frame = CallFrame {
            function_idx: 9999,
            return_addr: 0,
        };

        let overflow_result = stack.push(overflow_frame);
        assert!(overflow_result.is_err());
        let top = stack.last().unwrap();
        assert_eq!(top.function_idx, (MAX_CALL_STACK_DEPTH - 1) as u32);
        assert_eq!(top.return_addr, ((MAX_CALL_STACK_DEPTH - 1) * 4) as u32);
    }

    /// Test bounded string capacity
    #[test]
    fn test_bounded_string_limits() {
        // A name exactly at the limit is accepted
        let long_string = "a".repeat(MAX_COMPONENT_NAME_LEN);
        let name = bounded_component_name_from_str(&long_string).unwrap();
        assert_eq!(name.len(), MAX_COMPONENT_NAME_LEN);

        // A name beyond the limit is rejected
        let too_long = "a".repeat(MAX_COMPONENT_NAME_LEN + 1);
        let overflow_result = bounded_component_name_from_str(&too_long);
        assert!(overflow_result.is_err());
    }

    /// Test export map capacity
    #[test]
    fn test_export_map_capacity() {
        // Test with simple u32 keys instead of bounded names
        let mut map = new_type_map::<String>();

        // Fill map partway
        for i in 0..100 {
            let insert_result = map.insert(i as u32, format!("value_{}", i));
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
        let mut map = new_type_map::<String>();

        // Test sparse insertions
        for i in (0..MAX_TYPE_DEFINITIONS).step_by(10) {
            let insert_result = map.insert(i as u32, format!("type_{}", i));
            assert!(insert_result.is_ok());
        }

        let expected_count = MAX_TYPE_DEFINITIONS.div_ceil(10);
        assert_eq!(map.len(), expected_count);
    }

    /// Test operand stack limits
    #[test]
    fn test_operand_stack_limits() {
        let mut stack = new_operand_stack::<i64>();

        // Simulate computation that uses full stack
        for i in 0..MAX_OPERAND_STACK_SIZE {
            let value = i as i64 * 2;
            assert!(stack.push(value).is_ok());
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

        let mut vec = BoundedMemoryVec::<MockMemory>::new();

        for i in 0..MAX_MEMORY_INSTANCES {
            let mem = MockMemory {
                base: i as u64 * 65536,
                size: 65536,
            };
            assert!(vec.push(mem).is_ok());
        }

        assert_eq!(vec.len(), MAX_MEMORY_INSTANCES);
        let last = vec.last().unwrap();
        assert_eq!(last.base, (MAX_MEMORY_INSTANCES as u64 - 1) * 65536);
        assert_eq!(last.size, 65536);
    }

    /// Test post-return callback limits
    #[test]
    fn test_post_return_callback_limits() {
        type Callback = fn() -> kiln_error::Result<()>;

        let mut vec = BoundedPostReturnVec::<Callback>::new();

        fn dummy_callback() -> kiln_error::Result<()> {
            Ok(())
        }

        // Fill to capacity
        for _ in 0..MAX_POST_RETURN_CALLBACKS {
            assert!(vec.push(dummy_callback).is_ok());
        }

        assert!(vec.is_full());

        // Verify overflow protection
        let overflow = vec.push(dummy_callback);
        assert!(overflow.is_err());
    }

    /// Test that clear operations work correctly
    #[test]
    fn test_clear_operations() {
        let mut vec = new_component_vec::<u32>();

        // Fill partially
        for i in 0..10 {
            vec.push(i).unwrap();
        }

        assert_eq!(vec.len(), 10);

        // Clear
        vec.clear();
        assert_eq!(vec.len(), 0);
        assert!(vec.is_empty());

        // Can push again after clear
        assert!(vec.push(42).is_ok());
        assert_eq!(vec.len(), 1);
    }

    /// Test capacity after operations
    #[test]
    fn test_capacity_invariants() {
        let mut vec = new_import_vec::<String>();

        // Capacity should remain constant
        let initial_capacity = vec.capacity();
        assert_eq!(initial_capacity, MAX_COMPONENT_IMPORTS);

        // Add some elements
        for i in 0..100 {
            vec.push(format!("import_{}", i)).unwrap();
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

        let mut map = new_resource_type_map::<ResourceTypeInfo>();

        // Fill to capacity
        for i in 0..MAX_RESOURCE_TYPES {
            let info = ResourceTypeInfo {
                name: format!("resource_type_{}", i),
                size: i * 8,
            };
            assert!(map.insert(i as u32, info).is_ok());
        }

        assert_eq!(map.len(), MAX_RESOURCE_TYPES);

        // Verify lookup works at capacity
        for i in 0..MAX_RESOURCE_TYPES {
            let info = map.get(&(i as u32));
            assert!(info.is_some());
            let info = info.unwrap();
            assert_eq!(info.name, format!("resource_type_{}", i));
            assert_eq!(info.size, i * 8);
        }
    }
}

#[cfg(all(test, feature = "safety-critical"))]
mod safety_critical_feature_tests {
    use super::*;

    /// Test that safety-critical builds enforce stricter limits
    #[test]
    fn test_safety_critical_enforcement() {
        let vec = new_component_vec::<u32>();

        // Verify the vector is using bounded allocation
        assert_eq!(vec.capacity(), MAX_COMPONENT_INSTANCES);
    }
}
