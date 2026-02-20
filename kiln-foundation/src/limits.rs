//! Platform-specific WebAssembly limits for bounded memory allocation.
//!
//! This module defines compile-time limits for WebAssembly module decoding and
//! execution. Different platform profiles are supported via feature flags:
//!
//! - `embedded-small`: MCU targets with ~64KB RAM
//! - `embedded-medium`: Embedded Linux with ~1MB RAM
//! - Default: Desktop/Server with ample memory
//!
//! # Design Rationale
//!
//! WebAssembly's section format provides counts BEFORE data, enabling single-pass
//! streaming decode with bounded memory. These limits enable early rejection of
//! modules that exceed platform capacity, preventing OOM conditions.
//!
//! # Usage
//!
//! ```rust
//! use wrt_foundation::limits;
//!
//! // Check before allocation
//! if param_count > limits::MAX_FUNCTION_PARAMS {
//!     return Err(Error::module_exceeds_platform_limits());
//! }
//! ```
//!
//! # Safety
//!
//! These limits are designed to ensure deterministic memory usage for
//! ASIL-D and other safety-critical configurations.

/// WebAssembly specification constants (not platform-dependent)
pub mod spec {
    /// WebAssembly page size: 64 KiB
    pub const WASM_PAGE_SIZE: usize = 65536;

    /// Maximum linear memory pages per spec (4 GiB / 64 KiB = 65536)
    pub const MAX_WASM_MEMORY_PAGES: u32 = 65536;

    /// Maximum tables per module per spec
    pub const MAX_WASM_TABLES: u32 = 100;

    /// Maximum memories per module (multi-memory proposal)
    pub const MAX_WASM_MEMORIES: u32 = 100;
}

/// Platform profile for embedded microcontrollers (~64KB RAM)
#[cfg(feature = "embedded-small")]
pub mod platform {
    /// Maximum types in a module's type section
    pub const MAX_TYPES: usize = 64;

    /// Maximum functions in a module
    pub const MAX_FUNCTIONS: usize = 256;

    /// Maximum tables in a module
    pub const MAX_TABLES: usize = 4;

    /// Maximum memories in a module (typically 1)
    pub const MAX_MEMORIES: usize = 1;

    /// Maximum globals in a module
    pub const MAX_GLOBALS: usize = 128;

    /// Maximum imports in a module
    pub const MAX_IMPORTS: usize = 64;

    /// Maximum exports in a module
    pub const MAX_EXPORTS: usize = 64;

    /// Maximum elements in an element segment
    pub const MAX_ELEMENT_ITEMS: usize = 256;

    /// Maximum data segments in a module
    pub const MAX_DATA_SEGMENTS: usize = 64;

    /// Maximum parameters per function type
    pub const MAX_FUNCTION_PARAMS: usize = 16;

    /// Maximum results per function type
    pub const MAX_FUNCTION_RESULTS: usize = 8;

    /// Maximum locals per function (not counting params)
    pub const MAX_FUNCTION_LOCALS: usize = 128;

    /// Maximum code size per function in bytes
    pub const MAX_FUNCTION_CODE_SIZE: usize = 16384; // 16 KiB

    /// Maximum total module size in bytes
    pub const MAX_MODULE_SIZE: usize = 65536; // 64 KiB

    /// Maximum value stack depth during execution
    pub const MAX_VALUE_STACK: usize = 1024;

    /// Maximum call stack depth during execution
    pub const MAX_CALL_STACK: usize = 64;

    /// Arena size for decode-phase temporary allocation
    pub const DECODE_ARENA_SIZE: usize = 64 * 1024; // 64 KiB

    /// Maximum memory pages for this platform
    pub const MAX_MEMORY_PAGES: u32 = 16; // 1 MiB max

    /// Maximum custom section name length
    pub const MAX_CUSTOM_SECTION_NAME: usize = 64;

    /// Maximum import/export name length
    pub const MAX_NAME_LENGTH: usize = 128;
}

/// Platform profile for embedded Linux (~1MB RAM)
/// Note: embedded-small takes priority if both features are enabled
#[cfg(all(feature = "embedded-medium", not(feature = "embedded-small")))]
pub mod platform {
    /// Maximum types in a module's type section
    pub const MAX_TYPES: usize = 1024;

    /// Maximum functions in a module
    pub const MAX_FUNCTIONS: usize = 4096;

    /// Maximum tables in a module
    pub const MAX_TABLES: usize = 16;

    /// Maximum memories in a module
    pub const MAX_MEMORIES: usize = 4;

    /// Maximum globals in a module
    pub const MAX_GLOBALS: usize = 1024;

    /// Maximum imports in a module
    pub const MAX_IMPORTS: usize = 512;

    /// Maximum exports in a module
    pub const MAX_EXPORTS: usize = 512;

    /// Maximum elements in an element segment
    pub const MAX_ELEMENT_ITEMS: usize = 4096;

    /// Maximum data segments in a module
    pub const MAX_DATA_SEGMENTS: usize = 256;

    /// Maximum parameters per function type
    pub const MAX_FUNCTION_PARAMS: usize = 64;

    /// Maximum results per function type
    pub const MAX_FUNCTION_RESULTS: usize = 32;

    /// Maximum locals per function (not counting params)
    pub const MAX_FUNCTION_LOCALS: usize = 2048;

    /// Maximum code size per function in bytes
    pub const MAX_FUNCTION_CODE_SIZE: usize = 262144; // 256 KiB

    /// Maximum total module size in bytes
    pub const MAX_MODULE_SIZE: usize = 1024 * 1024; // 1 MiB

    /// Maximum value stack depth during execution
    pub const MAX_VALUE_STACK: usize = 8192;

    /// Maximum call stack depth during execution
    pub const MAX_CALL_STACK: usize = 256;

    /// Arena size for decode-phase temporary allocation
    pub const DECODE_ARENA_SIZE: usize = 512 * 1024; // 512 KiB

    /// Maximum memory pages for this platform
    pub const MAX_MEMORY_PAGES: u32 = 256; // 16 MiB max

    /// Maximum custom section name length
    pub const MAX_CUSTOM_SECTION_NAME: usize = 256;

    /// Maximum import/export name length
    pub const MAX_NAME_LENGTH: usize = 512;
}

/// Platform profile for desktop/server (default - ample memory)
#[cfg(not(any(feature = "embedded-small", feature = "embedded-medium")))]
pub mod platform {
    /// Maximum types in a module's type section
    pub const MAX_TYPES: usize = 65536;

    /// Maximum functions in a module
    pub const MAX_FUNCTIONS: usize = 65536;

    /// Maximum tables in a module
    pub const MAX_TABLES: usize = 64;

    /// Maximum memories in a module
    pub const MAX_MEMORIES: usize = 64;

    /// Maximum globals in a module
    pub const MAX_GLOBALS: usize = 4096;

    /// Maximum imports in a module
    pub const MAX_IMPORTS: usize = 16384;

    /// Maximum exports in a module
    pub const MAX_EXPORTS: usize = 16384;

    /// Maximum elements in an element segment
    pub const MAX_ELEMENT_ITEMS: usize = 65536;

    /// Maximum data segments in a module
    pub const MAX_DATA_SEGMENTS: usize = 4096;

    /// Maximum parameters per function type
    pub const MAX_FUNCTION_PARAMS: usize = 1000;

    /// Maximum results per function type (multi-value)
    pub const MAX_FUNCTION_RESULTS: usize = 1000;

    /// Maximum locals per function (WebAssembly allows up to 50,000)
    pub const MAX_FUNCTION_LOCALS: usize = 50000;

    /// Maximum code size per function in bytes
    pub const MAX_FUNCTION_CODE_SIZE: usize = 8 * 1024 * 1024; // 8 MiB

    /// Maximum total module size in bytes
    pub const MAX_MODULE_SIZE: usize = 128 * 1024 * 1024; // 128 MiB

    /// Maximum value stack depth during execution
    pub const MAX_VALUE_STACK: usize = 65536;

    /// Maximum call stack depth during execution
    pub const MAX_CALL_STACK: usize = 512;

    /// Arena size for decode-phase temporary allocation
    pub const DECODE_ARENA_SIZE: usize = 4 * 1024 * 1024; // 4 MiB

    /// Maximum memory pages for this platform (spec max)
    pub const MAX_MEMORY_PAGES: u32 = 65536; // 4 GiB max

    /// Maximum custom section name length
    pub const MAX_CUSTOM_SECTION_NAME: usize = 1024;

    /// Maximum import/export name length
    pub const MAX_NAME_LENGTH: usize = 4096;
}

// Re-export platform limits at module level for convenience
pub use platform::*;

/// Validates a count against a limit, returning an error message if exceeded.
///
/// This is a helper for decoder implementations to provide consistent error messages.
#[inline]
pub const fn check_limit(count: usize, limit: usize, what: &'static str) -> Option<&'static str> {
    if count > limit {
        Some(what)
    } else {
        None
    }
}

/// Compile-time assertions to ensure limits are reasonable
mod assertions {
    use super::platform::*;

    // Ensure params/results fit WebAssembly reasonable bounds
    const _: () = assert!(MAX_FUNCTION_PARAMS >= 16, "At least 16 params required");
    const _: () = assert!(MAX_FUNCTION_RESULTS >= 1, "At least 1 result required");

    // Ensure execution limits are reasonable
    const _: () = assert!(MAX_VALUE_STACK >= 256, "Value stack too small");
    const _: () = assert!(MAX_CALL_STACK >= 32, "Call stack too small");

    // Ensure memory limits don't exceed spec
    const _: () = assert!(
        MAX_MEMORY_PAGES as usize <= super::spec::MAX_WASM_MEMORY_PAGES as usize,
        "Memory pages exceed WebAssembly spec"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Basic sanity tests
    // =========================================================================

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn test_limits_are_defined() {
        // Ensure all limits are accessible and non-zero
        assert!(MAX_TYPES > 0);
        assert!(MAX_FUNCTIONS > 0);
        assert!(MAX_FUNCTION_PARAMS > 0);
        assert!(MAX_FUNCTION_RESULTS > 0);
        assert!(MAX_FUNCTION_LOCALS > 0);
        assert!(MAX_TABLES > 0);
        assert!(MAX_MEMORIES > 0);
        assert!(MAX_GLOBALS > 0);
        assert!(MAX_IMPORTS > 0);
        assert!(MAX_EXPORTS > 0);
        assert!(MAX_ELEMENT_ITEMS > 0);
        assert!(MAX_DATA_SEGMENTS > 0);
        assert!(MAX_VALUE_STACK > 0);
        assert!(MAX_CALL_STACK > 0);
        assert!(MAX_MODULE_SIZE > 0);
        assert!(MAX_FUNCTION_CODE_SIZE > 0);
        assert!(DECODE_ARENA_SIZE > 0);
        assert!(MAX_MEMORY_PAGES > 0);
        assert!(MAX_CUSTOM_SECTION_NAME > 0);
        assert!(MAX_NAME_LENGTH > 0);
    }

    #[test]
    fn test_spec_constants() {
        assert_eq!(spec::WASM_PAGE_SIZE, 65536);
        assert_eq!(spec::MAX_WASM_MEMORY_PAGES, 65536);
        assert_eq!(spec::MAX_WASM_TABLES, 100);
        assert_eq!(spec::MAX_WASM_MEMORIES, 100);
    }

    // =========================================================================
    // check_limit helper tests
    // =========================================================================

    #[test]
    fn test_check_limit_under() {
        assert!(check_limit(10, 100, "test").is_none());
        assert!(check_limit(0, 100, "test").is_none());
        assert!(check_limit(99, 100, "test").is_none());
    }

    #[test]
    fn test_check_limit_equal() {
        // Equal to limit should pass (not exceed)
        assert!(check_limit(100, 100, "test").is_none());
        assert!(check_limit(0, 0, "test").is_none());
    }

    #[test]
    fn test_check_limit_over() {
        assert!(check_limit(101, 100, "test").is_some());
        assert_eq!(check_limit(101, 100, "params"), Some("params"));
        assert!(check_limit(1, 0, "test").is_some());
    }

    #[test]
    fn test_check_limit_max_values() {
        // Test with maximum usize values
        assert!(check_limit(usize::MAX, usize::MAX, "max").is_none());
        assert!(check_limit(usize::MAX - 1, usize::MAX, "max").is_none());
    }

    // =========================================================================
    // Consistency tests - verify limits make sense together
    // =========================================================================

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn test_limits_consistency() {
        // Function params should be less than or equal to locals limit
        // (params are part of local variable space)
        assert!(
            MAX_FUNCTION_PARAMS <= MAX_FUNCTION_LOCALS,
            "params ({}) should fit within locals ({})",
            MAX_FUNCTION_PARAMS,
            MAX_FUNCTION_LOCALS
        );

        // Module size should be larger than individual function code size
        assert!(
            MAX_FUNCTION_CODE_SIZE <= MAX_MODULE_SIZE,
            "function code ({}) should fit in module ({})",
            MAX_FUNCTION_CODE_SIZE,
            MAX_MODULE_SIZE
        );

        // Decode arena should be reasonable fraction of module size
        assert!(
            DECODE_ARENA_SIZE <= MAX_MODULE_SIZE,
            "decode arena ({}) should not exceed module size ({})",
            DECODE_ARENA_SIZE,
            MAX_MODULE_SIZE
        );

        // Tables limit should not exceed spec maximum
        assert!(
            MAX_TABLES as u32 <= spec::MAX_WASM_TABLES,
            "tables ({}) should not exceed spec ({})",
            MAX_TABLES,
            spec::MAX_WASM_TABLES
        );

        // Memories limit should not exceed spec maximum
        assert!(
            MAX_MEMORIES as u32 <= spec::MAX_WASM_MEMORIES,
            "memories ({}) should not exceed spec ({})",
            MAX_MEMORIES,
            spec::MAX_WASM_MEMORIES
        );
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn test_memory_limits_within_spec() {
        // Platform memory pages must not exceed spec
        assert!(
            MAX_MEMORY_PAGES <= spec::MAX_WASM_MEMORY_PAGES,
            "memory pages ({}) exceed spec maximum ({})",
            MAX_MEMORY_PAGES,
            spec::MAX_WASM_MEMORY_PAGES
        );

        // Calculate max memory in bytes and verify it's representable
        let max_memory_bytes = (MAX_MEMORY_PAGES as u64) * (spec::WASM_PAGE_SIZE as u64);
        assert!(max_memory_bytes > 0);
    }

    // =========================================================================
    // Practical scenario tests
    // =========================================================================

    #[test]
    fn test_typical_function_signature() {
        // A typical function: (i32, i32, i32) -> i32
        let params = 3usize;
        let results = 1usize;

        assert!(
            check_limit(params, MAX_FUNCTION_PARAMS, "params").is_none(),
            "typical function params should pass"
        );
        assert!(
            check_limit(results, MAX_FUNCTION_RESULTS, "results").is_none(),
            "typical function results should pass"
        );
    }

    #[test]
    fn test_complex_function_signature() {
        // A more complex function with many params (e.g., SIMD operations)
        let params = 8usize;
        let results = 4usize;

        assert!(
            check_limit(params, MAX_FUNCTION_PARAMS, "params").is_none(),
            "complex function params should pass"
        );
        assert!(
            check_limit(results, MAX_FUNCTION_RESULTS, "results").is_none(),
            "complex function results should pass"
        );
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn test_small_module_scenario() {
        // Simulate a small utility module
        let types = 5usize;
        let functions = 10usize;
        let imports = 3usize;
        let exports = 5usize;

        assert!(check_limit(types, MAX_TYPES, "types").is_none());
        assert!(check_limit(functions, MAX_FUNCTIONS, "functions").is_none());
        assert!(check_limit(imports, MAX_IMPORTS, "imports").is_none());
        assert!(check_limit(exports, MAX_EXPORTS, "exports").is_none());
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn test_medium_module_scenario() {
        // Simulate a moderate module that fits all platform profiles
        // Use fractions of limits to ensure cross-platform compatibility
        let types = MAX_TYPES / 2;
        let functions = MAX_FUNCTIONS / 2;
        let imports = MAX_IMPORTS / 2;
        let exports = MAX_EXPORTS / 2;
        let globals = MAX_GLOBALS / 2;

        // These should pass on all platform profiles
        assert!(types <= MAX_TYPES);
        assert!(functions <= MAX_FUNCTIONS);
        assert!(imports <= MAX_IMPORTS);
        assert!(exports <= MAX_EXPORTS);
        assert!(globals <= MAX_GLOBALS);
    }

    // =========================================================================
    // Memory budget estimation tests
    // =========================================================================

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn test_decode_phase_memory_budget() {
        // Estimate memory needed for decode phase
        // Each type entry: ~32 bytes (params + results vecs)
        // Each function entry: ~64 bytes
        // Each import: ~128 bytes (name strings)
        // Each export: ~128 bytes (name strings)

        let estimated_types_mem = MAX_TYPES * 32;
        let estimated_funcs_mem = MAX_FUNCTIONS * 64;
        let estimated_imports_mem = MAX_IMPORTS * 128;
        let estimated_exports_mem = MAX_EXPORTS * 128;

        let total_estimate =
            estimated_types_mem + estimated_funcs_mem + estimated_imports_mem + estimated_exports_mem;

        // This is just an estimate - actual usage depends on implementation
        // The point is to verify limits produce reasonable memory estimates
        assert!(
            total_estimate > 0,
            "memory estimate should be positive"
        );

        // Log the estimate for debugging (won't print unless test fails)
        if total_estimate > 1_000_000_000 {
            panic!(
                "Memory estimate seems too high: {} bytes ({} MB)",
                total_estimate,
                total_estimate / (1024 * 1024)
            );
        }
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn test_value_stack_depth_reasonable() {
        // Value stack should be larger than call stack to support
        // expression evaluation within function calls.
        // A modest assumption: at least 8 values per call frame
        // (return address, locals, a few expression temporaries)

        let min_values_per_frame = 8usize;
        let min_reasonable_stack = MAX_CALL_STACK * min_values_per_frame;

        assert!(
            min_reasonable_stack <= MAX_VALUE_STACK,
            "value stack ({}) should support {} calls with {} values per frame",
            MAX_VALUE_STACK,
            MAX_CALL_STACK,
            min_values_per_frame
        );

        // Also verify value stack is meaningfully larger than call stack
        assert!(
            MAX_VALUE_STACK >= MAX_CALL_STACK,
            "value stack ({}) should be at least as large as call stack ({})",
            MAX_VALUE_STACK,
            MAX_CALL_STACK
        );
    }

    // =========================================================================
    // Edge case tests
    // =========================================================================

    #[test]
    fn test_zero_count_always_valid() {
        // Zero items should always be valid
        assert!(check_limit(0, MAX_TYPES, "types").is_none());
        assert!(check_limit(0, MAX_FUNCTIONS, "functions").is_none());
        assert!(check_limit(0, MAX_FUNCTION_PARAMS, "params").is_none());
        assert!(check_limit(0, 0, "zero_limit").is_none());
    }

    #[test]
    fn test_exact_limit_boundary() {
        // Exactly at limit should pass
        assert!(check_limit(MAX_TYPES, MAX_TYPES, "types").is_none());
        assert!(check_limit(MAX_FUNCTIONS, MAX_FUNCTIONS, "funcs").is_none());
        assert!(check_limit(MAX_FUNCTION_PARAMS, MAX_FUNCTION_PARAMS, "params").is_none());
    }

    #[test]
    fn test_one_over_limit() {
        // One over limit should fail
        assert!(check_limit(MAX_TYPES + 1, MAX_TYPES, "types").is_some());
        assert!(check_limit(MAX_FUNCTIONS + 1, MAX_FUNCTIONS, "funcs").is_some());
        assert!(check_limit(MAX_FUNCTION_PARAMS + 1, MAX_FUNCTION_PARAMS, "params").is_some());
    }
}
