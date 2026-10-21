//! Safety-Critical Concurrency Tests
//!
//! This module tests thread-safety of the bounded collections and the
//! resource management types under concurrent access.
//!
//! # Safety Requirements
//! - SW-REQ-ID: REQ_CONC_001 - Thread-safe collections
//! - SW-REQ-ID: REQ_CONC_002 - Deadlock-free operation
//! - ASIL Level: ASIL-C

#![forbid(unsafe_code)]

use std::{
    sync::{Arc, Barrier, Mutex, RwLock},
    thread,
};

use kiln_component::{
    bounded_component_infra::*,
    resource_management::ResourceTable,
    resources::resource_lifecycle::ResourceLifecycleManager,
};
use kiln_error::codes;

#[cfg(test)]
mod concurrency_tests {
    use super::*;

    /// Test concurrent access to resource table
    #[test]
    fn test_resource_table_concurrent_access() {
        let table = Arc::new(Mutex::new(ResourceTable::new()));
        let num_threads = 10;
        let resources_per_thread = 100;
        let barrier = Arc::new(Barrier::new(num_threads));

        let mut handles = vec![];

        for _ in 0..num_threads {
            let table_clone = Arc::clone(&table);
            let barrier_clone = Arc::clone(&barrier);

            let handle = thread::spawn(move || {
                // Wait for all threads to be ready
                barrier_clone.wait();

                let mut local_handles = Vec::new();

                // Allocate resources
                for _ in 0..resources_per_thread {
                    let mut table = table_clone.lock().unwrap();
                    let handle = table.allocate().expect("allocation within capacity must succeed");
                    local_handles.push(handle);
                }

                // Deallocate half of the resources
                for handle in local_handles.iter().take(resources_per_thread / 2) {
                    let mut table = table_clone.lock().unwrap();
                    table.deallocate(*handle).expect("deallocating a live handle must succeed");
                }

                local_handles.len()
            });

            handles.push(handle);
        }

        // Wait for all threads to complete
        let total_allocated: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
        assert_eq!(total_allocated, num_threads * resources_per_thread);

        // Each thread allocated resources_per_thread and deallocated half
        let table = table.lock().unwrap();
        assert_eq!(table.len(), num_threads * resources_per_thread / 2);
    }

    /// Test concurrent bounded vector operations
    #[test]
    fn test_bounded_vec_concurrent_push_pop() {
        let vec = Arc::new(Mutex::new(new_component_vec::<u32>()));
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

                    match vec.push(value as u32) {
                        Ok(()) => successful_pushes += 1,
                        Err(e) => {
                            // Only capacity exhaustion is an acceptable failure
                            assert_eq!(e.code, codes::FOUNDATION_BOUNDED_CAPACITY_EXCEEDED);
                        },
                    }
                }
                successful_pushes
            });

            handles.push(handle);
        }

        // Spawn pop threads
        for _ in 0..num_threads {
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
        assert_eq!(vec.len() + total_pops, total_pushes);
    }

    /// Test concurrent map operations
    #[test]
    fn test_export_map_concurrent_operations() {
        let map = Arc::new(RwLock::new(new_type_map::<u32>()));
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
                        match map.insert(key_num, key_num) {
                            Ok(_) => successful_ops += 1,
                            Err(e) => {
                                // Only capacity exhaustion is an acceptable failure
                                assert_eq!(e.code, codes::FOUNDATION_BOUNDED_CAPACITY_EXCEEDED);
                            },
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
    #[test]
    fn test_resource_lifecycle_concurrent() {
        let mut manager = ResourceLifecycleManager::new();
        manager
            .register_type(1, "TestResource", Some(100))
            .expect("Failed to register resource type");

        let manager = Arc::new(Mutex::new(manager));
        let num_threads = 6;
        let barrier = Arc::new(Barrier::new(num_threads));

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let manager_clone = Arc::clone(&manager);
            let barrier_clone = Arc::clone(&barrier);

            let handle = thread::spawn(move || {
                barrier_clone.wait();

                let mut created = 0;
                let mut borrowed = 0;
                let mut released = 0;

                for _ in 0..10 {
                    let mut manager = manager_clone.lock().unwrap();
                    if let Ok(handle) = manager.create_resource(1, thread_id as u32, None) {
                        created += 1;

                        // Try to borrow
                        if manager.borrow_resource(handle, thread_id as u32, false).is_ok() {
                            borrowed += 1;

                            // Release borrow
                            if manager.release_borrow(handle, thread_id as u32).is_ok() {
                                released += 1;
                            }
                        }
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

    /// Test deadlock prevention with multiple locks
    #[test]
    fn test_deadlock_prevention() {
        let resource1 = Arc::new(Mutex::new(new_component_vec::<u32>()));
        let resource2 = Arc::new(Mutex::new(new_export_map::<u32>()));

        let num_threads = 2;
        let barrier = Arc::new(Barrier::new(num_threads));
        let mut handles = vec![];

        // Both threads take the locks in the same order, which prevents deadlock
        for _ in 0..num_threads {
            let r1 = Arc::clone(&resource1);
            let r2 = Arc::clone(&resource2);
            let barrier_clone = Arc::clone(&barrier);

            let handle = thread::spawn(move || {
                barrier_clone.wait();

                for _ in 0..100 {
                    let _lock1 = r1.lock().unwrap();
                    thread::yield_now(); // Increase chance of interleaving
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
