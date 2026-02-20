//! Runtime Verification Tests for Static Memory Enforcement
//!
//! This module provides comprehensive tests to verify that static memory
//! principles are properly enforced throughout the WRT runtime.

// Tests currently disabled - to be updated for modern capability-based system
#![cfg(all(test, feature = "disabled_during_migration"))]

use crate::{
    bounded::{
        BoundedString,
        BoundedVec,
    },
    bounded_infrastructure::{
        BoundedEventSystem,
        BoundedMemoryPool,
        BoundedSystemRegistry,
        EventType,
    },
    memory_enforcement::{
        pre_allocate_component_memory,
        BoundedMemoryBlock,
        ComponentMemoryBlocks,
    },
    memory_system::{
        ConfigurableProvider,
        MediumProvider,
        SmallProvider,
        UnifiedMemoryProvider,
    },
    no_std_hashmap::BoundedHashMap as BoundedMap,
    safe_memory::{
        NoStdProvider,
        Provider,
    },
    safety_system::SafetyLevel,
    Error,
    Result,
};

#[test]
fn test_bounded_vec_capacity_enforcement() {
    type TestProvider = NoStdProvider<1024>;
    let provider = TestProvider::default();

    // Create bounded vec with capacity 10
    let mut vec: BoundedVec<u32, 10, TestProvider> = BoundedVec::new(provider).unwrap();

    // Should succeed for 10 items
    for i in 0..10 {
        assert!(vec.push(i).is_ok());
    }

    // Should fail on 11th item
    assert!(vec.push(10).is_err());
    assert_eq!(vec.len(), 10);
}

#[test]
fn test_bounded_map_capacity_enforcement() {
    type TestProvider = NoStdProvider<2048>;
    let provider = TestProvider::default();

    // Create bounded map with capacity 5
    let mut map: BoundedMap<u32, u32, 5, TestProvider> = BoundedMap::new(provider).unwrap();

    // Should succeed for 5 items
    for i in 0..5 {
        assert!(map.insert(i, i * 2).is_ok());
    }

    // Should fail on 6th item
    assert!(map.insert(5, 10).is_err());
    assert_eq!(map.len(), 5);
}

#[test]
fn test_bounded_string_length_enforcement() {
    type TestProvider = NoStdProvider<256>;
    let provider = TestProvider::default();

    // Create bounded string with max length 10
    let short_str = "Hello";
    let long_str = "This is a very long string that exceeds the limit";

    let bounded_short = BoundedString::<10, TestProvider>::from_str(short_str, provider.clone());
    assert!(bounded_short.is_ok());

    let bounded_long = BoundedString::<10, TestProvider>::from_str(long_str, provider);
    assert!(bounded_long.is_err());
}

#[test]
fn test_memory_block_static_allocation() {
    type TestProvider = NoStdProvider<4096>;

    // Create memory block
    let block = BoundedMemoryBlock::<TestProvider>::new(1024, "Test block", 1).unwrap();

    // Verify static allocation
    assert_eq!(block.size(), 1024);
    assert_eq!(block.description(), "Test block");

    // Try to create oversized block
    let oversized = BoundedMemoryBlock::<TestProvider>::new(2 * 1024 * 1024, "Too big", 1);
    assert!(oversized.is_err());
}

#[test]
fn test_component_memory_pre_allocation() {
    type TestProvider = NoStdProvider<65536>;

    // Define allocations
    let allocations = &[
        (1024, "Component buffer"),
        (2048, "Component workspace"),
        (512, "Component state"),
    ];

    // Pre-allocate memory
    let blocks = pre_allocate_component_memory::<TestProvider>(
        1,
        allocations,
        SafetyLevel::default(),
        TestProvider::default(),
    )
    .unwrap();

    // Verify allocations
    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[0].size(), 1024);
    assert_eq!(blocks[1].size(), 2048);
    assert_eq!(blocks[2].size(), 512);
}

#[test]
fn test_configurable_provider_bounds() {
    let mut small_provider = SmallProvider::new();
    let mut medium_provider = MediumProvider::new();

    // Small provider should have 8KB
    assert_eq!(small_provider.total_memory(), 8192);

    // Should succeed for reasonable allocation
    let alloc1 = small_provider.allocate(1024);
    assert!(alloc1.is_ok());

    // Should fail for oversized allocation
    let alloc2 = small_provider.allocate(10000);
    assert!(alloc2.is_err());

    // Medium provider should have 64KB
    assert_eq!(medium_provider.total_memory(), 65536);

    // Should succeed for larger allocation
    let alloc3 = medium_provider.allocate(10000);
    assert!(alloc3.is_ok());
}

#[test]
fn test_system_registry_bounded_operations() {
    type TestProvider = NoStdProvider<8192>;

    let mut registry = BoundedSystemRegistry::<TestProvider>::new().unwrap();

    // Register components
    for i in 0..10 {
        let name = format!("component_{}", i);
        assert!(registry.register_component(&name, 1, &[]).is_ok());
    }

    // Verify bounded behavior
    let long_name = "a".repeat(200); // Exceeds MAX_COMPONENT_NAME_LEN
    assert!(registry.register_component(&long_name, 1, &[]).is_err());
}

#[test]
fn test_event_system_bounded_queue() {
    type TestProvider = NoStdProvider<16384>;

    let mut events = BoundedEventSystem::<TestProvider>::new().unwrap();

    // Register handler
    let _handler = events.register_handler(EventType::ComponentInitialized).unwrap();

    // Emit events up to capacity
    for i in 0..100 {
        let source = format!("source_{}", i);
        let payload = format!("event_{}", i);
        assert!(events
            .emit_event(EventType::ComponentInitialized, &source, payload.as_bytes())
            .is_ok());
    }

    // Process events
    let processed = events.process_events().unwrap();
    assert_eq!(processed, 100);
}

#[test]
fn test_memory_pool_static_blocks() {
    type TestProvider = NoStdProvider<1024>;

    let mut pool = BoundedMemoryPool::<128, 8, TestProvider>::new();

    // Allocate all blocks
    let mut blocks = Vec::new();
    for _ in 0..8 {
        match pool.allocate() {
            Ok(block) => blocks.push(block.as_ptr()),
            Err(_) => break,
        }
    }

    assert_eq!(blocks.len(), 8);
    assert_eq!(pool.free_count(), 0);

    // Should fail - pool exhausted
    assert!(pool.allocate().is_err());
}

#[test]
fn test_no_dynamic_allocation_in_critical_path() {
    type TestProvider = NoStdProvider<4096>;
    let provider = TestProvider::default();

    // Create all collections with static capacity
    let vec: BoundedVec<u32, 100, TestProvider> = BoundedVec::new(provider.clone()).unwrap();
    let map: BoundedMap<u32, u32, 50, TestProvider> = BoundedMap::new(provider.clone()).unwrap();
    let string: BoundedString<128, TestProvider> =
        BoundedString::try_from_str("test", provider).unwrap();

    // Verify static allocation
    assert_eq!(vec.capacity(), 100);
    assert_eq!(map.capacity(), 50);
    assert!(string.capacity() >= 128);
}

/// Performance comparison test between bounded and unbounded collections
#[cfg(feature = "std")]
#[test]
fn test_bounded_vs_unbounded_performance() {
    use std::time::Instant;

    type TestProvider = NoStdProvider<65536>;
    let provider = TestProvider::default();

    const ITERATIONS: usize = 1000;

    // Test BoundedVec performance
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let mut vec: BoundedVec<u32, 100, TestProvider> =
            BoundedVec::new(provider.clone()).unwrap();
        for i in 0..50 {
            drop(vec.push(i));
        }
    }
    let bounded_time = start.elapsed();

    // Test std::vec::Vec performance
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let mut vec = std::vec::Vec::with_capacity(100);
        for i in 0..50 {
            vec.push(i);
        }
    }
    let unbounded_time = start.elapsed();

    // Bounded collections should be competitive
    println!(
        "Bounded time: {:?}, Unbounded time: {:?}",
        bounded_time, unbounded_time
    );

    // Allow up to 2x overhead for safety
    assert!(bounded_time.as_nanos() < unbounded_time.as_nanos() * 2);
}

/// Integration test for complete static memory system
#[test]
fn test_integrated_static_memory_system() {
    type TestProvider = NoStdProvider<32768>;

    // Initialize memory budget (would normally be done at startup)
    // Note: This is commented out as it requires global state initialization
    // initialize_global_budget(1024 * 1024, SafetyLevel::AsilB).unwrap();

    // Create system components
    let mut registry = BoundedSystemRegistry::<TestProvider>::new().unwrap();
    let mut events = BoundedEventSystem::<TestProvider>::new().unwrap();
    let mut pool = BoundedMemoryPool::<256, 16, TestProvider>::new();

    // Register core components
    registry.register_component("memory", 1, &[]).unwrap();
    registry.register_component("events", 1, &["memory"]).unwrap();
    registry.register_component("runtime", 1, &["memory", "events"]).unwrap();

    // Initialize all components
    registry.initialize_all().unwrap();

    // Allocate memory from pool
    let block1 = pool.allocate().unwrap();
    let block2 = pool.allocate().unwrap();

    // Emit events
    events.emit_event(EventType::MemoryAllocated, "pool", b"256").unwrap();
    events.emit_event(EventType::ComponentInitialized, "runtime", b"ready").unwrap();

    // Verify system state
    assert_eq!(pool.free_count(), 14); // 16 - 2 allocated
    let processed = events.process_events().unwrap();
    assert_eq!(processed, 2);

    // Free memory
    pool.free(block1).unwrap();
    pool.free(block2).unwrap();
    assert_eq!(pool.free_count(), 16);
}

// Compile-time verification tests using const assertions
const _: () = {
    // Verify bounded collections have compile-time known sizes
    const VEC_SIZE: usize = core::mem::size_of::<BoundedVec<u32, 100, NoStdProvider<1024>>>();
    const MAP_SIZE: usize = core::mem::size_of::<BoundedMap<u32, u32, 50, NoStdProvider<1024>>>();

    // These should be compile-time constants
    assert!(VEC_SIZE > 0);
    assert!(MAP_SIZE > 0);
};

#[cfg(all(test, feature = "std"))]
mod format_tests {
    use super::*;

    fn format<T: core::fmt::Display>(value: T) -> String {
        format!("{}", value)
    }
}
