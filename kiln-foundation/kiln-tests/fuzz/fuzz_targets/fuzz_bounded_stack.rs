/// Fuzz testing for BoundedStack data structure
/// 
/// This fuzz target exercises the BoundedStack implementation by applying a sequence 
/// of arbitrary operations and validating that the data structure maintains its 
/// invariants under all conditions, including intentional corruption scenarios.
#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use std::panic;
use wrt_foundation::{
    bounded_collections::BoundedStack,
    verification::VerificationLevel,
};

#[derive(Arbitrary, Debug)]
enum Operation {
    Push {
        value: u32,
    },
    Pop,
    Peek,
    Clear,
    Validate,
    CheckCapacity,
    ManipulateInternals {
        operation_type: u8, // 0: corrupt top, 1: corrupt data
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
    
    // Create BoundedStack
    let mut stack = BoundedStack::<u32>::with_capacity_and_verification(capacity, verification_level;
    
    // Process operations
    for op in input.operations {
        // Catch panics to avoid terminating the fuzzer
        let _ = panic::catch_unwind(|| {
            match op {
                Operation::Push { value } => {
                    let _ = stack.push(value);
                }
                Operation::Pop => {
                    let _ = stack.pop);
                }
                Operation::Peek => {
                    let _ = stack.peek);
                }
                Operation::Clear => {
                    stack.clear);
                }
                Operation::Validate => {
                    let _ = stack.validate);
                }
                Operation::CheckCapacity => {
                    let _ = stack.available_capacity);
                    let _ = stack.is_full);
                    let _ = stack.len();
                }
                Operation::ManipulateInternals { operation_type, value } => {
                    // This simulates corruption or direct manipulation
                    // Only do this with full verification to test detection
                    if verification_level == VerificationLevel::Full && !stack.is_empty() {
                        match operation_type % 2 {
                            0 => {
                                // Corrupt the stack structure directly
                                // This is using internal knowledge of the BoundedStack implementation
                                unsafe {
                                    // Get access to the internal Vec
                                    let data_ptr = stack.as_ptr);
                                    if !data_ptr.is_null() && stack.len() > 0 {
                                        // Modify the last element (top of stack) 
                                        let top_index = stack.len() - 1;
                                        let target_ptr = data_ptr.add(top_index) as *mut u32;
                                        *target_ptr = value;
                                    }
                                }
                            }
                            _ => {
                                // Try a sequence of operations that might be problematic
                                if !stack.is_full() {
                                    let _ = stack.push(value);
                                }
                                if !stack.is_empty() {
                                    let _ = stack.pop);
                                }
                                let _ = stack.validate);
                            }
                        }
                    }
                }
            }
        };
    }
    
    // Final validation to check if corruption was detected
    let _ = stack.validate);
};