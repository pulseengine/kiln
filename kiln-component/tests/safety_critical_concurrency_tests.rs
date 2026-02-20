//! Safety-Critical Concurrency Tests
//!
//! This module tests concurrent access patterns for thread safety,
//! ensuring that all shared resources are properly synchronized and
//! no data races or panics occur under concurrent load.
//!
//! # Safety Requirements
//! - SW-REQ-ID: REQ_THREAD_001 - Thread-safe resource access
//! - SW-REQ-ID: REQ_SYNC_001 - Synchronization primitives
//! - ASIL Level: ASIL-C

#![forbid(unsafe_code)]

extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::sync::Arc;
#[cfg(feature = "std")]
use std::sync::{Arc, Barrier, Mutex, RwLock};
#[cfg(feature = "std")]
use std::thread;

use wrt_component::{
    bounded_component_infra::*,
    resource_management::ResourceTable,
    resources::{
        ResourceStrategy,
        resource_lifecycle::{Resource, ResourceHandle, ResourceState, ResourceType},
    },
};
use wrt_foundation::{
    WrtError, budget_aware_provider::CrateId, collections::StaticVec as BoundedVec, managed_alloc,
};
#[cfg(not(feature = "std"))]
use wrt_platform::threading::{JoinHandle, spawn_bounded};
#[cfg(not(feature = "std"))]
use wrt_sync::{Mutex, RwLock};

#[cfg(test)]
mod concurrency_tests {
    use super::*;

    /// Test concurrent access to resource table
    #[cfg(feature = "std")]
    #[test]
    fn test_resource_table_concurrent_access() {
        let table = Arc::new(Mutex::new(ResourceTable::new()));
        let num_threads = 10;
        let resources_per_thread = 100;
        let barrier = Arc::new(Barrier::new(num_threads));

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let table_clone = Arc::clone(&table);
            let barrier_clone = Arc::clone(&barrier);

            let handle = thread::spawn(move || {
                // Wait for all threads to be ready
                barrier_clone.wait();

                let mut local_handles = Vec::new();

                // Allocate resources
                for i in 0..resources_per_thread {
                    let mut table = table_clone.lock().unwrap();
                    match table.allocate() {
                        Ok(handle) => local_handles.push(handle),
                        Err(e) => {
                            // Should not panic, just log error
                            eprintln!(
                                "Thread {} failed to allocate resource {}: {:?}",
                                thread_id, i, e
                            );
                        },
                    }
                }

                // Deallocate half of the resources
                for (i, handle) in local_handles.iter().take(resources_per_thread / 2).enumerate() {
                    let mut table = table_clone.lock().unwrap();
                    match table.deallocate(*handle) {
                        Ok(_) => {},
                        Err(e) => {
                            eprintln!(
                                "Thread {} failed to deallocate resource {}: {:?}",
                                thread_id, i, e
                            );
                        },
                    }
                }

                local_handles.len()
            });

            handles.push(handle);
        }

        // Wait for all threads to complete
        let total_allocated: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();

        // Verify final state
        let table = table.lock().unwrap();

        // Each thread allocated resources_per_thread and deallocated half
        let expected_active = num_threads * resources_per_thread / 2;

        // Allow some variance due to allocation failures near capacity
        assert!(table.len() <= expected_active + num_threads);
    }

    /// Test concurrent bounded vector operations
    #[cfg(feature = "std")]
    #[test]
    fn test_bounded_vec_concurrent_push_pop() {
        let vec_result = new_component_vec::<u32>();
        assert!(vec_result.is_ok());

        let vec = Arc::new(Mutex::new(vec_result.unwrap()));
        let num_threads = 5;
        let ops_per_thread = 20;
        let barrier = Arc::new(Barrier::new(num_threads * 2)); // Push + pop threads

        let mut handles = vec![];

        // Spawn push threads
        for thread_id in 0..num_threads {
            let vec_clone = Arc::clone(&vec);
            let barrier_clone = Arc::clone(&barrier);

            let handle = thread::spawn(move || {
                barrier_clone.wait();

                let mut successful_pushes = 0;
                for i in 0..ops_per_thread {
                    let value = thread_id * 1000 + i;
                    let mut vec = vec_clone.lock().unwrap();

                    match vec.try_push(value as u32) {
                        Ok(_) => successful_pushes += 1,
                        Err(WrtError::CapacityExceeded) => {
                            // Expected when near capacity
                        },
                        Err(e) => panic!("Unexpected error: {:?}", e),
                    }
                }
                successful_pushes
            });

            handles.push(handle);
        }

        // Spawn pop threads
        for thread_id in 0..num_threads {
            let vec_clone = Arc::clone(&vec);
            let barrier_clone = Arc::clone(&barrier);

            let handle = thread::spawn(move || {
                barrier_clone.wait();

                let mut successful_pops = 0;
                for _ in 0..ops_per_thread {
                    let mut vec = vec_clone.lock().unwrap();

                    if vec.pop().is_some() {
                        successful_pops += 1;
                    }
                }
                successful_pops
            });

            handles.push(handle);
        }

        // Collect results
        let results: Vec<usize> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        let total_pushes: usize = results[..num_threads].iter().sum();
        let total_pops: usize = results[num_threads..].iter().sum();

        // Verify consistency
        let vec = vec.lock().unwrap();
        let final_size = vec.len();

        assert_eq!(final_size + total_pops, total_pushes);
    }

    /// Test concurrent map operations
    #[cfg(feature = "std")]
    #[test]
    fn test_export_map_concurrent_operations() {
        let map_result = new_type_map::<u32>();
        assert!(map_result.is_ok());

        let map = Arc::new(RwLock::new(map_result.unwrap()));
        let num_threads = 8;
        let barrier = Arc::new(Barrier::new(num_threads));

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let map_clone = Arc::clone(&map);
            let barrier_clone = Arc::clone(&barrier);

            let handle = thread::spawn(move || {
                barrier_clone.wait();

                let mut successful_ops = 0;

                // Each thread operates on its own key range
                for i in 0..50 {
                    let key_num = (thread_id * 100 + i) as u32;

                    // Write operation
                    {
                        let mut map = map_clone.write().unwrap();
                        match map.try_insert(key_num, key_num) {
                            Ok(_) => successful_ops += 1,
                            Err(WrtError::CapacityExceeded) => {
                                // Expected near capacity
                            },
                            Err(e) => panic!("Unexpected error: {:?}", e),
                        }
                    }

                    // Read operation
                    {
                        let map = map_clone.read().unwrap();
                        if map.get(&key_num).is_some() {
                            successful_ops += 1;
                        }
                    }
                }

                successful_ops
            });

            handles.push(handle);
        }

        let total_ops: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();

        // Verify final state
        let map = map.read().unwrap();
        assert!(map.len() <= MAX_TYPE_DEFINITIONS);
        assert!(total_ops > 0);
    }

    /// Test resource lifecycle under concurrent access
    #[cfg(feature = "std")]
    #[test]
    fn test_resource_lifecycle_concurrent() {
        use wrt_component::resources::resource_lifecycle::{
            ResourceLifecycleManager, ResourceMetadata,
        };

        // Create mock resource type
        let resource_type = ResourceType {
            type_idx: 1,
            name: bounded_component_name_from_str("TestResource").unwrap(),
            destructor: Some(100),
        };

        let manager = Arc::new(Mutex::new(ResourceLifecycleManager::new()));
        let num_threads = 6;
        let barrier = Arc::new(Barrier::new(num_threads));

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let manager_clone = Arc::clone(&manager);
            let barrier_clone = Arc::clone(&barrier);
            let resource_type_clone = resource_type.clone();

            let handle = thread::spawn(move || {
                barrier_clone.wait();

                let mut created = 0;
                let mut borrowed = 0;
                let mut released = 0;

                // Create resources
                for i in 0..10 {
                    let metadata = ResourceMetadata {
                        created_at: Some(i as u64),
                        last_accessed: None,
                        creator: thread_id as u32,
                        owner: thread_id as u32,
                        user_data: None,
                    };

                    let mut manager = manager_clone.lock().unwrap();
                    match manager.create_resource(resource_type_clone.clone(), metadata) {
                        Ok(handle) => {
                            created += 1;

                            // Try to borrow
                            if manager.borrow_resource(handle).is_ok() {
                                borrowed += 1;

                                // Release borrow
                                if manager.release_borrow(handle).is_ok() {
                                    released += 1;
                                }
                            }
                        },
                        Err(_) => {
                            // Capacity reached
                        },
                    }
                }

                (created, borrowed, released)
            });

            handles.push(handle);
        }

        let results: Vec<(usize, usize, usize)> =
            handles.into_iter().map(|h| h.join().unwrap()).collect();

        let total_created: usize = results.iter().map(|r| r.0).sum();
        let total_borrowed: usize = results.iter().map(|r| r.1).sum();
        let total_released: usize = results.iter().map(|r| r.2).sum();

        // Verify consistency
        assert_eq!(total_borrowed, total_released);
        assert!(total_created > 0);
    }

    /// Test concurrent access with memory pressure
    #[cfg(feature = "std")]
    #[test]
    fn test_concurrent_memory_pressure() {
        let num_threads = 4;
        let barrier = Arc::new(Barrier::new(num_threads));
        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let barrier_clone = Arc::clone(&barrier);

            let handle = thread::spawn(move || {
                barrier_clone.wait();

                let mut allocations = 0;
                let mut failures = 0;

                // Try to allocate multiple collections
                for i in 0..10 {
                    // Try different allocation types
                    match i % 4 {
                        0 => match new_component_vec::<u64>() {
                            Ok(_) => allocations += 1,
                            Err(_) => failures += 1,
                        },
                        1 => match new_export_map::<String>() {
                            Ok(_) => allocations += 1,
                            Err(_) => failures += 1,
                        },
                        2 => match new_resource_vec::<u32>() {
                            Ok(_) => allocations += 1,
                            Err(_) => failures += 1,
                        },
                        3 => match new_call_stack::<u32>() {
                            Ok(_) => allocations += 1,
                            Err(_) => failures += 1,
                        },
                        _ => unreachable!(),
                    }
                }

                (allocations, failures)
            });

            handles.push(handle);
        }

        let results: Vec<(usize, usize)> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        let total_allocations: usize = results.iter().map(|r| r.0).sum();
        let total_failures: usize = results.iter().map(|r| r.1).sum();

        // Some allocations should succeed
        assert!(total_allocations > 0);

        // Under memory pressure, some failures are expected
        println!(
            "Allocations: {}, Failures: {}",
            total_allocations, total_failures
        );
    }

    /// Test deadlock prevention with multiple locks
    #[cfg(feature = "std")]
    #[test]
    fn test_deadlock_prevention() {
        let resource1 = Arc::new(Mutex::new(new_component_vec::<u32>().unwrap()));
        let resource2 = Arc::new(Mutex::new(new_export_map::<u32>().unwrap()));

        let num_threads = 2;
        let barrier = Arc::new(Barrier::new(num_threads));
        let mut handles = vec![];

        // Thread 1: Lock order resource1 -> resource2
        {
            let r1 = Arc::clone(&resource1);
            let r2 = Arc::clone(&resource2);
            let barrier_clone = Arc::clone(&barrier);

            let handle = thread::spawn(move || {
                barrier_clone.wait();

                for i in 0..100 {
                    let _lock1 = r1.lock().unwrap();
                    thread::yield_now(); // Increase chance of interleaving
                    let _lock2 = r2.lock().unwrap();

                    // Simulate work
                    thread::sleep(std::time::Duration::from_micros(10));
                }
            });

            handles.push(handle);
        }

        // Thread 2: Same lock order to prevent deadlock
        {
            let r1 = Arc::clone(&resource1);
            let r2 = Arc::clone(&resource2);
            let barrier_clone = Arc::clone(&barrier);

            let handle = thread::spawn(move || {
                barrier_clone.wait();

                for i in 0..100 {
                    let _lock1 = r1.lock().unwrap();
                    thread::yield_now();
                    let _lock2 = r2.lock().unwrap();

                    // Simulate work
                    thread::sleep(std::time::Duration::from_micros(10));
                }
            });

            handles.push(handle);
        }

        // Should complete without deadlock
        for handle in handles {
            handle.join().unwrap();
        }
    }
}

#[cfg(all(test, not(feature = "std")))]
mod no_std_concurrency_tests {
    use super::*;

    /// Test basic synchronization in no_std environment
    #[test]
    fn test_no_std_mutex_operations() {
        let vec_result = new_component_vec::<u32>();
        assert!(vec_result.is_ok());

        let vec = Arc::new(Mutex::new(vec_result.unwrap()));

        // In no_std, we can't spawn threads, but we can test mutex behavior
        {
            let mut vec = vec.lock().unwrap();
            assert!(vec.try_push(42).is_ok());
        }

        {
            let vec = vec.lock().unwrap();
            assert_eq!(vec.len(), 1);
        }
    }
}
