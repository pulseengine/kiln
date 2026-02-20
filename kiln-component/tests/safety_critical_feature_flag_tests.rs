//! Safety-Critical Feature Flag Tests
//!
//! This module tests the behavior differences between safety-critical
//! and non-safety-critical feature flags, ensuring proper compilation
//! and runtime behavior in both configurations.
//!
//! # Safety Requirements
//! - SW-REQ-ID: REQ_FEAT_001 - Feature flag validation
//! - SW-REQ-ID: REQ_BUILD_001 - Build configuration testing
//! - ASIL Level: ASIL-C

#![forbid(unsafe_code)]

extern crate alloc;

use wrt_component::bounded_component_infra::*;
use wrt_foundation::{
    WrtError, budget_aware_provider::CrateId, collections::StaticVec as BoundedVec, managed_alloc,
    safe_managed_alloc,
};

#[cfg(test)]
mod feature_flag_tests {
    use super::*;

    /// Test that collections work in both std and no_std
    #[test]
    fn test_std_no_std_compatibility() {
        // These should work regardless of std feature
        let vec = new_component_vec::<u32>();
        assert!(vec.is_ok());

        let map = new_export_map::<String>();
        assert!(map.is_ok());

        let name = new_component_name();
        assert!(name.is_ok());
    }

    #[cfg(feature = "safety-critical")]
    #[test]
    fn test_safety_critical_enabled() {
        // When safety-critical is enabled, all allocations must be bounded

        // Verify we're using WRT allocator
        let guard_result = safe_managed_alloc!(1024, CrateId::Component);
        assert!(guard_result.is_ok() || matches!(guard_result, Err(WrtError::OutOfMemory)));

        // Verify collections have fixed capacity
        let vec = new_component_vec::<u32>().unwrap();
        assert_eq!(vec.capacity(), MAX_COMPONENT_INSTANCES);

        // Capacity should not change
        let initial_capacity = vec.capacity();
        drop(vec);

        let vec2 = new_component_vec::<u32>().unwrap();
        assert_eq!(vec2.capacity(), initial_capacity);
    }

    #[cfg(not(feature = "safety-critical"))]
    #[test]
    fn test_safety_critical_disabled() {
        // When safety-critical is disabled, we still use bounded collections
        // but may have more relaxed constraints

        let vec = new_component_vec::<u32>().unwrap();

        // Still bounded, but might allow different behavior
        assert!(vec.capacity() > 0);
        assert!(vec.capacity() <= MAX_COMPONENT_INSTANCES);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_std_features() {
        // Test features that require std
        use std::{sync::Arc, thread};

        let vec = Arc::new(new_component_vec::<u32>().unwrap());
        let vec_clone = Arc::clone(&vec);

        // Can use std threading
        let handle = thread::spawn(move || vec_clone.len());

        let len = handle.join().unwrap();
        assert_eq!(len, 0);
    }

    #[cfg(not(feature = "std"))]
    #[test]
    fn test_no_std_features() {
        // Test no_std specific behavior
        use wrt_sync::Mutex;

        let vec = new_component_vec::<u32>().unwrap();
        let mutex = Mutex::new(vec);

        // Can use no_std mutex
        {
            let guard = mutex.lock();
            assert_eq!(guard.len(), 0);
        }
    }

    #[cfg(all(feature = "std", feature = "safety-critical"))]
    #[test]
    fn test_std_with_safety_critical() {
        // Most restrictive configuration
        // Should use WRT allocator even with std available

        let vec_result = new_component_vec::<u64>();
        assert!(vec_result.is_ok());

        let vec = vec_result.unwrap();

        // Should have bounded capacity
        assert_eq!(vec.capacity(), MAX_COMPONENT_INSTANCES);

        // Should handle errors gracefully
        let mut filled_vec = new_component_vec::<u32>().unwrap();
        for i in 0..MAX_COMPONENT_INSTANCES {
            assert!(filled_vec.try_push(i as u32).is_ok());
        }

        // No panic on overflow
        assert!(matches!(
            filled_vec.try_push(999),
            Err(WrtError::CapacityExceeded)
        ));
    }

    #[cfg(all(not(feature = "std"), not(feature = "safety-critical")))]
    #[test]
    fn test_no_std_without_safety_critical() {
        // no_std but not safety-critical
        // Still uses bounded collections but might have different trade-offs

        let vec = new_component_vec::<u32>().unwrap();
        assert!(vec.capacity() > 0);

        // Basic operations should still work
        let mut test_vec = new_export_vec::<i32>().unwrap();
        assert!(test_vec.try_push(42).is_ok());
        assert_eq!(test_vec.pop(), Some(42));
    }

    /// Test compilation with different type parameters
    #[test]
    fn test_generic_type_support() {
        // Test with various types to ensure generic support

        // Primitive types
        let _u8_vec = new_component_vec::<u8>().unwrap();
        let _u16_vec = new_component_vec::<u16>().unwrap();
        let _u32_vec = new_component_vec::<u32>().unwrap();
        let _u64_vec = new_component_vec::<u64>().unwrap();

        // Composite types
        #[derive(Clone)]
        struct TestStruct {
            a: u32,
            b: u64,
        }

        let _struct_vec = new_component_vec::<TestStruct>().unwrap();

        // Option types
        let _option_vec = new_component_vec::<Option<u32>>().unwrap();

        // Result types
        let _result_vec = new_component_vec::<Result<u32, ()>>().unwrap();
    }

    /// Test that safety-critical APIs are consistent
    #[test]
    fn test_api_consistency() {
        // All factory functions should return wrt_error::Result
        fn assert_returns_result<T>(_: wrt_error::Result<T>) {}

        assert_returns_result(new_component_vec::<u32>());
        assert_returns_result(new_export_vec::<u32>());
        assert_returns_result(new_import_vec::<u32>());
        assert_returns_result(new_resource_vec::<u32>());
        assert_returns_result(new_call_stack::<u32>());
        assert_returns_result(new_operand_stack::<u32>());
        assert_returns_result(new_locals_vec::<u32>());
        assert_returns_result(new_component_name());
        assert_returns_result(new_export_name());
        assert_returns_result(new_export_map::<u32>());
        assert_returns_result(new_import_map::<u32>());
        assert_returns_result(new_type_map::<u32>());
        assert_returns_result(new_resource_type_map::<u32>());
    }

    /// Test memory provider abstraction
    #[test]
    fn test_memory_provider_abstraction() {
        // ComponentProvider should work consistently
        type TestVec = BoundedVec<u32, 100>;

        // Direct provider usage with modern safe pattern
        let provider = ComponentProvider::default();
        let vec_result = TestVec::new(provider);
        assert!(vec_result.is_ok());

        let mut vec = vec_result.unwrap();
        assert!(vec.try_push(42).is_ok());
        assert_eq!(vec.len(), 1);
    }

    /// Test that limits are enforced at compile time where possible
    #[test]
    fn test_compile_time_limits() {
        // These constants should be available at compile time
        const _MAX_COMP: usize = MAX_COMPONENT_INSTANCES;
        const _MAX_EXP: usize = MAX_COMPONENT_EXPORTS;
        const _MAX_IMP: usize = MAX_COMPONENT_IMPORTS;
        const _MAX_RES: usize = MAX_RESOURCE_HANDLES;

        // Can use in const contexts
        const ARRAY: [u8; MAX_COMPONENT_INSTANCES] = [0; MAX_COMPONENT_INSTANCES];
        assert_eq!(ARRAY.len(), MAX_COMPONENT_INSTANCES);
    }
}

#[cfg(all(test, feature = "safety-critical"))]
mod safety_critical_only_tests {
    use super::*;

    /// Test safety-critical specific behaviors
    #[test]
    fn test_deterministic_allocation() {
        // In safety-critical mode, allocations should be deterministic

        let mut vecs = Vec::new();

        // Allocate multiple vectors
        for _ in 0..5 {
            if let Ok(vec) = new_component_vec::<u32>() {
                vecs.push(vec);
            }
        }

        // All should have the same capacity
        if vecs.len() > 1 {
            let first_capacity = vecs[0].capacity();
            for vec in &vecs {
                assert_eq!(vec.capacity(), first_capacity);
            }
        }
    }

    /// Test that no dynamic allocation occurs
    #[test]
    fn test_no_dynamic_allocation() {
        // Create a vector
        let mut vec = new_component_vec::<u64>().unwrap();

        let initial_capacity = vec.capacity();

        // Fill it
        for i in 0..initial_capacity {
            if vec.try_push(i as u64).is_err() {
                break;
            }
        }

        // Capacity should not have changed
        assert_eq!(vec.capacity(), initial_capacity);

        // Clear and refill
        vec.clear();
        assert_eq!(vec.capacity(), initial_capacity);

        for i in 0..initial_capacity {
            if vec.try_push(i as u64).is_err() {
                break;
            }
        }

        // Still same capacity
        assert_eq!(vec.capacity(), initial_capacity);
    }
}

#[cfg(test)]
mod cross_feature_tests {
    use super::*;

    /// Test that code compiles and works with any feature combination
    #[test]
    fn test_universal_compatibility() {
        // This test should pass regardless of features

        // Basic allocation
        let vec = new_component_vec::<u32>();
        assert!(vec.is_ok());

        // Basic operations
        let mut v = vec.unwrap();
        assert_eq!(v.len(), 0);
        assert!(!v.is_full());
        assert!(v.is_empty());

        // Push and pop
        if v.try_push(100).is_ok() {
            assert_eq!(v.len(), 1);
            assert_eq!(v.pop(), Some(100));
        }

        // Error handling
        for i in 0..v.capacity() + 10 {
            let _ = v.try_push(i as u32);
        }

        // Should be full or close to it
        assert!(v.len() <= v.capacity());
    }
}
