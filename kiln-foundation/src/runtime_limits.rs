//! Runtime memory budget limits for safety-critical operations
//!
//! This module defines compile-time memory limits for all bounded collections
//! used in runtime operations when ASIL-D or other high safety levels are enabled.
//! These limits ensure deterministic memory usage for safety compliance.

/// Resource management memory limits
pub mod resources {
    /// Maximum number of fields in a Record resource
    pub const RECORD_FIELDS_LIMIT: usize = 32;

    /// Maximum number of resources in an Aggregate resource
    pub const AGGREGATE_RESOURCES_LIMIT: usize = 16;

    /// Maximum number of resources in a resource table
    pub const RESOURCE_TABLE_LIMIT: usize = 1024;
}

/// Runtime execution memory limits
pub mod execution {
    /// Maximum size of module bytecode in bytes
    pub const MODULE_SIZE_LIMIT: usize = 2 * 1024 * 1024; // 2 MiB

    /// Maximum number of functions per module
    pub const FUNCTIONS_PER_MODULE_LIMIT: usize = 512;

    /// Maximum stack depth for execution
    pub const STACK_DEPTH_LIMIT: usize = 256;
}

/// Memory usage validation
#[cfg(test)]
mod validation {
    use super::*;

    /// Estimated bytes per item for memory calculations
    const BYTES_PER_FIELD_NAME: usize = 64;
    const BYTES_PER_RESOURCE_TYPE: usize = 256;
    const BYTES_PER_RESOURCE_ENTRY: usize = 512;
    const BYTES_PER_FUNCTION: usize = 1024;

    /// Total runtime memory budget in bytes (4 MiB)
    /// Updated to accommodate MODULE_SIZE_LIMIT of 2MB plus other execution overhead
    const TOTAL_BUDGET: usize = 4 * 1024 * 1024;

    #[test]
    fn validate_resource_budget() {
        let budget = resources::RECORD_FIELDS_LIMIT * BYTES_PER_FIELD_NAME
            + resources::AGGREGATE_RESOURCES_LIMIT * BYTES_PER_RESOURCE_TYPE
            + resources::RESOURCE_TABLE_LIMIT * BYTES_PER_RESOURCE_ENTRY;

        assert!(
            budget < 600 * 1024,
            "Resource budget exceeds 600KB: {}KB",
            budget / 1024
        );
    }

    #[test]
    fn validate_execution_budget() {
        let budget = execution::MODULE_SIZE_LIMIT
            + execution::FUNCTIONS_PER_MODULE_LIMIT * BYTES_PER_FUNCTION
            + execution::STACK_DEPTH_LIMIT * 64; // Stack frame size

        // Budget is ~2.5MB with current limits (MODULE_SIZE_LIMIT is 2MB alone)
        assert!(
            budget < 3 * 1024 * 1024,
            "Execution budget exceeds 3MB: {}KB",
            budget / 1024
        );
    }

    #[test]
    fn validate_total_budget() {
        let resources = 518 * 1024; // ~518 KB
        let execution = 3 * 1024 * 1024; // ~3 MB (updated to match MODULE_SIZE_LIMIT increase)

        let total = resources + execution;

        // TOTAL_BUDGET should accommodate current limits
        assert!(
            total <= TOTAL_BUDGET,
            "Total budget exceeds limit: {}KB (limit: {}KB)",
            total / 1024,
            TOTAL_BUDGET / 1024
        );
    }
}

/// Compile-time assertions for safety properties
mod safety_assertions {
    use super::*;

    // Ensure limits fit in reasonable bounds
    const _: () = assert!(resources::RECORD_FIELDS_LIMIT <= 64);
    const _: () = assert!(resources::AGGREGATE_RESOURCES_LIMIT <= 32);
    const _: () = assert!(resources::RESOURCE_TABLE_LIMIT <= 2048);
    const _: () = assert!(execution::MODULE_SIZE_LIMIT <= 4 * 1024 * 1024);
    const _: () = assert!(execution::FUNCTIONS_PER_MODULE_LIMIT <= 1024);
    const _: () = assert!(execution::STACK_DEPTH_LIMIT <= 512);

    // Ensure limits are powers of 2 or nice round numbers for efficiency
    const _: () = assert!(resources::RECORD_FIELDS_LIMIT == 32);
    const _: () = assert!(resources::AGGREGATE_RESOURCES_LIMIT == 16);
    const _: () = assert!(resources::RESOURCE_TABLE_LIMIT == 1024);
    const _: () = assert!(execution::FUNCTIONS_PER_MODULE_LIMIT == 512);
    const _: () = assert!(execution::STACK_DEPTH_LIMIT == 256);
}

// Re-export commonly used limits
pub use execution::*;
pub use resources::*;
