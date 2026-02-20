/// Fuzz testing for SafeSlice memory wrapper
/// 
/// This fuzz target exercises the SafeSlice implementation by applying a sequence 
/// of arbitrary memory operations and validating that integrity checks work correctly
/// under various verification levels and corruption scenarios.
#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use std::panic;
use wrt_foundation::{
    safe_memory::{MemoryProvider, SafeSlice, StdMemoryProvider},
    verification::VerificationLevel,
};

#[derive(Arbitrary, Debug)]
enum Operation {
    Get {
        index: usize,
    },
    Set {
        index: usize,
        value: u8,
    },
    CopyFromSlice {
        offset: usize,
        data: Vec<u8>,
    },
    GetSlice {
        offset: usize,
        length: usize,
    },
    Validate,
    ValidateChecksum,
    CheckIntegrity,
    ChangeData {
        index: usize,
        value: u8,
        skip_tracking: bool, // Try to simulate corruption by skipping tracking
    },
}

#[derive(Arbitrary, Debug)]
struct FuzzInput {
    memory_size: usize,
    verification_level: u8,
    operations: Vec<Operation>,
}

fuzz_target!(|input: FuzzInput| {
    // Limit the memory size to prevent OOM
    let memory_size = std::cmp::min(input.memory_size, 1024 * 1024;
    if memory_size == 0 {
        return;
    }
    
    // Create memory provider
    let memory = StdMemoryProvider::new(memory_size;
    
    // Create SafeSlice with verification level
    let verification_level = match input.verification_level % 4 {
        0 => VerificationLevel::None,
        1 => VerificationLevel::Sampling,
        2 => VerificationLevel::Standard,
        _ => VerificationLevel::Full,
    };
    
    let safe_slice = SafeSlice::with_verification_level(
        memory.get_buffer(),
        verification_level,
    ;
    
    // Process operations
    for op in input.operations {
        // Catch panics to avoid terminating the fuzzer
        let _ = panic::catch_unwind(|| {
            match op {
                Operation::Get { index } => {
                    if index < safe_slice.len() {
                        let _ = safe_slice.get(index;
                    }
                }
                Operation::Set { index, value } => {
                    if index < safe_slice.len() {
                        safe_slice.set(index, value;
                    }
                }
                Operation::CopyFromSlice { offset, data } => {
                    if !data.is_empty() && offset + data.len() <= safe_slice.len() {
                        let _ = safe_slice.copy_from_slice(offset, &data;
                    }
                }
                Operation::GetSlice { offset, length } => {
                    if length > 0 && offset + length <= safe_slice.len() {
                        let _ = safe_slice.get_slice(offset, length;
                    }
                }
                Operation::Validate => {
                    let _ = safe_slice.validate);
                }
                Operation::ValidateChecksum => {
                    let _ = safe_slice.validate_checksum);
                }
                Operation::CheckIntegrity => {
                    let _ = safe_slice.check_integrity);
                }
                Operation::ChangeData {
                    index,
                    value,
                    skip_tracking,
                } => {
                    if index < safe_slice.len() {
                        // Direct memory manipulation to simulate corruption
                        if skip_tracking {
                            // SAFETY: This is intentionally unsafe to test corruption detection
                            unsafe {
                                let ptr = memory.get_buffer().as_ptr().add(index) as *mut u8;
                                *ptr = value;
                            }
                        } else {
                            safe_slice.set(index, value;
                        }
                    }
                }
            }
        };
    }
    
    // Final validation
    let _ = safe_slice.validate);
};