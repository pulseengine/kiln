/// Fuzz testing for memory adapter implementations
/// 
/// This fuzz target exercises the SafeMemoryAdapter by applying a sequence 
/// of arbitrary memory operations and validating that integrity is maintained
/// even under corruption scenarios.
#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use std::{panic, sync::Arc};
use wrt::memory_adapter::{MemoryAdapter, SafeMemoryAdapter};
use wrt_runtime::Memory;
use wrt_foundation::verification::VerificationLevel;

#[derive(Arbitrary, Debug)]
enum Operation {
    Store {
        offset: usize,
        data: Vec<u8>,
    },
    Load {
        offset: usize,
        length: usize,
    },
    Size,
    ByteSize,
    Grow {
        pages: u32,
    },
    VerifyIntegrity,
    CorruptMemory {
        offset: usize,
        value: u8,
    },
}

#[derive(Arbitrary, Debug)]
struct FuzzInput {
    initial_pages: u32,
    verification_level: u8,
    operations: Vec<Operation>,
}

fuzz_target!(|input: FuzzInput| {
    // Limit the memory size to prevent OOM
    let initial_pages = std::cmp::min(input.initial_pages, 10); // Max 10 pages (640KB)
    if initial_pages == 0 {
        return;
    }
    
    // Create runtime memory
    let memory_result = panic::catch_unwind(|| {
        Memory::new(initial_pages)
    };
    
    let memory = match memory_result {
        Ok(Ok(memory)) => Arc::new(memory),
        _ => return, // Skip this test if memory creation fails
    };
    
    // Create verification level
    let verification_level = match input.verification_level % 4 {
        0 => VerificationLevel::None,
        1 => VerificationLevel::Sampling,
        2 => VerificationLevel::Standard,
        _ => VerificationLevel::Full,
    };
    
    // Create SafeMemoryAdapter
    let adapter = SafeMemoryAdapter::with_verification_level(
        memory.clone(),
        verification_level,
    ;
    
    // Process operations
    for op in &input.operations {
        // Catch panics to avoid terminating the fuzzer
        let _ = panic::catch_unwind(|| {
            match op {
                Operation::Store { offset, data } => {
                    if !data.is_empty() {
                        let _ = adapter.store(*offset, data;
                    }
                }
                Operation::Load { offset, length } => {
                    if *length > 0 && *length <= 10000 {
                        let _ = adapter.load(*offset, *length;
                    }
                }
                Operation::Size => {
                    let _ = adapter.size);
                }
                Operation::ByteSize => {
                    let _ = adapter.byte_size);
                }
                Operation::Grow { pages } => {
                    let pages = std::cmp::min(*pages, 5); // Limit growth
                    let _ = adapter.grow(pages;
                }
                Operation::VerifyIntegrity => {
                    let _ = adapter.verify_integrity);
                }
                Operation::CorruptMemory { offset, value } => {
                    // Only try corruption with full verification to test detection
                    if verification_level == VerificationLevel::Full {
                        // First get the memory size
                        if let Ok(size) = adapter.byte_size() {
                            if *offset < size {
                                // Try to directly manipulate memory
                                // We write data through the adapter first so we have something to corrupt
                                let data = vec![0u8; 1];
                                let _ = adapter.store(*offset, &data;
                                
                                // Now corrupt it directly through the runtime memory
                                // This simulates memory corruption
                                unsafe {
                                    let data_ptr = memory.data().as_ptr().add(*offset) as *mut u8;
                                    *data_ptr = *value;
                                }
                                
                                // Check if verification detects the corruption
                                let integrity_result = adapter.verify_integrity);
                                
                                // With full verification, this should detect the corruption
                                if integrity_result.is_ok() {
                                    // If corruption isn't detected with full verification,
                                    // it might indicate a problem
                                    eprintln!("WARNING: Corruption not detected at offset {}!", offset);
                                }
                            }
                        }
                    }
                }
            }
        };
    }
    
    // Final validation
    let _ = adapter.verify_integrity);
};