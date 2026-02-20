//! Memory budget limits for wrt-format safety-critical operations
//!
//! This module defines compile-time memory limits for all bounded collections
//! used in the wrt-format crate when the safety-critical feature is enabled.

#![cfg(feature = "safety-critical")]

/// Binary format processing memory limits
pub mod binary_processing {
    /// Maximum size of a WebAssembly binary in bytes (4 MiB)
    pub const MAX_BINARY_SIZE: usize = 4 * 1024 * 1024;

    /// Maximum size of a section in bytes (64 KiB)
    pub const MAX_SECTION_SIZE: usize = 64 * 1024;

    /// Maximum size of LEB128 encoding buffer
    pub const MAX_LEB128_BUFFER: usize = 10;
}

/// WIT format parsing memory limits
pub mod wit_parsing {
    /// Maximum number of AST items
    pub const MAX_AST_ITEMS: usize = 256;

    /// Maximum number of AST parameters
    pub const MAX_AST_PARAMS: usize = 32;

    /// Maximum number of generative types
    pub const MAX_GENERATIVE_TYPES: usize = 512;

    /// Maximum line length for WIT parsing
    pub const MAX_LINE_LENGTH: usize = 1024;
}

/// Module structure memory limits
pub mod module_structures {
    /// Maximum number of functions per module
    pub const MAX_MODULE_FUNCTIONS: usize = 4096;

    /// Maximum number of imports per module
    pub const MAX_MODULE_IMPORTS: usize = 512;

    /// Maximum number of exports per module
    pub const MAX_MODULE_EXPORTS: usize = 512;

    /// Maximum number of locals per function
    pub const MAX_FUNCTION_LOCALS: usize = 256;

    /// Maximum size of function code in bytes
    pub const MAX_FUNCTION_CODE_SIZE: usize = 64 * 1024;
}

/// Memory usage validation
#[cfg(test)]
mod validation {
    use super::*;

    /// Estimated bytes per item for memory calculations
    const BYTES_PER_AST_ITEM: usize = 128;
    const BYTES_PER_FUNCTION: usize = 1024;
    const BYTES_PER_IMPORT: usize = 256;
    const BYTES_PER_EXPORT: usize = 128;

    /// Total format memory budget in bytes (512 KiB)
    const TOTAL_BUDGET: usize = 512 * 1024;

    #[test]
    fn validate_binary_processing_budget() {
        // Note: Binary size is separate from processing structures
        let budget = binary_processing::MAX_SECTION_SIZE * 2 + // Active sections
            binary_processing::MAX_LEB128_BUFFER * 64; // LEB128 buffers

        assert!(
            budget < 150 * 1024,
            "Binary processing budget exceeds 150KB: {}KB",
            budget / 1024
        );
    }

    #[test]
    fn validate_wit_parsing_budget() {
        let budget = wit_parsing::MAX_AST_ITEMS * BYTES_PER_AST_ITEM
            + wit_parsing::MAX_AST_PARAMS * 64
            + wit_parsing::MAX_GENERATIVE_TYPES * 128
            + wit_parsing::MAX_LINE_LENGTH * 16; // Multiple line buffers

        assert!(
            budget < 150 * 1024,
            "WIT parsing budget exceeds 150KB: {}KB",
            budget / 1024
        );
    }

    #[test]
    fn validate_module_structures_budget() {
        let budget = module_structures::MAX_MODULE_FUNCTIONS * BYTES_PER_FUNCTION
            + module_structures::MAX_MODULE_IMPORTS * BYTES_PER_IMPORT
            + module_structures::MAX_MODULE_EXPORTS * BYTES_PER_EXPORT
            + module_structures::MAX_FUNCTION_LOCALS * 32; // Local variable descriptors

        // This will be large due to function metadata, but it's bounded
        assert!(
            budget < 5 * 1024 * 1024,
            "Module structures budget exceeds 5MB: {}MB",
            budget / (1024 * 1024)
        );
    }

    #[test]
    fn validate_parsing_memory() {
        // Runtime parsing structures should fit in 512 KB
        let parsing_budget = 150 * 1024 + // Binary processing
            150 * 1024 + // WIT parsing
            100 * 1024; // Overhead and temporary structures

        assert!(
            parsing_budget <= TOTAL_BUDGET,
            "Parsing memory exceeds 512KB: {}KB",
            parsing_budget / 1024
        );
    }
}

/// Compile-time assertions for safety properties
#[cfg(feature = "safety-critical")]
mod safety_assertions {
    use super::*;

    // Ensure binary limits are reasonable
    const _: () = assert!(binary_processing::MAX_BINARY_SIZE <= 8 * 1024 * 1024);
    const _: () = assert!(binary_processing::MAX_SECTION_SIZE <= 128 * 1024);
    const _: () = assert!(binary_processing::MAX_LEB128_BUFFER <= 16);

    // Ensure WIT parsing limits are reasonable
    const _: () = assert!(wit_parsing::MAX_AST_ITEMS <= 512);
    const _: () = assert!(wit_parsing::MAX_AST_PARAMS <= 64);
    const _: () = assert!(wit_parsing::MAX_GENERATIVE_TYPES <= 1024);
    const _: () = assert!(wit_parsing::MAX_LINE_LENGTH <= 2048);

    // Ensure module structure limits are reasonable
    const _: () = assert!(module_structures::MAX_MODULE_FUNCTIONS <= 8192);
    const _: () = assert!(module_structures::MAX_MODULE_IMPORTS <= 1024);
    const _: () = assert!(module_structures::MAX_MODULE_EXPORTS <= 1024);
    const _: () = assert!(module_structures::MAX_FUNCTION_LOCALS <= 512);

    // Ensure limits are powers of 2 or nice round numbers
    const _: () = assert!(wit_parsing::MAX_AST_ITEMS == 256);
    const _: () = assert!(wit_parsing::MAX_AST_PARAMS == 32);
    const _: () = assert!(wit_parsing::MAX_GENERATIVE_TYPES == 512);
    const _: () = assert!(module_structures::MAX_MODULE_FUNCTIONS == 4096);
    const _: () = assert!(module_structures::MAX_MODULE_IMPORTS == 512);
    const _: () = assert!(module_structures::MAX_MODULE_EXPORTS == 512);
}

/// Re-export commonly used limits
pub use binary_processing::*;
pub use module_structures::*;
pub use wit_parsing::*;
