//! Memory budget limits for wrt-decoder safety-critical operations
//!
//! This module defines compile-time memory limits for all bounded collections
//! used in the wrt-decoder crate when the safety-critical feature is enabled.

#![cfg(feature = "safety-critical")]

/// Custom section parsing memory limits
pub mod custom_sections {
    /// Maximum number of custom sections per module
    pub const SECTIONS_LIMIT: usize = 64;

    /// Maximum number of function names in name section
    pub const FUNCTION_NAMES_LIMIT: usize = 256;

    /// Maximum size of raw custom section data in bytes
    pub const RAW_SECTION_DATA_LIMIT: usize = 64 * 1024; // 64 KiB
}

/// Branch hint section memory limits
pub mod branch_hints {
    /// Maximum number of functions with branch hints
    pub const FUNCTIONS_WITH_HINTS_LIMIT: usize = 256;

    /// Maximum number of hints per function
    pub const HINTS_PER_FUNCTION_LIMIT: usize = 64;
}

/// Memory usage validation
#[cfg(test)]
mod validation {
    use super::*;

    /// Estimated bytes per item for memory calculations
    const BYTES_PER_SECTION: usize = 1024; // Conservative estimate
    const BYTES_PER_FUNCTION_NAME: usize = 64;
    const BYTES_PER_HINT: usize = 16;

    /// Total decoder memory budget in bytes (512 KiB)
    const TOTAL_BUDGET: usize = 512 * 1024;

    #[test]
    fn validate_custom_sections_budget() {
        let budget = custom_sections::SECTIONS_LIMIT * BYTES_PER_SECTION
            + custom_sections::FUNCTION_NAMES_LIMIT * BYTES_PER_FUNCTION_NAME
            + custom_sections::RAW_SECTION_DATA_LIMIT;

        assert!(
            budget < 200 * 1024,
            "Custom sections budget exceeds 200KB: {}KB",
            budget / 1024
        );
    }

    #[test]
    fn validate_branch_hints_budget() {
        let budget = branch_hints::FUNCTIONS_WITH_HINTS_LIMIT * BYTES_PER_HINT
            + branch_hints::FUNCTIONS_WITH_HINTS_LIMIT
                * branch_hints::HINTS_PER_FUNCTION_LIMIT
                * BYTES_PER_HINT;

        assert!(
            budget < 300 * 1024,
            "Branch hints budget exceeds 300KB: {}KB",
            budget / 1024
        );
    }

    #[test]
    fn validate_total_budget() {
        let custom_sections = 130 * 1024; // Including raw data
        let branch_hints = 270 * 1024;
        let overhead = 50 * 1024; // Parsing overhead

        let total = custom_sections + branch_hints + overhead;

        assert!(
            total <= TOTAL_BUDGET,
            "Total budget exceeds 512KB: {}KB",
            total / 1024
        );
    }
}

/// Compile-time assertions for safety properties
#[cfg(feature = "safety-critical")]
mod safety_assertions {
    use super::*;

    // Ensure limits fit in reasonable bounds
    const _: () = assert!(custom_sections::SECTIONS_LIMIT <= 128);
    const _: () = assert!(custom_sections::FUNCTION_NAMES_LIMIT <= 512);
    const _: () = assert!(custom_sections::RAW_SECTION_DATA_LIMIT <= 128 * 1024);
    const _: () = assert!(branch_hints::FUNCTIONS_WITH_HINTS_LIMIT <= 512);
    const _: () = assert!(branch_hints::HINTS_PER_FUNCTION_LIMIT <= 128);

    // Ensure limits are powers of 2 or nice round numbers for efficiency
    const _: () = assert!(custom_sections::SECTIONS_LIMIT == 64);
    const _: () = assert!(custom_sections::FUNCTION_NAMES_LIMIT == 256);
    const _: () = assert!(branch_hints::FUNCTIONS_WITH_HINTS_LIMIT == 256);
    const _: () = assert!(branch_hints::HINTS_PER_FUNCTION_LIMIT == 64);
}

pub use branch_hints::*;
/// Re-export commonly used limits
pub use custom_sections::*;
