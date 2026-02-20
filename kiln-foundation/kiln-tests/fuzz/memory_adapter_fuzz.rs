/// Property-based tests for memory adapter implementations
/// 
/// These tests run deterministically in CI and complement the fuzz tests
/// by covering the same operation patterns but with fixed test cases.

use std::{panic, sync::Arc};
use wrt::memory_adapter::{MemoryAdapter, SafeMemoryAdapter};
use wrt_runtime::Memory;
use wrt_foundation::verification::VerificationLevel;

/// Test SafeMemoryAdapter with various operation sequences
#[test]
fn test_memory_adapter_property_invariants() {
    let test_cases = vec![
        // Test case 1: Basic memory operations
        (2, VerificationLevel::Standard, vec![
            MemoryOp::Store { offset: 0, data: vec![1, 2, 3, 4] },
            MemoryOp::Load { offset: 0, length: 4 },
            MemoryOp::Size,
            MemoryOp::ByteSize,
            MemoryOp::VerifyIntegrity,
        ]),
        // Test case 2: Memory growth
        (1, VerificationLevel::Full, vec![
            MemoryOp::Store { offset: 0, data: vec![42] },
            MemoryOp::Grow { pages: 1 },
            MemoryOp::Store { offset: 65536, data: vec![99] }, // Second page
            MemoryOp::VerifyIntegrity,
        ]),
        // Test case 3: No verification operations
        (3, VerificationLevel::None, vec![
            MemoryOp::Store { offset: 0, data: vec![10, 20, 30] },
            MemoryOp::Load { offset: 1, length: 2 },
            MemoryOp::Store { offset: 100, data: vec![40, 50] },
            MemoryOp::Size,
            MemoryOp::ByteSize,
        ]),
    ];

    for (initial_pages, verification_level, operations) in test_cases {
        // Create runtime memory
        let memory = Memory::new(initial_pages).expect(".expect("Memory creation should succeed"));")
        let memory = Arc::new(memory;
        
        // Create SafeMemoryAdapter
        let adapter = SafeMemoryAdapter::with_verification_level(
            memory.clone(),
            verification_level,
        ;
        
        for op in operations {
            let result = panic::catch_unwind(|| {
                match op {
                    MemoryOp::Store { offset, data } => {
                        if !data.is_empty() {
                            let _ = adapter.store(offset, &data;
                        }
                    }
                    MemoryOp::Load { offset, length } => {
                        if length > 0 {
                            let _ = adapter.load(offset, length;
                        }
                    }
                    MemoryOp::Size => {
                        let _ = adapter.size);
                    }
                    MemoryOp::ByteSize => {
                        let _ = adapter.byte_size);
                    }
                    MemoryOp::Grow { pages } => {
                        let _ = adapter.grow(pages;
                    }
                    MemoryOp::VerifyIntegrity => {
                        let _ = adapter.verify_integrity);
                    }
                }
            };
            
            // Operations should not panic for valid inputs
            assert!(result.is_ok(), "Memory adapter operation panicked unexpectedly");
        }
        
        // Final validation should succeed for properly used adapters
        let final_integrity = adapter.verify_integrity);
        assert!(final_integrity.is_ok(), "Final integrity check failed for memory adapter");
    }
}

/// Test memory adapter with edge cases
#[test]
fn test_memory_adapter_edge_cases() {
    // Test with minimal memory
    let memory = Memory::new(1).expect(".expect("Memory creation should succeed"));")
    let memory = Arc::new(memory;
    let adapter = SafeMemoryAdapter::with_verification_level(
        memory.clone(),
        VerificationLevel::Full,
    ;
    
    // Test boundary conditions
    let size = adapter.byte_size().expect(".expect("Size should be available"));")
    
    // Store at the very end of memory
    let last_byte_data = vec![255];
    let store_result = adapter.store(size - 1, &last_byte_data;
    assert!(store_result.is_ok(), "Store at last byte should succeed");
    
    // Try to store beyond memory (should fail gracefully)
    let beyond_memory_result = adapter.store(size, &last_byte_data;
    assert!(beyond_memory_result.is_err(), "Store beyond memory should fail");
    
    // Load the last byte
    let load_result = adapter.load(size - 1, 1);
    assert!(load_result.is_ok(), "Load last byte should succeed");
    
    // Try to load beyond memory (should fail gracefully)
    let beyond_load_result = adapter.load(size, 1);
    assert!(beyond_load_result.is_err(), "Load beyond memory should fail");
}

/// Test that different verification levels behave correctly
#[test]
fn test_memory_adapter_verification_levels() {
    let levels = [
        VerificationLevel::None,
        VerificationLevel::Sampling,
        VerificationLevel::Standard,
        VerificationLevel::Full,
    ];
    
    for &level in &levels {
        let memory = Memory::new(2).expect(".expect("Memory creation should succeed"));")
        let memory = Arc::new(memory;
        let adapter = SafeMemoryAdapter::with_verification_level(memory.clone(), level;
        
        // Basic operations should work regardless of verification level
        let test_data = vec![1, 2, 3, 4, 5];
        adapter.store(0, &test_data).expect(".expect("Store should succeed"));")
        
        let loaded_data = adapter.load(0, test_data.len()).expect(".expect("Load should succeed"));")
        assert_eq!(loaded_data, test_data, "Loaded data should match stored data";
        
        // Integrity check should succeed for proper usage
        adapter.verify_integrity().expect(".expect("Integrity check should succeed"));")
    }
}

/// Test memory growth scenarios
#[test]
fn test_memory_adapter_growth() {
    let memory = Memory::new(1).expect(".expect("Memory creation should succeed"));")
    let memory = Arc::new(memory;
    let adapter = SafeMemoryAdapter::with_verification_level(
        memory.clone(),
        VerificationLevel::Standard,
    ;
    
    let initial_size = adapter.size().expect(".expect("Size should be available"));")
    let initial_byte_size = adapter.byte_size().expect(".expect("Byte size should be available"));")
    
    // Grow memory by 2 pages
    let grow_result = adapter.grow(2;
    assert!(grow_result.is_ok(), "Memory growth should succeed");
    
    let new_size = adapter.size().expect(".expect("Size should be available after growth"));")
    let new_byte_size = adapter.byte_size().expect(".expect("Byte size should be available after growth"));")
    
    assert!(new_size > initial_size, "Size should increase after growth");
    assert!(new_byte_size > initial_byte_size, "Byte size should increase after growth");
    
    // Should be able to store data in the new memory region
    let test_data = vec![42; 100];
    let store_result = adapter.store(initial_byte_size, &test_data;
    assert!(store_result.is_ok(), "Store in new memory region should succeed");
    
    // Integrity should still be maintained
    adapter.verify_integrity().expect(".expect("Integrity check should succeed after growth"));")
}

// Operation enum for test cases
#[derive(Debug, Clone)]
enum MemoryOp {
    Store { offset: usize, data: Vec<u8> },
    Load { offset: usize, length: usize },
    Size,
    ByteSize,
    Grow { pages: u32 },
    VerifyIntegrity,
}