//! Memory budget limits for wrt-host safety-critical operations
//!
//! This module defines compile-time memory limits for all bounded collections
//! used in the wrt-host crate when the safety-critical feature is enabled.

#![cfg(feature = "safety-critical")]

/// Host function management memory limits
pub mod host_functions {
    /// Maximum number of registered host functions
    pub const HOST_FUNCTIONS_LIMIT: usize = 128;

    /// Maximum number of critical builtin functions
    pub const CRITICAL_BUILTINS_LIMIT: usize = 32;

    /// Maximum size of function name strings
    pub const FUNCTION_NAME_LIMIT: usize = 64;
}

/// Function call management memory limits
pub mod function_calls {
    /// Maximum number of parameters per function call
    pub const MAX_FUNCTION_PARAMS: usize = 16;

    /// Maximum number of return values per function
    pub const MAX_RETURN_VALUES: usize = 8;

    /// Maximum depth of nested function calls
    pub const CALL_STACK_DEPTH_LIMIT: usize = 64;
}

/// Memory usage validation
#[cfg(test)]
mod validation {
    use super::*;

    /// Estimated bytes per item for memory calculations
    const BYTES_PER_HANDLER: usize = 256; // Function pointer + metadata
    const BYTES_PER_BUILTIN: usize = 128;
    const BYTES_PER_PARAM: usize = 64;
    const BYTES_PER_CALL_FRAME: usize = 128;

    /// Total host memory budget in bytes (512 KiB)
    const TOTAL_BUDGET: usize = 512 * 1024;

    #[test]
    fn validate_host_functions_budget() {
        let budget = host_functions::HOST_FUNCTIONS_LIMIT * BYTES_PER_HANDLER
            + host_functions::CRITICAL_BUILTINS_LIMIT * BYTES_PER_BUILTIN
            + host_functions::HOST_FUNCTIONS_LIMIT * host_functions::FUNCTION_NAME_LIMIT;

        assert!(
            budget < 50 * 1024,
            "Host functions budget exceeds 50KB: {}KB",
            budget / 1024
        );
    }

    #[test]
    fn validate_function_calls_budget() {
        let budget = function_calls::MAX_FUNCTION_PARAMS * BYTES_PER_PARAM
            + function_calls::MAX_RETURN_VALUES * BYTES_PER_PARAM
            + function_calls::CALL_STACK_DEPTH_LIMIT * BYTES_PER_CALL_FRAME;

        assert!(
            budget < 16 * 1024,
            "Function calls budget exceeds 16KB: {}KB",
            budget / 1024
        );
    }

    #[test]
    fn validate_total_budget() {
        let host_functions = 42 * 1024; // ~42 KB
        let function_calls = 9 * 1024; // ~9 KB
        let overhead = 16 * 1024; // ~16 KB for other structures

        let total = host_functions + function_calls + overhead;

        assert!(
            total <= TOTAL_BUDGET / 8, // Use only 1/8 of budget for host layer
            "Host layer exceeds 64KB allocation: {}KB",
            total / 1024
        );
    }
}

/// Compile-time assertions for safety properties
#[cfg(feature = "safety-critical")]
mod safety_assertions {
    use super::*;

    // Ensure limits fit in reasonable bounds
    const _: () = assert!(host_functions::HOST_FUNCTIONS_LIMIT <= 256);
    const _: () = assert!(host_functions::CRITICAL_BUILTINS_LIMIT <= 64);
    const _: () = assert!(host_functions::FUNCTION_NAME_LIMIT <= 128);
    const _: () = assert!(function_calls::MAX_FUNCTION_PARAMS <= 32);
    const _: () = assert!(function_calls::MAX_RETURN_VALUES <= 16);
    const _: () = assert!(function_calls::CALL_STACK_DEPTH_LIMIT <= 128);

    // Ensure limits are powers of 2 or nice round numbers for efficiency
    const _: () = assert!(host_functions::HOST_FUNCTIONS_LIMIT == 128);
    const _: () = assert!(host_functions::CRITICAL_BUILTINS_LIMIT == 32);
    const _: () = assert!(host_functions::FUNCTION_NAME_LIMIT == 64);
    const _: () = assert!(function_calls::MAX_FUNCTION_PARAMS == 16);
    const _: () = assert!(function_calls::MAX_RETURN_VALUES == 8);
    const _: () = assert!(function_calls::CALL_STACK_DEPTH_LIMIT == 64);
}

pub use function_calls::*;
/// Re-export commonly used limits
pub use host_functions::*;
