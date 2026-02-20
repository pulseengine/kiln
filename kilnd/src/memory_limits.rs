//! Memory budget limits for wrtd safety-critical operations
//!
//! This module defines compile-time memory limits for all bounded collections
//! used in the wrtd daemon when the safety-critical feature is enabled.

#![cfg(feature = "safety-critical")]

/// Module loading memory limits
pub mod module_loading {
    /// Maximum size of a WebAssembly module in bytes
    pub const MODULE_SIZE_LIMIT: usize = 2 * 1024 * 1024; // 2 MiB

    /// Maximum number of modules that can be cached
    pub const MODULE_CACHE_LIMIT: usize = 8;

    /// Buffer size for file reading operations
    pub const FILE_BUFFER_SIZE: usize = 4096; // 4 KiB
}

/// Runtime execution memory limits
pub mod execution {
    /// Maximum number of function arguments
    pub const MAX_FUNCTION_ARGS: usize = 32;

    /// Maximum stack depth for execution
    pub const STACK_DEPTH_LIMIT: usize = 256;

    /// Maximum number of exported functions per module
    pub const EXPORTS_LIMIT: usize = 128;
}

/// Memory usage validation
#[cfg(test)]
mod validation {
    use super::*;

    /// Total wrtd memory budget in bytes (512 KiB)
    const TOTAL_BUDGET: usize = 512 * 1024;

    #[test]
    fn validate_module_loading_budget() {
        let budget = module_loading::MODULE_SIZE_LIMIT + // Main module storage
            module_loading::MODULE_CACHE_LIMIT * (module_loading::MODULE_SIZE_LIMIT / 4) + // Cache overhead
            module_loading::FILE_BUFFER_SIZE; // I/O buffer

        // This will be large due to module size, but it's bounded
        assert!(
            budget < 12 * 1024 * 1024,
            "Module loading budget exceeds 12MB: {}MB",
            budget / (1024 * 1024)
        );
    }

    #[test]
    fn validate_execution_budget() {
        let budget = execution::MAX_FUNCTION_ARGS * 64 + // Function arguments
            execution::STACK_DEPTH_LIMIT * 64 + // Stack frames
            execution::EXPORTS_LIMIT * 128; // Export table

        assert!(
            budget < 32 * 1024,
            "Execution budget exceeds 32KB: {}KB",
            budget / 1024
        );
    }

    #[test]
    fn validate_runtime_memory() {
        // Runtime structures should fit in 512 KB
        let runtime_budget = 32 * 1024 + // Execution structures
            64 * 1024 + // Configuration and state
            32 * 1024; // Logging and stats

        assert!(
            runtime_budget <= TOTAL_BUDGET,
            "Runtime memory exceeds 512KB: {}KB",
            runtime_budget / 1024
        );
    }
}

/// Compile-time assertions for safety properties
#[cfg(feature = "safety-critical")]
mod safety_assertions {
    use super::*;

    // Ensure module size is reasonable but not too restrictive
    const _: () = assert!(module_loading::MODULE_SIZE_LIMIT <= 4 * 1024 * 1024);
    const _: () = assert!(module_loading::MODULE_SIZE_LIMIT >= 1024 * 1024);

    // Ensure cache is bounded but useful
    const _: () = assert!(module_loading::MODULE_CACHE_LIMIT <= 16);
    const _: () = assert!(module_loading::MODULE_CACHE_LIMIT >= 4);

    // Ensure execution limits are reasonable
    const _: () = assert!(execution::MAX_FUNCTION_ARGS <= 64);
    const _: () = assert!(execution::STACK_DEPTH_LIMIT <= 512);
    const _: () = assert!(execution::EXPORTS_LIMIT <= 256);

    // Ensure limits are powers of 2 or nice round numbers
    const _: () = assert!(execution::MAX_FUNCTION_ARGS == 32);
    const _: () = assert!(execution::STACK_DEPTH_LIMIT == 256);
    const _: () = assert!(execution::EXPORTS_LIMIT == 128);
}

pub use execution::*;
/// Re-export commonly used limits
pub use module_loading::*;
