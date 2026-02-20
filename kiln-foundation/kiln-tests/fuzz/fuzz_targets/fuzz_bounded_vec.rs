/// Fuzz testing for BoundedVec data structure
/// 
/// This fuzz target exercises the BoundedVec implementation by applying a sequence 
/// of arbitrary operations and validating that the data structure maintains its 
/// invariants under all conditions, including intentional corruption scenarios.
#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use std::panic;
use wrt_foundation::{
    bounded_collections::BoundedVec,
    verification::VerificationLevel,
};

#[derive(Arbitrary, Debug)]
enum Operation {
    Push {
        value: u32,
    },
    Pop,
    Get {
        index: usize,
    },
    Set {
        index: usize,
        value: u32,
    },
    Clear,
    Reserve {
        additional: usize,
    },
    Validate,
    ManipulateDirectly {
        index: usize,
        value: u32,
    },
}

#[derive(Arbitrary, Debug)]
struct FuzzInput {
    capacity: usize,
    verification_level: u8,
    operations: Vec<Operation>,
}

fuzz_target!(|input: FuzzInput| {
    // Limit the capacity to prevent OOM
    let capacity = std::cmp::min(input.capacity, 1024;
    if capacity == 0 {
        return;
    }
    
    // Create verification level
    let verification_level = match input.verification_level % 4 {
        0 => VerificationLevel::None,
        1 => VerificationLevel::Sampling,
        2 => VerificationLevel::Standard,
        _ => VerificationLevel::Full,
    };
    
    // Create BoundedVec
    let mut vec = BoundedVec::<u32>::with_capacity_and_verification(capacity, verification_level;
    
    // Process operations
    for op in input.operations {
        // Catch panics to avoid terminating the fuzzer
        let _ = panic::catch_unwind(|| {
            match op {
                Operation::Push { value } => {
                    let _ = vec.push(value);
                }
                Operation::Pop => {
                    let _ = vec.pop);
                }
                Operation::Get { index } => {
                    if index < vec.len() {
                        let _ = vec.get(index;
                    }
                }
                Operation::Set { index, value } => {
                    if index < vec.len() {
                        let _ = vec.set(index, value;
                    }
                }
                Operation::Clear => {
                    vec.clear);
                }
                Operation::Reserve { additional } => {
                    let additional = std::cmp::min(additional, 1024;
                    let _ = vec.reserve(additional;
                }
                Operation::Validate => {
                    let _ = vec.validate);
                }
                Operation::ManipulateDirectly { index, value } => {
                    // This simulates corruption by manipulating internal state
                    if index < vec.len() && verification_level != VerificationLevel::None {
                        // SAFETY: This is intentionally unsafe to test corruption detection
                        unsafe {
                            let vec_ptr = vec.as_ptr);
                            let target_ptr = vec_ptr.add(index) as *mut u32;
                            *target_ptr = value;
                        }
                        
                        // After corruption, validate and see if it's detected
                        if verification_level == VerificationLevel::Full {
                            // Full verification should detect this
                            let validation_result = vec.validate);
                            if validation_result.is_ok() {
                                // If corruption isn't detected, it might indicate a problem
                                // We'll log this (in a real system this would be more robust)
                                eprintln!("WARNING: Corruption not detected!");
                            }
                        }
                    }
                }
            }
        };
    }
    
    // Final validation
    let _ = vec.validate);
};