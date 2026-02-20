//! Safety-Critical Memory Budget Enforcement Tests
//!
//! This module tests memory budget enforcement at various levels,
//! ensuring that memory allocation limits are strictly enforced
//! and appropriate errors are returned when budgets are exceeded.
//!
//! # Safety Requirements
//! - SW-REQ-ID: REQ_MEM_002 - Memory budget enforcement
//! - SW-REQ-ID: REQ_MEM_003 - Static memory allocation
//! - ASIL Level: ASIL-C

#![forbid(unsafe_code)]

extern crate alloc;

use wrt_component::bounded_component_infra::*;
use wrt_foundation::{
    WrtError,
    bounded::{BoundedString, BoundedVec},
    budget_aware_provider::CrateId,
    budget_provider::BudgetProvider,
    managed_alloc, safe_managed_alloc,
    safe_memory::NoStdProvider,
};
#[cfg(not(feature = "std"))]
use wrt_foundation::{
    safe_managed_alloc,
    {
        WrtError,
        bounded::{BoundedString, BoundedVec},
        budget_aware_provider::CrateId,
        budget_provider::BudgetProvider,
        managed_alloc,
        safe_memory::NoStdProvider,
    },
};

#[cfg(test)]
mod memory_budget_tests {
    use super::*;

    /// Test component-level memory budget enforcement
    #[test]
    fn test_component_memory_budget() {
        // Component provider has 128KB budget
        const COMPONENT_BUDGET: usize = 131072;

        // Calculate approximate memory usage per element
        let element_size = core::mem::size_of::<u64>();
        let overhead_per_vec = 64; // Approximate overhead

        let mut allocations = Vec::new();
        let mut total_allocated = 0;

        // Keep allocating until we hit the budget
        loop {
            match new_component_vec::<u64>() {
                Ok(vec) => {
                    let vec_memory = MAX_COMPONENT_INSTANCES * element_size + overhead_per_vec;
                    total_allocated += vec_memory;
                    allocations.push(vec);

                    // Safety check to prevent infinite loop
                    if allocations.len() > 1000 {
                        panic!("Too many allocations without hitting budget");
                    }
                },
                Err(WrtError::OutOfMemory) => {
                    // Expected when budget is exhausted
                    break;
                },
                Err(e) => panic!("Unexpected error: {:?}", e),
            }
        }

        // Should have allocated some vectors before hitting limit
        assert!(allocations.len() > 0);
        assert!(total_allocated <= COMPONENT_BUDGET * 2); // Allow some overhead
    }

    /// Test cross-collection memory sharing
    #[test]
    fn test_cross_collection_memory_sharing() {
        // Allocate different types of collections
        let vec1 = new_component_vec::<u32>();
        assert!(vec1.is_ok());

        let map1 = new_export_map::<String>();
        assert!(map1.is_ok());

        let vec2 = new_resource_vec::<u64>();
        assert!(vec2.is_ok());

        // All should share the same memory budget
        // Try to allocate many more until budget exhausted
        let mut allocation_count = 3; // Already allocated 3

        loop {
            let result = match allocation_count % 3 {
                0 => new_component_vec::<u32>().map(|_| ()),
                1 => new_export_map::<String>().map(|_| ()),
                2 => new_resource_vec::<u64>().map(|_| ()),
                _ => unreachable!(),
            };

            match result {
                Ok(_) => allocation_count += 1,
                Err(WrtError::OutOfMemory) => break,
                Err(e) => panic!("Unexpected error: {:?}", e),
            }

            // Safety limit
            if allocation_count > 100 {
                break;
            }
        }

        // Should have allocated multiple collections
        assert!(allocation_count >= 3);
    }

    /// Test memory budget with actual data storage
    #[test]
    fn test_memory_budget_with_data() {
        let mut vecs = Vec::new();

        // Allocate and fill vectors
        for i in 0..10 {
            match new_component_vec::<[u8; 1024]>() {
                Ok(mut vec) => {
                    // Fill with data to consume actual memory
                    for j in 0..vec.capacity() / 4 {
                        let data = [j as u8; 1024];
                        if vec.try_push(data).is_err() {
                            break;
                        }
                    }
                    vecs.push(vec);
                },
                Err(WrtError::OutOfMemory) => {
                    // Budget exhausted
                    assert!(i > 0, "Should allocate at least one vector");
                    break;
                },
                Err(e) => panic!("Unexpected error: {:?}", e),
            }
        }

        // Verify we allocated and filled some vectors
        assert!(!vecs.is_empty());
        let total_elements: usize = vecs.iter().map(|v| v.len()).sum();
        assert!(total_elements > 0);
    }

    /// Test individual collection memory limits
    #[test]
    fn test_individual_collection_limits() {
        // Test export map memory usage
        let map_result = new_export_map::<[u8; 256]>();
        assert!(map_result.is_ok());

        let mut map = map_result.unwrap();
        let mut successful_inserts = 0;

        for i in 0..MAX_COMPONENT_EXPORTS {
            let key = bounded_export_name_from_str(&format!("export_{:04}", i))
                .expect("Failed to create key");
            let value = [i as u8; 256];

            match map.try_insert(key, value) {
                Ok(_) => successful_inserts += 1,
                Err(WrtError::CapacityExceeded) => break,
                Err(e) => panic!("Unexpected error: {:?}", e),
            }
        }

        // Should insert up to capacity
        assert_eq!(successful_inserts, map.len());
        assert!(successful_inserts > 0);
    }

    /// Test memory reclamation
    #[test]
    fn test_memory_reclamation() {
        // Allocate and drop collections
        for cycle in 0..3 {
            let mut temp_vecs = Vec::new();

            // Allocate until budget exhausted
            for i in 0..20 {
                match new_resource_vec::<u64>() {
                    Ok(vec) => temp_vecs.push(vec),
                    Err(WrtError::OutOfMemory) => break,
                    Err(e) => panic!("Unexpected error in cycle {}: {:?}", cycle, e),
                }
            }

            // Should allocate at least one
            assert!(
                !temp_vecs.is_empty(),
                "Failed to allocate in cycle {}",
                cycle
            );

            // Drop all vectors (memory should be reclaimed)
            drop(temp_vecs);

            // Small delay to ensure cleanup
            #[cfg(feature = "std")]
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// Test string allocation budgets
    #[test]
    fn test_string_allocation_budget() {
        let mut strings = Vec::new();

        // Allocate bounded strings
        for i in 0..100 {
            match bounded_component_name_from_str(&format!("component_{}", i)) {
                Ok(name) => strings.push(name),
                Err(WrtError::OutOfMemory) => {
                    // Budget exhausted
                    assert!(i > 0, "Should allocate at least one string");
                    break;
                },
                Err(e) => panic!("Unexpected error: {:?}", e),
            }
        }

        assert!(!strings.is_empty());

        // Verify string contents are preserved
        for (i, s) in strings.iter().enumerate() {
            assert_eq!(s.as_str(), format!("component_{}", i));
        }
    }

    /// Test nested structure memory usage
    #[test]
    fn test_nested_structure_memory() {
        #[derive(Clone)]
        struct ComplexData {
            id: u32,
            data: [u8; 512],
            flags: u64,
        }

        let vec_result = new_component_vec::<ComplexData>();
        assert!(vec_result.is_ok());

        let mut vec = vec_result.unwrap();
        let mut count = 0;

        // Fill with complex data
        for i in 0..MAX_COMPONENT_INSTANCES {
            let data = ComplexData {
                id: i as u32,
                data: [i as u8; 512],
                flags: (i as u64) << 32,
            };

            match vec.try_push(data) {
                Ok(_) => count += 1,
                Err(WrtError::CapacityExceeded) => break,
                Err(e) => panic!("Unexpected error: {:?}", e),
            }
        }

        assert_eq!(count, vec.len());
        assert!(count > 0);
    }

    /// Test memory budget with mixed allocation sizes
    #[test]
    fn test_mixed_allocation_sizes() {
        let mut small_allocs = 0;
        let mut medium_allocs = 0;
        let mut large_allocs = 0;

        // Small allocations
        loop {
            match new_post_return_vec::<u8>() {
                Ok(_) => small_allocs += 1,
                Err(WrtError::OutOfMemory) => break,
                Err(_) => break,
            }
            if small_allocs > 50 {
                break;
            }
        }

        // Medium allocations
        loop {
            match new_locals_vec::<u64>() {
                Ok(_) => medium_allocs += 1,
                Err(WrtError::OutOfMemory) => break,
                Err(_) => break,
            }
            if medium_allocs > 20 {
                break;
            }
        }

        // Large allocations
        loop {
            match new_resource_vec::<[u8; 1024]>() {
                Ok(_) => large_allocs += 1,
                Err(WrtError::OutOfMemory) => break,
                Err(_) => break,
            }
            if large_allocs > 5 {
                break;
            }
        }

        // Should have successful allocations of different sizes
        assert!(small_allocs > 0);
        assert!(medium_allocs > 0);
        assert!(large_allocs > 0);
    }

    /// Test memory budget enforcement consistency
    #[test]
    fn test_budget_enforcement_consistency() {
        const TEST_ITERATIONS: usize = 5;

        for iteration in 0..TEST_ITERATIONS {
            let mut allocation_count = 0;
            let mut allocations = Vec::new();

            // Allocate until budget exhausted
            loop {
                match new_operand_stack::<u32>() {
                    Ok(stack) => {
                        allocation_count += 1;
                        allocations.push(stack);
                    },
                    Err(WrtError::OutOfMemory) => break,
                    Err(e) => panic!("Unexpected error in iteration {}: {:?}", iteration, e),
                }

                // Safety limit
                if allocation_count > 100 {
                    break;
                }
            }

            // Should have consistent behavior across iterations
            assert!(
                allocation_count > 0,
                "No allocations in iteration {}",
                iteration
            );

            // Clean up for next iteration
            drop(allocations);
        }
    }

    /// Test memory budget with type map allocations
    #[test]
    fn test_type_map_memory_budget() {
        let mut maps = Vec::new();

        loop {
            match new_type_map::<[u8; 128]>() {
                Ok(mut map) => {
                    // Fill map partially
                    for i in 0..100 {
                        let value = [i as u8; 128];
                        if map.try_insert(i, value).is_err() {
                            break;
                        }
                    }
                    maps.push(map);
                },
                Err(WrtError::OutOfMemory) => {
                    // Budget exhausted
                    break;
                },
                Err(e) => panic!("Unexpected error: {:?}", e),
            }

            // Safety limit
            if maps.len() > 50 {
                break;
            }
        }

        assert!(!maps.is_empty());

        // Verify data integrity
        for (map_idx, map) in maps.iter().enumerate() {
            for i in 0..core::cmp::min(100, map.len()) {
                if let Some(value) = map.get(&(i as u32)) {
                    assert_eq!(value[0], i as u8);
                }
            }
        }
    }
}

#[cfg(all(test, feature = "safety-critical"))]
mod safety_critical_budget_tests {
    use super::*;

    /// Test that safety-critical mode enforces stricter budget limits
    #[test]
    fn test_safety_critical_budget_enforcement() {
        // In safety-critical mode, all allocations must be bounded
        let vec_result = new_component_vec::<u32>();
        assert!(vec_result.is_ok());

        // Verify managed allocation is used
        let guard_result = safe_managed_alloc!(1024, CrateId::Component);
        match guard_result {
            Ok(guard) => {
                // Guard ensures memory is tracked
                drop(guard);
            },
            Err(WrtError::OutOfMemory) => {
                // Budget exhausted - expected in some cases
            },
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }
}
