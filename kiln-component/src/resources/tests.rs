// WRT - wrt-component
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

#[cfg(feature = "std")]
use std::{
    sync::{
        Arc,
        Mutex,
    },
    time::Instant,
};

#[cfg(not(feature = "std"))]
use alloc::sync::Arc;

use super::{
    Resource,
    ResourceArena,
    ResourceId,
    ResourceManager,
    ResourceTable,
};
#[cfg(feature = "std")]
use super::size_class_buffer_pool::SizeClassBufferPool;
#[cfg(feature = "std")]
use super::buffer_pool::BufferPool;
use crate::prelude::*;

#[cfg(feature = "std")]
fn test_size_class_buffer_pool() {
    // Create the pool
    let mut pool = SizeClassBufferPool::new();

    // Binary std/no_std choice
    let sizes = [15, 64, 200, 1024, 4096, 16385];
    let mut buffers = Vec::new();

    // Allocate buffers
    for &size in &sizes {
        let buffer = pool.allocate(size);
        assert!(
            buffer.capacity() >= size,
            "Buffer capacity {} should be >= requested size {}",
            buffer.capacity(),
            size
        );
        buffers.push(buffer);
    }

    // Return buffers to pool
    for buffer in buffers {
        pool.return_buffer(buffer);
    }

    // Pool should now have buffers
    let stats = pool.stats();
    assert!(
        stats.total_buffers > 0,
        "Buffer pool should contain returned buffers"
    );

    // Reset the pool
    pool.reset();

    // Verify pool is empty
    let stats_after = pool.stats();
    assert_eq!(
        stats_after.total_buffers, 0,
        "Buffer pool should be empty after reset"
    );
}

    // Create a resource table
    let table = Arc::new(Mutex::new(ResourceTable::new()));

    // Create a resource arena
    let mut arena = ResourceArena::new(table.clone());

    // Create resources in the arena
    let handle1 = arena.create_resource(1, Arc::new(String::from("test1"))).unwrap();
    let handle2 = arena.create_resource(2, Arc::new(42i32)).unwrap();

    // Verify resources exist
    assert!(arena.has_resource(ResourceId(handle1)).unwrap());
    assert!(arena.has_resource(ResourceId(handle2)).unwrap());

    // Get resources and verify data
    let resource1 = arena.get_resource(handle1).unwrap();
    let string_data = resource1.lock().unwrap().data.downcast_ref::<String>().unwrap();
    assert_eq!(*string_data, "test1");

    // Drop a specific resource
    arena.drop_resource(handle1).unwrap();

    // Verify it's gone but the other remains
    assert!(!arena.has_resource(ResourceId(handle1)).unwrap());
    assert!(arena.has_resource(ResourceId(handle2)).unwrap());

    // Release all resources
    arena.release_all().unwrap();

    // Verify all resources are gone
    assert_eq!(arena.resource_count(), 0);
    let locked_table = table.lock().unwrap();
    assert_eq!(locked_table.resource_count(), 0);
}

    // Verify resources were cleaned up
    let locked_table = table.lock().unwrap();
    assert_eq!(locked_table.resource_count(), 0);
}

    // Create a resource manager
    let manager = ResourceManager::new();

    // Create two arenas sharing the same resource table
    let table = Arc::clone(&manager.get_resource_table());
    let mut arena1 = ResourceArena::new_with_name(table.clone(), "arena1");
    let mut arena2 = ResourceArena::new_with_name(table.clone(), "arena2");

    // Create resources in each arena
    let handle1 = arena1.create_resource(1, Arc::new(String::from("test1"))).unwrap();
    let handle2 = arena2.create_resource(2, Arc::new(String::from("test2"))).unwrap();

    // Verify each arena only knows about its own resources
    assert!(arena1.has_resource(ResourceId(handle1)).unwrap());
    assert!(!arena1.has_resource(ResourceId(handle2)).unwrap());

    assert!(!arena2.has_resource(ResourceId(handle1)).unwrap());
    assert!(arena2.has_resource(ResourceId(handle2)).unwrap());

    // But the manager knows about all resources
    assert!(manager.has_resource(ResourceId(handle1)).unwrap());
    assert!(manager.has_resource(ResourceId(handle2)).unwrap());

    // Release arena1's resources
    arena1.release_all().unwrap();

    // Verify arena1's resources are gone but arena2's remain
    assert!(!manager.has_resource(ResourceId(handle1)).unwrap());
    assert!(manager.has_resource(ResourceId(handle2)).unwrap());
}

#[cfg(feature = "std")]
fn test_performance_comparison() {
    // This is a simple benchmark to compare standard and optimized buffer pools
    const NUM_ALLOCATIONS: usize = 1000;
    const SIZES: [usize; 6] = [32, 64, 128, 512, 1024, 4096];

    // Test standard buffer pool
    let mut standard_pool = BufferPool::new();
    let start_standard = Instant::now();

    for _ in 0..NUM_ALLOCATIONS {
        for &size in &SIZES {
            let buffer = standard_pool.allocate(size);
            standard_pool.return_buffer(buffer);
        }
    }

    let standard_duration = start_standard.elapsed();

    // Test size class buffer pool
    let mut optimized_pool = SizeClassBufferPool::new();
    let start_optimized = Instant::now();

    for _ in 0..NUM_ALLOCATIONS {
        for &size in &SIZES {
            let buffer = optimized_pool.allocate(size);
            optimized_pool.return_buffer(buffer);
        }
    }

    let optimized_duration = start_optimized.elapsed();

    // We're not making assertions here because performance can vary by system,
    // but we can log the results in debug output
    println!("Standard pool: {:?}", standard_duration);
    println!("Optimized pool: {:?}", optimized_duration);
    println!(
        "Improvement factor: {:.2}x",
        standard_duration.as_secs_f64() / optimized_duration.as_secs_f64()
    );
