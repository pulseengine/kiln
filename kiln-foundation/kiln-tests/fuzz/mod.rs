/// Fuzz testing module for WRT Foundation components
/// 
/// This module contains both actual fuzz targets (in fuzz_targets/) and 
/// property-based tests that can run deterministically in CI.

// Property-based test modules (run in CI)
pub mod bounded_collections_fuzz;
pub mod memory_adapter_fuzz; 
pub mod safe_memory_fuzz;

// Re-export commonly used verification types for tests
pub use wrt_foundation::verification::VerificationLevel;

/// Common test utilities for fuzz testing
pub mod test_utils {
    use super::VerificationLevel;
    
    /// Standard verification levels for testing
    pub const TEST_VERIFICATION_LEVELS: &[VerificationLevel] = &[
        VerificationLevel::None,
        VerificationLevel::Sampling,
        VerificationLevel::Standard,
        VerificationLevel::Full,
    ];
    
    /// Standard test capacities
    pub const TEST_CAPACITIES: &[usize] = &[16, 64, 256, 1024];
    
    /// Helper function to generate test data patterns
    pub fn generate_test_pattern(size: usize, seed: u8) -> Vec<u8> {
        (0..size).map(|i| ((i + seed as usize) % 256) as u8).collect()
    }
    
    /// Helper to verify that a panic doesn't occur
    pub fn assert_no_panic<F, R>(f: F) -> R 
    where 
        F: FnOnce() -> R + std::panic::UnwindSafe,
    {
        match std::panic::catch_unwind(f) {
            Ok(result) => result,
            Err(_) => panic!("Operation panicked unexpectedly"),
        }
    }
}